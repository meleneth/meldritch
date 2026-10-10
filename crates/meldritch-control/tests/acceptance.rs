use meldritch_app::{AppCommand, AppController, CockpitMode, Selection};
use meldritch_audio::device_output::{PlaybackEngine, playback_session_parts};
use meldritch_control::devices::*;
use meldritch_control::*;
use meldritch_core::{FrameRange, Pattern, PatternId, ProbabilitySeed, StepIndex, Tempo, TrackId};
use meldritch_render::RenderSettings;
use meldritch_render::coordinator::{
    RenderCoordinator, RenderCoordinatorConfig, SampleRenderState,
};
use meldritch_render::live_edit::LivePatternEditor;
use meldritch_render::song_render::compile_drone_song;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../songs/examples/18-lua-control-plane")
}
fn source() -> String {
    std::fs::read_to_string(root().join("main.lua")).unwrap()
}
fn project(source: &str) -> LuaProject {
    LuaProject::load(root(), source).unwrap()
}

// Actual app/worker/audio-publication machinery, without a host device.
fn controller() -> (AppController, PlaybackEngine) {
    let (playback, engine) = playback_session_parts(8).unwrap();
    let pattern = Pattern::new(PatternId::new(1), 4, 4).unwrap();
    let state = SampleRenderState::new(
        pattern,
        Tempo::new(120.0, 48000).unwrap(),
        ProbabilitySeed::new(1),
        RenderSettings::new(1).unwrap(),
        Arc::new(BTreeMap::new()),
    );
    let coordinator = RenderCoordinator::new(
        RenderCoordinatorConfig::new(1, 24000, 6000, 1, Duration::from_millis(2)).unwrap(),
        state.pattern().clone(),
        state.tempo(),
        state.probability_seed(),
        state.settings(),
        Arc::clone(state.samples_by_note()),
        playback.status_monitor(),
    )
    .unwrap();
    (
        AppController::new(
            playback,
            coordinator,
            LivePatternEditor::new(state, 24000),
            Selection {
                track: TrackId::new(1),
                step: StepIndex::new(0),
            },
            256,
        ),
        engine,
    )
}

#[test]
fn lua_composes_existing_typed_model_and_roundtrips_deterministically() {
    let project = project(&source());
    let patch = compile_drone_song(&project.song).unwrap();
    let block = patch.render(FrameRange::new(0, 4096).unwrap()).unwrap();
    assert!(block.peak_abs() > 0.1);
    assert!(block.samples().iter().all(|s| s.is_finite()));
    let saved = project.save().unwrap();
    let restored = LuaProject::restore(root(), &saved).unwrap();
    assert_eq!(restored.save().unwrap(), saved);
    assert_eq!(restored.id, project.id);
    assert_eq!(restored.song.fingerprint(), project.song.fingerprint());
    assert_eq!(
        compile_drone_song(&restored.song)
            .unwrap()
            .render(FrameRange::new(0, 4096).unwrap())
            .unwrap(),
        block
    );
    let mut envelope: SavedProject = toml::from_str(&saved).unwrap();
    envelope.song_fingerprint = "wrong".into();
    assert!(LuaProject::restore(root(), &toml::to_string(&envelope).unwrap()).is_err());
}

#[test]
fn one_widget_drives_real_commands_and_coherent_surfaces() {
    let (mut app, _engine) = controller();
    let mut plane = ControlPlane::new(project(&source()), &app).unwrap();
    let apc = SimulatedDevice::new("apc");
    let deck = SimulatedDevice::new("deck");
    let tui = SimulatedDevice::new("terminal");
    apc.apc_message(&mut plane, 48000, &[0x90, 0, 127]).unwrap();
    assert_eq!(plane.pump(&mut app), 2); // input plus queued domain event
    assert_eq!(app.view_model().cockpit_mode, CockpitMode::AllParameters);
    assert_eq!(
        app.history().back().unwrap().command,
        AppCommand::SetCockpitMode(CockpitMode::AllParameters)
    );
    for device in [&apc, &deck, &tui] {
        assert_eq!(device.output(&plane).unwrap().cells[&0].color, "amber");
        assert!(
            device.output(&plane).unwrap().cells[&0]
                .label
                .contains("all_parameters")
        );
    }
    assert_eq!(deck.output(&plane).unwrap().cells[&0].style, "lcd");
    assert_eq!(apc.output(&plane).unwrap().cells[&0].style, "led");
    assert_eq!(tui.output(&plane).unwrap().cells[&0].style, "text");
    deck.press(&mut plane, 48001, 0).unwrap();
    plane.pump(&mut app);
    assert_eq!(app.view_model().cockpit_mode, CockpitMode::Performance);
    assert_eq!(apc.output(&plane).unwrap().cells[&0].color, "green");
    // Commands from an existing frontend are visible through the same projection.
    app.dispatch(AppCommand::SetCockpitMode(CockpitMode::AllParameters))
        .unwrap();
    plane.refresh(&app).unwrap();
    assert_eq!(apc.output(&plane).unwrap().cells[&0].color, "amber");
    let phases: Vec<_> = plane
        .trace()
        .iter()
        .filter(|t| t.frame == 48000 && t.source == "apc")
        .map(|t| t.phase)
        .collect();
    assert!(phases.starts_with(&[
        Phase::Input,
        Phase::Handler,
        Phase::Requested,
        Phase::Accepted,
        Phase::Diagnostic,
        Phase::Domain,
        Phase::Projection
    ]));
}

#[test]
fn hierarchical_pages_bubble_guard_reject_and_order_lifecycle() {
    let (mut app, _engine) = controller();
    let mut plane = ControlPlane::new(project(&source()), &app).unwrap();
    let deck = SimulatedDevice::new("deck");
    assert!(plane.output("deck").unwrap().cells.contains_key(&1));
    deck.press(&mut plane, 1, 31).unwrap();
    plane.pump(&mut app);
    assert_eq!(plane.active_state(), "home");
    assert!(
        plane
            .trace()
            .iter()
            .any(|t| t.phase == Phase::Rejected && t.detail == "inspect mode required")
    );
    deck.press(&mut plane, 2, 0).unwrap();
    deck.press(&mut plane, 3, 31).unwrap();
    plane.pump(&mut app);
    assert_eq!(plane.active_state(), "detail");
    assert!(!plane.output("deck").unwrap().cells.contains_key(&1));
    assert_eq!(
        plane.output("apc").unwrap().cells[&0].label,
        "detail: all_parameters"
    );
    let lifecycle: Vec<_> = plane
        .trace()
        .iter()
        .filter(|t| t.frame == 3 && [Phase::Exit, Phase::Entry].contains(&t.phase))
        .map(|t| (t.phase, t.detail.as_str()))
        .collect();
    assert_eq!(lifecycle, [(Phase::Exit, "home"), (Phase::Entry, "detail")]);
    // detail has no handler; online parent handles back.
    deck.press(&mut plane, 4, 30).unwrap();
    plane.pump(&mut app);
    assert_eq!(plane.active_state(), "home");
    assert!(
        plane
            .trace()
            .iter()
            .any(|t| t.frame == 4 && t.detail == "state online: back")
    );
    plane
        .enqueue("terminal", 5, Input::Named("unknown".into()))
        .unwrap();
    plane.pump(&mut app);
    assert!(
        plane
            .trace()
            .iter()
            .any(|t| t.frame == 5 && t.phase == Phase::Rejected && t.detail.contains("unhandled"))
    );
    assert_eq!(plane.take_journal().commands.len(), 1);
}

#[test]
fn reconnect_restores_current_page_without_losing_app_state() {
    let (mut app, _engine) = controller();
    let mut plane = ControlPlane::new(project(&source()), &app).unwrap();
    let deck = SimulatedDevice::new("deck");
    deck.connection(&mut plane, 1, false).unwrap();
    plane.pump(&mut app);
    assert!(deck.output(&plane).is_none());
    deck.press(&mut plane, 2, 0).unwrap();
    plane.pump(&mut app);
    assert!(app.history().is_empty());
    plane.enqueue("apc", 3, Input::Press(0)).unwrap();
    plane
        .enqueue("terminal", 4, Input::Named("next".into()))
        .unwrap();
    plane.pump(&mut app);
    assert_eq!(plane.active_state(), "detail");
    deck.connection(&mut plane, 5, true).unwrap();
    plane.pump(&mut app);
    assert_eq!(deck.output(&plane).unwrap().page, "detail");
    assert_eq!(deck.output(&plane).unwrap().cells[&0].color, "amber");
    assert_eq!(app.history().len(), 1);
}

#[test]
fn accepted_actions_have_lossless_frame_journal_and_typed_replay() {
    let (mut app, _engine) = controller();
    let mut plane = ControlPlane::new(project(&source()), &app).unwrap();
    for frame in [u64::from(u32::MAX) + 10, u64::from(u32::MAX) + 20] {
        plane.enqueue("apc", frame, Input::Press(0)).unwrap();
    }
    plane.pump(&mut app);
    let journal = plane.take_journal();
    let encoded = journal.encode().unwrap();
    assert_eq!(Journal::decode(&encoded).unwrap(), journal);
    assert_eq!(
        Journal::decode(&encoded).unwrap().encode().unwrap(),
        encoded
    );
    let (mut replay, _engine2) = controller();
    let frames = journal.replay(&plane.project, &mut replay).unwrap();
    assert_eq!(frames, [u64::from(u32::MAX) + 10, u64::from(u32::MAX) + 20]);
    assert_eq!(
        replay.view_model().cockpit_mode,
        app.view_model().cockpit_mode
    );
    assert_eq!(replay.history(), app.history());
    let mut wrong = journal.clone();
    wrong.song_fingerprint = "changed".into();
    assert!(wrong.replay(&plane.project, &mut replay).is_err());
    let directory = std::env::temp_dir().join(format!(
        "meldritch-lua-journal-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let first = journal.write_session(&directory).unwrap();
    let second = journal.write_session(&directory).unwrap();
    assert_ne!(first, second);
    let text = std::fs::read_to_string(first).unwrap();
    let saved: toml::Value = toml::from_str(&text).unwrap();
    assert_eq!(saved["meldritch"]["kind"].as_str(), Some("session"));
    let restored: Journal = saved["lua_control"].clone().try_into().unwrap();
    assert_eq!(restored, journal);
}

#[test]
fn negative_fixtures_and_command_validation_leave_state_unchanged() {
    for (name, diagnostic) in [
        ("malformed-binding", "binding"),
        ("invalid-transition", "transition"),
        ("duplicate-identity", "duplicate"),
        ("invalid-reference", "escapes"),
        ("parent-cycle", "cycle"),
    ] {
        let source = std::fs::read_to_string(root().join(format!("invalid/{name}.lua"))).unwrap();
        let err = LuaProject::load(root(), &source)
            .err()
            .expect("must reject");
        assert!(err.contains(diagnostic), "{name}: {err}");
    }
    let bad = std::fs::read_to_string(root().join("invalid/unknown-command.lua")).unwrap();
    let (mut app, _engine) = controller();
    let mut plane = ControlPlane::new(project(&bad), &app).unwrap();
    plane.enqueue("deck", 1, Input::Press(0)).unwrap();
    plane.pump(&mut app);
    assert!(app.history().is_empty());
    assert!(plane.take_journal().commands.is_empty());
    assert!(
        plane
            .trace()
            .iter()
            .any(|t| t.phase == Phase::Rejected && t.detail.contains("explode"))
    );
    // Known request rejected by actual AppController, not by a mock.
    let bad = bad.replace(
        "{name = \"explode\"}",
        "{name = \"select_page\", id = \"missing\"}",
    );
    let mut plane = ControlPlane::new(project(&bad), &app).unwrap();
    plane.enqueue("deck", 2, Input::Press(0)).unwrap();
    plane.pump(&mut app);
    assert!(app.history().is_empty());
    assert!(
        plane
            .trace()
            .iter()
            .any(|t| t.phase == Phase::Rejected && t.detail.contains("UnknownPerformancePage"))
    );
}

#[test]
fn sandbox_budget_isolation_and_readonly_projection() {
    assert!(LuaProject::load(root(), &" ".repeat(256 * 1024 + 1)).is_err());
    assert!(
        LuaProject::load(root(), "return string.rep('x', 8 * 1024 * 1024)")
            .err()
            .unwrap()
            .contains("memory")
    );
    assert!(
        LuaProject::load(root(), "while true do end")
            .err()
            .unwrap()
            .contains("budget")
    );
    for expression in [
        "os.execute('true')",
        "io.open('/etc/passwd')",
        "require('os')",
        "loadfile('/etc/passwd')",
        "load('return 1')",
    ] {
        assert!(LuaProject::load(root(), expression).is_err());
    }
    let probe = format!(
        "assert(_VERSION == 'Lua 5.4')\nassert(os == nil and io == nil and package == nil and debug == nil and coroutine == nil and private == nil)\nprivate = 1\n{}",
        source()
    );
    let _first = project(&probe);
    let _second = project(&probe);
    let bad = source().replace(
        "return {name = \"set_mode\",",
        "state.mode = 'bad'; return {name = \"set_mode\",",
    );
    let (mut app, _engine) = controller();
    let mut plane = ControlPlane::new(project(&bad), &app).unwrap();
    plane.enqueue("apc", 1, Input::Press(0)).unwrap();
    plane.pump(&mut app);
    assert!(app.history().is_empty());
    assert!(plane.trace().iter().any(|t| t.detail.contains("read-only")));
    let bad = source().replace(
        "return {name = \"set_mode\",",
        "while true do end; return {name = \"set_mode\",",
    );
    let mut plane = ControlPlane::new(project(&bad), &app).unwrap();
    plane.enqueue("apc", 2, Input::Press(0)).unwrap();
    plane.pump(&mut app);
    assert!(plane.trace().iter().any(|t| t.detail.contains("budget")));
}

#[test]
fn bounded_input_journal_and_causal_followups_are_explicit() {
    let (mut app, _engine) = controller();
    let mut plane = ControlPlane::new(project(&source()), &app).unwrap();
    assert!(
        plane
            .enqueue("apc", 0, Input::Named("command_accepted".into()))
            .is_err()
    );
    for _ in 0..QUEUE_CAPACITY {
        plane.enqueue("apc", 1, Input::Press(0)).unwrap();
    }
    assert!(
        plane
            .enqueue("apc", 1, Input::Press(0))
            .unwrap_err()
            .contains("full")
    );
    while plane.pending() != 0 {
        assert!(plane.pump(&mut app) <= QUEUE_CAPACITY);
    }
    assert_eq!(plane.take_journal().commands.len(), QUEUE_CAPACITY);
    for i in 0..JOURNAL_CAPACITY {
        plane.enqueue("apc", 2 + i as u64, Input::Press(0)).unwrap();
        plane.pump(&mut app);
    }
    let before = app.view_model().cockpit_mode;
    plane.enqueue("apc", 1000, Input::Press(0)).unwrap();
    plane.pump(&mut app);
    assert_eq!(app.view_model().cockpit_mode, before);
    assert!(
        plane
            .trace()
            .iter()
            .any(|t| t.detail.contains("journal full"))
    );
    assert_eq!(plane.take_journal().commands.len(), JOURNAL_CAPACITY);
    assert!(plane.dropped_trace() > 0);
    assert!(plane.enqueue("apc", 999, Input::Press(0)).is_err());
    // Feedback handler always requests another command; depth cap terminates it.
    let source = source().replace("if event.name == \"back\" then", "if event.name == 'command_accepted' then return {command = {name = 'set_mode', mode = 'performance'}} end\n        if event.name == \"back\" then");
    let mut plane = ControlPlane::new(project(&source), &app).unwrap();
    plane.enqueue("apc", 1, Input::Press(0)).unwrap();
    assert!(plane.pump(&mut app) <= QUEUE_CAPACITY);
    assert_eq!(plane.pending(), 0);
    assert!(
        plane
            .trace()
            .iter()
            .any(|t| t.detail.contains("causal event depth"))
    );
}

#[test]
fn projection_failure_retains_accepted_state_and_complete_previous_frames() {
    let (mut app, _engine) = controller();
    let bad = source().replace("return {label = page", "if state.mode == 'all_parameters' then error('display failed') end\n    return {label = page");
    let mut plane = ControlPlane::new(project(&bad), &app).unwrap();
    let before = plane.output("deck").unwrap().clone();
    plane.enqueue("apc", 1, Input::Press(0)).unwrap();
    plane.pump(&mut app);
    assert_eq!(app.view_model().cockpit_mode, CockpitMode::AllParameters);
    assert_eq!(plane.take_journal().commands.len(), 1);
    assert_eq!(plane.output("deck").unwrap(), &before);
    assert_eq!(plane.output("apc").unwrap().cells[&0].color, "green");
    assert!(
        plane
            .trace()
            .iter()
            .any(|t| t.phase == Phase::Diagnostic && t.detail.contains("display failed"))
    );
}

#[test]
fn saved_behavior_drift_and_undeclared_runtime_transitions_are_rejected() {
    let source = source();
    let saved = project(&source).save().unwrap();
    let mut envelope: SavedProject = toml::from_str(&saved).unwrap();
    envelope
        .lua_source
        .push_str("\n-- altered behavior source\n");
    assert!(LuaProject::restore(root(), &toml::to_string(&envelope).unwrap()).is_err());
    let bad = source.replace(
        "return {transition = \"detail\"}",
        "return {transition = \"online\"}",
    );
    let (mut app, _engine) = controller();
    let mut plane = ControlPlane::new(project(&bad), &app).unwrap();
    plane.enqueue("apc", 0, Input::Press(0)).unwrap();
    plane.enqueue("deck", 1, Input::Press(31)).unwrap();
    plane.pump(&mut app);
    assert_eq!(plane.active_state(), "home");
    assert!(
        plane
            .trace()
            .iter()
            .any(|t| t.phase == Phase::Rejected && t.detail.contains("undeclared transition"))
    );
    let bad = source.replace("if event.name == \"back\" then", "if event.name == 'exact_frame' then assert(event.frame == '18446744073709551615'); return {} end\n        if event.name == \"back\" then");
    let mut plane = ControlPlane::new(project(&bad), &app).unwrap();
    plane
        .enqueue("apc", u64::MAX, Input::Named("exact_frame".into()))
        .unwrap();
    plane.pump(&mut app);
    assert!(!plane.trace().iter().any(|t| t.phase == Phase::Rejected));
}

#[test]
fn protocol_codecs_and_takeover_match_documented_capabilities() {
    assert_eq!(
        decode_apc(0, &[0x90, 63, 127]),
        Some(ApcInput::Grid {
            cell: 63,
            pressed: true
        })
    );
    assert_eq!(
        decode_apc(0, &[0x90, 0, 0]),
        Some(ApcInput::Grid {
            cell: 0,
            pressed: false
        })
    );
    for cc in 0x30..=0x38 {
        assert_eq!(
            decode_apc(0, &[0xb0, cc, 127]),
            Some(ApcInput::Fader {
                strip: cc - 0x30,
                normalized: 1.0
            })
        );
    }
    assert!(decode_apc(1, &[0xb0, 0x30, 127]).is_none());
    assert!(decode_apc(0, &[0xb1, 0x30, 127]).is_none());
    assert_eq!(
        apc_rgb(0, [255, 128, 1]).unwrap(),
        [
            0xf0, 0x47, 0x7f, 0x4f, 0x24, 0, 8, 0, 0, 1, 127, 1, 0, 0, 1, 0xf7
        ]
    );
    assert!(apc_rgb(64, [0; 3]).is_err());
    let mut pickup = SoftTakeover::new(0.5).unwrap();
    assert_eq!(pickup.observe(0.1).unwrap(), None);
    assert_eq!(pickup.observe(0.4).unwrap(), None);
    assert_eq!(pickup.observe(0.6).unwrap(), Some(0.6));
    pickup.reset(0.2).unwrap();
    assert_eq!(pickup.observe(0.6).unwrap(), None);
    assert_eq!(pickup.observe(0.1).unwrap(), Some(0.1));
    assert!(pickup.observe(f64::NAN).is_err());
    assert_eq!(
        decode_keyboard(&[0x92, 60, 127], 1, 64),
        Some(KeyboardInput::Note {
            channel: 2,
            note: 60,
            velocity: 1.0,
            pressed: true
        })
    );
    assert_eq!(
        decode_keyboard(&[0xe0, 0, 64], 1, 64),
        Some(KeyboardInput::Pitch {
            channel: 0,
            value: 8192
        })
    );
    assert_eq!(
        decode_keyboard(&[0xb0, 64, 127], 1, 64),
        Some(KeyboardInput::Sustain {
            channel: 0,
            pressed: true
        })
    );
    assert!(decode_midi(&[0x90, 128, 1]).is_none());
}
