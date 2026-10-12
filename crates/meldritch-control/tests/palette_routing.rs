use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use meldritch_app::{AppCommand, AppController, CockpitMode};
use meldritch_control::{
    native_capture::{NativeCommandEnvelope, NativeProjectCapture, load_native_capture_baseline},
    native_project::{NativeProjectStore, load_native_project},
};
use meldritch_core::FrameRange;
use meldritch_render::{
    dsp::{Oscillator, Waveform},
    song_host::{CompiledSongPatch, SongLiveOverrides},
};
use meldritch_tui::{
    project_binding_command, project_construction_command, project_pattern_command,
};
use std::{fs, path::PathBuf, sync::Arc};

const FIXTURE: &str =
    include_str!("../../../songs/examples/35-project-palette-routing/main.project.lua");

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "meldritch-palette-{}-{}",
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

#[test]
fn native_keys_route_two_patterns_and_selection_survives_typed_capture() {
    let directory = Directory::new();
    let mut app = directory.app();
    let mut capture = NativeProjectCapture::new(app.authored_source().unwrap(), 96_000);
    let gesture = |app: &mut AppController, capture: &mut NativeProjectCapture, code| {
        let key = KeyEvent::new(code, KeyModifiers::NONE);
        let source = app.authored_source().unwrap();
        let command = project_pattern_command(key, &app.view_model(), &source)
            .or_else(|| project_binding_command(key, &app.view_model(), &source))
            .or_else(|| project_construction_command(key, &source))
            .unwrap();
        let creation = matches!(command, AppCommand::CreatePatch(_));
        app.dispatch(command).unwrap();
        capture.checkpoint(app).unwrap();
        if creation {
            app.dispatch(AppCommand::SelectInstrument("voice-1".into()))
                .unwrap();
            capture.checkpoint(app).unwrap();
        }
    };
    for code in [
        KeyCode::Char('n'),
        KeyCode::Char('c'),
        KeyCode::Enter,
        KeyCode::Right,
        KeyCode::Right,
        KeyCode::Up,
        KeyCode::Up,
        KeyCode::Up,
        KeyCode::Up,
        KeyCode::Up,
        KeyCode::Up,
        KeyCode::Up,
        KeyCode::Enter,
        KeyCode::Esc,
        KeyCode::Char('v'),
        KeyCode::Char('p'),
        KeyCode::Char('c'),
        KeyCode::Enter,
        KeyCode::Char(']'),
        KeyCode::Char(']'),
        KeyCode::Esc,
        KeyCode::Char('g'),
    ] {
        gesture(&mut app, &mut capture, code);
    }
    let source = app.authored_source().unwrap();
    let voice = &source.pattern_voices()["voice-1-instance"];
    assert_eq!(voice.pitch_pattern.as_deref(), Some("pattern-1"));
    assert_eq!(voice.gate_pattern.as_deref(), Some("pattern-2"));
    let range = FrameRange::new(0, 192_000).unwrap();
    let expected = CompiledSongPatch::compile_initial(&source)
        .unwrap()
        .render_default(range)
        .unwrap();
    gesture(&mut app, &mut capture, KeyCode::PageUp);
    assert!(Arc::ptr_eq(&source, &app.authored_source().unwrap()));
    let selected = app.view_model().palette_pattern_editor.unwrap();
    assert_eq!(selected.pattern.id, "pattern-1");
    assert!(!selected.visible);
    assert_eq!((selected.cursor_tick, selected.cursor_pitch), (0, 48));
    // Selection wraps without rebinding or changing the musical revision.
    gesture(&mut app, &mut capture, KeyCode::PageUp);
    assert_eq!(
        app.view_model().palette_pattern_editor.unwrap().pattern.id,
        "pattern-2"
    );
    gesture(&mut app, &mut capture, KeyCode::PageDown);
    assert_eq!(
        app.view_model().palette_pattern_editor.unwrap().pattern.id,
        "pattern-1"
    );
    app.dispatch(AppCommand::TogglePatternEditor).unwrap();
    capture.checkpoint(&app).unwrap();
    app.dispatch(AppCommand::MovePatternCursor {
        tick_delta: 960,
        pitch_delta: 7,
    })
    .unwrap();
    capture.checkpoint(&app).unwrap();
    app.dispatch(AppCommand::SelectNote {
        pattern: "pattern-1".into(),
        id: "note-2".into(),
    })
    .unwrap();
    capture.checkpoint(&app).unwrap();
    app.dispatch(AppCommand::SelectPalettePattern("pattern-1".into()))
        .unwrap();
    capture.checkpoint(&app).unwrap();
    let selection = app.view_model().palette_pattern_editor.unwrap();
    assert_eq!(selection.selected_note.as_deref(), Some("note-2"));
    assert_eq!((selection.cursor_tick, selection.cursor_pitch), (960, 55));
    let history = app.history().len();
    assert!(
        app.dispatch(AppCommand::SelectPalettePattern("missing".into()))
            .is_err()
    );
    assert_eq!(app.history().len(), history);
    assert_eq!(
        app.view_model()
            .palette_pattern_editor
            .unwrap()
            .selected_note,
        selection.selected_note
    );
    gesture(&mut app, &mut capture, KeyCode::PageDown);
    let selection = app.view_model().palette_pattern_editor.unwrap();
    assert!(selection.visible && selection.selected_note.is_none());
    assert_eq!((selection.cursor_tick, selection.cursor_pitch), (0, 48));
    for kind in [KeyEventKind::Repeat, KeyEventKind::Release] {
        let mut event = KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE);
        event.kind = kind;
        assert!(project_pattern_command(event, &app.view_model(), &source).is_none());
    }
    assert!(
        project_pattern_command(
            KeyEvent::new(KeyCode::PageDown, KeyModifiers::CONTROL),
            &app.view_model(),
            &source
        )
        .is_none()
    );
    app.dispatch(AppCommand::TogglePatternEditor).unwrap();
    capture.checkpoint(&app).unwrap();
    app.dispatch(AppCommand::ToggleInstrumentEditor).unwrap();
    capture.checkpoint(&app).unwrap();
    assert!(
        project_pattern_command(
            KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE),
            &app.view_model(),
            &source
        )
        .is_none()
    );
    app.dispatch(AppCommand::ToggleInstrumentEditor).unwrap();
    capture.checkpoint(&app).unwrap();
    app.dispatch(AppCommand::SetCockpitMode(CockpitMode::Performance))
        .unwrap();
    capture.checkpoint(&app).unwrap();
    assert!(
        project_pattern_command(
            KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE),
            &app.view_model(),
            &source
        )
        .is_none()
    );
    assert!(Arc::ptr_eq(&source, &app.authored_source().unwrap()));
    let mut store =
        NativeProjectStore::open(&meldritch_dsl::load_project_directory(&directory.0).unwrap())
            .unwrap();
    store.save(&source).unwrap();
    capture.finish(&app).unwrap();
    let text = fs::read_to_string(capture.session.as_ref().unwrap().path()).unwrap();
    let mut reconstructed = directory.app();
    reconstructed
        .enable_native_project_source(load_native_capture_baseline(&directory.0, &text).unwrap())
        .unwrap();
    let document: toml::Value = toml::from_str(&text).unwrap();
    for event in document["events"].as_array().unwrap() {
        let payload =
            NativeCommandEnvelope::decode(&toml::to_string(&event["native"]).unwrap()).unwrap();
        reconstructed.dispatch(payload.command.into()).unwrap();
    }
    assert_eq!(
        reconstructed
            .view_model()
            .palette_pattern_editor
            .unwrap()
            .pattern
            .id,
        "pattern-2"
    );
    assert_eq!(
        reconstructed.authored_source().unwrap().fingerprint(),
        source.fingerprint()
    );
    let reopened =
        NativeProjectStore::open(&meldritch_dsl::load_project_directory(&directory.0).unwrap())
            .unwrap();
    let saved = reopened.source().unwrap();
    assert_eq!(saved.fingerprint(), source.fingerprint());
    let patch = CompiledSongPatch::compile_initial(saved).unwrap();
    assert_eq!(
        patch.render_default(range).unwrap().samples(),
        expected.samples()
    );
    let mut split = patch
        .render_default(FrameRange::new(0, 97_331).unwrap())
        .unwrap()
        .samples()
        .to_vec();
    split.extend(
        patch
            .render_default(FrameRange::new(97_331, 192_000).unwrap())
            .unwrap()
            .samples(),
    );
    assert_eq!(split, expected.samples());
}

#[test]
fn shared_patch_fanout_keeps_pitch_gate_and_instance_gain_independent() {
    let directory = Directory::new();
    let source = load_native_project(&directory.0, FIXTURE).unwrap();
    assert_eq!(source.synths().len(), 1);
    assert_eq!(source.pattern_voices().len(), 2);
    let range = FrameRange::new(0, 192_000).unwrap();
    let patch = CompiledSongPatch::compile_initial(&source).unwrap();
    let audio = patch.render_default(range).unwrap();
    let mut oscillator = Oscillator::new(Waveform::Sine);
    for (frame, sample) in audio.samples().iter().enumerate() {
        let position = frame % 96_000;
        let pitch = if position < 24_000 { 48.0 } else { 55.0 };
        let wave = oscillator.next(440.0 * 2_f64.powf((pitch - 69.0) / 12.0), 48_000);
        let expected = if position < 48_000 {
            let gain = 1.0 / 2.0_f64.sqrt();
            wave * 0.8 * gain + wave * 0.4 * gain
        } else {
            0.0
        };
        assert_eq!(
            *sample, expected,
            "independent shared source at frame {frame}"
        );
    }
    let altered_rhythm = load_native_project(&directory.0, &FIXTURE.replace("C6", "C0")).unwrap();
    assert_eq!(
        CompiledSongPatch::compile_initial(&altered_rhythm)
            .unwrap()
            .render_default(range)
            .unwrap()
            .samples(),
        audio.samples()
    );
    let mut overrides = SongLiveOverrides::default();
    overrides.voice_levels.insert("bass-copy".into(), 0.0);
    let first_only = patch.render(range, &overrides).unwrap();
    overrides.voice_levels.clear();
    overrides
        .voice_levels
        .insert("voice-1-instance".into(), 0.0);
    let second_only = patch.render(range, &overrides).unwrap();
    for ((both, first), second) in audio
        .samples()
        .iter()
        .zip(first_only.samples())
        .zip(second_only.samples())
    {
        assert_eq!(*both, first + second);
    }
    assert_eq!(source.pattern_voices()["bass-copy"].level, 0.4);
    // Editing rhythm duration changes only the gate window; melody keeps its
    // two pitch starts. Editing melody pitch keeps that same gate window.
    for (fixture, end, second_pitch) in [
        (
            FIXTURE.replace("duration_ticks = 1920", "duration_ticks = 960"),
            24_000,
            55.0,
        ),
        (FIXTURE.replace("G3", "F3"), 48_000, 53.0),
    ] {
        let edited = load_native_project(&directory.0, &fixture).unwrap();
        let audio = CompiledSongPatch::compile_initial(&edited)
            .unwrap()
            .render_default(range)
            .unwrap();
        let mut oscillator = Oscillator::new(Waveform::Sine);
        for (frame, sample) in audio.samples().iter().enumerate() {
            let position = frame % 96_000;
            let pitch = if position < 24_000 {
                48.0
            } else {
                second_pitch
            };
            let wave = oscillator.next(440.0 * 2_f64.powf((pitch - 69.0) / 12.0), 48_000);
            let gain = 1.0 / 2.0_f64.sqrt();
            let expected = if position < end {
                wave * 0.8 * gain + wave * 0.4 * gain
            } else {
                0.0
            };
            assert_eq!(
                *sample, expected,
                "independent edited signal at frame {frame}"
            );
        }
    }
}

#[test]
fn track_owned_pattern_selection_rejects_without_changing_existing_editor() {
    let directory = Directory::new();
    let source = meldritch_dsl::load_song_directory(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../songs/examples/09-curated-performance-controls"),
    )
    .unwrap();
    let id = source.note_patterns().keys().next().unwrap().clone();
    let mut app = directory.app();
    app.enable_project_source(source).unwrap();
    app.enable_pattern_editor(&id).unwrap();
    let before = app.authored_source().unwrap();
    assert!(
        app.dispatch(AppCommand::SelectPalettePattern(id.clone()))
            .is_err()
    );
    assert!(app.history().is_empty());
    assert!(Arc::ptr_eq(&before, &app.authored_source().unwrap()));
    assert_eq!(app.view_model().pattern_editor.unwrap().pattern.id, id);
}

#[test]
fn incompatible_gate_clock_preserves_source_selection_and_history() {
    let directory = Directory::new();
    let mut app = directory.app();
    app.enable_native_project_source(load_native_project(&directory.0, FIXTURE).unwrap())
        .unwrap();
    app.dispatch(AppCommand::CreatePalettePattern {
        id: "half-bar".into(),
        length_ticks: 1920,
        looped: true,
    })
    .unwrap();
    let source = app.authored_source().unwrap();
    let history = app.history().len();
    let error = app
        .dispatch(AppCommand::BindPatternVoice {
            voice: "voice-1-instance".into(),
            signal: meldritch_app::PatternSignal::Gate,
            pattern: Some("half-bar".into()),
        })
        .unwrap_err();
    assert!(format!("{error:?}").contains("share the voice length and loop policy"));
    assert!(Arc::ptr_eq(&source, &app.authored_source().unwrap()));
    assert_eq!(app.history().len(), history);
    assert_eq!(
        app.view_model().palette_pattern_editor.unwrap().pattern.id,
        "half-bar"
    );
    assert!(
        app.dispatch(AppCommand::SelectPalettePattern(String::new()))
            .is_err()
    );
    assert_eq!(app.history().len(), history);
}
