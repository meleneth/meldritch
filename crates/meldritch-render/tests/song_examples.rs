use meldritch_core::FrameRange;
use meldritch_dsl::load_song_directory;
use meldritch_render::song_render::{
    CompiledSynthFilterOverride, compile_automated_delayed_note_song, compile_delayed_note_song,
    compile_delayed_note_song_for_pattern, compile_drone_song, compile_filtered_note_song,
    compile_mixed_note_song, compile_mixed_note_song_with_lane_state,
    compile_mixed_note_song_with_lane_transposes, compile_mixed_note_song_with_lane_variation,
    compile_note_song,
};
use std::collections::{BTreeMap, BTreeSet};
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

#[test]
fn curated_performance_control_song_compiles_renders_and_accepts_live_feedback() {
    let song = load_song_directory(example("09-curated-performance-controls"))
        .expect("curated-control song should load");
    let patch = compile_delayed_note_song(&song).expect("curated song DSP patch should compile");
    let baseline = patch.render(FrameRange::new(0, 96_000).unwrap()).unwrap();
    let adjusted = patch
        .render_with_feedback_override(FrameRange::new(0, 96_000).unwrap(), Some(0.75))
        .unwrap();

    assert_eq!(patch.feedback(), 0.35);
    assert_eq!(baseline.frames(), 96_000);
    assert!(baseline.peak_abs() > 0.01);
    assert_ne!(baseline.samples(), adjusted.samples());
    assert!(adjusted.samples().iter().all(|sample| sample.is_finite()));
}

#[test]
fn launch_control_playground_compiles_and_accepts_live_feedback_and_cutoff() {
    let song = load_song_directory(example("16-launch-control-xl-playground"))
        .expect("LaunchControl XL playground song should load");
    let patch = compile_delayed_note_song(&song).expect("playground DSP patch should compile");
    let baseline = patch.render(FrameRange::new(0, 96_000).unwrap()).unwrap();
    let adjusted = patch
        .render_with_overrides(FrameRange::new(0, 96_000).unwrap(), Some(0.75), Some(500.0))
        .unwrap();
    let tone_adjusted = patch
        .render_with_extended_overrides(
            FrameRange::new(0, 96_000).unwrap(),
            None,
            None,
            Some(0.85),
            Some(0.75),
        )
        .unwrap();

    assert_eq!(patch.feedback(), 0.35);
    assert_eq!(patch.mix(), 0.25);
    assert_eq!(patch.cutoff_hz(), Some(4350.0));
    assert_eq!(patch.resonance(), Some(0.2));
    assert_eq!(baseline.frames(), 96_000);
    assert!(baseline.peak_abs() > 0.01);
    assert_ne!(baseline.samples(), adjusted.samples());
    assert_ne!(baseline.samples(), tone_adjusted.samples());
    assert!(adjusted.samples().iter().all(|sample| sample.is_finite()));
    assert!(
        tone_adjusted
            .samples()
            .iter()
            .all(|sample| sample.is_finite())
    );

    let [track] = song.performance().tracks() else {
        panic!("playground should have one track");
    };
    for pattern in track.pattern_ids() {
        let patch = compile_delayed_note_song_for_pattern(&song, pattern)
            .expect("playground scene pattern should compile");
        let block = patch.render(FrameRange::new(0, 96_000).unwrap()).unwrap();
        assert!(block.peak_abs() > 0.01, "pattern {pattern} rendered silent");
    }
}

#[test]
fn launch_control_ensemble_compiles_multiple_tracks_into_one_mix() {
    let song = load_song_directory(example("17-launch-control-xl-ensemble"))
        .expect("LaunchControl XL ensemble song should load");
    let patch = compile_mixed_note_song(&song).expect("ensemble mix should compile");
    let block = patch.render(FrameRange::new(0, 96_000).unwrap()).unwrap();

    assert_eq!(patch.song_fingerprint(), song.fingerprint());
    assert_eq!(patch.track_count(), 9);
    assert_eq!(patch.sample_rate(), 48_000);
    assert_eq!(patch.channels(), 1);
    assert_eq!(block.frames(), 96_000);
    assert!(block.peak_abs() > 0.01);
    assert!(block.samples().iter().all(|sample| sample.is_finite()));
}

#[test]
fn launch_control_ensemble_mix_is_sample_identical_across_chunks() {
    let song = load_song_directory(example("17-launch-control-xl-ensemble"))
        .expect("LaunchControl XL ensemble song should load");
    let patch = compile_mixed_note_song(&song).expect("ensemble mix should compile");
    let whole = patch.render(FrameRange::new(0, 96_000).unwrap()).unwrap();
    let first = patch.render(FrameRange::new(0, 37_111).unwrap()).unwrap();
    let second = patch
        .render(FrameRange::new(37_111, 96_000).unwrap())
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
fn launch_control_ensemble_lane_variation_changes_one_track_in_the_mix() {
    let song = load_song_directory(example("17-launch-control-xl-ensemble"))
        .expect("LaunchControl XL ensemble song should load");
    let initial = compile_mixed_note_song(&song).expect("ensemble mix should compile");
    let varied =
        compile_mixed_note_song_with_lane_variation(&song, "rhythm-drum-a", "rhythm-drum-a-b")
            .expect("ensemble lane variation should compile");
    let initial_block = initial.render(FrameRange::new(0, 96_000).unwrap()).unwrap();
    let varied_block = varied.render(FrameRange::new(0, 96_000).unwrap()).unwrap();

    assert_eq!(varied.track_count(), 9);
    assert_ne!(initial_block.samples(), varied_block.samples());
    assert!(varied_block.peak_abs() > 0.01);
}

#[test]
fn launch_control_ensemble_lane_transpose_changes_the_mix() {
    let song = load_song_directory(example("17-launch-control-xl-ensemble"))
        .expect("LaunchControl XL ensemble song should load");
    let initial = compile_mixed_note_song(&song).expect("ensemble mix should compile");
    let transposed = compile_mixed_note_song_with_lane_transposes(
        &song,
        &BTreeMap::from([("rhythm-drum-a".to_owned(), 12)]),
    )
    .expect("ensemble transposed mix should compile");
    let initial_block = initial.render(FrameRange::new(0, 96_000).unwrap()).unwrap();
    let transposed_block = transposed
        .render(FrameRange::new(0, 96_000).unwrap())
        .unwrap();

    assert_eq!(transposed.track_count(), 9);
    assert_ne!(initial_block.samples(), transposed_block.samples());
    assert!(transposed_block.peak_abs() > 0.01);
    assert!(
        transposed_block
            .samples()
            .iter()
            .all(|sample| sample.is_finite())
    );
}

#[test]
fn launch_control_ensemble_lane_mute_and_solo_change_the_mix() {
    let song = load_song_directory(example("17-launch-control-xl-ensemble"))
        .expect("LaunchControl XL ensemble song should load");
    let initial = compile_mixed_note_song(&song).expect("ensemble mix should compile");
    let active_variations = song
        .performance()
        .lanes()
        .iter()
        .filter_map(|lane| Some((lane.id().to_owned(), lane.variation_ids().first()?.clone())))
        .collect::<BTreeMap<_, _>>();
    let all_lanes = song
        .performance()
        .lanes()
        .iter()
        .map(|lane| lane.id().to_owned())
        .collect::<BTreeSet<_>>();
    let without_rhythm_a = all_lanes
        .iter()
        .filter(|lane| lane.as_str() != "rhythm-drum-a")
        .cloned()
        .collect::<BTreeSet<_>>();
    let rhythm_a_solo = BTreeSet::from(["rhythm-drum-a".to_owned()]);
    let muted = compile_mixed_note_song_with_lane_state(
        &song,
        &active_variations,
        &BTreeMap::new(),
        &without_rhythm_a,
    )
    .expect("muted ensemble mix should compile");
    let soloed = compile_mixed_note_song_with_lane_state(
        &song,
        &active_variations,
        &BTreeMap::new(),
        &rhythm_a_solo,
    )
    .expect("soloed ensemble mix should compile");
    let initial_block = initial.render(FrameRange::new(0, 96_000).unwrap()).unwrap();
    let muted_block = muted.render(FrameRange::new(0, 96_000).unwrap()).unwrap();
    let soloed_block = soloed.render(FrameRange::new(0, 96_000).unwrap()).unwrap();

    assert_eq!(muted.track_count(), 8);
    assert_eq!(soloed.track_count(), 1);
    assert_ne!(initial_block.samples(), muted_block.samples());
    assert_ne!(initial_block.samples(), soloed_block.samples());
    assert!(muted_block.peak_abs() > 0.01);
    assert!(soloed_block.peak_abs() > 0.01);
}

#[test]
fn launch_control_ensemble_mixed_filter_override_changes_the_mix() {
    let song = load_song_directory(example("17-launch-control-xl-ensemble"))
        .expect("LaunchControl XL ensemble song should load");
    let patch = compile_mixed_note_song(&song).expect("ensemble mix should compile");
    let normal = patch.render(FrameRange::new(0, 96_000).unwrap()).unwrap();
    let filtered = patch
        .render_with_synth_filter_overrides(
            FrameRange::new(0, 96_000).unwrap(),
            &[CompiledSynthFilterOverride::new(
                "rhythm-drum-a",
                "filter",
                Some(100.0),
                None,
            )],
        )
        .unwrap();

    assert_ne!(normal.samples(), filtered.samples());
    assert!(filtered.samples().iter().all(|sample| sample.is_finite()));
}

#[test]
fn session_capture_example_compiles_to_the_same_playable_song_shape() {
    let song = load_song_directory(example("11-session-capture"))
        .expect("session-capture song should load");
    let patch = compile_delayed_note_song(&song).expect("session song DSP patch should compile");
    let block = patch.render(FrameRange::new(0, 96_000).unwrap()).unwrap();

    assert_eq!(patch.feedback(), 0.35);
    assert!(block.peak_abs() > 0.01);
    assert!(block.samples().iter().all(|sample| sample.is_finite()));
}
