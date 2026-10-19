use meldritch2::{
    compiler::{Machine, compile_at_rate},
    device::fill_interleaved,
    live::LiveRuntime,
    lua,
    model::{ControlScale, ParameterId},
    panel::Panel,
    poly::{PolyNoteEvent, PolyNotePlan},
};

const SUBTRACTIVE: &str = include_str!("../../../examples/instruments/subtractive.lua");
const DRONE: &str = include_str!("../../../examples/instruments/drone.lua");

fn note_events(off_frame: u64) -> Vec<PolyNoteEvent> {
    let mut events = [60, 64, 67, 72]
        .into_iter()
        .map(|key| PolyNoteEvent::On {
            frame: 0,
            channel: 1,
            key,
            velocity: 127,
        })
        .collect::<Vec<_>>();
    events.extend([60, 64, 67, 72].into_iter().map(|key| PolyNoteEvent::Off {
        frame: off_frame,
        channel: 1,
        key,
    }));
    events
}

fn render_poly(machine: &mut Machine, frames: usize, off_frame: u64) -> Vec<f64> {
    let mut plan = PolyNotePlan::prepare(machine, 0, &note_events(off_frame)).unwrap();
    let mut output = vec![0.0; frames];
    for chunk in output.chunks_mut(1024) {
        plan.render_next(machine, chunk).unwrap();
    }
    output
}

fn energy(samples: &[f64]) -> f64 {
    samples.iter().map(|sample| sample * sample).sum::<f64>() / samples.len() as f64
}

#[test]
fn subtractive_is_an_authored_four_voice_instrument() {
    let graph = lua::read(SUBTRACTIVE).unwrap();
    let mut machine = compile_at_rate(&graph, 48_000.0).unwrap();
    assert!(machine.is_stereo());
    assert!(machine.has_poly_notes());
    assert_eq!(machine.poly_note_channels().collect::<Vec<_>>(), [1]);
    assert_eq!(
        machine.control_ids().collect::<Vec<_>>(),
        ["Attack", "Cutoff", "Release", "Resonance"]
    );
    let panel = Panel::new(
        machine
            .control_ids()
            .map(|id| {
                let handle = machine.control_handle(id).unwrap();
                let authored = machine.effective_value(handle.target()).unwrap();
                (handle, authored)
            })
            .collect(),
    );
    assert_eq!(panel.rows().len(), 4);
    assert_eq!(
        machine.control_handle("Cutoff").unwrap().scale(),
        ControlScale::Logarithmic
    );
    for voice in 1..=4 {
        assert!(
            machine
                .order()
                .iter()
                .any(|id| id == &format!("voice_{voice}/oscillator_1"))
        );
    }

    let mut silent = [f64::NAN; 128];
    machine.process(&mut silent);
    assert_eq!(silent, [0.0; 128]);

    let rendered = render_poly(&mut machine, 24_576, 2048);
    assert!(rendered.iter().all(|sample| sample.is_finite()));
    assert!(energy(&rendered[512..2048]) > 1e-8);
    assert!(energy(&rendered[rendered.len() - 1024..]) < 1e-10);
}

#[test]
fn subtractive_controls_fan_out_and_change_filter_and_envelope_behavior() {
    let graph = lua::read(SUBTRACTIVE).unwrap();
    let machine = compile_at_rate(&graph, 48_000.0).unwrap();
    for (id, suffix) in [
        ("Cutoff", ("lowpass_1", "cutoff")),
        ("Resonance", ("lowpass_1", "resonance")),
        ("Attack", ("adsr_1", "attack")),
        ("Release", ("adsr_1", "release")),
    ] {
        let control = machine.control_handle(id).unwrap();
        let targets = control.targets().cloned().collect::<Vec<_>>();
        assert_eq!(targets.len(), 4);
        for (voice, target) in (1..=4).zip(targets) {
            assert_eq!(target.module, format!("voice_{voice}/{}", suffix.0));
            assert_eq!(target.name, suffix.1);
        }
    }

    let render_with = |id: &str, value: f64, frames: usize, off: u64| {
        let mut machine = compile_at_rate(&graph, 48_000.0).unwrap();
        let control = machine.control_handle(id).unwrap();
        control.publish(value).unwrap();
        for target in control.targets() {
            assert_eq!(machine.effective_value(target), Some(value));
        }
        render_poly(&mut machine, frames, off)
    };

    let dark = render_with("Cutoff", 80.0, 2048, 2048);
    let bright = render_with("Cutoff", 12_000.0, 2048, 2048);
    assert!((energy(&dark[512..]) - energy(&bright[512..])).abs() > 1e-4);

    let dry = render_with("Resonance", 0.0, 2048, 2048);
    let resonant = render_with("Resonance", 0.95, 2048, 2048);
    assert!((energy(&dry[512..]) - energy(&resonant[512..])).abs() > 1e-5);

    let fast_attack = render_with("Attack", 0.001, 256, 256);
    let slow_attack = render_with("Attack", 2.0, 256, 256);
    assert!(energy(&fast_attack) > energy(&slow_attack) * 100.0);

    let fast_release = render_with("Release", 0.01, 4096, 1024);
    let slow_release = render_with("Release", 5.0, 4096, 1024);
    assert!(energy(&fast_release[3072..]) < energy(&slow_release[3072..]) * 0.01);
}

fn render_drone(
    graph: &meldritch2::model::Graph,
    update: Option<(&str, f64)>,
) -> (Vec<f64>, Vec<f64>) {
    let mut machine = compile_at_rate(graph, 48_000.0).unwrap();
    if let Some((id, value)) = update {
        machine.control_handle(id).unwrap().publish(value).unwrap();
    }
    let mut left = vec![f64::NAN; 4096];
    let mut right = vec![f64::NAN; 4096];
    machine.process_stereo(&mut left, &mut right);
    (left, right)
}

#[test]
fn drone_is_immediate_finite_stereo_from_three_independent_composites() {
    let graph = lua::read(DRONE).unwrap();
    let machine = compile_at_rate(&graph, 48_000.0).unwrap();
    assert!(machine.is_stereo());
    assert!(machine.midi_notes().is_empty());
    assert!(!machine.has_poly_notes());
    let oscillators = machine
        .order()
        .iter()
        .filter(|id| id.ends_with("/oscillator_1"))
        .count();
    let lfos = machine
        .order()
        .iter()
        .filter(|id| id.ends_with("/lfo_1"))
        .count();
    assert_eq!((oscillators, lfos), (3, 3));

    let (left, right) = render_drone(&graph, None);
    assert!(left.iter().chain(&right).all(|sample| sample.is_finite()));
    assert!(energy(&left) > 1e-4);
    assert!(energy(&right) > 1e-4);
    assert!(
        left.iter()
            .zip(&right)
            .any(|(left, right)| (left - right).abs() > 1e-4)
    );

    let (mut runtime, _) = LiveRuntime::new(compile_at_rate(&graph, 48_000.0).unwrap());
    let mut output = vec![f64::NAN; 1024];
    let mut left_scratch = [0.0; 256];
    let mut right_scratch = [0.0; 256];
    fill_interleaved(
        &mut runtime,
        &mut output,
        2,
        &mut left_scratch,
        &mut right_scratch,
    );
    assert!(
        output
            .as_chunks::<2>()
            .0
            .iter()
            .any(|pair| pair[0] != pair[1])
    );
}

#[test]
fn drone_exposed_parameters_own_their_instances_and_change_the_signal() {
    let graph = lua::read(DRONE).unwrap();
    let machine = compile_at_rate(&graph, 48_000.0).unwrap();
    for (id, instance, module, parameter) in [
        ("Left modulation rate", "subpatch_1", "lfo_1", "frequency"),
        ("Right modulation rate", "subpatch_2", "lfo_1", "frequency"),
        ("Left modulation depth", "subpatch_1", "constant_1", "value"),
        (
            "Right modulation depth",
            "subpatch_2",
            "constant_1",
            "value",
        ),
    ] {
        let target = machine.control_handle(id).unwrap().target().clone();
        assert_eq!(
            target,
            ParameterId::new(format!("instrument/{instance}/{module}"), parameter)
        );
    }

    let (base_left, base_right) = render_drone(&graph, None);
    let (rate_left, rate_right) = render_drone(&graph, Some(("Left modulation rate", 8.0)));
    let (depth_left, depth_right) = render_drone(&graph, Some(("Right modulation depth", 5.0)));
    assert_ne!(base_left, rate_left);
    assert_eq!(base_right, rate_right);
    assert_eq!(base_left, depth_left);
    assert_ne!(base_right, depth_right);

    let (mut runtime, control) = LiveRuntime::new(compile_at_rate(&graph, 48_000.0).unwrap());
    control.submit(&graph).unwrap();
    let mut samples = [f64::NAN; 64];
    assert!(runtime.process(&mut samples));
    assert!(samples.iter().all(|sample| sample.is_finite()));
    assert!(control.reclaim().unwrap().is_some());
}
