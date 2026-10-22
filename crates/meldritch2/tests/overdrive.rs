use std::{
    fs,
    io::Cursor,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

use meldritch2::{
    compiler::{OfflineRenderer, RenderDiagnostics, compile_at_rate},
    lua,
    model::Graph,
};

const CORE: &str = include_str!("../../../examples/overdrive/overdrive_core.lua");
const RATE: f64 = 8_000.0;
static FIXTURE_ID: AtomicUsize = AtomicUsize::new(0);

fn fixture_source(mode: &str) -> (PathBuf, String) {
    let root = std::env::temp_dir().join(format!(
        "meldritch-overdrive-{}-{}",
        std::process::id(),
        FIXTURE_ID.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let mut wav = Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(
            &mut wav,
            hound::WavSpec {
                channels: 2,
                sample_rate: RATE as u32,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        for frame in 0..192 {
            let envelope = f64::from(192 - frame) / 192.0;
            let left = ((frame as f64 * 0.31).sin() * envelope * 24_000.0) as i16;
            let right = ((frame as f64 * 0.37).sin() * envelope * 20_000.0) as i16;
            writer.write_sample(left).unwrap();
            writer.write_sample(right).unwrap();
        }
        writer.finalize().unwrap();
    }
    fs::write(root.join("sample.wav"), wav.into_inner()).unwrap();
    fs::write(
        root.join("kit.json"),
        r#"{"Fixture_bd":"sample.wav","Fixture_sd":"sample.wav","Fixture_hh":"sample.wav","Fixture_oh":"sample.wav","Fixture_cp":"sample.wav","Fixture_lt":"sample.wav"}"#,
    )
    .unwrap();
    let source = CORE.replace(
        "source=\"strudel\",bank=\"RolandTR909\",cache=true",
        "source=\"local\",bank=\"Fixture\",manifest=\"kit.json\",cache=false",
    );
    let script = format!("local build=(function() {source} end)(); return build(\"{mode}\")");
    (root, script)
}

fn fixture(mode: &str) -> (PathBuf, Graph) {
    let (root, source) = fixture_source(mode);
    let graph = lua::read_with_root(&source, &root).unwrap();
    (root, graph)
}

fn render(graph: &Graph, frames: usize, controls: &[(&str, f64)]) -> (Vec<f64>, Vec<f64>) {
    let mut machine = compile_at_rate(graph, RATE).unwrap();
    for (id, value) in controls {
        machine.control_handle(id).unwrap().publish(*value).unwrap();
    }
    let mut left = vec![0.0; frames];
    let mut right = vec![0.0; frames];
    for (left, right) in left.chunks_mut(512).zip(right.chunks_mut(512)) {
        machine.process_stereo(left, right);
    }
    assert!(left.iter().chain(&right).all(|sample| sample.is_finite()));
    (left, right)
}

fn energy(samples: &[f64]) -> f64 {
    samples.iter().map(|sample| sample * sample).sum::<f64>() / samples.len() as f64
}

#[test]
fn project_loads_eight_independent_tracks_patterns_effects_and_generic_surface() {
    let (root, graph) = fixture("full");
    let performance = graph.performance.as_ref().unwrap();
    assert_eq!(
        performance.sequencer.track_names(),
        [
            "Kick",
            "SnareClap",
            "CHat",
            "OHat",
            "Perc",
            "Pulse",
            "Chime",
            "Lead"
        ]
    );
    assert_eq!(
        performance.sequencer.pattern_names(),
        [
            "Atmosphere",
            "Percussion",
            "Groove",
            "Poly",
            "Breakdown",
            "Overdrive",
            "Outro",
            "Tail"
        ]
    );
    assert_eq!(
        performance.sequencer.pattern_at(0).unwrap().lengths[..8],
        [32, 32, 32, 32, 32, 32, 48, 64]
    );
    assert_eq!(
        performance.sequencer.pattern_at(3).unwrap().lengths[..8],
        [48, 48, 48, 48, 48, 32, 48, 64]
    );
    assert_eq!(
        performance.sequencer.pattern_at(5).unwrap().lengths[..8],
        [64, 64, 64, 64, 64, 32, 48, 64]
    );

    let machine = compile_at_rate(&graph, RATE).unwrap();
    assert_eq!(
        machine
            .order()
            .iter()
            .filter(|id| id.contains("/ducker_"))
            .count(),
        4
    );
    assert_eq!(
        machine
            .order()
            .iter()
            .filter(|id| id.contains("/stereo_delay_"))
            .count(),
        2
    );
    assert_eq!(
        machine
            .order()
            .iter()
            .filter(|id| id.contains("/plaits_"))
            .count(),
        3
    );
    assert_eq!(
        machine
            .order()
            .iter()
            .filter(|id| id.contains("/stereo_compressor_"))
            .count(),
        1
    );
    assert_eq!(
        machine
            .control_handle("dry_duck")
            .unwrap()
            .targets()
            .count(),
        2
    );
    assert_eq!(
        machine
            .control_handle("duck_attack")
            .unwrap()
            .targets()
            .count(),
        4
    );
    assert_eq!(machine.screens()[0].title, "Delay Lab: Overdrive");
    assert_eq!(
        performance.apc.as_ref().unwrap().fader_controls[8],
        "master_level"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn production_composition_reports_every_drum_bus_sidechain_and_master_activity() {
    let (root, graph) = fixture("showcase");
    let mut machine = compile_at_rate(&graph, RATE).unwrap();
    let mut diagnostics = RenderDiagnostics::default();
    let mut left = vec![0.0; 80_000];
    let mut right = vec![0.0; left.len()];
    for (left, right) in left.chunks_mut(512).zip(right.chunks_mut(512)) {
        machine.process_stereo_diagnostic(left, right, &mut diagnostics);
    }
    for (track, minimum) in [
        ("Kick", 2),
        ("SnareClap", 1),
        ("CHat", 4),
        ("OHat", 1),
        ("Perc", 2),
        ("Pulse", 8),
        ("Chime", 3),
        ("Lead", 4),
    ] {
        diagnostics.expect_track_events(track, minimum).unwrap();
    }
    for player in 1..=6 {
        diagnostics
            .expect_output(&format!("instrument/sample_player_{player}"), 0, 1e-5)
            .unwrap();
    }
    diagnostics
        .expect_output("instrument/mixer_1", 0, 1e-4)
        .unwrap();
    diagnostics
        .expect_output("instrument/mixer_2", 0, 1e-5)
        .unwrap();
    for ducker in 1..=4 {
        let id = format!("instrument/ducker_{ducker}");
        diagnostics.expect_input(&id, 1, 1e-5).unwrap();
        let gain = &diagnostics.module(&id).unwrap().outputs[1];
        assert!(gain.rms() < 0.999, "{id} never reduced gain: {gain:?}");
    }
    diagnostics
        .expect_output("instrument/stereo_compressor_1", 2, 0.01)
        .unwrap();
    diagnostics
        .expect_output("instrument/stereo_compressor_1", 0, 1e-4)
        .unwrap();
    assert!(left.iter().chain(&right).all(|sample| sample.is_finite()));
    assert_ne!(left, right);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn parallel_showcase_preserves_events_ducking_compression_and_sample_order() {
    let (root_a, scalar_graph) = fixture("showcase");
    let (root_b, serial_graph) = fixture("showcase");
    let (root_c, parallel_graph) = fixture("showcase");
    let frames = 24_000;
    let scalar = render(&scalar_graph, frames, &[]);
    let run = |graph: &Graph, workers| {
        let mut renderer =
            OfflineRenderer::new(compile_at_rate(graph, RATE).unwrap(), 512, workers).unwrap();
        let mut left = vec![0.0; frames];
        let mut right = vec![0.0; frames];
        for (left, right) in left.chunks_mut(512).zip(right.chunks_mut(512)) {
            renderer.process_stereo(left, right);
        }
        (left, right)
    };
    let serial = run(&serial_graph, 1);
    let parallel = run(&parallel_graph, 4);
    assert_eq!(scalar, serial, "block processing changed Overdrive output");
    assert_eq!(
        serial, parallel,
        "parallel processing changed Overdrive output"
    );
    let positions: Vec<_> = [&scalar_graph, &serial_graph, &parallel_graph]
        .into_iter()
        .map(|graph| {
            graph
                .performance
                .as_ref()
                .unwrap()
                .sequencer
                .position_steps()
        })
        .collect();
    assert_eq!(positions[0], positions[1], "block scheduling lost events");
    assert_eq!(
        positions[1], positions[2],
        "parallel scheduling lost events"
    );
    fs::remove_dir_all(root_a).unwrap();
    fs::remove_dir_all(root_b).unwrap();
    fs::remove_dir_all(root_c).unwrap();
}

#[test]
fn edits_persist_as_a_lua_layer_over_the_authored_composition() {
    let (root, source) = fixture_source("full");
    let first = lua::read_with_root(&source, &root).unwrap();
    let sequence = &first.performance.as_ref().unwrap().sequencer;
    sequence.view_pattern(5).unwrap();
    sequence.select_track(0).unwrap();
    let before = sequence.pattern().velocities[0][1];
    sequence.execute("toggle:2").unwrap();
    let path = root.join("overdrive-project.lua");
    sequence.save_project(&path).unwrap();

    let rebuilt = lua::read_with_root(&source, &root).unwrap();
    let rebuilt_sequence = &rebuilt.performance.as_ref().unwrap().sequencer;
    rebuilt_sequence.load_project(&path).unwrap();
    assert_ne!(
        rebuilt_sequence.pattern_at(5).unwrap().velocities[0][1],
        before
    );
    let first_save = fs::read(&path).unwrap();
    rebuilt_sequence.save_project(&path).unwrap();
    assert_eq!(fs::read(&path).unwrap(), first_save);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn kick_ducks_delay_lab_buses_without_touching_drums_or_independent_lead() {
    let (root, graph) = fixture("showcase");
    let sequence = &graph.performance.as_ref().unwrap().sequencer;

    // Kick + lead only: duck controls have no route to either audible branch.
    sequence.toggle_solo(0).unwrap();
    sequence.toggle_solo(7).unwrap();
    let untouched = render(&graph, 36_000, &[("dry_duck", 0.0), ("wet_duck", 0.0)]);
    let still_untouched = render(&graph, 36_000, &[("dry_duck", 1.0), ("wet_duck", 1.0)]);
    assert_eq!(untouched, still_untouched);
    assert!(energy(&untouched.0) > 1e-8);

    sequence.toggle_solo(7).unwrap();
    sequence.toggle_solo(5).unwrap();
    let open = render(&graph, 36_000, &[("dry_duck", 0.0), ("wet_duck", 0.0)]);
    let pumping = render(&graph, 36_000, &[("dry_duck", 0.9), ("wet_duck", 0.95)]);
    assert_ne!(open, pumping);
    assert!((energy(&open.0) - energy(&pumping.0)).abs() > 1e-7);

    let dry_only = render(&graph, 36_000, &[("dry_duck", 1.0), ("wet_duck", 0.0)]);
    let wet_only = render(&graph, 36_000, &[("dry_duck", 0.0), ("wet_duck", 1.0)]);
    assert_ne!(
        dry_only, wet_only,
        "dry voice and wet return must be separate routes"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn lead_ambience_is_independent_and_seeded_rendering_is_deterministic() {
    let (root_a, first) = fixture("showcase");
    first
        .performance
        .as_ref()
        .unwrap()
        .sequencer
        .toggle_solo(7)
        .unwrap();
    let dry = render(
        &first,
        24_000,
        &[("lead_space", 0.0), ("delay_feedback", 0.0)],
    );
    let changed_delay_lab = render(
        &first,
        24_000,
        &[("lead_space", 0.0), ("delay_feedback", 0.9)],
    );
    assert_eq!(
        dry, changed_delay_lab,
        "Delay Lab feedback leaked into lead route"
    );
    let ambient = render(
        &first,
        24_000,
        &[("lead_space", 0.9), ("delay_feedback", 0.0)],
    );
    assert_ne!(dry, ambient);

    let (root_b, second) = fixture("showcase");
    second
        .performance
        .as_ref()
        .unwrap()
        .sequencer
        .toggle_solo(7)
        .unwrap();
    assert_eq!(
        ambient,
        render(
            &second,
            24_000,
            &[("lead_space", 0.9), ("delay_feedback", 0.0)]
        )
    );
    fs::remove_dir_all(root_a).unwrap();
    fs::remove_dir_all(root_b).unwrap();
}

#[test]
fn seven_minute_arrangement_advances_on_the_shared_sixty_four_step_boundary() {
    let (root, graph) = fixture("full");
    let sequence = &graph.performance.as_ref().unwrap().sequencer;
    assert_eq!(sequence.active_pattern(), 0);
    // 448 Hz makes one sixteenth step exactly 60 frames at 112 BPM.
    let rate = 448.0;
    let mut machine = compile_at_rate(&graph, rate).unwrap();
    let mut left = vec![0.0; 2 * 64 * 60 + 1];
    let mut right = vec![0.0; left.len()];
    machine.process_stereo(&mut left, &mut right);
    assert_eq!(sequence.active_pattern(), 1);
    assert!(left.iter().chain(&right).all(|sample| sample.is_finite()));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn full_arrangement_contains_drums_after_the_authored_intro() {
    let (root, graph) = fixture("full");
    // At 448 Hz and 112 BPM each sixteenth is exactly 60 samples. Five
    // 64-step cycles cover the atmospheric and percussion introductions;
    // the following Groove section must contain the complete drum kit.
    let rate = 448.0;
    let intro_frames = 5 * 64 * 60;
    let mut machine = compile_at_rate(&graph, rate).unwrap();
    let mut diagnostics = RenderDiagnostics::default();
    let mut left = vec![0.0; intro_frames];
    let mut right = vec![0.0; intro_frames];
    machine.process_stereo_diagnostic(&mut left, &mut right, &mut diagnostics);
    assert_eq!(diagnostics.track("Kick").unwrap().events, 0);

    let mut left = vec![0.0; 64 * 60];
    let mut right = vec![0.0; left.len()];
    machine.process_stereo_diagnostic(&mut left, &mut right, &mut diagnostics);
    for track in ["Kick", "SnareClap", "CHat", "OHat", "Perc"] {
        diagnostics.expect_track_events(track, 1).unwrap();
    }
    diagnostics
        .expect_output("instrument/mixer_1", 0, 1e-6)
        .unwrap();
    fs::remove_dir_all(root).unwrap();
}
