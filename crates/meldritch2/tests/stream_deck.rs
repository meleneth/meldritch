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
    let controls = machine
        .control_ids()
        .map(|id| {
            let handle = machine.control_handle(id).unwrap();
            let value = machine.effective_value(handle.target()).unwrap();
            (handle, value)
        })
        .collect();
    let runtime = SurfaceRuntime::new(
        graph.control_surfaces[0].clone(),
        controls,
        graph.performance.clone(),
    );
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
    let (mut machine, mut runtime) = runtime(&graph, 8_000.0);
    let mut adapter = SimulatedAdapter::default();
    adapter.connect(&mut runtime);
    adapter
        .input(
            &mut runtime,
            ButtonEdge {
                index: 3,
                pressed: true,
            },
        )
        .unwrap();
    assert_eq!(
        machine.control_handle("pingpong").unwrap().override_value(),
        Some(0.78)
    );
    adapter
        .input(
            &mut runtime,
            ButtonEdge {
                index: 0,
                pressed: true,
            },
        )
        .unwrap();
    let mut left = vec![0.0; 24_000];
    let mut right = vec![0.0; left.len()];
    machine.process_stereo(&mut left[..128], &mut right[..128]);
    adapter
        .input(
            &mut runtime,
            ButtonEdge {
                index: 0,
                pressed: false,
            },
        )
        .unwrap();
    machine.process_stereo(&mut left[128..], &mut right[128..]);
    assert!(left.iter().chain(&right).all(|sample| sample.is_finite()));
    assert!(left.iter().chain(&right).any(|sample| sample.abs() > 1e-6));
    assert!(
        left.iter()
            .zip(&right)
            .any(|(left, right)| (left - right).abs() > 1e-7)
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
