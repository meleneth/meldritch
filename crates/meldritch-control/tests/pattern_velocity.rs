use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use meldritch_app::{AppCommand, AppCommandResult, AppController, NoteEdit};
use meldritch_control::{
    native_capture::{NativeCommandEnvelope, NativeProjectCapture, load_native_capture_baseline},
    native_project::{NativeProjectStore, load_native_project},
};
use meldritch_core::FrameRange;
use meldritch_dsl::{DependentCables, PatchEdit, PatchOperation, PatternSignal, ValidatedSong};
use meldritch_render::song_host::CompiledSongPatch;
use meldritch_tui::project_binding_command;
use std::{fs, path::PathBuf, sync::Arc};

const FIXTURE: &str =
    include_str!("../../../songs/examples/36-patch-lfo/pattern-velocity.project.lua");

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "meldritch-pattern-velocity-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn source(&self) -> ValidatedSong {
        load_native_project(&self.0, FIXTURE).unwrap()
    }
    fn app(&self, source: ValidatedSong) -> AppController {
        let mut app = AppController::new_empty_project(
            &meldritch_dsl::load_project_directory(&self.0).unwrap(),
        )
        .unwrap()
        .0;
        app.enable_native_project_source(source).unwrap();
        app
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn render(source: &ValidatedSong, range: FrameRange) -> Vec<f64> {
    CompiledSongPatch::compile_initial(source)
        .unwrap()
        .render_default(range)
        .unwrap()
        .samples()
        .to_vec()
}

#[test]
fn independent_velocity_changes_held_gain_without_pitch_or_gate_articulation() {
    let directory = Directory::new();
    let source = directory.source();
    let unbound = source
        .bind_pattern_voice("bass-instance", PatternSignal::Velocity, None)
        .unwrap();
    let range = FrameRange::new(0, 96_000).unwrap();
    let bound = render(&source, range);
    let unity = render(&unbound, range);
    assert_eq!(bound.len(), unity.len());
    for (frame, (&actual, &reference)) in bound.iter().zip(&unity).enumerate() {
        let pattern_frame = frame % 48_000;
        let velocity = if pattern_frame < 12_000 {
            0.25
        } else if pattern_frame < 24_000 {
            0.75
        } else {
            0.5
        };
        assert!(
            (actual - reference * velocity).abs() < 1e-12,
            "frame {frame}"
        );
    }
    assert!(
        bound[12_000..24_000]
            .iter()
            .any(|sample| sample.abs() > 0.1)
    );
    assert!(
        bound[48_000..60_000]
            .iter()
            .any(|sample| sample.abs() > 0.01)
    );
    let mut split = Vec::new();
    for (start, end) in [
        (0, 1),
        (1, 12_000),
        (12_000, 36_001),
        (36_001, 48_000),
        (48_000, 96_000),
    ] {
        split.extend(render(&source, FrameRange::new(start, end).unwrap()));
    }
    assert_eq!(split, bound);
    // Declared but uncabled Control receives events without changing sound.
    let uncabled = source
        .edit_patch(&PatchEdit {
            synth: "bass".into(),
            operations: vec![
                PatchOperation::Remove {
                    id: "shape".into(),
                    cables: DependentCables::Disconnect,
                },
                PatchOperation::Connect {
                    from: "env.control".into(),
                    to: "amp.level".into(),
                },
            ],
        })
        .unwrap();
    let uncabled_unbound = uncabled
        .bind_pattern_voice("bass-instance", PatternSignal::Velocity, None)
        .unwrap();
    assert_eq!(render(&uncabled, range), render(&uncabled_unbound, range));
    assert_ne!(source.fingerprint(), unbound.fingerprint());
}

#[test]
fn velocity_binding_note_edit_capture_and_store_reopen() {
    let directory = Directory::new();
    let source = directory.source();
    let original = render(&source, FrameRange::new(0, 48_000).unwrap());
    let mut app = directory.app(source.clone());
    app.dispatch(AppCommand::SelectPatternVoice("bass-instance".into()))
        .unwrap();
    app.dispatch(AppCommand::SelectPalettePattern("dynamics".into()))
        .unwrap();
    let key = KeyEvent::new(KeyCode::Char('u'), KeyModifiers::NONE);
    let command = project_binding_command(key, &app.view_model(), &source).unwrap();
    assert!(matches!(
        command,
        AppCommand::BindPatternVoice {
            signal: PatternSignal::Velocity,
            pattern: None,
            ..
        }
    ));
    let mut capture = NativeProjectCapture::new(Arc::new(source.clone()), 96_000);
    app.dispatch(command).unwrap();
    capture.checkpoint(&app).unwrap();
    let bind = AppCommand::BindPatternVoice {
        voice: "bass-instance".into(),
        signal: PatternSignal::Velocity,
        pattern: Some("dynamics".into()),
    };
    app.dispatch(bind).unwrap();
    capture.checkpoint(&app).unwrap();
    let edit = AppCommand::EditNote {
        pattern: "dynamics".into(),
        edit: NoteEdit::Velocity {
            id: "note-2".into(),
            velocity: 0.4,
        },
    };
    let result = app.dispatch(edit).unwrap();
    assert!(matches!(
        result,
        AppCommandResult::PatternEdited { changed: true, ref dirty_ranges, .. }
            if dirty_ranges.len() == 1 && dirty_ranges[0].start() == 12_000
    ));
    let invalidation = &app.history().back().unwrap().dirty_ranges;
    assert_eq!(invalidation.len(), 1);
    assert_eq!(invalidation[0].range().start(), 12_000);
    capture.finish(&app).unwrap();
    let accepted = app.authored_source().unwrap();
    let changed = render(&accepted, FrameRange::new(0, 48_000).unwrap());
    assert_eq!(changed[..12_000], original[..12_000]);
    assert_ne!(changed[12_000..24_000], original[12_000..24_000]);
    assert_eq!(changed[24_000..], original[24_000..]);
    let session = fs::read_to_string(capture.session.as_ref().unwrap().path()).unwrap();
    let document: toml::Value = toml::from_str(&session).unwrap();
    let mut replay = directory.app(load_native_capture_baseline(&directory.0, &session).unwrap());
    for event in document["events"].as_array().unwrap() {
        let envelope =
            NativeCommandEnvelope::decode(&toml::to_string(&event["native"]).unwrap()).unwrap();
        replay.dispatch(envelope.command.into()).unwrap();
    }
    assert_eq!(
        replay.authored_source().unwrap().fingerprint(),
        accepted.fingerprint()
    );
    let mut store =
        NativeProjectStore::open(&meldritch_dsl::load_project_directory(&directory.0).unwrap())
            .unwrap();
    store.save(&accepted).unwrap();
    let reopened =
        NativeProjectStore::open(&meldritch_dsl::load_project_directory(&directory.0).unwrap())
            .unwrap();
    let loaded = reopened.source().unwrap();
    assert_eq!(loaded.fingerprint(), accepted.fingerprint());
    assert_eq!(render(loaded, FrameRange::new(0, 48_000).unwrap()), changed);
}

#[test]
fn invalid_velocity_bindings_and_events_preserve_accepted_state() {
    let directory = Directory::new();
    let source = directory.source();
    let fixture: toml::Value = toml::from_str(include_str!(
        "../../../songs/examples/36-patch-lfo/invalid/pattern-velocity.toml"
    ))
    .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let case = case.as_str().unwrap();
        match case {
            "unknown-pattern" | "invalid-edit" => {
                let mut app = directory.app(source.clone());
                let before = app.authored_source().unwrap();
                let history = app.history().len();
                let command = if case == "unknown-pattern" {
                    AppCommand::BindPatternVoice {
                        voice: "bass-instance".into(),
                        signal: PatternSignal::Velocity,
                        pattern: Some("missing".into()),
                    }
                } else {
                    AppCommand::EditNote {
                        pattern: "dynamics".into(),
                        edit: NoteEdit::Velocity {
                            id: "note-1".into(),
                            velocity: f64::NAN,
                        },
                    }
                };
                assert!(app.dispatch(command).is_err(), "{case}");
                assert!(Arc::ptr_eq(&before, &app.authored_source().unwrap()));
                assert_eq!(app.history().len(), history);
            }
            _ => {
                let candidate = match case {
                    "missing-port" => {
                        FIXTURE.replace("{ id = \"velocity\", signal = \"control\" }", "")
                    }
                    "wrong-port-type" => FIXTURE.replace(
                        "id = \"velocity\", signal = \"control\"",
                        "id = \"velocity\", signal = \"gate\"",
                    ),
                    "length-mismatch" => FIXTURE.replace(
                        "id = \"dynamics\", length_ticks = 1920",
                        "id = \"dynamics\", length_ticks = 3840",
                    ),
                    "loop-mismatch" => FIXTURE.replace(
                        "id = \"dynamics\", length_ticks = 1920, looped = true",
                        "id = \"dynamics\", length_ticks = 1920, looped = false",
                    ),
                    "simultaneous-start" => FIXTURE.replace(
                        "note = \"C4\", start_tick = 480",
                        "note = \"C4\", start_tick = 0",
                    ),
                    "sample-event" => FIXTURE.replace(
                        "note = \"C4\", start_tick = 480",
                        "note = \"C4\", slot = \"kick\", start_tick = 480",
                    ),
                    _ => panic!("unknown case {case}"),
                };
                assert_ne!(candidate, FIXTURE, "fixture replacement failed: {case}");
                assert!(
                    load_native_project(&directory.0, &candidate).is_err(),
                    "{case}"
                );
            }
        }
    }
}
