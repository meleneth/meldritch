use meldritch2::{compiler::compile_at_rate, lua};
use std::fs;

fn example(path: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

fn load(path: &std::path::Path) -> meldritch2::model::Graph {
    lua::read_with_root(&fs::read_to_string(path).unwrap(), path.parent().unwrap()).unwrap()
}

#[test]
fn seeded_program_uses_shared_patterns_and_renders_real_audio() {
    let graph = load(&example("examples/generative/generative.lua"));
    let sequence = &graph.performance.as_ref().unwrap().sequencer;
    assert_eq!(sequence.track_names(), ["Orbit", "Dust", "Pulse"]);
    assert_eq!(sequence.pattern_names(), ["Generated", "Sparse"]);
    let generated = sequence.pattern_at(0).unwrap();
    assert_eq!(generated.lengths[..3], [24, 32, 48]);
    let mut machine = compile_at_rate(&graph, 8_000.0).unwrap();
    let mut left = vec![0.0; 8_000];
    let mut right = vec![0.0; 8_000];
    machine.process_stereo(&mut left, &mut right);
    assert!(left.iter().chain(&right).all(|sample| sample.is_finite()));
    assert!(left.iter().any(|sample| sample.abs() > 1e-7));
}

#[test]
fn procedural_base_rebuilds_then_applies_lua_edits_without_rewriting_generator() {
    let source_path = example("examples/generative/generative.lua");
    let source_before = fs::read(&source_path).unwrap();
    let first = load(&source_path);
    let sequence = &first.performance.as_ref().unwrap().sequencer;
    sequence.view_pattern(0).unwrap();
    sequence.select_track(0).unwrap();
    sequence.set_step_bank(2).unwrap();
    let before = sequence.pattern().velocities[0][16];
    sequence.execute("toggle:1").unwrap();
    let path = std::env::temp_dir().join(format!(
        "meldritch2-generative-edits-{}.lua",
        std::process::id()
    ));
    sequence.save_project(&path).unwrap();

    let rebuilt = load(&source_path);
    let rebuilt_sequence = &rebuilt.performance.as_ref().unwrap().sequencer;
    assert_eq!(rebuilt_sequence.pattern().velocities[0][16], before);
    rebuilt_sequence.load_project(&path).unwrap();
    assert_ne!(rebuilt_sequence.pattern().velocities[0][16], before);
    assert_eq!(fs::read(source_path).unwrap(), source_before);
    fs::remove_file(path).unwrap();
}
