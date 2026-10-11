use meldritch_core::FrameRange;
use meldritch_dsl::{
    NoteEdit, PatchCreation, PatternSignal, PatternVoiceDefinition, ValidatedSong,
    load_project_directory,
};
use meldritch_render::{
    dsp::{Oscillator, Waveform},
    song_host::CompiledSongPatch,
};
use std::path::Path;

fn source() -> ValidatedSong {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../songs/examples/34-empty-project-patch");
    let mut song = load_project_directory(root)
        .unwrap()
        .authoring_source()
        .create_patch(&PatchCreation::mono_voice("voice-1"))
        .unwrap();
    for pattern in ["pitch", "gate"] {
        song = song.create_palette_pattern(pattern, 3840, true).unwrap();
    }
    for (pattern, start_tick, duration_ticks, pitch, velocity) in [
        ("pitch", 0, 480, 48, 0.8),
        ("pitch", 960, 480, 55, 0.2),
        ("gate", 0, 1920, 84, 0.1),
    ] {
        song = song
            .edit_note(
                pattern,
                &NoteEdit::Insert {
                    start_tick,
                    duration_ticks,
                    pitch,
                    velocity,
                },
            )
            .unwrap()
            .0;
    }
    song.create_pattern_voice(PatternVoiceDefinition {
        id: "voice-instance-1".into(),
        synth: "voice-1".into(),
        length_ticks: 3840,
        looped: true,
        initial_pitch: 48,
        level: 0.8,
        pitch_pattern: Some("pitch".into()),
        gate_pattern: Some("gate".into()),
    })
    .unwrap()
}

#[test]
fn independent_pitch_changes_under_a_held_gate_and_reuses_continuous_oscillator_phase() {
    let song = source();
    assert!(song.performance().tracks().is_empty());
    let patch = CompiledSongPatch::compile_initial(&song).unwrap();
    let audio = patch
        .render_default(FrameRange::new(0, 200_000).unwrap())
        .unwrap();
    let mut oscillator = Oscillator::new(Waveform::Sine);
    for (frame, sample) in audio.samples().iter().enumerate() {
        let position = frame % 96_000;
        let pitch = if position < 24_000 { 48.0 } else { 55.0 };
        let frequency = 440.0 * 2.0_f64.powf((pitch - 69.0) / 12.0);
        let wave = oscillator.next(frequency, 48_000);
        let expected = if position < 48_000 { wave * 0.8 } else { 0.0 };
        assert_eq!(
            *sample, expected,
            "independent pitch/gate/explicit level at frame {frame}"
        );
    }
    let mut split = Vec::new();
    for (start, end) in [(0, 11_000), (11_000, 97_331), (97_331, 200_000)] {
        split.extend_from_slice(
            patch
                .render_default(FrameRange::new(start, end).unwrap())
                .unwrap()
                .samples(),
        );
    }
    assert_eq!(split, audio.samples());
    assert!(audio.peak_abs() > 0.7);
}

#[test]
fn independent_disconnections_have_explicit_fallbacks_and_do_not_change_the_other_binding() {
    let song = source();
    let range = FrameRange::new(0, 96_000).unwrap();
    let no_gate = song
        .bind_pattern_voice("voice-instance-1", PatternSignal::Gate, None)
        .unwrap();
    assert_eq!(
        no_gate.pattern_voices()["voice-instance-1"].pitch_pattern,
        Some("pitch".into())
    );
    assert_eq!(no_gate.note_patterns(), song.note_patterns());
    assert_eq!(
        CompiledSongPatch::compile_initial(&no_gate)
            .unwrap()
            .render_default(range)
            .unwrap()
            .peak_abs(),
        0.0
    );
    let restored = no_gate
        .bind_pattern_voice("voice-instance-1", PatternSignal::Gate, Some("gate".into()))
        .unwrap();
    assert_eq!(restored.fingerprint(), song.fingerprint());
    let no_pitch = song
        .bind_pattern_voice("voice-instance-1", PatternSignal::Pitch, None)
        .unwrap();
    assert_eq!(
        no_pitch.pattern_voices()["voice-instance-1"].gate_pattern,
        Some("gate".into())
    );
    let audio = CompiledSongPatch::compile_initial(&no_pitch)
        .unwrap()
        .render_default(range)
        .unwrap();
    let mut oscillator = Oscillator::new(Waveform::Sine);
    let frequency = 440.0 * 2.0_f64.powf((48.0 - 69.0) / 12.0);
    for (frame, sample) in audio.samples().iter().enumerate() {
        let wave = oscillator.next(frequency, 48_000);
        assert_eq!(*sample, if frame < 48_000 { wave * 0.8 } else { 0.0 });
    }
}

#[test]
fn initial_pitch_and_loop_holds_are_explicit_and_adjacent_gates_retrigger_before_sampling() {
    use meldritch_dsl::{
        InstrumentEdit, InstrumentParameter, InstrumentParameterId, InstrumentValue,
    };
    use meldritch_render::dsp::{AdsrEnvelope, AdsrSettings};
    let mut song = source()
        .edit_note(
            "pitch",
            &NoteEdit::Delete {
                id: "note-1".into(),
            },
        )
        .unwrap()
        .0;
    song = song
        .edit_note(
            "gate",
            &NoteEdit::Duration {
                id: "note-1".into(),
                duration_ticks: 960,
            },
        )
        .unwrap()
        .0;
    song = song
        .edit_note(
            "gate",
            &NoteEdit::Insert {
                start_tick: 960,
                duration_ticks: 960,
                pitch: 84,
                velocity: 0.1,
            },
        )
        .unwrap()
        .0;
    for (parameter, value) in [
        (InstrumentParameter::Attack, 0.005),
        (InstrumentParameter::Release, 0.01),
    ] {
        song = song
            .edit_instrument(&InstrumentEdit {
                target: InstrumentParameterId {
                    synth: "voice-1".into(),
                    module: "env".into(),
                    parameter,
                },
                value: InstrumentValue::Number(value),
            })
            .unwrap();
    }
    let audio = CompiledSongPatch::compile_initial(&song)
        .unwrap()
        .render_default(FrameRange::new(0, 192_000).unwrap())
        .unwrap();
    let mut oscillator = Oscillator::new(Waveform::Sine);
    let mut envelope = AdsrEnvelope::new(
        AdsrSettings {
            attack_seconds: 0.005,
            decay_seconds: 0.0,
            sustain_level: 1.0,
            release_seconds: 0.01,
        },
        48_000,
    );
    for (frame, sample) in audio.samples().iter().enumerate() {
        let position = frame % 96_000;
        if position == 24_000 || position == 48_000 || (frame > 0 && position == 0) {
            envelope.note_off();
        }
        if position == 0 || position == 24_000 {
            envelope.note_on();
        }
        let pitch = if frame < 24_000 { 48.0 } else { 55.0 };
        let frequency = 440.0 * 2.0_f64.powf((pitch - 69.0) / 12.0);
        assert_eq!(
            *sample,
            oscillator.next(frequency, 48_000) * envelope.next_value() * 0.8,
            "initial/held pitch, retrigger and release at frame {frame}"
        );
    }
    assert!(
        audio.samples()[48_000..48_240]
            .iter()
            .any(|sample| sample.abs() > 0.001)
    );
    assert!(
        audio.samples()[48_480..96_000]
            .iter()
            .all(|sample| *sample == 0.0)
    );
}

#[test]
fn bound_voice_compilation_rejects_ambiguous_edits_and_incompatible_clocks() {
    let song = source();
    let overlapping = song
        .edit_note(
            "gate",
            &NoteEdit::Insert {
                start_tick: 480,
                duration_ticks: 480,
                pitch: 48,
                velocity: 0.8,
            },
        )
        .unwrap()
        .0;
    assert!(
        CompiledSongPatch::compile_initial(&overlapping)
            .unwrap_err()
            .contains("overlapping")
    );
    let simultaneous = song
        .edit_note(
            "pitch",
            &NoteEdit::Insert {
                start_tick: 0,
                duration_ticks: 480,
                pitch: 60,
                velocity: 0.8,
            },
        )
        .unwrap()
        .0;
    assert!(
        CompiledSongPatch::compile_initial(&simultaneous)
            .unwrap_err()
            .contains("simultaneous")
    );
    let wrong_clock = song.create_palette_pattern("short", 1920, true).unwrap();
    assert!(
        wrong_clock
            .bind_pattern_voice(
                "voice-instance-1",
                PatternSignal::Gate,
                Some("short".into())
            )
            .unwrap_err()
            .contains("length")
    );
    assert!(
        song.bind_pattern_voice("missing", PatternSignal::Gate, None)
            .is_err()
    );
    assert!(
        song.bind_pattern_voice(
            "voice-instance-1",
            PatternSignal::Pitch,
            Some("missing".into())
        )
        .is_err()
    );
    let voice = song.pattern_voices()["voice-instance-1"].clone();
    assert!(song.create_pattern_voice(voice.clone()).is_err());
    for level in [f64::NAN, -1.0, 1.1] {
        let mut invalid = voice.clone();
        invalid.id = "new".into();
        invalid.level = level;
        assert!(song.create_pattern_voice(invalid).is_err());
    }
}

#[test]
fn independent_bindings_render_on_the_worker_and_publish_to_the_production_callback() {
    use meldritch_audio::{
        AudioBlock,
        audio_publication::audio_publication,
        device_output::{PublishedAudioRenderer, playback_session_parts},
        published_audio::PublishedAudio,
    };
    use meldritch_render::song_host::{SongLiveOverrides, SongRerenderWorker, publish_song_audio};
    use std::time::{Duration, Instant};
    let song = source();
    let initial = PublishedAudio::from_block(&AudioBlock::silent(1, 96_000), 6000).unwrap();
    let (publisher, reader) = audio_publication(initial);
    let (playback, engine) = playback_session_parts(32).unwrap();
    let mut callback = PublishedAudioRenderer::from_engine(reader, 1, u32::MAX, engine).unwrap();
    let mut worker = SongRerenderWorker::new(96_000);
    let mut render = |generation, source: &ValidatedSong| {
        worker.submit(
            generation,
            CompiledSongPatch::compile_initial(source).unwrap(),
            SongLiveOverrides::default(),
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some((completed, audio, _)) = worker.latest_completed() {
                assert_eq!(completed, generation);
                let audio = audio.unwrap();
                publish_song_audio(&publisher, &audio, 6000).unwrap();
                break audio;
            }
            assert!(Instant::now() < deadline, "worker did not complete");
            std::thread::sleep(Duration::from_millis(2));
        }
    };
    let audible = render(1, &song);
    playback.play().unwrap();
    let mut first = vec![0.0; 257];
    assert!(!callback.render_f64(&mut first));
    assert_eq!(first, audible.samples()[..257]);
    let disconnected = song
        .bind_pattern_voice("voice-instance-1", PatternSignal::Gate, None)
        .unwrap();
    let silence = render(2, &disconnected);
    assert_eq!(silence.peak_abs(), 0.0);
    let mut second = vec![1.0; 257];
    callback.render_f64(&mut second);
    assert_eq!(second, vec![0.0; 257]);
    render(3, &song);
    let mut resumed = vec![0.0; 257];
    callback.render_f64(&mut resumed);
    assert_eq!(resumed, audible.samples()[514..771]);
    // Snapshot replacement preserved transport position, not continuous DSP state.
    assert_eq!(playback.status_monitor().snapshot().position, 771);
}
