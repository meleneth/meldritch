use meldritch2::{compiler::compile_at_rate, lua};
use std::{
    fs,
    io::Cursor,
    sync::atomic::{AtomicUsize, Ordering},
};

const CORE: &str = include_str!("../../../examples/groovebox/groovebox_core.lua");
const SAMPLE_RATE: f64 = 8_000.0;
const PATTERN_FRAMES: usize = 15_000;
static FIXTURE_ID: AtomicUsize = AtomicUsize::new(0);

fn fixture() -> (std::path::PathBuf, meldritch2::model::Graph) {
    let root = std::env::temp_dir().join(format!(
        "meldritch-groovebox-{}-{}",
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
                sample_rate: SAMPLE_RATE as u32,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        for frame in 0..96 {
            let sample = if frame < 32 {
                ((32 - frame) * 800) as i16
            } else {
                0
            };
            writer.write_sample(sample).unwrap();
            writer.write_sample(-sample).unwrap();
        }
        writer.finalize().unwrap();
    }
    fs::write(root.join("sample.wav"), wav.into_inner()).unwrap();
    fs::write(root.join("kit.json"),r#"{"Fixture_bd":"sample.wav","Fixture_sd":"sample.wav","Fixture_hh":"sample.wav","Fixture_oh":"sample.wav","Fixture_cp":"sample.wav","Fixture_lt":"sample.wav"}"#).unwrap();
    fs::write(
        root.join("patterns.lua"),
        include_str!("../../../examples/groovebox/patterns.lua"),
    )
    .unwrap();
    let source = CORE.replace(
        "source=\"strudel\",bank=\"RolandTR909\",cache=true",
        "source=\"local\",bank=\"Fixture\",manifest=\"kit.json\",cache=false",
    );
    let script = format!("local build=(function() {source} end)(); return build(false)");
    (root.clone(), lua::read_with_root(&script, &root).unwrap())
}
fn render(graph: &meldritch2::model::Graph, frames: usize) -> (Vec<f64>, Vec<f64>) {
    let mut machine = compile_at_rate(graph, SAMPLE_RATE).unwrap();
    let mut left = vec![0.0; frames];
    let mut right = vec![0.0; frames];
    for (l, r) in left.chunks_mut(512).zip(right.chunks_mut(512)) {
        machine.process_stereo(l, r)
    }
    (left, right)
}
fn energy(values: &[f64]) -> f64 {
    values.iter().map(|v| v * v).sum::<f64>() / values.len() as f64
}

#[test]
fn lua_authors_eight_tracks_four_patterns_controls_screen_and_apc_mapping() {
    let (root, graph) = fixture();
    let performance = graph.performance.as_ref().unwrap();
    assert_eq!(
        performance.sequencer.track_names(),
        [
            "Kick", "Snare", "CHat", "OHat", "Clap", "Perc", "Bass", "Lead"
        ]
    );
    assert_eq!(
        performance.sequencer.pattern_names(),
        ["Foundation", "Broken", "Acid", "Atmosphere"]
    );
    assert_eq!(
        performance
            .key_bindings
            .iter()
            .find(|v| v.key == "m")
            .unwrap()
            .command,
        "mute"
    );
    let apc = performance.apc.as_ref().unwrap();
    assert_eq!(apc.pages, ["steps", "performance"]);
    assert_eq!(apc.fader_controls[8], "master_level");
    let machine = compile_at_rate(&graph, SAMPLE_RATE).unwrap();
    for index in 1..=8 {
        assert!(machine.control_handle(&format!("volume_{index}")).is_some());
        assert!(machine.control_handle(&format!("voice_{index}")).is_some());
    }
    assert_eq!(machine.screens()[0].title, "Groovebox");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn all_tracks_render_and_mute_solo_remove_audio_without_losing_steps() {
    let (root, graph) = fixture();
    let sequence = &graph.performance.as_ref().unwrap().sequencer;
    let original = sequence.pattern();
    let (all, right) = render(&graph, PATTERN_FRAMES);
    assert!(energy(&all) > 1e-8 && all.iter().all(|v| v.is_finite()));
    assert!(right.iter().all(|v| v.is_finite()));
    assert_ne!(all, right, "authored track placement must produce stereo");
    for track in 0..8 {
        sequence.toggle_solo(track).unwrap();
        let (solo, _) = render(&graph, PATTERN_FRAMES);
        assert!(
            energy(&solo) > 1e-10,
            "track {track} must produce audio (energy {})",
            energy(&solo)
        );
        sequence.toggle_mute(track).unwrap();
        let (muted, _) = render(&graph, PATTERN_FRAMES);
        assert!(
            energy(&muted) < energy(&solo) * 0.01 + 1e-12,
            "track {track} mute"
        );
        sequence.toggle_mute(track).unwrap();
        sequence.toggle_solo(track).unwrap();
    }
    assert_eq!(sequence.pattern().velocities, original.velocities);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn patterns_controls_and_melodic_sequences_change_real_audio() {
    let (root, graph) = fixture();
    let sequence = &graph.performance.as_ref().unwrap().sequencer;
    let foundation = render(&graph, PATTERN_FRAMES).0;
    sequence.queue_pattern(2).unwrap();
    let acid_transition = render(&graph, PATTERN_FRAMES * 2).0;
    assert_ne!(
        &foundation[..PATTERN_FRAMES],
        &acid_transition[PATTERN_FRAMES..]
    );
    let mut machine = compile_at_rate(&graph, SAMPLE_RATE).unwrap();
    machine
        .control_handle("voice_7")
        .unwrap()
        .publish(12000.0)
        .unwrap();
    machine
        .control_handle("master_effect")
        .unwrap()
        .publish(1.0)
        .unwrap();
    let mut changed = vec![0.0; PATTERN_FRAMES];
    machine.process(&mut changed);
    assert!(changed.iter().all(|v| v.is_finite()));
    assert_ne!(foundation, changed);
    let acid = sequence.pattern_at(2).unwrap();
    let bass = acid.pitches_hz[6]
        .iter()
        .zip(acid.velocities[6])
        .filter(|(_, v)| *v > 0.0)
        .map(|(p, _)| *p as i32)
        .collect::<std::collections::BTreeSet<_>>();
    assert!(bass.len() > 3);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn shared_commands_edit_only_selected_cells_and_persistence_round_trips() {
    let (root, graph) = fixture();
    let sequence = &graph.performance.as_ref().unwrap().sequencer;
    let mut transport = compile_at_rate(&graph, SAMPLE_RATE).unwrap();
    sequence.execute("stop").unwrap();
    transport.process(&mut [0.0]);
    assert!(!sequence.playing());
    sequence.execute("start").unwrap();
    transport.process(&mut [0.0]);
    assert!(sequence.playing());
    sequence.execute("copy:next").unwrap();
    assert_eq!(
        sequence.pattern_at(0).unwrap().velocities,
        sequence.pattern_at(1).unwrap().velocities
    );
    sequence.execute("track:6").unwrap();
    sequence.execute("bank:1").unwrap();
    let before = sequence.pattern();
    sequence.execute("toggle:3").unwrap();
    sequence.execute("velocity:down").unwrap();
    sequence.execute("pitch:up").unwrap();
    sequence.execute("gate:up").unwrap();
    let after = sequence.pattern();
    assert_ne!(before.velocities[6][10], after.velocities[6][10]);
    assert_eq!(before.velocities[5], after.velocities[5]);
    let path = root.join("project.json");
    sequence.save_project(&path).unwrap();
    sequence.execute("clear").unwrap();
    assert_ne!(sequence.pattern().velocities[6], after.velocities[6]);
    sequence.load_project(&path).unwrap();
    assert_eq!(sequence.pattern().velocities[6], after.velocities[6]);
    fs::remove_dir_all(root).unwrap();
}
