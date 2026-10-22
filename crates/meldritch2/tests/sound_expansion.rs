use meldritch2::{
    compiler::compile_at_rate,
    lua,
    poly::{PolyNoteEvent, PolyNotePlan},
};
use std::{fs, io::Cursor};

const OSCILLATOR_LAB: &str = include_str!("../../../examples/instruments/oscillator_lab.lua");
const FILTERED_NOISE: &str = include_str!("../../../examples/instruments/filtered_noise.lua");
const DIRTY: &str = include_str!("../../../examples/instruments/dirty_subtractive.lua");
const DELAYED: &str = include_str!("../../../examples/instruments/delayed_sequence.lua");
const PLAITS_HARMONIC: &str = include_str!("../../../examples/instruments/plaits_harmonic.lua");
const PLAITS_MODAL: &str = include_str!("../../../examples/instruments/plaits_modal.lua");
const SIDECHAIN: &str = include_str!("../../../examples/instruments/rhythmic_sidechain.lua");

fn energy(samples: &[f64]) -> f64 {
    samples.iter().map(|x| x * x).sum::<f64>() / samples.len() as f64
}
fn render_plan(
    plan: &mut PolyNotePlan,
    machine: &mut meldritch2::compiler::Machine,
    output: &mut [f64],
) {
    for chunk in output.chunks_mut(1024) {
        plan.render_next(machine, chunk).unwrap();
    }
}

fn oscillator(waveform: &str, extra: &str, rate: f64) -> Vec<f64> {
    let source = format!(
        r#"
      local i=meldritch.instrument("test")
      local o=i:oscillator {{ waveform="{waveform}", frequency=100, {extra} }}
      i:connect(o.out,i.output.left); i:connect(o.out,i.output.right); return i
    "#
    );
    let graph = lua::read(&source).unwrap();
    let mut machine = compile_at_rate(&graph, rate).unwrap();
    let mut out = vec![0.0; rate as usize];
    machine.process(&mut out);
    out
}

#[test]
fn all_oscillators_have_expected_frequency_shape_bounds_and_sample_rate_behavior() {
    for rate in [8_000.0, 44_100.0, 96_000.0] {
        for waveform in ["sine", "saw", "triangle", "square", "pulse"] {
            let rendered = oscillator(waveform, "pulse_width=0.25", rate);
            assert!(
                rendered.iter().all(|x| x.is_finite() && x.abs() <= 1.1),
                "{waveform} at {rate}"
            );
            let rising = rendered
                .windows(2)
                .filter(|w| w[0] <= 0.0 && w[1] > 0.0)
                .count();
            assert!(
                (99..=101).contains(&rising),
                "{waveform} crossings {rising} at {rate}"
            );
        }
    }
    let sine = oscillator("sine", "", 8_000.0);
    let triangle = oscillator("triangle", "", 8_000.0);
    let square = oscillator("square", "", 8_000.0);
    let pulse = oscillator("pulse", "pulse_width=0.25", 8_000.0);
    assert_ne!(sine, triangle);
    assert_ne!(triangle, square);
    assert!(pulse.iter().sum::<f64>() / (pulse.len() as f64) < -0.35);
}

#[test]
fn actual_oscillator_and_filtered_noise_examples_compile_render_and_modulate() {
    for source in [OSCILLATOR_LAB, FILTERED_NOISE] {
        let graph = lua::read(source).unwrap();
        let mut machine = compile_at_rate(&graph, 48_000.0).unwrap();
        let mut left = vec![0.0; 8192];
        let mut right = vec![0.0; 8192];
        machine.process_stereo(&mut left, &mut right);
        assert!(left.iter().chain(&right).all(|x| x.is_finite()));
        assert!(energy(&left) > 1e-5);
        assert_ne!(left, right);
    }
    let graph = lua::read(FILTERED_NOISE).unwrap();
    let machine = compile_at_rate(&graph, 48_000.0).unwrap();
    let brightness = machine.control_handle("brightness").unwrap();
    assert_eq!(brightness.label(), "Brightness");
    assert_eq!(brightness.range(), (80.0, 12_000.0));
}

#[test]
fn multiple_modulations_lower_to_summed_cables_without_replacing_base_value() {
    let source = r#"local i=meldritch.instrument("multi")
      local o=i:oscillator{waveform="saw",frequency=110}; local f=i:svf{cutoff=1234,resonance=0.2}
      local a=i:lfo{frequency=1}; local b=i:lfo{frequency=2}
      f:modulate("cutoff",a,{depth=100}); f:modulate("cutoff",b,{depth=200})
      i:connect(o.out,f.input); i:connect(f.lowpass,i.output.left); i:connect(f.lowpass,i.output.right); return i"#;
    let graph = lua::read(source).unwrap();
    let machine = compile_at_rate(&graph, 48_000.0).unwrap();
    assert_eq!(
        machine.effective_value(&meldritch2::model::ParameterId::new(
            "instrument/svf_1",
            "cutoff"
        )),
        Some(1234.0)
    );
    let summed = machine
        .effective_connections()
        .iter()
        .filter(|edge| edge.to.module == "instrument/svf_1" && edge.to.port == "cutoff_mod")
        .count();
    assert_eq!(summed, 2);
    assert_eq!(
        machine
            .order()
            .iter()
            .filter(|id| id.contains("modulation_"))
            .count(),
        2
    );
}

#[test]
fn fixed_state_sound_contracts_keep_extremes_and_nonfinite_signals_out_of_output() {
    for name in [
        "oscillator_sine",
        "saw",
        "triangle",
        "square",
        "pulse",
        "noise",
        "svf",
        "drive",
        "ducker",
        "stereo_compressor",
        "envelope_follower",
        "compressor",
    ] {
        let contract = meldritch2::contract::find(name).unwrap();
        let mut state = vec![0.0; contract.state_len];
        let input = vec![f64::NAN; contract.inputs.len()];
        for extremes in [false, true] {
            let params = contract
                .params
                .iter()
                .map(|p| {
                    p.range
                        .map(|r| if extremes { r.1 } else { r.0 })
                        .unwrap_or(p.default)
                })
                .collect::<Vec<_>>();
            for _ in 0..256 {
                let mut output = vec![f64::NAN; contract.outputs.len()];
                (contract.process)(&mut state, &input, &params, 96_000.0, &mut output);
                assert!(
                    output.iter().all(|v| v.is_finite()),
                    "{name} {params:?} => {output:?}"
                );
            }
        }
    }
}

fn render_dirty(control: Option<(&str, f64)>) -> Vec<f64> {
    let graph = lua::read(DIRTY).unwrap();
    let mut machine = compile_at_rate(&graph, 48_000.0).unwrap();
    if let Some((id, value)) = control {
        machine.control_handle(id).unwrap().publish(value).unwrap();
    }
    let events = [
        PolyNoteEvent::On {
            frame: 0,
            channel: 1,
            key: 48,
            velocity: 110,
        },
        PolyNoteEvent::Off {
            frame: 7000,
            channel: 1,
            key: 48,
        },
    ];
    let mut plan = PolyNotePlan::prepare(&machine, 0, &events).unwrap();
    let mut out = vec![0.0; 16_000];
    render_plan(&mut plan, &mut machine, &mut out);
    out
}

#[test]
fn dirty_subtractive_is_real_polyphonic_graph_with_controls_and_modulation_lowering() {
    let graph = lua::read(DIRTY).unwrap();
    let machine = compile_at_rate(&graph, 48_000.0).unwrap();
    for id in ["pitch", "cutoff", "resonance", "drive", "delay_mix"] {
        assert!(machine.control_handle(id).is_some(), "{id}");
    }
    assert!(
        machine
            .order()
            .iter()
            .filter(|id| id.ends_with("/svf_1"))
            .count()
            == 4
    );
    assert!(
        machine
            .order()
            .iter()
            .filter(|id| id.contains("modulation_1"))
            .count()
            == 4
    );
    let clean = render_dirty(Some(("drive", 0.1)));
    let dirty = render_dirty(Some(("drive", 12.0)));
    assert!(energy(&clean) > 1e-7 && dirty.iter().all(|x| x.is_finite()));
    assert_ne!(clean, dirty);
}

#[test]
fn delayed_sequence_uses_stateful_runtime_and_wet_control_changes_audio() {
    let graph = lua::read(DELAYED).unwrap();
    let render = |mix| {
        let mut m = compile_at_rate(&graph, 8_000.0).unwrap();
        m.control_handle("delay_mix").unwrap().publish(mix).unwrap();
        let mut l = vec![0.0; 20_000];
        let mut r = vec![0.0; 20_000];
        m.process_stereo(&mut l, &mut r);
        (l, r)
    };
    let (left, dry) = render(0.0);
    let (_, wet) = render(1.0);
    assert!(energy(&left) > 1e-8);
    assert_ne!(dry, wet);
    assert!(wet.iter().all(|x| x.is_finite()));
}

fn render_plaits(source: &str) -> (Vec<f64>, Vec<f64>) {
    let graph = lua::read(source).unwrap();
    let mut machine = compile_at_rate(&graph, 48_000.0).unwrap();
    let events = [
        PolyNoteEvent::On {
            frame: 0,
            channel: 1,
            key: 60,
            velocity: 100,
        },
        PolyNoteEvent::Off {
            frame: 5000,
            channel: 1,
            key: 60,
        },
    ];
    let mut plan = PolyNotePlan::prepare(&machine, 0, &events).unwrap();
    let mut left = vec![0.0; 10_000];
    let mut right = vec![0.0; 10_000];
    // Poly plans render mono; direct stereo processing still exercises authored gate/pitch controls below.
    render_plan(&mut plan, &mut machine, &mut left);
    let graph = lua::read(source).unwrap();
    let mut stereo = compile_at_rate(&graph, 48_000.0).unwrap();
    let pitch = stereo.control_handle("__voice_1_pitch").unwrap();
    let gate = stereo.control_handle("__voice_1_gate").unwrap();
    pitch.publish(261.6256).unwrap();
    gate.publish(1.0).unwrap();
    stereo.process_stereo(&mut left, &mut right);
    (left, right)
}

#[test]
fn two_actual_plaits_models_render_independent_finite_main_and_aux_outputs() {
    let harmonic = render_plaits(PLAITS_HARMONIC);
    let modal = render_plaits(PLAITS_MODAL);
    for (left, right) in [&harmonic, &modal] {
        assert!(left.iter().chain(right).all(|x| x.is_finite()));
        assert!(energy(left) > 1e-8);
        assert_ne!(left, right);
    }
    assert_ne!(harmonic.0, modal.0);
    let graph = lua::read(PLAITS_HARMONIC).unwrap();
    let machine = compile_at_rate(&graph, 48_000.0).unwrap();
    for id in ["harmonics", "timbre", "morph", "delay_mix"] {
        assert!(machine.control_handle(id).is_some());
    }
}

#[test]
fn actual_909_sidechain_patch_uses_kick_audio_to_duck_and_recover() {
    let root = std::env::temp_dir().join(format!("meldritch-sidechain-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let mut wav = Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(
            &mut wav,
            hound::WavSpec {
                channels: 2,
                sample_rate: 800,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        for frame in 0..80 {
            let sample = if frame < 16 {
                (30_000 - frame * 1_500) as i16
            } else {
                0
            };
            writer.write_sample(sample).unwrap();
            writer.write_sample(sample).unwrap();
        }
        writer.finalize().unwrap();
    }
    fs::write(root.join("kick.wav"), wav.into_inner()).unwrap();
    fs::write(root.join("kit.json"), r#"{"Fixture_bd":"kick.wav"}"#).unwrap();
    let source = SIDECHAIN.replace(
        "source = \"strudel\", bank = \"RolandTR909\", cache = true",
        "source = \"local\", bank = \"Fixture\", manifest = \"kit.json\", cache = false",
    );
    let graph = lua::read_with_root(&source, &root).unwrap();
    let mut machine = compile_at_rate(&graph, 800.0).unwrap();
    for name in ["envelope_follower_1", "affine_1", "vca_1", "compressor_1"] {
        assert!(
            machine.order().iter().any(|id| id.ends_with(name)),
            "{name}"
        );
    }
    let mut left = vec![0.0; 780];
    let mut right = vec![0.0; 780];
    machine.process_stereo(&mut left, &mut right);
    assert!(left.iter().chain(&right).all(|x| x.is_finite()));
    assert!(
        energy(&right[20..100]) < energy(&right[240..340]) * 0.7,
        "kick must attenuate synth and synth must recover between kicks"
    );
    assert!(left.iter().zip(&right).any(|(l, r)| (l - r).abs() > 0.05));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn stereo_compressor_is_a_generic_insert_with_discoverable_controls() {
    let source = r#"
      local i=meldritch.instrument("compressor insert")
      local left=i:constant {value=.8}; local right=i:constant {value=.4}
      local c=i:stereo_compressor {threshold=-24,ratio=4,attack=.0001,
        release=.1,knee=6,makeup_gain=0,bypass=0}
      i:connect(left.out,c.input.left); i:connect(right.out,c.input.right)
      i:connect(c.left,i.output.left)
      -- The right output deliberately bypasses the insert: the compressor is
      -- an ordinary branch processor rather than privileged master behavior.
      i:connect(right.out,i.output.right)
      i:control {id="insert_ratio",label="Insert ratio",target=c.ratio,range={1,12}}
      return i
    "#;
    let graph = lua::read(source).unwrap();
    let mut machine = compile_at_rate(&graph, 48_000.0).unwrap();
    assert!(machine.control_handle("insert_ratio").is_some());
    let mut left = vec![0.0; 4_800];
    let mut right = vec![0.0; left.len()];
    machine.process_stereo(&mut left, &mut right);
    assert!(left.iter().chain(&right).all(|sample| sample.is_finite()));
    assert!(left.last().unwrap().abs() < right.last().unwrap().abs());
    assert!(right.iter().all(|sample| (*sample - 0.4).abs() < 1e-12));
}
