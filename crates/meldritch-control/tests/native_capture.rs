use meldritch_app::{
    AppCommand, AppController, CockpitMode, NoteEdit, PatchCreation, PatternSignal,
    PatternVoiceDefinition,
};
use meldritch_control::native_capture::{
    NativeCommandEnvelope, NativeProjectCapture, load_native_capture_baseline,
};
use meldritch_core::FrameRange;
use meldritch_render::song_host::{CompiledSongPatch, SongLiveOverrides};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "meldritch-native-capture-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn app(&self) -> AppController {
        AppController::new_empty_project(&meldritch_dsl::load_project_directory(&self.0).unwrap())
            .unwrap()
            .0
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn accept(app: &mut AppController, capture: &mut NativeProjectCapture, command: AppCommand) {
    app.dispatch(command).unwrap();
    capture.checkpoint(app).unwrap();
    assert!(
        capture
            .session
            .as_ref()
            .is_none_or(|session| session.events.len() <= 8)
    );
}

#[test]
fn lazy_typed_capture_reconstructs_source_runtime_and_pcm_after_history_eviction() {
    let directory = Directory::new();
    let mut app = directory.app();
    let mut capture = NativeProjectCapture::new(app.authored_source().unwrap(), 96_000);
    capture.checkpoint(&app).unwrap();
    capture.finish(&app).unwrap();
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 0);
    for command in [
        AppCommand::CreatePatch(PatchCreation::mono_voice("voice-1")),
        AppCommand::SelectInstrument("voice-1".into()),
        AppCommand::CreatePalettePattern {
            id: "pattern-1".into(),
            length_ticks: 3840,
            looped: true,
        },
        AppCommand::EditNote {
            pattern: "pattern-1".into(),
            edit: NoteEdit::Insert {
                start_tick: 0,
                duration_ticks: 960,
                pitch: 48,
                velocity: 0.8,
            },
        },
        AppCommand::CreatePatternVoice(PatternVoiceDefinition {
            id: "voice-1-instance".into(),
            synth: "voice-1".into(),
            length_ticks: 3840,
            looped: true,
            initial_pitch: 48,
            level: 0.8,
            pitch_pattern: None,
            gate_pattern: None,
            velocity_pattern: None,
            sustain_pattern: None,
        }),
        AppCommand::BindPatternVoice {
            voice: "voice-1-instance".into(),
            signal: PatternSignal::Pitch,
            pattern: Some("pattern-1".into()),
        },
        AppCommand::BindPatternVoice {
            voice: "voice-1-instance".into(),
            signal: PatternSignal::Gate,
            pattern: Some("pattern-1".into()),
        },
        AppCommand::ExposeOutputLevel {
            synth: "voice-1".into(),
            control: "level".into(),
        },
        AppCommand::SetCockpitMode(CockpitMode::Performance),
        AppCommand::SetOutputLevel {
            voice: "voice-1-instance".into(),
            control: "level".into(),
            value: Some(0.12345678901234567),
        },
        AppCommand::SetCockpitMode(CockpitMode::AllParameters),
        AppCommand::TogglePatternEditor,
        AppCommand::MovePatternCursor {
            tick_delta: 480,
            pitch_delta: 7,
        },
        AppCommand::SelectNote {
            pattern: "pattern-1".into(),
            id: "note-1".into(),
        },
        AppCommand::SavePattern,
        AppCommand::SaveInstrument,
        AppCommand::ToggleInstrumentEditor,
        AppCommand::TogglePatchEditor,
    ] {
        accept(&mut app, &mut capture, command);
    }
    // Exercise drafts and a successful atomic submit, not just direct edits.
    for line in ["set osc waveform triangle", "commit"] {
        for character in line.chars() {
            accept(
                &mut app,
                &mut capture,
                AppCommand::PatchCharacter(character),
            );
        }
        accept(&mut app, &mut capture, AppCommand::PatchSubmit);
    }
    for _ in 0..300 {
        accept(&mut app, &mut capture, AppCommand::PatchScroll(1));
    }
    let source = app.authored_source().unwrap();
    let history = app.history().len();
    assert!(
        app.dispatch(AppCommand::SetOutputLevel {
            voice: "voice-1-instance".into(),
            control: "level".into(),
            value: Some(f64::NAN)
        })
        .is_err()
    );
    capture.checkpoint(&app).unwrap();
    assert_eq!(app.history().len(), history);
    assert!(Arc::ptr_eq(&source, &app.authored_source().unwrap()));
    capture.finish(&app).unwrap();
    let session = capture.session.as_ref().unwrap();
    assert!(session.events.is_empty());
    let text = fs::read_to_string(session.path()).unwrap();
    let document: toml::Table = toml::from_str(&text).unwrap();
    assert_eq!(
        document["meldritch"]["kind"].as_str(),
        Some("performance_session")
    );
    assert_eq!(
        document["session"]["native_command_version"].as_integer(),
        Some(1)
    );
    assert_eq!(
        document["session"]["clean_termination"].as_bool(),
        Some(true)
    );
    assert_eq!(
        document["session"]["final_source_fingerprint"].as_str(),
        Some(format!("{:016x}", source.fingerprint()).as_str())
    );
    let events = document["events"].as_array().unwrap();
    assert!(events.len() > app.history().len());
    assert!(events.iter().any(
        |event| event["kind"].as_str() == Some("runtime_output_level")
            && event["current"].as_str() == Some("0.12345678901234566")
    ));
    let mut reconstructed = directory.app();
    reconstructed
        .enable_native_project_source(
            load_native_capture_baseline(
                &directory.0,
                &fs::read_to_string(capture.session.as_ref().unwrap().path()).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    for (sequence, event) in events.iter().enumerate() {
        assert_eq!(event["sequence"].as_integer(), Some(sequence as i64));
        let payload =
            NativeCommandEnvelope::decode(&toml::to_string(&event["native"]).unwrap()).unwrap();
        assert_eq!(
            payload.source_before,
            format!(
                "{:016x}",
                reconstructed.authored_source().unwrap().fingerprint()
            )
        );
        reconstructed.dispatch(payload.command.into()).unwrap();
        assert_eq!(
            payload.source_after,
            format!(
                "{:016x}",
                reconstructed.authored_source().unwrap().fingerprint()
            )
        );
        assert_eq!(
            reconstructed.history().back().unwrap().changed,
            event["changed"].as_bool().unwrap()
        );
    }
    assert_eq!(
        reconstructed.authored_source().unwrap().fingerprint(),
        source.fingerprint()
    );
    assert_eq!(reconstructed.output_levels(), app.output_levels());
    assert_eq!(
        reconstructed
            .view_model()
            .instrument_editor
            .unwrap()
            .patch_scroll,
        app.view_model().instrument_editor.unwrap().patch_scroll
    );
    let mut overrides = SongLiveOverrides::default();
    overrides
        .voice_levels
        .insert("voice-1-instance".into(), 0.12345678901234567);
    let range = FrameRange::new(0, 96_000).unwrap();
    let expected = CompiledSongPatch::compile_initial(&source)
        .unwrap()
        .render(range, &overrides)
        .unwrap();
    let actual = CompiledSongPatch::compile_initial(&reconstructed.authored_source().unwrap())
        .unwrap()
        .render(range, &overrides)
        .unwrap();
    assert_eq!(actual.samples(), expected.samples());
    assert!(actual.peak_abs() > 0.1);
    assert!(
        meldritch_dsl::load_project_directory(&directory.0)
            .unwrap()
            .composition()
            .is_none()
    );
}

#[test]
fn unsupported_baseline_fails_before_creating_capture_files() {
    let directory = Directory::new();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../songs/examples/09-curated-performance-controls");
    for relative in [
        "main.mlperformance",
        "patterns/bass-line.mlpattern",
        "synths/bass.mlsynth",
        "synths/drone.mlsynth",
        "dsp/echo.mldsp",
    ] {
        let destination = directory.0.join(relative);
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        fs::copy(fixture.join(relative), destination).unwrap();
    }
    let source = meldritch_dsl::load_song_directory(&directory.0).unwrap();
    let fingerprint = source.fingerprint();
    assert!(
        meldritch_control::session::PerformanceSessionCapture::create_native_at(
            &source,
            96_000,
            "2026-10-11T00-00-00Z"
        )
        .is_err()
    );
    assert_eq!(source.fingerprint(), fingerprint);
    assert!(!directory.0.join("performances").exists());
}

#[test]
fn captured_baseline_survives_multiple_authored_saves_and_root_relocation() {
    use meldritch_control::native_project::{NativeProjectStore, load_native_project};
    let origin = Directory::new();
    let project = meldritch_dsl::load_project_directory(&origin.0).unwrap();
    let mut store = NativeProjectStore::open(&project).unwrap();
    let a = load_native_project(
        &origin.0,
        include_str!("../../../songs/examples/34-empty-project-patch/authored-project.lua"),
    )
    .unwrap();
    store.save(&a).unwrap();
    let mut app = origin.app();
    app.enable_native_project_source(a.clone()).unwrap();
    let mut capture = NativeProjectCapture::new(app.authored_source().unwrap(), 96_000);
    for duration_ticks in [960, 1440] {
        accept(
            &mut app,
            &mut capture,
            AppCommand::EditNote {
                pattern: "melody".into(),
                edit: NoteEdit::Duration {
                    id: "note-2".into(),
                    duration_ticks,
                },
            },
        );
        accept(&mut app, &mut capture, AppCommand::SaveProject);
        store.save(&app.authored_source().unwrap()).unwrap();
    }
    accept(
        &mut app,
        &mut capture,
        AppCommand::SetOutputLevel {
            voice: "voice-1-instance".into(),
            control: "level".into(),
            value: Some(0.12345678901234567),
        },
    );
    capture.finish(&app).unwrap();
    let captured = fs::read_to_string(capture.session.as_ref().unwrap().path()).unwrap();
    let final_source = app.authored_source().unwrap();
    let mut overrides = SongLiveOverrides::default();
    overrides
        .voice_levels
        .insert("voice-1-instance".into(), 0.12345678901234567);
    let range = FrameRange::new(0, 192_000).unwrap();
    let expected = CompiledSongPatch::compile_initial(&final_source)
        .unwrap()
        .render(range, &overrides)
        .unwrap();
    let fingerprint_a = a.fingerprint();
    drop(capture);
    drop(app);
    drop(store);
    drop(origin);
    let destination = Directory::new();
    let baseline = load_native_capture_baseline(&destination.0, &captured).unwrap();
    assert_eq!(baseline.fingerprint(), fingerprint_a);
    assert_ne!(baseline.fingerprint(), final_source.fingerprint());
    let mut restored = destination.app();
    restored.enable_native_project_source(baseline).unwrap();
    let document: toml::Value = toml::from_str(&captured).unwrap();
    for event in document["events"].as_array().unwrap() {
        let payload =
            NativeCommandEnvelope::decode(&toml::to_string(&event["native"]).unwrap()).unwrap();
        assert_eq!(
            payload.source_before,
            format!("{:016x}", restored.authored_source().unwrap().fingerprint())
        );
        restored.dispatch(payload.command.into()).unwrap();
        assert_eq!(
            payload.source_after,
            format!("{:016x}", restored.authored_source().unwrap().fingerprint())
        );
    }
    assert_eq!(
        restored
            .output_levels()
            .values()
            .copied()
            .collect::<Vec<_>>(),
        vec![0.12345678901234567]
    );
    let actual = CompiledSongPatch::compile_initial(&restored.authored_source().unwrap())
        .unwrap()
        .render(range, &overrides)
        .unwrap();
    assert_eq!(actual.samples(), expected.samples());
    assert_eq!(fs::read_dir(&destination.0).unwrap().count(), 0);
    for (field, value) in [
        ("native_source", toml::Value::String("return {}".into())),
        (
            "native_source",
            toml::Value::String(" ".repeat(256 * 1024 + 1)),
        ),
        (
            "source_fingerprint",
            toml::Value::String("0000000000000000".into()),
        ),
        ("native_command_version", toml::Value::Integer(2)),
    ] {
        let mut invalid = document.clone();
        invalid.as_table_mut().unwrap().remove("events");
        invalid["session"][field] = value;
        assert!(
            load_native_capture_baseline(&destination.0, &toml::to_string(&invalid).unwrap())
                .is_err()
        );
    }
    let mut missing = document.clone();
    missing.as_table_mut().unwrap().remove("events");
    missing["session"]
        .as_table_mut()
        .unwrap()
        .remove("native_source");
    assert!(
        load_native_capture_baseline(&destination.0, &toml::to_string(&missing).unwrap())
            .unwrap_err()
            .contains("original source")
    );
    assert!(load_native_capture_baseline(&destination.0, &" ".repeat(1024 * 1024 + 1)).is_err());
    assert_eq!(fs::read_dir(&destination.0).unwrap().count(), 0);
}

#[test]
fn failed_checkpoint_retains_last_good_file_and_bounded_pending_events_without_duplicates() {
    let directory = Directory::new();
    let mut app = directory.app();
    let mut capture = NativeProjectCapture::new(app.authored_source().unwrap(), 96_000);
    accept(
        &mut app,
        &mut capture,
        AppCommand::CreatePatch(PatchCreation::mono_voice("voice-1")),
    );
    let session = capture.session.as_ref().unwrap();
    let path = session.path().to_owned();
    let temporary = session.temp_path.clone();
    let last_good = fs::read(&path).unwrap();
    fs::create_dir(&temporary).unwrap();
    app.dispatch(AppCommand::SelectInstrument("voice-1".into()))
        .unwrap();
    for _ in 0..12 {
        app.dispatch(AppCommand::NavigateInstrument {
            synth_delta: 0,
            parameter_delta: 1,
        })
        .unwrap();
        assert!(capture.checkpoint(&app).is_err());
        assert!(capture.session.as_ref().unwrap().events.len() <= 8);
        assert_eq!(fs::read(&path).unwrap(), last_good);
    }
    let source = app.authored_source().unwrap();
    fs::remove_dir(&temporary).unwrap();
    capture.checkpoint(&app).unwrap();
    assert!(Arc::ptr_eq(&source, &app.authored_source().unwrap()));
    let document: toml::Table = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        document["events"].as_array().unwrap().len(),
        app.history().len()
    );
    assert_eq!(
        document["session"]["clean_termination"].as_bool(),
        Some(false)
    );
    // A failed clean marker must never get published by a later running poll.
    fs::create_dir(&temporary).unwrap();
    assert!(capture.finish(&app).is_err());
    assert!(!capture.session.as_ref().unwrap().clean_termination);
    fs::remove_dir(&temporary).unwrap();
    capture.checkpoint(&app).unwrap();
    assert_eq!(toml::from_str::<toml::Table>(&fs::read_to_string(&path).unwrap()).unwrap()["session"]["clean_termination"].as_bool(), Some(false));
    capture.finish(&app).unwrap();
    assert_eq!(toml::from_str::<toml::Table>(&fs::read_to_string(&path).unwrap()).unwrap()["session"]["clean_termination"].as_bool(), Some(true));
}

#[test]
fn evicted_history_and_unrecorded_source_loading_are_diagnosed_before_file_creation() {
    let directory = Directory::new();
    let mut app = directory.app();
    let mut capture = NativeProjectCapture::new(app.authored_source().unwrap(), 96_000);
    for _ in 0..300 {
        app.dispatch(AppCommand::SetCockpitMode(CockpitMode::AllParameters))
            .unwrap();
    }
    assert!(
        capture
            .checkpoint(&app)
            .unwrap_err()
            .contains("history gap")
    );
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 0);
    let mut app = directory.app();
    let mut capture = NativeProjectCapture::new(app.authored_source().unwrap(), 96_000);
    let modified = app
        .authored_source()
        .unwrap()
        .create_patch(&PatchCreation::mono_voice("voice-1"))
        .unwrap();
    app.enable_project_source(modified).unwrap();
    app.dispatch(AppCommand::SelectInstrument("voice-1".into()))
        .unwrap();
    assert!(
        capture
            .checkpoint(&app)
            .unwrap_err()
            .contains("continuity mismatch")
    );
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 0);
}

#[test]
fn typed_envelope_rejects_unknown_fields_versions_and_bad_fingerprints() {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../songs/examples/34-empty-project-patch");
    let invalid = fs::read_to_string(root.join("invalid/capture-command.toml")).unwrap();
    assert!(NativeCommandEnvelope::decode(&invalid).is_err());
    let valid = fs::read_to_string(root.join("capture-command.toml")).unwrap();
    assert!(NativeCommandEnvelope::decode(&valid).is_ok());
    assert!(
        NativeCommandEnvelope::decode(&valid.replacen("version = 1", "version = 2", 1)).is_err()
    );
    assert!(
        NativeCommandEnvelope::decode(&valid.replace("0000000000000000", "not-a-fingerprint"))
            .is_err()
    );
}

#[test]
fn capture_size_limit_and_finished_lifecycle_preserve_pending_facts() {
    let directory = Directory::new();
    let mut app = directory.app();
    let mut capture = NativeProjectCapture::new(app.authored_source().unwrap(), 96_000);
    accept(
        &mut app,
        &mut capture,
        AppCommand::CreatePatch(PatchCreation::mono_voice("voice-1")),
    );
    let path = capture.session.as_ref().unwrap().path().to_owned();
    let original_bytes = fs::metadata(&path).unwrap().len();
    // Sparse extension tests the bound without retaining a huge session in RAM.
    let file = fs::OpenOptions::new().write(true).open(&path).unwrap();
    file.set_len(64 * 1024 * 1024).unwrap();
    app.dispatch(AppCommand::SelectInstrument("voice-1".into()))
        .unwrap();
    assert!(capture.checkpoint(&app).unwrap_err().contains("64 MiB"));
    assert_eq!(capture.session.as_ref().unwrap().events.len(), 1);
    assert_eq!(fs::metadata(&path).unwrap().len(), 64 * 1024 * 1024);
    assert!(!capture.session.as_ref().unwrap().temp_path.exists());
    file.set_len(original_bytes).unwrap();
    capture.finish(&app).unwrap();
    let final_bytes = fs::read(&path).unwrap();
    app.dispatch(AppCommand::SetCockpitMode(CockpitMode::Performance))
        .unwrap();
    assert!(
        capture
            .checkpoint(&app)
            .unwrap_err()
            .contains("cleanly finished")
    );
    assert_eq!(fs::read(&path).unwrap(), final_bytes);
}

#[cfg(unix)]
#[test]
fn escaped_capture_directories_and_foreign_temporary_symlinks_are_never_written() {
    use std::os::unix::fs::symlink;
    let directory = Directory::new();
    let outside = Directory::new();
    let initial = Directory::new();
    let source = initial.app().authored_source().unwrap();
    fs::create_dir(initial.0.join("performances")).unwrap();
    let foreign_initial = outside.0.join("initial-foreign");
    fs::write(&foreign_initial, "keep initial too").unwrap();
    let foreign_temporary = initial
        .0
        .join("performances/session-20261011T120000Z.mlperformance.tmp");
    symlink(&foreign_initial, &foreign_temporary).unwrap();
    assert!(
        meldritch_control::session::PerformanceSessionCapture::create_native_at(
            &source,
            96_000,
            "20261011T120000Z"
        )
        .is_err()
    );
    assert_eq!(
        fs::read_to_string(&foreign_initial).unwrap(),
        "keep initial too"
    );
    assert_eq!(
        fs::read_dir(initial.0.join("performances"))
            .unwrap()
            .count(),
        1
    );
    let mut app = directory.app();
    let mut capture = NativeProjectCapture::new(app.authored_source().unwrap(), 96_000);
    symlink(&outside.0, directory.0.join("performances")).unwrap();
    app.dispatch(AppCommand::CreatePatch(PatchCreation::mono_voice(
        "voice-1",
    )))
    .unwrap();
    assert!(
        capture
            .checkpoint(&app)
            .unwrap_err()
            .contains("escapes project root")
    );
    assert_eq!(fs::read_dir(&outside.0).unwrap().count(), 1);
    assert_eq!(
        fs::read_to_string(&foreign_initial).unwrap(),
        "keep initial too"
    );
    fs::remove_file(directory.0.join("performances")).unwrap();
    capture.checkpoint(&app).unwrap();
    let temporary = capture.session.as_ref().unwrap().temp_path.clone();
    let foreign = outside.0.join("foreign");
    fs::write(&foreign, "keep me").unwrap();
    symlink(&foreign, &temporary).unwrap();
    app.dispatch(AppCommand::SelectInstrument("voice-1".into()))
        .unwrap();
    assert!(capture.checkpoint(&app).is_err());
    assert_eq!(fs::read_to_string(&foreign).unwrap(), "keep me");
    fs::remove_file(&temporary).unwrap();
    capture.finish(&app).unwrap();
}
