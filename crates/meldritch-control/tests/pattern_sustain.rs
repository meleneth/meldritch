use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use meldritch_app::{AppCommand, AppCommandResult, AppController, NoteEdit};
use meldritch_control::{
    native_capture::{NativeCommandEnvelope, NativeProjectCapture, load_native_capture_baseline},
    native_project::{NativeProjectStore, load_native_project},
};
use meldritch_core::FrameRange;
use meldritch_dsl::{PatternSignal, ValidatedSong};
use meldritch_render::song_host::CompiledSongPatch;
use meldritch_tui::project_binding_command;
use std::{fs, path::PathBuf, sync::Arc};

const FIXTURE: &str =
    include_str!("../../../songs/examples/36-patch-lfo/pattern-sustain.project.lua");

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "meldritch-pattern-sustain-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn source(&self) -> ValidatedSong {
        load_native_project(&self.0, FIXTURE).unwrap()
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn render(source: &ValidatedSong, start: u64, end: u64) -> Vec<f64> {
    CompiledSongPatch::compile_initial(source)
        .unwrap()
        .render_default(FrameRange::new(start, end).unwrap())
        .unwrap()
        .samples()
        .to_vec()
}

#[test]
fn independent_hold_matches_explicit_resolved_gate_and_survives_reopen() {
    let directory = Directory::new();
    let source = directory.source();
    let held = render(&source, 0, 96_000);
    let (reference, _, _) = source
        .edit_note(
            "rhythm",
            &NoteEdit::Duration {
                id: "note-1".into(),
                duration_ticks: 1440,
            },
        )
        .unwrap();
    let reference = reference
        .bind_pattern_voice("bass-instance", PatternSignal::Sustain, None)
        .unwrap();
    assert_eq!(held, render(&reference, 0, 96_000));
    let unbound = source
        .bind_pattern_voice("bass-instance", PatternSignal::Sustain, None)
        .unwrap();
    assert!(
        render(&unbound, 13_000, 24_000)
            .iter()
            .all(|sample| sample.abs() < 1e-6)
    );
    assert!(held[12_000..24_000].iter().any(|sample| sample.abs() > 0.1));
    assert!(
        held[45_000..48_000]
            .iter()
            .all(|sample| sample.abs() < 1e-5)
    );
    let mut split = Vec::new();
    for (start, end) in [
        (0, 1),
        (1, 12_000),
        (12_000, 36_000),
        (36_000, 48_000),
        (48_000, 96_000),
    ] {
        split.extend(render(&source, start, end));
    }
    assert_eq!(held, split);

    let mut app = AppController::new_empty_project(
        &meldritch_dsl::load_project_directory(&directory.0).unwrap(),
    )
    .unwrap()
    .0;
    app.enable_native_project_source(source.clone()).unwrap();
    app.dispatch(AppCommand::SelectPatternVoice("bass-instance".into()))
        .unwrap();
    app.dispatch(AppCommand::SelectPalettePattern("hold-time".into()))
        .unwrap();
    let mut capture = NativeProjectCapture::new(Arc::new(source.clone()), 96_000);
    let command = project_binding_command(
        KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE),
        &app.view_model(),
        &source,
    )
    .unwrap();
    assert!(matches!(
        command,
        AppCommand::BindPatternVoice {
            signal: PatternSignal::Sustain,
            pattern: None,
            ..
        }
    ));
    app.dispatch(command).unwrap();
    capture.checkpoint(&app).unwrap();
    assert_eq!(
        render(&app.authored_source().unwrap(), 0, 48_000),
        render(&unbound, 0, 48_000)
    );
    app.dispatch(AppCommand::BindPatternVoice {
        voice: "bass-instance".into(),
        signal: PatternSignal::Sustain,
        pattern: Some("hold-time".into()),
    })
    .unwrap();
    capture.checkpoint(&app).unwrap();
    capture.finish(&app).unwrap();
    let accepted = app.authored_source().unwrap();
    let session = fs::read_to_string(capture.session.as_ref().unwrap().path()).unwrap();
    let document: toml::Value = toml::from_str(&session).unwrap();
    let mut replay = AppController::new_empty_project(
        &meldritch_dsl::load_project_directory(&directory.0).unwrap(),
    )
    .unwrap()
    .0;
    replay
        .enable_native_project_source(load_native_capture_baseline(&directory.0, &session).unwrap())
        .unwrap();
    for event in document["events"].as_array().unwrap() {
        let command =
            NativeCommandEnvelope::decode(&toml::to_string(&event["native"]).unwrap()).unwrap();
        replay.dispatch(command.command.into()).unwrap();
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
    assert_eq!(
        reopened.source().unwrap().fingerprint(),
        accepted.fingerprint()
    );
    assert_eq!(render(reopened.source().unwrap(), 0, 96_000), held);
    let edit = app
        .dispatch(AppCommand::EditNote {
            pattern: "hold-time".into(),
            edit: NoteEdit::Duration {
                id: "note-1".into(),
                duration_ticks: 720,
            },
        })
        .unwrap();
    assert!(matches!(
        edit,
        AppCommandResult::PatternEdited { changed: true, .. }
    ));
    assert_eq!(app.history().back().unwrap().dirty_ranges.len(), 1);
    assert_ne!(
        render(&app.authored_source().unwrap(), 0, 48_000),
        held[..48_000]
    );
}

#[test]
fn invalid_hold_bindings_reject_without_changing_source() {
    let directory = Directory::new();
    let source = directory.source();
    let mut app = AppController::new_empty_project(
        &meldritch_dsl::load_project_directory(&directory.0).unwrap(),
    )
    .unwrap()
    .0;
    app.enable_native_project_source(source.clone()).unwrap();
    let before = app.authored_source().unwrap();
    let history = app.history().len();
    assert!(
        app.dispatch(AppCommand::BindPatternVoice {
            voice: "bass-instance".into(),
            signal: PatternSignal::Sustain,
            pattern: Some("missing".into()),
        })
        .is_err()
    );
    assert!(Arc::ptr_eq(&before, &app.authored_source().unwrap()));
    assert_eq!(app.history().len(), history);
    let fixture: toml::Value = toml::from_str(include_str!(
        "../../../songs/examples/36-patch-lfo/invalid/pattern-sustain.toml"
    ))
    .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let case = case.as_str().unwrap();
        let candidate = match case {
            "unknown-pattern" => source.bind_pattern_voice(
                "bass-instance",
                PatternSignal::Sustain,
                Some("missing".into()),
            ),
            "invalid-edit" => source
                .edit_note(
                    "hold-time",
                    &NoteEdit::Duration {
                        id: "note-1".into(),
                        duration_ticks: 0,
                    },
                )
                .map(|(source, _, _)| source),
            _ => {
                let modified = match case {
                    "missing-port" => FIXTURE.replace("{ id = \"sustain\", signal = \"gate\" }", ""),
                    "wrong-port-type" => FIXTURE.replace("id = \"sustain\", signal = \"gate\"", "id = \"sustain\", signal = \"control\""),
                    "length-mismatch" => FIXTURE.replace("id = \"hold-time\", length_ticks = 1920", "id = \"hold-time\", length_ticks = 3840"),
                    "loop-mismatch" => FIXTURE.replace("id = \"hold-time\", length_ticks = 1920, looped = true", "id = \"hold-time\", length_ticks = 1920, looped = false"),
                    "overlapping-hold" => FIXTURE.replace("next_note_id = 2,\n      notes = { { id = \"note-1\", note = \"C1\"", "next_note_id = 3,\n      notes = { { id = \"note-2\", note = \"C1\", start_tick = 960, duration_ticks = 480, velocity = 0.2 }, { id = \"note-1\", note = \"C1\""),
                    "sample-event" => FIXTURE.replace("note = \"C1\", start_tick = 480", "note = \"C1\", slot = \"kick\", start_tick = 480"),
                    _ => panic!("unknown case {case}"),
                };
                assert_ne!(modified, FIXTURE, "negative fixture did not change: {case}");
                load_native_project(&directory.0, &modified)
            }
        };
        assert!(candidate.is_err(), "{case}");
        assert_eq!(
            render(&source, 0, 48_000),
            render(&directory.source(), 0, 48_000)
        );
    }
}
