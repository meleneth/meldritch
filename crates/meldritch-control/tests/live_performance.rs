use meldritch_app::{AppCommand, AppController, Selection};
use meldritch_audio::audio_publication::audio_publication;
use meldritch_audio::device_output::{PlaybackEngine, playback_session_parts};
use meldritch_audio::published_audio::PublishedAudio;
use meldritch_control::devices::*;
use meldritch_control::performance::LuaPerformance;
use meldritch_control::session::PerformanceSessionCapture;
use meldritch_control::song_host::*;
use meldritch_control::*;
use meldritch_core::{FrameRange, Pattern, PatternId, ProbabilitySeed, StepIndex, Tempo, TrackId};
use meldritch_render::RenderSettings;
use meldritch_render::coordinator::{
    RenderCoordinator, RenderCoordinatorConfig, SampleRenderState,
};
use meldritch_render::live_edit::LivePatternEditor;
use meldritch_render::song_host::*;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

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

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../songs/examples/21-lua-apc-performance")
}

fn wait(host: &mut SongAudioHost) {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while host.poll_publish().unwrap().is_none() {
        assert!(
            std::time::Instant::now() < deadline,
            "render worker deadline"
        );
        std::thread::yield_now();
    }
}

#[test]
fn lua_apc_commands_compose_audio_surfaces_and_shared_session() {
    let source = std::fs::read_to_string(root().join("main.lua")).unwrap();
    let project = LuaProject::load(root(), &source).unwrap();
    let saved = project.save().unwrap();
    assert_eq!(
        LuaProject::restore(root(), &saved).unwrap().save().unwrap(),
        saved
    );
    let song = project.song.clone();
    let patch = CompiledSongPatch::compile_initial(&song).unwrap();
    let frames = 96000;
    let initial = patch
        .render_default(FrameRange::new(0, frames.into()).unwrap())
        .unwrap();
    let (publisher, reader) =
        audio_publication(PublishedAudio::from_block(&initial, 1024).unwrap());
    let retained = reader.snapshot();
    let (mut app, _engine) = controller();
    app.set_performance_pages(song_performance_pages_for_view(&song));
    let mut controls = song_controls_for_view(
        &song,
        patch.feedback(),
        patch.mix(),
        patch.cutoff_hz(),
        patch.resonance(),
    );
    initialize_pickup_controls(&song, &mut controls);
    app.set_curated_controls(controls);
    assert_eq!(
        app.view_model()
            .curated_controls
            .iter()
            .find(|control| control.id == "pad-cutoff")
            .unwrap()
            .value,
        Some(2100.0)
    );
    // Pad starts on C in the authored track, not the first listed variation A.
    assert_eq!(active_lane_variations(&app.view_model())["pad"], "pad-c");
    let mut audio = SongAudioHost::new(song.clone(), patch, publisher, frames, 1024);
    let tempo = Tempo::new(song.performance().bpm(), song.performance().sample_rate()).unwrap();
    let scratch = std::env::temp_dir().join(format!(
        "meldritch-lua-live-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut capture =
        PerformanceSessionCapture::create_at(&scratch, &song, frames, "20261010T000000Z").unwrap();
    let mut live = LuaPerformance::new(project, &app, tempo).unwrap();
    let (route, receiver) = ApcInputRoute::new([8, 17, 32, 41].into_iter().collect());
    assert!(route.observe(&[0x90, 8, 127]));
    assert!(route.observe(&[0x90, 8, 0])); // owned release, never a second command
    assert!(!route.observe(&[0x90, 63, 127])); // legacy binding remains reachable
    let cell = receiver.try_recv().unwrap();
    assert!(receiver.try_recv().is_err());
    assert_eq!(
        live.press("apc-mini", cell, 960, &mut app, &mut audio, &mut capture)
            .unwrap(),
        2
    );
    assert_eq!(app.history().len(), 1);
    wait(&mut audio);
    assert!(!Arc::ptr_eq(&retained, &reader.snapshot()));
    assert!(
        (0..frames)
            .any(|frame| retained.frame(frame).unwrap() != reader.snapshot().frame(frame).unwrap())
    );
    // A second lane change must retain the first lane's selection.
    live.press("apc-mini", 17, 1920, &mut app, &mut audio, &mut capture)
        .unwrap();
    wait(&mut audio);
    let view = app.view_model();
    assert_eq!(active_lane_variations(&view)["beat-drum"], "beat-drum-b");
    assert_eq!(
        active_lane_variations(&view)["rhythm-drum-a"],
        "rhythm-drum-a-c"
    );
    let expected = compile_mixed_song_for_active_lanes(
        &song,
        &active_lane_variations(&view),
        &BTreeMap::new(),
        &included_lanes_from_view(&view),
    )
    .unwrap()
    .render_default(FrameRange::new(0, frames.into()).unwrap())
    .unwrap();
    for frame in 0..frames {
        assert_eq!(
            reader.snapshot().frame(frame).unwrap(),
            &expected.samples()[frame as usize..frame as usize + 1]
        );
    }
    let apc = live.plane.output("apc-mini").unwrap();
    let deck = live.plane.output("deck").unwrap();
    let terminal = live.plane.output("terminal").unwrap();
    assert_eq!(apc.cells[&8].label, deck.cells[&1].label);
    assert_eq!(apc.cells[&8].color, terminal.cells[&1].color);
    assert_eq!(apc.cells[&8].color, "amber");
    // Another physical projection drives the same reusable widget.
    live.plane
        .enqueue("apc-mini", 2500, Input::Connection(false))
        .unwrap();
    live.plane.pump(&mut app);
    assert!(live.plane.output("apc-mini").is_none());
    live.press("deck", 4, 2880, &mut app, &mut audio, &mut capture)
        .unwrap(); // beat mute
    live.plane
        .enqueue("apc-mini", 3000, Input::Connection(true))
        .unwrap();
    live.plane.pump(&mut app);
    assert_eq!(
        live.plane.output("apc-mini").unwrap().cells[&32].color,
        "red"
    );
    live.press("apc-mini", 41, 3840, &mut app, &mut audio, &mut capture)
        .unwrap(); // rhythm solo
    wait(&mut audio);
    assert_eq!(
        live.plane.output("apc-mini").unwrap().cells[&32].color,
        "red"
    );
    assert_eq!(
        live.plane.output("deck").unwrap().cells[&11].color,
        "yellow"
    );
    // Existing typed fader path composes with both lane choices and solo.
    let result = app
        .dispatch(AppCommand::SetCuratedControlNormalized {
            id: "rhythm-drum-a-level".into(),
            value: 0.0,
        })
        .unwrap();
    audio.accepted(&app.view_model(), &result).unwrap();
    capture
        .record(
            &app,
            &meldritch_app::AppInput::SetCuratedControlNormalized {
                id: "rhythm-drum-a-level".into(),
                value: 0.0,
            },
            &result,
            frames,
            tempo,
        )
        .unwrap();
    live.external_accepted(&app, &result).unwrap();
    wait(&mut audio);
    assert!((0..frames).all(|frame| reader.snapshot().frame(frame).unwrap() == [0.0]));
    let before = app.history().len();
    live.press("apc-mini", 63, 4800, &mut app, &mut audio, &mut capture)
        .unwrap();
    assert_eq!(app.history().len(), before);
    assert_eq!(capture.events.len(), 5);
    assert_eq!(capture.events[0].absolute_frame, 960);
    assert_eq!(capture.events[0].provenance, "lua:apc-mini");
    assert!(capture.events[0].command.contains("SelectLaneVariation"));
    capture.finish_clean(&app).unwrap();
    let document: toml::Value =
        toml::from_str(&std::fs::read_to_string(capture.path()).unwrap()).unwrap();
    assert_eq!(
        document["meldritch"]["kind"].as_str(),
        Some("performance_session")
    );
    assert!(
        live.plane
            .trace()
            .iter()
            .any(|trace| trace.phase == Phase::Rejected)
    );
    std::fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn apc_transport_bounds_and_transactional_feedback() {
    let (route, receiver) = ApcInputRoute::new([0].into_iter().collect());
    for _ in 0..QUEUE_CAPACITY + 3 {
        assert!(route.observe(&[0x90, 0, 100]));
    }
    assert_eq!(route.take_dropped(), 3);
    assert_eq!(receiver.try_iter().count(), QUEUE_CAPACITY);
    assert!(!route.observe(&[0x91, 0, 100]));
    let mut frame = SurfaceFrame {
        revision: 1,
        page: "one".into(),
        cells: [(
            0,
            Presentation {
                label: "launch".into(),
                color: "green".into(),
                style: "led".into(),
            },
        )]
        .into_iter()
        .collect(),
    };
    let mut feedback = ApcFeedback::default();
    let mut messages = Vec::new();
    assert_eq!(
        feedback
            .update(&frame, |bytes| {
                messages.push(bytes.to_vec());
                Ok(())
            })
            .unwrap(),
        1
    );
    assert_eq!(messages[0], apc_rgb(0, [0, 180, 0]).unwrap());
    assert_eq!(
        feedback
            .update(&frame, |_| panic!("unchanged refresh"))
            .unwrap(),
        0
    );
    frame.cells.insert(
        1,
        Presentation {
            label: "invalid".into(),
            color: "puce".into(),
            style: "led".into(),
        },
    );
    assert!(
        feedback
            .update(&frame, |_| panic!("invalid frame partially sent"))
            .unwrap_err()
            .contains("unsupported")
    );
    frame.cells.remove(&1);
    frame.cells.get_mut(&0).unwrap().color = "red".into();
    assert!(
        feedback
            .update(&frame, |_| Err("disconnected".into()))
            .is_err()
    );
    assert_eq!(feedback.update(&frame, |_| Ok(())).unwrap(), 1); // failed diff retried
    feedback.invalidate();
    assert_eq!(feedback.update(&frame, |_| Ok(())).unwrap(), 1); // reconnect restored
    frame.cells.clear();
    assert_eq!(
        feedback
            .update(&frame, |bytes| {
                assert_eq!(bytes, apc_rgb(0, [0; 3]).unwrap());
                Ok(())
            })
            .unwrap(),
        1
    );
    assert_eq!(
        feedback
            .update(&frame, |_| panic!("clear repeated"))
            .unwrap(),
        0
    );
}

#[test]
fn invalid_lua_command_reference_is_rejected_before_recording_or_render() {
    assert_rejection("unknown-variation.lua", "UnknownLaneVariation");
    assert_rejection("unknown-command.lua", "unknown variant");
}

fn assert_rejection(fixture: &str, diagnostic: &str) {
    let source = std::fs::read_to_string(root().join("invalid").join(fixture)).unwrap();
    let project = LuaProject::load(root(), &source).unwrap();
    let song = project.song.clone();
    let patch = CompiledSongPatch::compile_initial(&song).unwrap();
    let initial = patch
        .render_default(FrameRange::new(0, 4096).unwrap())
        .unwrap();
    let (publisher, reader) =
        audio_publication(PublishedAudio::from_block(&initial, 1024).unwrap());
    let retained = reader.snapshot();
    let (mut app, _engine) = controller();
    app.set_performance_pages(song_performance_pages_for_view(&song));
    let mut audio = SongAudioHost::new(song.clone(), patch, publisher, 4096, 1024);
    let scratch =
        std::env::temp_dir().join(format!("meldritch-invalid-live-{}", std::process::id()));
    let mut capture =
        PerformanceSessionCapture::create_at(&scratch, &song, 4096, "20261010T000000Z").unwrap();
    let mut live = LuaPerformance::new(project, &app, Tempo::new(120.0, 48000).unwrap()).unwrap();
    live.press("apc-mini", 0, 0, &mut app, &mut audio, &mut capture)
        .unwrap();
    assert!(app.history().is_empty());
    assert!(capture.events.is_empty());
    assert!(audio.poll_publish().unwrap().is_none());
    assert!(Arc::ptr_eq(&retained, &reader.snapshot()));
    assert!(
        live.plane
            .trace()
            .iter()
            .any(|entry| entry.phase == Phase::Rejected && entry.detail.contains(diagnostic))
    );
    assert!(
        live.press("apc-mini", 0, u64::MAX, &mut app, &mut audio, &mut capture)
            .is_err()
    );
    std::fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn launch_control_single_track_keeps_delay_variations_and_mute() {
    let song =
        meldritch_dsl::load_song_directory(root().join("../16-launch-control-xl-playground"))
            .unwrap();
    let patch = CompiledSongPatch::compile_initial(&song).unwrap();
    let frames = 24000;
    let initial = patch
        .render_default(FrameRange::new(0, frames.into()).unwrap())
        .unwrap();
    let (publisher, reader) =
        audio_publication(PublishedAudio::from_block(&initial, 1024).unwrap());
    let (mut app, _engine) = controller();
    app.set_performance_pages(song_performance_pages_for_view(&song));
    let mut host = SongAudioHost::new(song.clone(), patch, publisher, frames, 1024);
    let result = app
        .dispatch(AppCommand::SelectLaneVariation {
            lane_id: "playground".into(),
            variation_id: "playground-bounce-fill".into(),
        })
        .unwrap();
    host.accepted(&app.view_model(), &result).unwrap();
    wait(&mut host);
    let expected = meldritch_render::song_render::compile_delayed_note_song_for_pattern(
        &song,
        "playground-bounce-fill",
    )
    .unwrap()
    .render(FrameRange::new(0, frames.into()).unwrap())
    .unwrap();
    for frame in 0..frames {
        assert_eq!(
            reader.snapshot().frame(frame).unwrap(),
            &expected.samples()[frame as usize..frame as usize + 1]
        );
    }
    let result = app
        .dispatch(AppCommand::ToggleLaneMute("playground".into()))
        .unwrap();
    host.accepted(&app.view_model(), &result).unwrap();
    wait(&mut host);
    assert!((0..frames).all(|frame| reader.snapshot().frame(frame).unwrap() == [0.0]));
    let result = app
        .dispatch(AppCommand::ToggleLaneMute("playground".into()))
        .unwrap();
    host.accepted(&app.view_model(), &result).unwrap();
    wait(&mut host);
    for frame in 0..frames {
        assert_eq!(
            reader.snapshot().frame(frame).unwrap(),
            &expected.samples()[frame as usize..frame as usize + 1]
        );
    }
}

#[test]
fn invalid_color_fixture_is_diagnosed_without_device_writes() {
    let source = std::fs::read_to_string(root().join("invalid/unknown-color.lua")).unwrap();
    let project = LuaProject::load(root(), &source).unwrap();
    let (app, _engine) = controller();
    let plane = ControlPlane::new(project, &app).unwrap();
    assert!(
        ApcFeedback::default()
            .update(plane.output("apc-mini").unwrap(), |_| panic!(
                "invalid color sent"
            ))
            .unwrap_err()
            .contains("puce")
    );
}

#[test]
fn fader_pickup_composes_with_lua_pages_audio_and_capture() {
    let root = root().join("../22-midi-soft-takeover");
    let project = LuaProject::load(
        &root,
        &std::fs::read_to_string(root.join("main.lua")).unwrap(),
    )
    .unwrap();
    let song = project.song.clone();
    let patch = CompiledSongPatch::compile_initial(&song).unwrap();
    let initial = patch
        .render_default(FrameRange::new(0, 24000).unwrap())
        .unwrap();
    let (publisher, reader) =
        audio_publication(PublishedAudio::from_block(&initial, 1024).unwrap());
    let (mut app, _engine) = controller();
    app.set_performance_pages(song_performance_pages_for_view(&song));
    let mut controls = song_controls_for_view(&song, None, None, None, None);
    initialize_pickup_controls(&song, &mut controls);
    app.set_curated_controls(controls);
    assert_eq!(app.view_model().curated_controls[0].value, Some(1.0));
    let mut audio = SongAudioHost::new(song.clone(), patch, publisher, 24000, 1024);
    let tempo = Tempo::new(120.0, 48000).unwrap();
    let scratch = std::env::temp_dir().join(format!("meldritch-pickup-{}", std::process::id()));
    let mut capture =
        PerformanceSessionCapture::create_at(&scratch, &song, 24000, "20261010T000000Z").unwrap();
    let mut live = LuaPerformance::new(project, &app, tempo).unwrap();
    let mut pickup = MidiPickup::default();
    let mut physical = meldritch_app::MidiControlInput {
        device: "apc-mini".into(),
        channel: 1,
        cc: 48,
        value: 0,
    };
    let input = |id: &str, value| meldritch_app::AppInput::SetCuratedControlNormalized {
        id: id.into(),
        value,
    };
    let retained = reader.snapshot();
    assert!(
        !pickup
            .observe(&physical, &input("a-level", 0.0), &app.view_model())
            .unwrap()
    );
    assert!(app.history().is_empty());
    assert!(audio.poll_publish().unwrap().is_none());
    assert!(capture.events.is_empty());
    assert!(Arc::ptr_eq(&retained, &reader.snapshot()));
    live.press("deck", 0, 0, &mut app, &mut audio, &mut capture)
        .unwrap();
    wait(&mut audio); // authoritative A = 0.5 from another interface
    assert!(
        !pickup
            .observe(&physical, &input("a-level", 0.0), &app.view_model())
            .unwrap()
    );
    physical.value = 64;
    let acquired = input("a-level", 64.0 / 127.0);
    assert!(
        pickup
            .observe(&physical, &acquired, &app.view_model())
            .unwrap()
    );
    let result = app.handle_input(acquired.clone()).unwrap();
    pickup.acknowledge(&physical, &app.view_model());
    audio.accepted(&app.view_model(), &result).unwrap();
    capture
        .record(&app, &acquired, &result, 24000, tempo)
        .unwrap();
    live.external_accepted(&app, &result).unwrap();
    wait(&mut audio);
    assert_eq!(capture.events.len(), 2);
    physical.value = 80;
    assert!(
        pickup
            .observe(
                &physical,
                &input("a-level", 80.0 / 127.0),
                &app.view_model()
            )
            .unwrap()
    );
    let previous = reader.snapshot();
    let movement = input("a-level", 80.0 / 127.0);
    let result = app.handle_input(movement.clone()).unwrap();
    pickup.acknowledge(&physical, &app.view_model());
    audio.accepted(&app.view_model(), &result).unwrap();
    capture
        .record(&app, &movement, &result, 24000, tempo)
        .unwrap();
    live.external_accepted(&app, &result).unwrap();
    wait(&mut audio);
    assert!(
        (0..24000)
            .any(|frame| previous.frame(frame).unwrap() != reader.snapshot().frame(frame).unwrap())
    );
    assert!(
        live.plane.output("deck").unwrap().cells[&0]
            .label
            .contains("0.63")
    );
    assert_eq!(capture.events.len(), 3);
    // Changing away and back with no physical movement still rearms.
    for page in ["b", "a"] {
        let selected = meldritch_app::AppInput::SelectPerformancePage(page.into());
        let result = app.handle_input(selected.clone()).unwrap();
        capture
            .record(&app, &selected, &result, 24000, tempo)
            .unwrap();
    }
    physical.value = 127;
    assert!(
        !pickup
            .observe(&physical, &input("a-level", 1.0), &app.view_model())
            .unwrap()
    );
    // Crossing from the other side is supported.
    assert!(
        pickup
            .observe(&physical, &input("a-level", 0.4), &app.view_model())
            .unwrap()
    );
    let result = app.handle_input(input("a-level", 0.4)).unwrap();
    pickup.acknowledge(&physical, &app.view_model());
    audio.accepted(&app.view_model(), &result).unwrap();
    capture
        .record(&app, &input("a-level", 0.4), &result, 24000, tempo)
        .unwrap();
    live.external_accepted(&app, &result).unwrap();
    // A Lua edit must release physical ownership, even after acquisition.
    live.press("deck", 0, 0, &mut app, &mut audio, &mut capture)
        .unwrap();
    assert!(
        !pickup
            .observe(&physical, &input("a-level", 0.1), &app.view_model())
            .unwrap()
    );
    assert!(
        pickup
            .observe(&physical, &input("a-level", 0.5), &app.view_model())
            .unwrap()
    );
    capture.finish_clean(&app).unwrap();
    assert_eq!(capture.events.len(), 7);
    std::fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn bounded_midi_queue_retains_page_and_fader_order() {
    let queue = MidiInputQueue::default();
    let route = queue.route("apc-mini");
    assert!(route.observe(&[0x90, 101, 127]));
    assert!(route.observe(&[0xb0, 48, 0]));
    assert_eq!(queue.next().unwrap().bytes, [0x90, 101, 127]);
    assert_eq!(queue.next().unwrap().bytes, [0xb0, 48, 0]);
    assert!(!route.observe(&[0xb0, 48, 128]));
    for _ in 0..QUEUE_CAPACITY + 2 {
        route.observe(&[0xb0, 48, 127]);
    }
    assert_eq!(queue.take_dropped(), 2);
    assert_eq!(
        (0..QUEUE_CAPACITY).filter_map(|_| queue.next()).count(),
        QUEUE_CAPACITY
    );
    assert!(queue.next().is_none());
}

#[test]
fn pickup_rejects_invalid_observations_and_rearms_other_controllers() {
    let song = meldritch_dsl::load_song_directory(root().join("../22-midi-soft-takeover")).unwrap();
    let (mut app, _engine) = controller();
    app.set_performance_pages(song_performance_pages_for_view(&song));
    let mut controls = song_controls_for_view(&song, None, None, None, None);
    initialize_pickup_controls(&song, &mut controls);
    app.set_curated_controls(controls);
    let fixture: toml::Value = toml::from_str(
        &std::fs::read_to_string(song.root().join("invalid/observations.toml")).unwrap(),
    )
    .unwrap();
    let mut pickup = MidiPickup::default();
    let physical = meldritch_app::MidiControlInput {
        device: "one".into(),
        channel: 1,
        cc: 48,
        value: 127,
    };
    for value in fixture["normalized_values"].as_array().unwrap() {
        let value: f64 = value.as_str().unwrap().parse().unwrap();
        assert!(
            pickup
                .observe(
                    &physical,
                    &meldritch_app::AppInput::SetCuratedControlNormalized {
                        id: "a-level".into(),
                        value
                    },
                    &app.view_model()
                )
                .is_err()
        );
    }
    let queue = MidiInputQueue::default();
    let route = queue.route("one");
    for message in fixture["malformed_messages"].as_array().unwrap() {
        let bytes = message
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_integer().unwrap() as u8)
            .collect::<Vec<_>>();
        assert!(!route.observe(&bytes));
    }
    assert!(queue.next().is_none());
    assert!(
        pickup
            .observe(
                &physical,
                &meldritch_app::AppInput::SetCuratedControlNormalized {
                    id: fixture["missing_control"].as_str().unwrap().into(),
                    value: 0.5
                },
                &app.view_model()
            )
            .is_err()
    );
    assert!(app.history().is_empty());
    let mut input = meldritch_app::AppInput::SetCuratedControlNormalized {
        id: "a-level".into(),
        value: 1.0,
    };
    let mut second = physical.clone();
    second.device = "two".into();
    assert!(
        pickup
            .observe(&physical, &input, &app.view_model())
            .unwrap()
    );
    assert!(pickup.observe(&second, &input, &app.view_model()).unwrap());
    input = meldritch_app::AppInput::SetCuratedControlNormalized {
        id: "a-level".into(),
        value: 0.5,
    };
    assert!(
        pickup
            .observe(&physical, &input, &app.view_model())
            .unwrap()
    );
    app.handle_input(input).unwrap();
    pickup.acknowledge(&physical, &app.view_model());
    let at_max = meldritch_app::AppInput::SetCuratedControlNormalized {
        id: "a-level".into(),
        value: 1.0,
    };
    assert!(!pickup.observe(&second, &at_max, &app.view_model()).unwrap());
    assert!(
        pickup
            .observe(&physical, &at_max, &app.view_model())
            .unwrap()
    );
    // Bounded storage, including when many unrelated device identities appear.
    let mut pickup = MidiPickup::default();
    for index in 0..QUEUE_CAPACITY {
        let mut physical = physical.clone();
        physical.device = format!("device-{index}");
        assert!(
            !pickup
                .observe(&physical, &at_max, &app.view_model())
                .unwrap()
        );
    }
    assert!(
        pickup
            .observe(&second, &at_max, &app.view_model())
            .unwrap_err()
            .contains("full")
    );
    // A history gap invalidates acquired state rather than trusting old context.
    let mut pickup = MidiPickup::default();
    let at_half = meldritch_app::AppInput::SetCuratedControlNormalized {
        id: "a-level".into(),
        value: 0.5,
    };
    assert!(
        pickup
            .observe(&physical, &at_half, &app.view_model())
            .unwrap()
    );
    for _ in 0..300 {
        app.dispatch(AppCommand::SetCockpitMode(
            meldritch_app::CockpitMode::Performance,
        ))
        .unwrap();
    }
    assert!(
        !pickup
            .observe(&physical, &at_max, &app.view_model())
            .unwrap()
    );
}
