#![allow(dead_code)]
#[path = "../examples/support/authoring.rs"]
mod support;
use crossterm::event::{KeyCode, KeyModifiers};
use meldritch_app::*;
use meldritch_control::project::*;
use std::path::PathBuf;
fn output() -> PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "meldritch-composed-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ))
}
#[test]
fn create_change_publish_save_exit_fresh_process_reload_exact_music() {
    let output = output();
    let song = support::run(&output);
    assert_eq!(
        std::fs::read_to_string(song.join("edited-project.lua")).unwrap(),
        std::fs::read_to_string(support::fixture().join("edited-project.lua")).unwrap()
    );
    let child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "fresh_process_reload_child",
            "--nocapture",
        ])
        .env("MELDRITCH_ACCEPTANCE_SONG", &song)
        .env("MELDRITCH_ACCEPTANCE_OUTPUT", &output)
        .output()
        .unwrap();
    assert!(
        child.status.success(),
        "{}",
        String::from_utf8_lossy(&child.stderr)
    );
    for (before, after) in [
        ("saved-state.json", "reloaded-state.json"),
        ("saved.f64le", "reloaded.f64le"),
    ] {
        assert_eq!(
            std::fs::read(output.join(before)).unwrap(),
            std::fs::read(output.join(after)).unwrap()
        );
    }
    std::fs::remove_dir_all(output).unwrap();
}
#[test]
#[ignore = "subprocess entry point called by the composed workflow test"]
fn fresh_process_reload_child() {
    let song =
        PathBuf::from(std::env::var_os("MELDRITCH_ACCEPTANCE_SONG").expect("child song path"));
    let output =
        PathBuf::from(std::env::var_os("MELDRITCH_ACCEPTANCE_OUTPUT").expect("child output path"));
    support::reload(&song, &output);
}
#[test]
fn invalid_commands_preserve_source_history_and_publication() {
    let mut h = support::piano::Harness::from_fixture(&support::fixture());
    let commands = [
        AppCommand::EditInstrument(InstrumentEdit {
            target: InstrumentParameterId {
                synth: "bass".into(),
                module: "missing".into(),
                parameter: InstrumentParameter::CutoffHz,
            },
            value: InstrumentValue::Number(1000.0),
        }),
        AppCommand::EditInstrument(InstrumentEdit {
            target: InstrumentParameterId {
                synth: "bass".into(),
                module: "filter".into(),
                parameter: InstrumentParameter::CutoffHz,
            },
            value: InstrumentValue::Waveform(InstrumentWaveform::Sine),
        }),
        AppCommand::EditInstrument(InstrumentEdit {
            target: InstrumentParameterId {
                synth: "bass".into(),
                module: "filter".into(),
                parameter: InstrumentParameter::Resonance,
            },
            value: InstrumentValue::Number(2.0),
        }),
        AppCommand::EditParameter(ParameterEdit {
            target: EditableTarget::Synth {
                id: "bass".into(),
                module: "osc".into(),
                parameter: InstrumentParameter::Waveform,
            },
            value: 0.5,
        }),
        AppCommand::EditNote {
            pattern: "seed".into(),
            edit: NoteEdit::Duration {
                id: "missing".into(),
                duration_ticks: 480,
            },
        },
        AppCommand::AssignFader {
            id: "fader-1".into(),
            target: Some("missing".into()),
        },
        AppCommand::EditPatch(PatchEdit {
            synth: "bass".into(),
            operations: vec![PatchOperation::Remove {
                id: "filter".into(),
                cables: meldritch_dsl::DependentCables::Disconnect,
            }],
        }),
    ];
    for command in commands {
        let state = support::state(&h);
        let history = h.app.history().len();
        let published = h.reader.snapshot();
        assert!(h.app.dispatch(command).is_err());
        assert_eq!(support::state(&h), state);
        assert_eq!(h.app.history().len(), history);
        assert!(std::sync::Arc::ptr_eq(&published, &h.reader.snapshot()));
    }
}
#[test]
fn disconnect_cancels_a_held_duration_and_late_releases_do_not_edit_music() {
    let mut h = support::piano::Harness::from_fixture(&support::fixture());
    h.run_script("press 116\nrelease 116\nwait\npress 0\npress 3\nwait\ndisconnect");
    let before = support::state(&h);
    h.run_script("release 0\nzero 3\nconnect");
    assert_eq!(support::state(&h), before);
    h.run_script("press 5\nrelease 5\nwait\nexpect_count 2\nexpect_note note-1 C3 0 1920 0.8\nexpect_note note-2 C3 2400 480 0.8");
}
#[test]
fn failed_save_reports_unsaved_music_and_preserves_prior_export_and_journal() {
    let mut h = support::piano::Harness::from_fixture(&support::fixture());
    let good = save_project(&h.app.view_model()).unwrap();
    let previous = std::fs::read_to_string(&good).unwrap();
    std::fs::write(h.root.join("good.lua"), &previous).unwrap();
    std::fs::rename(h.root.join("authored"), h.root.join("previous-authored")).unwrap();
    std::fs::write(h.root.join("authored"), "occupied").unwrap();
    let journal = std::fs::read(h.capture.path()).unwrap();
    let state = support::state(&h);
    let error = support::key(
        &mut h,
        KeyCode::Char('S'),
        KeyModifiers::CONTROL | KeyModifiers::SHIFT,
    )
    .unwrap_err();
    assert!(!error.is_empty());
    assert_eq!(support::state(&h), state);
    let view = h.app.view_model();
    for message in [
        view.instrument_editor.unwrap().save_status,
        view.pattern_editor.unwrap().save_status,
        view.assignments.unwrap().save_status,
    ] {
        assert!(message.unwrap().contains("edits remain unsaved"));
    }
    assert_eq!(
        std::fs::read_to_string(h.root.join("authored")).unwrap(),
        "occupied"
    );
    assert_eq!(
        std::fs::read_to_string(h.root.join("good.lua")).unwrap(),
        previous
    );
    assert!(load_project(&h.root, &previous).is_ok());
    // Recording the accepted save request may checkpoint later, but never overwrites authored Lua.
    assert_eq!(std::fs::read(h.capture.path()).unwrap(), journal);
}
#[test]
fn stale_draft_rejects_without_replacing_current_graph() {
    let mut h = support::piano::Harness::from_fixture(&support::fixture());
    support::key(&mut h, KeyCode::Char('e'), KeyModifiers::CONTROL).unwrap();
    support::key(&mut h, KeyCode::Char('g'), KeyModifiers::CONTROL).unwrap();
    support::line(&mut h, "disconnect filter.audio out.audio").unwrap();
    h.app
        .dispatch(AppCommand::EditParameter(ParameterEdit {
            target: EditableTarget::Track {
                id: "bass".into(),
                parameter: meldritch_dsl::MixerParameter::Volume,
            },
            value: 0.5,
        }))
        .unwrap();
    let before = support::state(&h);
    assert!(
        support::line(&mut h, "commit")
            .unwrap_err()
            .contains("base changed")
    );
    assert_eq!(support::state(&h), before);
}
#[test]
fn reload_rejects_stale_or_invalid_composition_and_uncaptured_edits() {
    let mut h = support::piano::Harness::from_fixture(&support::fixture());
    let saved = std::fs::read_to_string(support::fixture().join("edited-project.lua")).unwrap();
    for file in ["unknown-version.lua", "stale-binding.lua"] {
        let invalid =
            std::fs::read_to_string(support::fixture().join("invalid").join(file)).unwrap();
        assert!(load_project(&h.root, &invalid).is_err(), "{file}");
    }
    assert!(load_project_file(&h.root, std::path::Path::new("missing.lua")).is_err());
    let outside = h
        .root
        .parent()
        .unwrap()
        .join(format!("escaped-project-{}.lua", std::process::id()));
    std::fs::write(&outside, &saved).unwrap();
    assert!(
        load_project_file(&h.root, &outside)
            .unwrap_err()
            .contains("escapes")
    );
    std::fs::remove_file(outside).unwrap();
    for invalid in [
        saved.replacen("version = 1", "version = 99", 1),
        saved.replace("from = \"source.audio\"", "from = \"missing.audio\""),
        saved.replace("duration_ticks = 1920", "duration_ticks = 0"),
        saved.replace("target = \"cutoff\"", "target = \"missing\""),
        saved.replacen("base_fingerprint = \"", "base_fingerprint = \"bad-", 1),
        saved.replace("value = 6000.0", "value = 5000.0"),
    ] {
        assert!(load_project(&h.root, &invalid).is_err(), "{invalid}");
    }
    let original = std::fs::read_to_string(h.root.join("main.mlperformance")).unwrap();
    std::fs::write(
        h.root.join("main.mlperformance"),
        original.replace("bpm = 120.0", "bpm = 121.0"),
    )
    .unwrap();
    assert!(
        load_project(&h.root, &saved)
            .unwrap_err()
            .contains("fingerprint")
    );
    std::fs::write(h.root.join("main.mlperformance"), original).unwrap();
    // Two edited patterns exceed the deliberately selected single-pattern envelope.
    h.app
        .dispatch(AppCommand::EditNote {
            pattern: "seed".into(),
            edit: NoteEdit::Insert {
                start_tick: 1920,
                duration_ticks: 480,
                pitch: 52,
                velocity: 0.8,
            },
        })
        .unwrap();
    h.app
        .dispatch(AppCommand::CreateNotePattern {
            track: "bass".into(),
            id: "another".into(),
            length_ticks: 7680,
            looped: true,
        })
        .unwrap();
    assert!(
        encode_project(&h.app.view_model())
            .unwrap_err()
            .contains("uncaptured")
    );
    assert!(
        meldritch_control::instrument::encode_instrument(
            &h.app.view_model().instrument_editor.unwrap()
        )
        .unwrap_err()
        .contains("unrelated")
    );
}
