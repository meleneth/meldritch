use meldritch_app::*;
use meldritch_audio::audio_publication::audio_publication;
use meldritch_audio::device_output::{PlaybackEngine, playback_session_parts};
use meldritch_audio::published_audio::PublishedAudio;
use meldritch_control::instrument::*;
use meldritch_control::song_host::SongAudioHost;
use meldritch_control::{ControlPlane, Input, LuaProject};
use meldritch_core::{
    FrameRange, Pattern, PatternId, ProbabilitySeed, Step, StepIndex, Tempo, TrackId,
};
use meldritch_render::RenderSettings;
use meldritch_render::coordinator::{
    RenderCoordinator, RenderCoordinatorConfig, SampleRenderState,
};
use meldritch_render::live_edit::LivePatternEditor;
use meldritch_render::song_host::CompiledSongPatch;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../songs/examples/25-edit-existing-synth")
}
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "meldritch-instrument-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fn copy(from: &Path, to: &Path) {
            std::fs::create_dir_all(to).unwrap();
            for entry in std::fs::read_dir(from).unwrap() {
                let entry = entry.unwrap();
                let target = to.join(entry.file_name());
                if entry.file_type().unwrap().is_dir() {
                    copy(&entry.path(), &target);
                } else {
                    std::fs::copy(entry.path(), target).unwrap();
                }
            }
        }
        copy(&fixture(), &path);
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn controller(song: &ValidatedSong) -> (AppController, PlaybackEngine) {
    let (playback, engine) = playback_session_parts(8).unwrap();
    let state = SampleRenderState::new(
        Pattern::new(PatternId::new(1), 4, 4).unwrap(),
        Tempo::new(120.0, 48000).unwrap(),
        ProbabilitySeed::new(1),
        RenderSettings::new(1).unwrap(),
        Arc::new(BTreeMap::new()),
    );
    let coordinator = RenderCoordinator::new_from_state(
        RenderCoordinatorConfig::new(1, 96000, 6000, 16, Duration::from_millis(2)).unwrap(),
        state.clone(),
        playback.status_monitor(),
    )
    .unwrap();
    let mut app = AppController::new(
        playback,
        coordinator,
        LivePatternEditor::new(state, 96000),
        Selection {
            track: TrackId::new(1),
            step: StepIndex::new(0),
        },
        256,
    );
    app.enable_instrument_editor(song.clone()).unwrap();
    (app, engine)
}
fn edit(parameter: InstrumentParameter, value: InstrumentValue) -> InstrumentEdit {
    let module = match parameter {
        InstrumentParameter::Waveform => "osc",
        InstrumentParameter::CutoffHz | InstrumentParameter::Resonance => "filter",
        _ => "env",
    };
    InstrumentEdit {
        target: InstrumentParameterId {
            synth: "edited-bass".into(),
            module: module.into(),
            parameter,
        },
        value,
    }
}
fn changes() -> Vec<InstrumentEdit> {
    vec![
        edit(
            InstrumentParameter::Waveform,
            InstrumentValue::Waveform(InstrumentWaveform::Triangle),
        ),
        edit(InstrumentParameter::Attack, InstrumentValue::Number(0.04)),
        edit(InstrumentParameter::Decay, InstrumentValue::Number(0.32)),
        edit(InstrumentParameter::Sustain, InstrumentValue::Number(0.22)),
        edit(InstrumentParameter::Release, InstrumentValue::Number(0.4)),
        edit(
            InstrumentParameter::CutoffHz,
            InstrumentValue::Number(600.0),
        ),
        edit(InstrumentParameter::Resonance, InstrumentValue::Number(0.7)),
    ]
}
fn range() -> FrameRange {
    FrameRange::new(0, 96000).unwrap()
}
fn render(song: &ValidatedSong) -> meldritch_audio::AudioBlock {
    CompiledSongPatch::compile_initial(song)
        .unwrap()
        .render_default(range())
        .unwrap()
}
fn wait(host: &mut SongAudioHost) {
    let until = Instant::now() + Duration::from_secs(3);
    loop {
        if host.poll_publish().unwrap().is_some() {
            return;
        }
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(2));
    }
}
fn native_tui(app: &AppController) -> String {
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 24)).unwrap();
    terminal
        .draw(|frame| meldritch_tui::draw(frame, &app.view_model()))
        .unwrap();
    terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect()
}

#[test]
fn every_supported_parameter_edits_renders_and_roundtrips_independently() {
    let song = meldritch_dsl::load_song_directory(fixture()).unwrap();
    let original = render(&song);
    for edit in changes() {
        let (mut app, _engine) = controller(&song);
        let result = app
            .dispatch(AppCommand::EditInstrument(edit.clone()))
            .unwrap();
        assert!(matches!(
            result,
            AppCommandResult::InstrumentEdited { changed: true, .. }
        ));
        let view = app.view_model().instrument_editor.unwrap();
        assert_eq!(
            view.defaults,
            song.instrument_parameters("edited-bass").unwrap()
        );
        assert!(
            view.parameters
                .iter()
                .any(|p| p.id == edit.target && p.value.same_as(&edit.value))
        );
        assert_ne!(view.source.fingerprint(), song.fingerprint());
        let edited = render(&view.source);
        assert_ne!(
            edited.samples(),
            original.samples(),
            "{:?} must change sound",
            edit.target.parameter
        );
        assert!(edited.samples().iter().all(|x| x.is_finite()));
        let encoded = encode_instrument(&view).unwrap();
        let restored = load_instrument(fixture(), &encoded).unwrap();
        assert_eq!(restored.fingerprint(), view.source.fingerprint());
        assert_eq!(restored.synths(), view.source.synths());
        assert_eq!(render(&restored), edited);
        let (mut fresh, _) = controller(&song);
        fresh
            .enable_instrument_editor_from(song.clone(), restored)
            .unwrap();
        assert!(fresh.history().is_empty());
        assert_eq!(
            encode_instrument(&fresh.view_model().instrument_editor.unwrap()).unwrap(),
            encoded
        );
        // A repeated absolute edit neither invalidates nor alters the fingerprint.
        let before = view.source.fingerprint();
        assert!(matches!(
            app.dispatch(AppCommand::EditInstrument(edit)).unwrap(),
            AppCommandResult::InstrumentEdited { changed: false, .. }
        ));
        assert_eq!(
            app.view_model()
                .instrument_editor
                .unwrap()
                .source
                .fingerprint(),
            before
        );
    }
}

#[test]
fn invalid_types_ranges_ids_and_automation_leave_authority_and_publication_unchanged() {
    let song = meldritch_dsl::load_song_directory(fixture()).unwrap();
    let (mut app, _engine) = controller(&song);
    let block = render(&song);
    let (publisher, reader) = audio_publication(PublishedAudio::from_block(&block, 1024).unwrap());
    let retained = reader.snapshot();
    let mut host = SongAudioHost::new(
        song.clone(),
        CompiledSongPatch::compile_initial(&song).unwrap(),
        publisher,
        96000,
        1024,
    );
    let mut bad = vec![
        edit(InstrumentParameter::CutoffHz, InstrumentValue::Number(0.0)),
        edit(
            InstrumentParameter::CutoffHz,
            InstrumentValue::Number(f64::NAN),
        ),
        edit(
            InstrumentParameter::Release,
            InstrumentValue::Number(f64::INFINITY),
        ),
        edit(InstrumentParameter::Release, InstrumentValue::Number(-0.1)),
        edit(InstrumentParameter::Sustain, InstrumentValue::Number(1.1)),
        edit(
            InstrumentParameter::Resonance,
            InstrumentValue::Number(-0.1),
        ),
        edit(
            InstrumentParameter::Attack,
            InstrumentValue::Waveform(InstrumentWaveform::Saw),
        ),
        edit(InstrumentParameter::Waveform, InstrumentValue::Number(1.0)),
    ];
    let mut missing = changes()[0].clone();
    missing.target.module = "missing-module".into();
    bad.push(missing);
    let mut missing = changes()[0].clone();
    missing.target.synth = "missing-synth".into();
    bad.push(missing);
    let mut unsupported = changes()[0].clone();
    unsupported.target.module = "amp".into();
    bad.push(unsupported);
    for edit in bad {
        let error = format!(
            "{:?}",
            app.dispatch(AppCommand::EditInstrument(edit)).unwrap_err()
        );
        assert!(error.contains("parameter") || error.contains("instrument"));
        assert!(app.history().is_empty());
        assert_eq!(
            app.view_model()
                .instrument_editor
                .unwrap()
                .source
                .fingerprint(),
            song.fingerprint()
        );
        assert!(host.poll_publish().unwrap().is_none());
        assert!(Arc::ptr_eq(&retained, &reader.snapshot()));
    }
    let automated =
        meldritch_dsl::load_song_directory(fixture().with_file_name("02-synth-parameter-pattern"))
            .unwrap();
    let error = automated
        .edit_instrument(&InstrumentEdit {
            target: InstrumentParameterId {
                synth: "swept-bass".into(),
                module: "filter".into(),
                parameter: InstrumentParameter::CutoffHz,
            },
            value: InstrumentValue::Number(900.0),
        })
        .unwrap_err();
    assert!(error.contains("automation"));
}

#[test]
fn real_keyboard_and_lua_commands_share_state_worker_publication_and_journal() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use meldritch_audio::device_output::PublishedAudioRenderer;
    let song = meldritch_dsl::load_song_directory(fixture()).unwrap();
    let (mut app, engine) = controller(&song);
    let original = render(&song);
    let (publisher, reader) =
        audio_publication(PublishedAudio::from_block(&original, 1024).unwrap());
    let mut renderer =
        PublishedAudioRenderer::from_engine(reader.clone(), 1, u32::MAX, engine).unwrap();
    let mut host = SongAudioHost::new(
        song.clone(),
        CompiledSongPatch::compile_initial(&song).unwrap(),
        publisher,
        96000,
        1024,
    );
    let press = |app: &mut AppController, code, modifiers| {
        let action = meldritch_tui::map_key_for_view(
            KeyEvent::new(code, modifiers),
            &Step::new(36),
            &app.view_model(),
        )
        .unwrap();
        let meldritch_tui::TuiAction::Input(input) = action else {
            panic!("expected typed input");
        };
        app.handle_input(input).unwrap()
    };
    press(&mut app, KeyCode::Char('e'), KeyModifiers::CONTROL);
    assert!(native_tui(&app).contains("Instrument: edited-bass"));
    assert!(native_tui(&app).contains("Imported default:"));
    assert!(native_tui(&app).contains("Explicit cables:"));
    // Parameters are sorted by module and typed identity: env.attack first.
    let result = press(&mut app, KeyCode::Right, KeyModifiers::NONE);
    assert!(matches!(result, AppCommandResult::InstrumentEdited { .. }));
    host.accepted(&app.view_model(), &result).unwrap();
    host.refresh_view(&mut app);
    assert!(native_tui(&app).contains("pending publication"));
    press(&mut app, KeyCode::Down, KeyModifiers::NONE);
    assert!(native_tui(&app).contains("Parameter: Decay"));
    wait(&mut host);
    host.refresh_view(&mut app);
    assert!(native_tui(&app).contains("applied to published audio"));
    assert_eq!(reader.snapshot().generation(), 1);
    app.dispatch(AppCommand::Play).unwrap();
    renderer.render_f64(&mut [0.0; 256]);
    assert_eq!(reader.playback_monitor().generation(), Some(1));
    let source = std::fs::read_to_string(fixture().join("main.lua")).unwrap();
    let project = LuaProject::load(fixture(), &source).unwrap();
    let mut plane = ControlPlane::new(project, &app).unwrap();
    for surface in ["apc-mini", "deck", "terminal"] {
        plane.enqueue(surface, 256, Input::Press(0)).unwrap();
        plane.pump_with_observer(&mut app, |app, _, result, _, _| {
            host.accepted(&app.view_model(), result)?;
            Ok(None)
        });
    }
    let journal = plane.take_journal();
    assert_eq!(journal.commands.len(), 3); // one accepted absolute request per press
    assert_eq!(host.generation(), 2); // unchanged repeats do not rerender
    wait(&mut host);
    host.refresh_view(&mut app);
    plane.refresh(&app).unwrap();
    for surface in ["apc-mini", "deck", "terminal"] {
        assert!(
            plane.output(surface).unwrap().cells[&0]
                .label
                .contains("0.25")
        );
    }
    let edited = app.view_model().instrument_editor.unwrap();
    let restored = load_instrument(fixture(), &encode_instrument(&edited).unwrap()).unwrap();
    assert_eq!(restored.fingerprint(), edited.source.fingerprint());
    assert_ne!(render(&restored), original);
    let patch = CompiledSongPatch::compile_initial(&restored).unwrap();
    let full = patch.render_default(range()).unwrap();
    let first = patch
        .render_default(FrameRange::new(0, 17003).unwrap())
        .unwrap();
    let second = patch
        .render_default(FrameRange::new(17003, 96000).unwrap())
        .unwrap();
    assert_eq!([first.samples(), second.samples()].concat(), full.samples());
}

#[test]
fn atomic_save_preserves_toml_journals_and_previous_exports_and_reloads_without_session() {
    let scratch = Scratch::new();
    let song = meldritch_dsl::load_song_directory(&scratch.0).unwrap();
    let (mut app, _) = controller(&song);
    for edit in changes() {
        app.dispatch(AppCommand::EditInstrument(edit)).unwrap();
    }
    std::fs::create_dir_all(scratch.0.join("performances")).unwrap();
    let journal = scratch
        .0
        .join("performances/session-sentinel.mlperformance");
    std::fs::write(&journal, "existing journal").unwrap();
    let original = std::fs::read(scratch.0.join("synths/edited-bass.mlsynth")).unwrap();
    let view = app.view_model().instrument_editor.unwrap();
    let first = save_instrument(&view).unwrap();
    let second = save_instrument(&view).unwrap();
    assert_ne!(first, second);
    assert_eq!(
        std::fs::read(&first).unwrap(),
        std::fs::read(&second).unwrap()
    );
    assert_eq!(
        std::fs::read_to_string(journal).unwrap(),
        "existing journal"
    );
    assert_eq!(
        std::fs::read(scratch.0.join("synths/edited-bass.mlsynth")).unwrap(),
        original
    );
    assert_eq!(
        meldritch_dsl::load_song_directory(&scratch.0)
            .unwrap()
            .fingerprint(),
        song.fingerprint()
    );
    let loaded = load_instrument_file(&scratch.0, first.strip_prefix(&scratch.0).unwrap()).unwrap();
    assert_eq!(loaded.fingerprint(), view.source.fingerprint());
    assert_eq!(render(&loaded), render(&view.source));
    assert!(
        std::fs::read_dir(scratch.0.join("authored"))
            .unwrap()
            .all(|file| !file
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".tmp"))
    );
    let source = std::fs::read_to_string(first).unwrap();
    for invalid in [
        source.replace("version = 1", "version = 2"),
        source.replace("\"release\"", "\"unknown\""),
        source.replace("value = 0.4", "value = true"),
        source.replace("value = 0.4", "value = -1"),
        source.replace("value = 0.4", "value = \"saw\""),
        source.replace("kind = \"instrument\"", "kind = \"session\""),
        source.replace("version = 1,", "version = 1, speculative = true,"),
    ] {
        assert!(load_instrument(&scratch.0, &invalid).is_err());
    }
    assert!(load_instrument_file(&scratch.0, Path::new("../escape.lua")).is_err());
    let main = scratch.0.join("main.mlperformance");
    let changed = std::fs::read_to_string(&main)
        .unwrap()
        .replace("Edit Existing Synth", "Changed Base");
    std::fs::write(main, changed).unwrap();
    assert!(
        load_instrument(&scratch.0, &source)
            .unwrap_err()
            .contains("fingerprint")
    );
}

#[test]
fn save_failure_and_readonly_lua_failures_do_not_lose_authored_state() {
    let scratch = Scratch::new();
    let song = meldritch_dsl::load_song_directory(&scratch.0).unwrap();
    let (mut app, _) = controller(&song);
    app.dispatch(AppCommand::EditInstrument(changes()[0].clone()))
        .unwrap();
    let view = app.view_model().instrument_editor.unwrap();
    std::fs::write(scratch.0.join("authored"), "unrelated file").unwrap();
    assert!(save_instrument(&view).is_err());
    assert_eq!(
        std::fs::read_to_string(scratch.0.join("authored")).unwrap(),
        "unrelated file"
    );
    assert_eq!(
        app.view_model()
            .instrument_editor
            .unwrap()
            .source
            .fingerprint(),
        view.source.fingerprint()
    );
    let source = std::fs::read_to_string(fixture().join("main.lua")).unwrap();
    let source = source.replace(
        "return {name = \"edit_instrument\"",
        "state.instrument.parameters[1].value = 999; return {name = \"edit_instrument\"",
    );
    let project = LuaProject::load(&scratch.0, &source).unwrap();
    let mut plane = ControlPlane::new(project, &app).unwrap();
    let count = app.history().len();
    plane.enqueue("deck", 0, Input::Press(0)).unwrap();
    plane.pump(&mut app);
    assert_eq!(app.history().len(), count);
    assert!(plane.take_journal().commands.is_empty());
    assert!(
        plane
            .trace()
            .iter()
            .any(|trace| trace.detail.contains("read-only"))
    );
}

#[test]
fn invalid_lua_request_fixtures_are_rejected_without_accepted_facts() {
    let song = meldritch_dsl::load_song_directory(fixture()).unwrap();
    let source = std::fs::read_to_string(fixture().join("main.lua")).unwrap();
    let start = source.find("    interact = function(state)\n").unwrap()
        + "    interact = function(state)\n".len();
    let end = source[start..].find("    end,").unwrap() + start;
    for name in [
        "unknown-parameter.lua",
        "unknown-module.lua",
        "value-type.lua",
        "value-range.lua",
        "negative-release.lua",
    ] {
        let invalid = std::fs::read_to_string(fixture().join("invalid").join(name)).unwrap();
        let source = format!("{}{}{}", &source[..start], invalid, &source[end..]);
        let project = LuaProject::load(fixture(), &source).unwrap();
        let (mut app, _) = controller(&song);
        let mut plane = ControlPlane::new(project, &app).unwrap();
        plane.enqueue("terminal", 0, Input::Press(0)).unwrap();
        plane.pump(&mut app);
        assert!(app.history().is_empty(), "{name}");
        assert_eq!(
            app.view_model()
                .instrument_editor
                .unwrap()
                .source
                .fingerprint(),
            song.fingerprint()
        );
        assert!(plane.take_journal().commands.is_empty());
        assert!(
            plane
                .trace()
                .iter()
                .any(|trace| trace.phase == meldritch_control::Phase::Rejected),
            "{name}"
        );
    }
}

#[test]
fn rapid_source_edits_supersede_old_audio_and_save_through_the_production_observer() {
    let scratch = Scratch::new();
    let song = meldritch_dsl::load_song_directory(&scratch.0).unwrap();
    let (mut app, _) = controller(&song);
    let block = render(&song);
    let (publisher, reader) = audio_publication(PublishedAudio::from_block(&block, 1024).unwrap());
    let mut host = SongAudioHost::new(
        song.clone(),
        CompiledSongPatch::compile_initial(&song).unwrap(),
        publisher,
        96000,
        1024,
    );
    for edit in changes() {
        let result = app.handle_input(AppInput::EditInstrument(edit)).unwrap();
        assert!(host.accepted(&app.view_model(), &result).unwrap());
    }
    assert_eq!(host.generation(), 7);
    wait(&mut host);
    assert_eq!(reader.snapshot().generation(), 7);
    let view = app.view_model().instrument_editor.unwrap();
    let expected = render(&view.source);
    for frame in 0..96000 {
        assert_eq!(
            reader.snapshot().frame(frame).unwrap()[0],
            expected.samples()[frame as usize]
        );
    }
    let save = app.handle_input(AppInput::SaveInstrument).unwrap();
    assert!(!host.accepted(&app.view_model(), &save).unwrap());
    host.refresh_view(&mut app);
    assert!(
        app.view_model()
            .instrument_editor
            .unwrap()
            .save_status
            .unwrap()
            .contains("Saved edited-bass: authored/")
    );
    let saved = std::fs::read_dir(scratch.0.join("authored"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert_eq!(
        load_instrument_file(&scratch.0, saved.strip_prefix(&scratch.0).unwrap())
            .unwrap()
            .fingerprint(),
        view.source.fingerprint()
    );
    std::fs::remove_file(saved).unwrap();
    std::fs::remove_dir(scratch.0.join("authored")).unwrap();
    std::fs::write(scratch.0.join("authored"), "unrelated file").unwrap();
    let save = app.handle_input(AppInput::SaveInstrument).unwrap();
    assert!(host.accepted(&app.view_model(), &save).is_err());
    host.refresh_view(&mut app);
    assert!(
        app.view_model()
            .instrument_editor
            .unwrap()
            .save_status
            .unwrap()
            .contains("edits remain unsaved")
    );
    assert_eq!(
        app.view_model()
            .instrument_editor
            .unwrap()
            .source
            .fingerprint(),
        view.source.fingerprint()
    );
    assert_eq!(host.generation(), 7);
}

#[test]
fn selecting_an_existing_instrument_preserves_other_instruments_and_runtime() {
    let root = fixture().with_file_name("19-apc-mini-mk2-playground");
    let song = meldritch_dsl::load_song_directory(&root).unwrap();
    let (mut app, _) = controller(&song);
    let before = app.view_model();
    let first = before.instrument_editor.as_ref().unwrap().synth.clone();
    app.handle_input(AppInput::NavigateInstrument {
        synth_delta: 1,
        parameter_delta: 0,
    })
    .unwrap();
    let view = app.view_model().instrument_editor.unwrap();
    assert_ne!(view.synth, first);
    let parameter = view
        .parameters
        .iter()
        .find(|p| p.id.parameter == InstrumentParameter::CutoffHz)
        .unwrap();
    app.dispatch(AppCommand::EditInstrument(InstrumentEdit {
        target: parameter.id.clone(),
        value: InstrumentValue::Number(900.0),
    }))
    .unwrap();
    let view = app.view_model().instrument_editor.unwrap();
    assert_eq!(view.source.synths()[&first], song.synths()[&first]);
    let source = encode_instrument(&view).unwrap();
    let restored = load_instrument(&root, &source).unwrap();
    assert_eq!(restored.synths(), view.source.synths());
    assert_eq!(render(&restored), render(&view.source));
    assert_eq!(app.view_model().transport, before.transport);
    app.handle_input(AppInput::NavigateInstrument {
        synth_delta: -1,
        parameter_delta: 0,
    })
    .unwrap();
    assert_eq!(app.view_model().instrument_editor.unwrap().synth, first);
}

#[test]
fn checked_in_export_reloads_renders_and_session_capture_stays_separate() {
    use meldritch_control::session::PerformanceSessionCapture;
    let scratch = Scratch::new();
    let base = meldritch_dsl::load_song_directory(&scratch.0).unwrap();
    let loaded = load_instrument_file(&scratch.0, Path::new("edited-instrument.lua")).unwrap();
    let expected = base
        .edit_instrument(&edit(
            InstrumentParameter::CutoffHz,
            InstrumentValue::Number(900.0),
        ))
        .unwrap()
        .edit_instrument(&edit(
            InstrumentParameter::Release,
            InstrumentValue::Number(0.25),
        ))
        .unwrap();
    assert_eq!(loaded.fingerprint(), expected.fingerprint());
    assert_eq!(render(&loaded), render(&expected));
    assert_ne!(render(&loaded), render(&base));
    let (mut app, _) = controller(&base);
    app.enable_instrument_editor_from(base.clone(), loaded)
        .unwrap();
    let mut capture = PerformanceSessionCapture::create(&base, 96000).unwrap();
    let request = meldritch_control::CommandRequest::EditInstrument {
        edit: edit(InstrumentParameter::Release, InstrumentValue::Number(0.4)),
    };
    let result = app
        .dispatch(AppCommand::EditInstrument(edit(
            InstrumentParameter::Release,
            InstrumentValue::Number(0.4),
        )))
        .unwrap();
    capture
        .record_command(
            &app,
            &request,
            &result,
            "terminal",
            256,
            Tempo::new(120.0, 48000).unwrap(),
        )
        .unwrap();
    assert_eq!(capture.events.len(), 1);
    assert_eq!(capture.events[0].kind, "instrument_definition");
    assert!(capture.events[0].changed);
    capture.finish_clean(&app).unwrap();
    let journal = std::fs::read(capture.path()).unwrap();
    let view = app.view_model().instrument_editor.unwrap();
    let saved = save_instrument(&view).unwrap();
    assert_eq!(std::fs::read(capture.path()).unwrap(), journal);
    assert_eq!(
        load_instrument_file(&scratch.0, saved.strip_prefix(&scratch.0).unwrap())
            .unwrap()
            .fingerprint(),
        view.source.fingerprint()
    );
    let text = encode_instrument(&view).unwrap();
    let line = text
        .lines()
        .find(|line| line.contains("parameter = \"release\""))
        .unwrap();
    assert!(load_instrument(&scratch.0, &text.replace(line, "")).is_err());
    let duplicate = text.replace("parameter = \"release\"", "parameter = \"attack\"");
    assert!(
        load_instrument(&scratch.0, &duplicate)
            .unwrap_err()
            .contains("duplicate")
    );
}

#[test]
fn source_edit_rearms_matching_physical_pickup() {
    use meldritch_control::devices::MidiPickup;
    use meldritch_control::song_host::{initialize_pickup_controls, song_controls_for_view};
    let song =
        meldritch_dsl::load_song_directory(fixture().with_file_name("19-apc-mini-mk2-playground"))
            .unwrap();
    let (mut app, _) = controller(&song);
    let mut controls = song_controls_for_view(&song, None, None, None, None);
    initialize_pickup_controls(&song, &mut controls);
    app.set_curated_controls(controls);
    let view = app.view_model();
    let control = view
        .curated_controls
        .iter()
        .find(|c| c.target == "synth:bass-a/filter.cutoff_hz")
        .unwrap();
    let id = control.id.clone();
    let initial = (control.value.unwrap() - control.minimum) / (control.maximum - control.minimum);
    let input = |value| AppInput::SetCuratedControlNormalized {
        id: id.clone(),
        value,
    };
    let physical = MidiControlInput {
        device: "apc-mini".into(),
        channel: 1,
        cc: 52,
        value: 64,
    };
    let mut pickup = MidiPickup::default();
    assert!(
        pickup
            .observe(&physical, &input(initial), &app.view_model())
            .unwrap()
    );
    app.dispatch(AppCommand::EditInstrument(InstrumentEdit {
        target: InstrumentParameterId {
            synth: "bass-a".into(),
            module: "filter".into(),
            parameter: InstrumentParameter::CutoffHz,
        },
        value: InstrumentValue::Number(900.0),
    }))
    .unwrap();
    let view = app.view_model();
    let control = view.curated_controls.iter().find(|c| c.id == id).unwrap();
    assert_eq!(control.value, Some(900.0));
    assert!(!pickup.observe(&physical, &input(initial), &view).unwrap());
    let desired = (900.0 - control.minimum) / (control.maximum - control.minimum);
    assert!(pickup.observe(&physical, &input(desired), &view).unwrap());
}

#[test]
fn resetting_implicit_defaults_preserves_imported_representation_and_roundtrip() {
    let scratch = Scratch::new();
    let file = scratch.0.join("synths/edited-bass.mlsynth");
    let source = std::fs::read_to_string(&file)
        .unwrap()
        .replace("waveform = \"saw\"\n", "")
        .replace("cutoff_hz = 1200.0\n", "")
        .replace("resonance = 0.25\n", "");
    std::fs::write(file, source).unwrap();
    let base = meldritch_dsl::load_song_directory(&scratch.0).unwrap();
    let (mut app, _) = controller(&base);
    for change in changes().into_iter().filter(|p| {
        matches!(
            p.target.parameter,
            InstrumentParameter::Waveform
                | InstrumentParameter::CutoffHz
                | InstrumentParameter::Resonance
        )
    }) {
        let default = base
            .instrument_parameters("edited-bass")
            .unwrap()
            .into_iter()
            .find(|p| p.id == change.target)
            .unwrap();
        app.dispatch(AppCommand::EditInstrument(change)).unwrap();
        app.dispatch(AppCommand::EditInstrument(InstrumentEdit {
            target: default.id,
            value: default.value,
        }))
        .unwrap();
        let view = app.view_model().instrument_editor.unwrap();
        assert_eq!(view.source.fingerprint(), base.fingerprint());
        assert_eq!(view.source.synths(), base.synths());
        let restored = load_instrument(&scratch.0, &encode_instrument(&view).unwrap()).unwrap();
        assert_eq!(restored.fingerprint(), view.source.fingerprint());
        assert_eq!(render(&restored), render(&view.source));
    }
}
