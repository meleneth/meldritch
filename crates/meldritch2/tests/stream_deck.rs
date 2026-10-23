use std::sync::{Arc, atomic::AtomicUsize};
use std::{
    fs,
    path::{Path, PathBuf},
};

use meldritch2::{
    compiler::compile_at_rate,
    lua,
    stream_deck::{ButtonEdge, SimulatedAdapter, SurfaceRuntime},
};

fn example(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn load(relative: &str) -> meldritch2::model::Graph {
    let path = example(relative);
    lua::read_with_root(&fs::read_to_string(&path).unwrap(), path.parent().unwrap()).unwrap()
}

fn runtime(
    graph: &meldritch2::model::Graph,
    rate: f64,
) -> (meldritch2::compiler::Machine, SurfaceRuntime) {
    let machine = compile_at_rate(graph, rate).unwrap();
    let controls: Vec<_> = machine
        .control_ids()
        .map(|id| {
            let handle = machine.control_handle(id).unwrap();
            let value = machine.effective_value(handle.target()).unwrap();
            (handle, value)
        })
        .collect();
    let mut runtime = SurfaceRuntime::new(
        graph.control_surfaces[0].clone(),
        controls.clone(),
        graph.performance.clone(),
    );
    if let Some(focus) = graph
        .performance
        .as_ref()
        .and_then(|p| p.control_focus.as_ref())
    {
        let focused = focus
            .controls
            .iter()
            .map(|id| {
                controls
                    .iter()
                    .find(|(handle, _)| handle.id() == id)
                    .unwrap()
                    .clone()
            })
            .collect();
        runtime = runtime.with_focus(focused, Arc::new(AtomicUsize::new(0)));
    }
    (machine, runtime)
}

#[test]
fn playground_loads_pages_and_simulates_press_release_and_external_state() {
    let graph = load("examples/experiments/01_stream_deck.lua");
    assert_eq!(graph.control_surfaces.len(), 1);
    assert_eq!(graph.control_surfaces[0].pages.len(), 2);
    let (machine, mut runtime) = runtime(&graph, 8_000.0);
    let mut adapter = SimulatedAdapter::default();
    adapter.connect(&mut runtime);
    assert_eq!(adapter.uploads.len(), 32);

    adapter
        .input(
            &mut runtime,
            ButtonEdge {
                index: 0,
                pressed: true,
            },
        )
        .unwrap();
    assert_eq!(
        machine.control_handle("tone").unwrap().override_value(),
        Some(0.12)
    );
    assert!(
        adapter
            .uploads
            .iter()
            .rev()
            .find(|(_, visual)| visual.id == "tone_toggle")
            .unwrap()
            .1
            .active
    );
    adapter
        .input(
            &mut runtime,
            ButtonEdge {
                index: 0,
                pressed: false,
            },
        )
        .unwrap();
    assert_eq!(adapter.uploads.len(), 33, "toggle release must not repaint");

    machine
        .control_handle("tone")
        .unwrap()
        .publish(0.0)
        .unwrap();
    adapter.refresh(&mut runtime);
    assert!(!adapter.uploads.last().unwrap().1.active);

    adapter
        .input(
            &mut runtime,
            ButtonEdge {
                index: 31,
                pressed: true,
            },
        )
        .unwrap();
    assert_eq!(runtime.page(), "colors");
    assert!(adapter.uploads.len() <= 66);
}

#[test]
fn delay_lab_surface_drives_real_controls_and_audio_without_hardware() {
    let graph = load("examples/experiments/02_delay_feedback.lua");
    assert!(matches!(
        graph.screens[0].widgets.last(),
        Some(meldritch2::model::Widget::ControlFocus)
    ));
    assert_eq!(
        graph
            .performance
            .as_ref()
            .unwrap()
            .control_focus
            .as_ref()
            .unwrap()
            .controls
            .len(),
        18
    );
    let apc = graph.performance.as_ref().unwrap().apc.as_ref().unwrap();
    assert_eq!(apc.master_control, "master_volume");
    let roll = apc.piano_roll.as_ref().expect("authored APC piano roll");
    assert_eq!(roll.drum_tracks, [0, 1, 2]);
    assert_eq!(roll.melodic_track, 3);
    assert_eq!(roll.scale, [0, 2, 3, 5, 7]);
    let authored = &graph
        .subpatches
        .values()
        .next()
        .expect("instrument definition")
        .graph;
    assert_eq!(
        authored
            .modules
            .iter()
            .filter(|module| module.kind == "sample_player")
            .count(),
        3
    );
    let kick_sidechains = authored
        .connections
        .iter()
        .filter(|connection| {
            connection.from.module.ends_with("sample_player_1")
                && connection.from.port == "left"
                && connection.to.module.contains("ducker")
                && connection.to.port == "sidechain"
        })
        .count();
    assert_eq!(
        kick_sidechains, 2,
        "audible kick must feed both duck detectors"
    );
    assert!(
        authored.connections.iter().all(|connection| {
            !(connection.from.module.contains("sample_player")
                && connection.to.module.contains("stereo_delay"))
        }),
        "drums must remain outside the delay path"
    );
    assert_eq!(
        graph.devices.apc_mini.as_deref(),
        Some("APC mini mk2:APC mini mk2 Control")
    );
    let (mut machine, mut runtime) = runtime(&graph, 8_000.0);
    let mut adapter = SimulatedAdapter::default();
    adapter.connect(&mut runtime);
    assert_eq!(runtime.focused_control_id(), Some("source_level"));
    adapter
        .input(
            &mut runtime,
            ButtonEdge {
                index: 10,
                pressed: true,
            },
        )
        .unwrap();
    assert_eq!(runtime.focused_control_id(), Some("feedback"));
    let apc_mapping = graph.performance.as_ref().unwrap().apc.as_ref().unwrap();
    assert_eq!(apc_mapping.fader_banks.len(), 3);
    assert_eq!(
        apc_mapping
            .fader_bank
            .load(std::sync::atomic::Ordering::Acquire),
        0
    );
    adapter
        .input(
            &mut runtime,
            ButtonEdge {
                index: 25,
                pressed: true,
            },
        )
        .unwrap();
    assert_eq!(
        apc_mapping
            .fader_bank
            .load(std::sync::atomic::Ordering::Acquire),
        1
    );
    assert!(
        adapter
            .uploads
            .iter()
            .any(|(_, visual)| { visual.id == "faders_delay" || visual.id == "bass_faders_delay" })
    );
    let sequence = graph.performance.as_ref().unwrap().sequencer.clone();
    adapter
        .input(
            &mut runtime,
            ButtonEdge {
                index: 1,
                pressed: true,
            },
        )
        .unwrap();
    machine.process_stereo(&mut [0.0], &mut [0.0]);
    assert!(!sequence.playing());
    adapter
        .input(
            &mut runtime,
            ButtonEdge {
                index: 0,
                pressed: true,
            },
        )
        .unwrap();
    machine.process_stereo(&mut [0.0], &mut [0.0]);
    assert!(sequence.playing());
    let mut left = vec![0.0; 24_000];
    let mut right = vec![0.0; left.len()];
    machine.process_stereo(&mut left, &mut right);
    assert!(left.iter().chain(&right).all(|sample| sample.is_finite()));
    assert!(left.iter().chain(&right).any(|sample| sample.abs() > 1e-6));
    assert!(
        left.iter()
            .zip(&right)
            .any(|(left, right)| (left - right).abs() > 1e-7)
    );
    let master = machine.control_handle("master_volume").unwrap();
    assert_eq!(
        master.targets().count(),
        2,
        "master must link both stereo gains"
    );
    master.publish(0.0).unwrap();
    let mut silent_left = vec![1.0; 512];
    let mut silent_right = vec![1.0; 512];
    machine.process_stereo(&mut silent_left, &mut silent_right);
    assert!(
        silent_left
            .iter()
            .chain(&silent_right)
            .all(|sample| sample.abs() < 1e-12),
        "APC master destination must be after the complete dry-drum and delayed-bass mix"
    );
}

#[test]
fn invalid_surface_targets_fail_at_the_normal_compile_boundary() {
    let source = fs::read_to_string(example("examples/experiments/01_stream_deck.lua")).unwrap();
    let graph = lua::read(&source.replace("control=\"tone\"", "control=\"missing\"")).unwrap();
    assert!(
        compile_at_rate(&graph, 8_000.0)
            .unwrap_err()
            .contains("unknown control 'missing'")
    );
}
