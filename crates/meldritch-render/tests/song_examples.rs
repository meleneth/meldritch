use meldritch_core::FrameRange;
use meldritch_dsl::load_song_directory;
use meldritch_render::song_render::{
    compile_automated_delayed_note_song, compile_delayed_note_song, compile_drone_song,
    compile_filtered_note_song, compile_note_song,
};
use std::path::{Path, PathBuf};

fn example(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("songs/examples")
        .join(name)
}

#[test]
fn minimal_modular_synth_compiles_and_renders_finite_audio() {
    let song = load_song_directory(example("00-minimal-synth")).expect("song should load");
    let patch = compile_drone_song(&song).expect("minimal patch should compile");
    let block = patch
        .render(FrameRange::new(0, 48_000).unwrap())
        .expect("one second should render");

    assert_eq!(patch.song_fingerprint(), song.fingerprint());
    assert_eq!(patch.sample_rate(), 48_000);
    assert_eq!(block.channels(), 1);
    assert_eq!(block.frames(), 48_000);
    assert!(block.peak_abs() > 0.5);
    assert!(block.samples().iter().all(|sample| sample.is_finite()));
}

#[test]
fn minimal_modular_synth_is_sample_identical_across_render_ranges() {
    let song = load_song_directory(example("00-minimal-synth")).expect("song should load");
    let patch = compile_drone_song(&song).expect("minimal patch should compile");
    let whole = patch.render(FrameRange::new(0, 48_000).unwrap()).unwrap();
    let first = patch.render(FrameRange::new(0, 12_345).unwrap()).unwrap();
    let second = patch
        .render(FrameRange::new(12_345, 48_000).unwrap())
        .unwrap();
    let joined = first
        .samples()
        .iter()
        .chain(second.samples())
        .copied()
        .collect::<Vec<_>>();

    assert_eq!(joined, whole.samples());
}

#[test]
fn note_pattern_executes_pitch_gate_envelope_and_vca_patch() {
    let song = load_song_directory(example("01-synth-note-pattern")).expect("song should load");
    let patch = compile_note_song(&song).expect("note patch should compile");
    let block = patch
        .render(FrameRange::new(0, patch.pattern_length()).unwrap())
        .unwrap();

    assert_eq!(patch.pattern_length(), 96_000);
    assert_eq!(block.frames(), 96_000);
    assert!(block.peak_abs() > 0.1);
    assert!(block.samples().iter().all(|sample| sample.is_finite()));
    assert!(block.samples()[0].abs() < 0.01);
}

#[test]
fn note_pattern_patch_is_sample_identical_across_chunks_and_loops() {
    let song = load_song_directory(example("01-synth-note-pattern")).expect("song should load");
    let patch = compile_note_song(&song).expect("note patch should compile");
    let whole = patch.render(FrameRange::new(0, 120_000).unwrap()).unwrap();
    let first = patch.render(FrameRange::new(0, 31_337).unwrap()).unwrap();
    let second = patch
        .render(FrameRange::new(31_337, 120_000).unwrap())
        .unwrap();
    let joined = first
        .samples()
        .iter()
        .chain(second.samples())
        .copied()
        .collect::<Vec<_>>();

    assert_eq!(joined, whole.samples());
}

#[test]
fn synth_parameter_pattern_drives_filter_cutoff_sample_accurately() {
    let song =
        load_song_directory(example("02-synth-parameter-pattern")).expect("song should load");
    let patch = compile_filtered_note_song(&song).expect("filtered patch should compile");

    assert_eq!(patch.cutoff_at(0), 180.0);
    assert_eq!(patch.cutoff_at(48_000), 1_290.0);
    assert_eq!(patch.cutoff_at(95_999), 2_399.976875);
    assert_eq!(patch.cutoff_at(96_000), 180.0);

    let block = patch.render(FrameRange::new(0, 96_000).unwrap()).unwrap();
    assert!(block.peak_abs() > 0.01);
    assert!(block.samples().iter().all(|sample| sample.is_finite()));
}

#[test]
fn automated_filter_patch_is_sample_identical_across_chunks() {
    let song =
        load_song_directory(example("02-synth-parameter-pattern")).expect("song should load");
    let patch = compile_filtered_note_song(&song).expect("filtered patch should compile");
    let whole = patch.render(FrameRange::new(0, 110_000).unwrap()).unwrap();
    let first = patch.render(FrameRange::new(0, 42_123).unwrap()).unwrap();
    let second = patch
        .render(FrameRange::new(42_123, 110_000).unwrap())
        .unwrap();
    let joined = first
        .samples()
        .iter()
        .chain(second.samples())
        .copied()
        .collect::<Vec<_>>();

    assert_eq!(joined, whole.samples());
}

#[test]
fn referenced_dsp_graph_processes_synth_audio_with_tempo_delay() {
    let song = load_song_directory(example("03-dsp-chain")).expect("song should load");
    let dry = compile_note_song(&song).expect("source patch should compile");
    let delayed = compile_delayed_note_song(&song).expect("DSP patch should compile");

    assert_eq!(delayed.delay_frames(), 12_000);
    assert_eq!(delayed.feedback(), 0.35);
    assert_eq!(delayed.mix(), 0.25);

    let dry = dry.render(FrameRange::new(0, 24_000).unwrap()).unwrap();
    let wet = delayed.render(FrameRange::new(0, 24_000).unwrap()).unwrap();
    let dry_gap_energy = dry.samples()[18_000..22_000]
        .iter()
        .map(|sample| sample * sample)
        .sum::<f64>();
    let wet_gap_energy = wet.samples()[18_000..22_000]
        .iter()
        .map(|sample| sample * sample)
        .sum::<f64>();
    assert!(wet_gap_energy > dry_gap_energy);
    assert!(wet.samples().iter().all(|sample| sample.is_finite()));
}

#[test]
fn tempo_delay_feedback_tail_is_sample_identical_across_chunks() {
    let song = load_song_directory(example("03-dsp-chain")).expect("song should load");
    let patch = compile_delayed_note_song(&song).expect("DSP patch should compile");
    let whole = patch.render(FrameRange::new(0, 125_000).unwrap()).unwrap();
    let first = patch.render(FrameRange::new(0, 53_777).unwrap()).unwrap();
    let second = patch
        .render(FrameRange::new(53_777, 125_000).unwrap())
        .unwrap();
    let joined = first
        .samples()
        .iter()
        .chain(second.samples())
        .copied()
        .collect::<Vec<_>>();

    assert_eq!(joined, whole.samples());
}

#[test]
fn dsp_parameter_pattern_steps_feedback_on_musical_boundaries() {
    let song = load_song_directory(example("04-dsp-parameter-pattern")).expect("song should load");
    let patch =
        compile_automated_delayed_note_song(&song).expect("automated DSP patch should compile");

    assert_eq!(patch.feedback_at(0), 0.15);
    assert_eq!(patch.feedback_at(23_999), 0.15);
    assert_eq!(patch.feedback_at(24_000), 0.35);
    assert_eq!(patch.feedback_at(48_000), 0.55);
    assert_eq!(patch.feedback_at(72_000), 0.25);
    assert_eq!(patch.feedback_at(96_000), 0.15);
    let block = patch.render(FrameRange::new(0, 110_000).unwrap()).unwrap();
    assert!(block.peak_abs() > 0.01);
    assert!(block.samples().iter().all(|sample| sample.is_finite()));
}

#[test]
fn automated_dsp_feedback_is_sample_identical_across_chunks() {
    let song = load_song_directory(example("04-dsp-parameter-pattern")).expect("song should load");
    let patch =
        compile_automated_delayed_note_song(&song).expect("automated DSP patch should compile");
    let whole = patch.render(FrameRange::new(0, 125_000).unwrap()).unwrap();
    let first = patch.render(FrameRange::new(0, 61_111).unwrap()).unwrap();
    let second = patch
        .render(FrameRange::new(61_111, 125_000).unwrap())
        .unwrap();
    let joined = first
        .samples()
        .iter()
        .chain(second.samples())
        .copied()
        .collect::<Vec<_>>();

    assert_eq!(joined, whole.samples());
}

#[test]
fn live_curated_feedback_override_wins_over_authored_automation_deterministically() {
    let song = load_song_directory(example("04-dsp-parameter-pattern")).expect("song should load");
    let patch =
        compile_automated_delayed_note_song(&song).expect("automated DSP patch should compile");
    let authored = patch.render(FrameRange::new(0, 96_000).unwrap()).unwrap();
    let overridden = patch
        .render_with_feedback_override(FrameRange::new(0, 96_000).unwrap(), Some(0.8))
        .unwrap();
    let repeated = patch
        .render_with_feedback_override(FrameRange::new(0, 96_000).unwrap(), Some(0.8))
        .unwrap();

    assert_ne!(overridden.samples(), authored.samples());
    assert_eq!(overridden, repeated);
    assert!(overridden.samples().iter().all(|sample| sample.is_finite()));
    assert!(
        patch
            .render_with_feedback_override(FrameRange::new(0, 1).unwrap(), Some(f64::NAN))
            .is_err()
    );
    assert!(
        patch
            .render_with_feedback_override(FrameRange::new(0, 1).unwrap(), Some(1.0))
            .is_err()
    );
}
