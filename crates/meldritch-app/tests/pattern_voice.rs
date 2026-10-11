use meldritch_app::{
    AppCommand, AppController, NoteEdit, PatchCreation, PatternSignal, PatternVoiceDefinition,
};
use meldritch_core::FrameRange;
use meldritch_dsl::load_project_directory;
use meldritch_render::song_host::CompiledSongPatch;
use std::{path::Path, sync::Arc};

#[test]
fn declared_level_controls_are_instance_owned_runtime_overlays_not_authored_edits() {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../songs/examples/34-empty-project-patch");
    let project = load_project_directory(root).unwrap();
    let (mut app, _engine) = AppController::new_empty_project(&project).unwrap();
    app.dispatch(AppCommand::CreatePatch(PatchCreation::mono_voice("voice")))
        .unwrap();
    for id in ["a", "b"] {
        app.dispatch(AppCommand::CreatePatternVoice(PatternVoiceDefinition {
            id: id.into(),
            synth: "voice".into(),
            length_ticks: 3840,
            looped: true,
            initial_pitch: 48,
            level: 0.8,
            pitch_pattern: None,
            gate_pattern: None,
        }))
        .unwrap();
    }
    let set = |voice: &str, control: &str, value| AppCommand::SetOutputLevel {
        voice: voice.into(),
        control: control.into(),
        value,
    };
    assert!(app.dispatch(set("a", "level", Some(0.0))).is_err());
    let original = app.authored_source().unwrap();
    app.dispatch(AppCommand::ExposeOutputLevel {
        synth: "voice".into(),
        control: "level".into(),
    })
    .unwrap();
    let declared = app.authored_source().unwrap();
    assert_ne!(declared.fingerprint(), original.fingerprint());
    let history = app.history().len();
    for command in [
        AppCommand::ExposeOutputLevel {
            synth: "missing".into(),
            control: "level".into(),
        },
        AppCommand::ExposeOutputLevel {
            synth: "voice".into(),
            control: "../bad".into(),
        },
        AppCommand::ExposeOutputLevel {
            synth: "voice".into(),
            control: "renamed".into(),
        },
        set("missing", "level", Some(0.5)),
        set("a", "unexposed", Some(0.5)),
        set("a", "level", Some(f64::NAN)),
        set("a", "level", Some(-0.1)),
        set("a", "level", Some(1.1)),
    ] {
        assert!(app.dispatch(command).is_err());
        assert!(Arc::ptr_eq(&declared, &app.authored_source().unwrap()));
        assert_eq!(app.history().len(), history);
    }
    app.dispatch(set("a", "level", Some(0.4))).unwrap();
    assert_eq!(app.view_model().output_levels.len(), 1);
    assert_eq!(
        app.view_model().output_levels[&("a".into(), "level".into())],
        0.4
    );
    assert_eq!(
        app.authored_source().unwrap().pattern_voices()["b"].level,
        0.8
    );
    assert!(Arc::ptr_eq(&declared, &app.authored_source().unwrap()));
    for _ in 0..2 {
        app.handle_input(meldritch_app::AppInput::ToggleCockpitMode)
            .unwrap();
    }
    assert_eq!(app.view_model().output_levels.len(), 1);
    app.dispatch(set("b", "level", Some(0.2))).unwrap();
    app.dispatch(AppCommand::EditPatch(meldritch_app::PatchEdit {
        synth: "voice".into(),
        operations: vec![meldritch_app::PatchOperation::SetParameter {
            module: "osc".into(),
            parameter: meldritch_app::InstrumentParameter::Waveform,
            value: meldritch_app::InstrumentValue::Waveform(
                meldritch_app::InstrumentWaveform::Triangle,
            ),
        }],
    }))
    .unwrap();
    assert_eq!(
        app.authored_source().unwrap().synths()["voice"].output_level_control(),
        Some("level")
    );
    assert_eq!(app.view_model().output_levels.len(), 2);
    app.dispatch(set("a", "level", None)).unwrap();
    assert_eq!(app.view_model().output_levels.len(), 1);
    let history = app.history().len();
    let reloaded = declared
        .create_pattern_voice(PatternVoiceDefinition {
            id: "unsupported".into(),
            synth: "voice".into(),
            length_ticks: 3840,
            looped: true,
            initial_pitch: 48,
            level: 0.8,
            pitch_pattern: None,
            gate_pattern: None,
        })
        .unwrap();
    // Successful explicit source loading resets runtime values to authored defaults.
    app.enable_project_source(reloaded).unwrap();
    assert!(app.view_model().output_levels.is_empty());
    assert_eq!(app.history().len(), history);
}

#[test]
fn accepted_bindings_render_and_invalid_edits_preserve_source_provenance_and_history() {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../songs/examples/34-empty-project-patch");
    let project = load_project_directory(root).unwrap();
    let (mut app, _engine) = AppController::new_empty_project(&project).unwrap();
    app.dispatch(AppCommand::CreatePatch(PatchCreation::mono_voice(
        "voice-1",
    )))
    .unwrap();
    app.dispatch(AppCommand::CreatePalettePattern {
        id: "pattern-1".into(),
        length_ticks: 3840,
        looped: true,
    })
    .unwrap();
    for (start_tick, pitch) in [(0, 48), (960, 55)] {
        app.dispatch(AppCommand::EditNote {
            pattern: "pattern-1".into(),
            edit: NoteEdit::Insert {
                start_tick,
                duration_ticks: 480,
                pitch,
                velocity: 0.8,
            },
        })
        .unwrap();
    }
    app.dispatch(AppCommand::CreatePatternVoice(PatternVoiceDefinition {
        id: "voice-instance-1".into(),
        synth: "voice-1".into(),
        length_ticks: 3840,
        looped: true,
        initial_pitch: 48,
        level: 0.8,
        pitch_pattern: None,
        gate_pattern: None,
    }))
    .unwrap();
    for signal in [PatternSignal::Pitch, PatternSignal::Gate] {
        app.dispatch(AppCommand::BindPatternVoice {
            voice: "voice-instance-1".into(),
            signal,
            pattern: Some("pattern-1".into()),
        })
        .unwrap();
        assert!(app.history().back().unwrap().changed);
        assert_eq!(app.history().back().unwrap().dirty_ranges.len(), 1);
    }
    let accepted = app.authored_source().unwrap();
    let fingerprint = accepted.fingerprint();
    let range = FrameRange::new(0, 96_000).unwrap();
    let audio = CompiledSongPatch::compile_initial(&accepted)
        .unwrap()
        .render_default(range)
        .unwrap();
    assert!(audio.peak_abs() > 0.7);
    assert!(app.view_model().palette_pattern_editor.unwrap().visible);
    let history = app.history().len();
    for command in [
        AppCommand::BindPatternVoice {
            voice: "missing".into(),
            signal: PatternSignal::Gate,
            pattern: None,
        },
        AppCommand::BindPatternVoice {
            voice: "voice-instance-1".into(),
            signal: PatternSignal::Gate,
            pattern: Some("missing".into()),
        },
        AppCommand::EditNote {
            pattern: "pattern-1".into(),
            edit: NoteEdit::Insert {
                start_tick: 240,
                duration_ticks: 480,
                pitch: 60,
                velocity: 0.8,
            },
        },
        AppCommand::EditNote {
            pattern: "pattern-1".into(),
            edit: NoteEdit::Insert {
                start_tick: 0,
                duration_ticks: 480,
                pitch: 60,
                velocity: 0.8,
            },
        },
    ] {
        assert!(app.dispatch(command).is_err());
        assert!(Arc::ptr_eq(&accepted, &app.authored_source().unwrap()));
        assert_eq!(app.history().len(), history);
    }
    app.dispatch(AppCommand::BindPatternVoice {
        voice: "voice-instance-1".into(),
        signal: PatternSignal::Gate,
        pattern: Some("pattern-1".into()),
    })
    .unwrap();
    assert!(!app.history().back().unwrap().changed);
    assert!(app.history().back().unwrap().dirty_ranges.is_empty());
    assert!(Arc::ptr_eq(&accepted, &app.authored_source().unwrap()));
    app.dispatch(AppCommand::BindPatternVoice {
        voice: "voice-instance-1".into(),
        signal: PatternSignal::Gate,
        pattern: None,
    })
    .unwrap();
    assert_eq!(
        CompiledSongPatch::compile_initial(&app.authored_source().unwrap())
            .unwrap()
            .render_default(range)
            .unwrap()
            .peak_abs(),
        0.0
    );
    let (mut replay, _replay_engine) = AppController::new_empty_project(&project).unwrap();
    for record in app.history() {
        replay.dispatch(record.command.clone()).unwrap();
    }
    assert_eq!(
        app.authored_source().unwrap().fingerprint(),
        replay.authored_source().unwrap().fingerprint()
    );
    app.dispatch(AppCommand::BindPatternVoice {
        voice: "voice-instance-1".into(),
        signal: PatternSignal::Gate,
        pattern: Some("pattern-1".into()),
    })
    .unwrap();
    assert_eq!(app.authored_source().unwrap().fingerprint(), fingerprint);
    assert_eq!(
        CompiledSongPatch::compile_initial(&app.authored_source().unwrap())
            .unwrap()
            .render_default(range)
            .unwrap()
            .samples(),
        audio.samples()
    );
    app.dispatch(AppCommand::EditPatch(meldritch_app::PatchEdit {
        synth: "voice-1".into(),
        operations: vec![meldritch_app::PatchOperation::SetParameter {
            module: "osc".into(),
            parameter: meldritch_app::InstrumentParameter::Waveform,
            value: meldritch_app::InstrumentValue::Waveform(
                meldritch_app::InstrumentWaveform::Triangle,
            ),
        }],
    }))
    .unwrap();
    assert_eq!(app.history().back().unwrap().dirty_ranges.len(), 1);
    assert_ne!(
        CompiledSongPatch::compile_initial(&app.authored_source().unwrap())
            .unwrap()
            .render_default(range)
            .unwrap()
            .samples(),
        audio.samples()
    );
}
