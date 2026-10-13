use meldritch_app::{
    AppCommand, AppCommandResult, AppController, CockpitMode, KeyboardResetReason,
};
use meldritch_control::{
    native_capture::{NativeCommandEnvelope, NativeProjectCapture, load_native_capture_baseline},
    native_project::{encode_native_project, load_native_project},
};
use meldritch_core::FrameRange;
use meldritch_render::song_render::{LiveVoiceEvent, LiveVoiceInput, compile_pattern_voice};
use std::{fs, path::PathBuf};

const FIXTURE: &str = include_str!("../../../songs/examples/36-patch-lfo/main.project.lua");
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "meldritch-keys-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn app(&self) -> AppController {
        let mut app = AppController::new_empty_project(
            &meldritch_dsl::load_project_directory(&self.0).unwrap(),
        )
        .unwrap()
        .0;
        app.enable_native_project_source(load_native_project(&self.0, FIXTURE).unwrap())
            .unwrap();
        app
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn outputs(result: AppCommandResult) -> Vec<LiveVoiceInput> {
    match result {
        AppCommandResult::KeyboardUpdated {
            outputs,
            output_voice,
            ..
        } => {
            if !outputs.is_empty() {
                assert_eq!(output_voice.as_deref(), Some("bass-instance"));
            }
            outputs
        }
        other => panic!("{other:?}"),
    }
}
fn key(app: &AppController, frame: u64, channel: u8, note: u8, pressed: bool) -> AppCommand {
    AppCommand::KeyboardKey {
        epoch: app.keyboard().epoch,
        frame,
        channel,
        note,
        velocity: 127,
        pressed,
    }
}
fn bind(app: &mut AppController, frame: u64) {
    app.dispatch(AppCommand::BindKeyboard {
        voice: Some("bass-instance".into()),
        frame,
    })
    .unwrap();
}

#[test]
fn keyboard_priority_independent_signals_capture_replay_and_fresh_authored_reload() {
    let directory = Directory::new();
    let trace: toml::Value = toml::from_str(include_str!(
        "../../../songs/examples/36-patch-lfo/keyboard-lifecycle.toml"
    ))
    .unwrap();
    let origin = trace["origin_frame"].as_integer().unwrap() as u64;
    let mut app = directory.app();
    let source = app.authored_source().unwrap();
    let fingerprint = source.fingerprint();
    let encoded = encode_native_project(&source).unwrap();
    let patch = compile_pattern_voice(&source, &source.pattern_voices()["bass-instance"]).unwrap();
    let mut capture = NativeProjectCapture::new(source, 96_000);
    bind(&mut app, origin);
    capture.checkpoint(&app).unwrap();
    let mut emitted = Vec::new();
    for entry in trace["keys"].as_array().unwrap() {
        let frame = origin + entry["offset"].as_integer().unwrap() as u64;
        let command = key(
            &app,
            frame,
            entry["channel"].as_integer().unwrap() as u8,
            entry["note"].as_integer().unwrap() as u8,
            entry["pressed"].as_bool().unwrap(),
        );
        let actual = outputs(app.dispatch(command).unwrap());
        let expected: Vec<_> = entry["outputs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| {
                let (kind, value) = value.as_str().unwrap().split_once(':').unwrap();
                match kind {
                    "pitch" => LiveVoiceInput::Pitch(value.parse().unwrap()),
                    "velocity" => LiveVoiceInput::Velocity(value.parse().unwrap()),
                    "gate" => LiveVoiceInput::Gate(value.parse().unwrap()),
                    _ => unreachable!(),
                }
            })
            .collect();
        assert_eq!(actual, expected);
        emitted.extend(
            actual
                .into_iter()
                .map(|input| LiveVoiceEvent { frame, input }),
        );
        capture.checkpoint(&app).unwrap();
        assert_eq!(app.authored_source().unwrap().fingerprint(), fingerprint);
    }
    assert_eq!(app.keyboard().pitch, Some(55));
    assert!(!app.keyboard().gate);
    assert_eq!(app.view_model().keyboard, *app.keyboard());
    let mut live = patch.live_renderer(48, origin).unwrap();
    let range = FrameRange::new(origin, origin + 1000).unwrap();
    let audio = live.render(range, &emitted).unwrap();
    assert!(
        audio.samples()[..600]
            .iter()
            .any(|sample| sample.abs() > 0.01)
    );
    assert!(audio.samples()[600..].iter().all(|sample| *sample == 0.0));
    capture.finish(&app).unwrap();
    let text = fs::read_to_string(capture.session.as_ref().unwrap().path()).unwrap();
    let document: toml::Value = toml::from_str(&text).unwrap();
    // Restore the same fresh-load boundary as the captured app.
    let mut replay = AppController::new_empty_project(
        &meldritch_dsl::load_project_directory(&directory.0).unwrap(),
    )
    .unwrap()
    .0;
    replay
        .enable_native_project_source(load_native_capture_baseline(&directory.0, &text).unwrap())
        .unwrap();
    let mut reconstructed = Vec::new();
    for event in document["events"].as_array().unwrap() {
        let envelope =
            NativeCommandEnvelope::decode(&toml::to_string(&event["native"]).unwrap()).unwrap();
        assert_eq!(envelope.source_before, format!("{fingerprint:016x}"));
        assert_eq!(envelope.source_after, envelope.source_before);
        let command: AppCommand = envelope.command.into();
        let frame = match &command {
            AppCommand::BindKeyboard { frame, .. } | AppCommand::KeyboardKey { frame, .. } => {
                *frame
            }
            _ => panic!("unexpected capture"),
        };
        reconstructed.extend(
            outputs(replay.dispatch(command).unwrap())
                .into_iter()
                .map(|input| LiveVoiceEvent { frame, input }),
        );
    }
    assert_eq!(reconstructed, emitted);
    assert_eq!(replay.keyboard(), app.keyboard());
    assert_eq!(
        patch
            .live_renderer(48, origin)
            .unwrap()
            .render(range, &reconstructed)
            .unwrap()
            .samples(),
        audio.samples()
    );
    assert_eq!(
        encode_native_project(&app.authored_source().unwrap()).unwrap(),
        encoded
    );
    fs::write(directory.0.join("main.project.lua"), encoded).unwrap();
    let mut fresh = AppController::new_empty_project(
        &meldritch_dsl::load_project_directory(&directory.0).unwrap(),
    )
    .unwrap()
    .0;
    fresh
        .enable_native_project_source(
            load_native_project(
                &directory.0,
                &fs::read_to_string(directory.0.join("main.project.lua")).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(fresh.authored_source().unwrap().fingerprint(), fingerprint);
    assert_eq!(fresh.keyboard().voice, None);
    assert!(!fresh.keyboard().gate);
    assert_eq!(fresh.keyboard().held_keys, 0);
}

#[test]
fn keyboard_reset_epoch_boundedness_and_rejected_commands_are_atomic() {
    let directory = Directory::new();
    let cases: toml::Value = toml::from_str(include_str!(
        "../../../songs/examples/36-patch-lfo/invalid/keyboard-lifecycle.toml"
    ))
    .unwrap();
    for name in cases["cases"].as_array().unwrap() {
        let mut app = directory.app();
        bind(&mut app, 100);
        app.dispatch(key(&app, 100, 0, 48, true)).unwrap();
        let command = match name.as_str().unwrap() {
            "unknown-target" => AppCommand::BindKeyboard {
                voice: Some("missing".into()),
                frame: 100,
            },
            "inactive-target" => {
                app.dispatch(AppCommand::CreateComposition(
                    meldritch_dsl::CompositionDefinition {
                        id: "empty".into(),
                        voices: vec![],
                    },
                ))
                .unwrap();
                app.dispatch(AppCommand::SelectComposition("empty".into()))
                    .unwrap();
                AppCommand::BindKeyboard {
                    voice: Some("bass-instance".into()),
                    frame: 100,
                }
            }
            "rejected-source-edit" => AppCommand::CreatePalettePattern {
                id: "invalid".into(),
                length_ticks: 0,
                looped: true,
            },
            "polyphonic-target" => {
                assert!(
                    load_native_project(
                        &directory.0,
                        &FIXTURE.replace("polyphony = 1", "polyphony = 2")
                    )
                    .unwrap_err()
                    .contains("mono")
                );
                continue;
            }
            "legacy-source" => {
                app.enable_project_source(
                    meldritch_dsl::load_song_directory(
                        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                            .join("../../songs/examples/26-apc-note-pattern"),
                    )
                    .unwrap(),
                )
                .unwrap();
                AppCommand::BindKeyboard {
                    voice: Some("bass-instance".into()),
                    frame: 100,
                }
            }
            "channel-16" => key(&app, 100, 16, 48, true),
            "pitch-128" => key(&app, 100, 0, 128, true),
            "stale-epoch" => AppCommand::KeyboardKey {
                epoch: app.keyboard().epoch - 1,
                frame: 100,
                channel: 0,
                note: 48,
                velocity: 127,
                pressed: true,
            },
            "backwards-frame" => key(&app, 99, 0, 48, true),
            "held-key-129" => {
                for note in 0..128 {
                    app.dispatch(key(&app, 100, 0, note, true)).unwrap();
                }
                // Existing keys can retrigger at capacity without growing state.
                app.dispatch(key(&app, 100, 0, 0, true)).unwrap();
                assert_eq!(app.keyboard().held_keys, 128);
                key(&app, 100, 1, 0, true)
            }
            "unbound-key" | "unbound-reset" => {
                app.dispatch(AppCommand::BindKeyboard {
                    voice: None,
                    frame: 100,
                })
                .unwrap();
                if name.as_str() == Some("unbound-key") {
                    key(&app, 100, 0, 48, true)
                } else {
                    AppCommand::ResetKeyboard {
                        epoch: app.keyboard().epoch,
                        frame: 100,
                        reason: KeyboardResetReason::Panic,
                    }
                }
            }
            name => panic!("untested {name}"),
        };
        let previous = app.keyboard().clone();
        let history = app.history().len();
        let source = app.authored_source().unwrap().fingerprint();
        assert!(app.dispatch(command).is_err(), "{}", name);
        assert_eq!(app.keyboard(), &previous);
        assert_eq!(app.history().len(), history);
        assert_eq!(app.authored_source().unwrap().fingerprint(), source);
    }
    for reason in [
        KeyboardResetReason::Disconnect,
        KeyboardResetReason::Overflow,
        KeyboardResetReason::Panic,
    ] {
        let mut app = directory.app();
        bind(&mut app, 0);
        app.dispatch(key(&app, 0, 0, 48, true)).unwrap();
        app.dispatch(key(&app, 0, 1, 55, true)).unwrap();
        let stale = key(&app, 1, 0, 48, false);
        let reset = AppCommand::ResetKeyboard {
            epoch: app.keyboard().epoch,
            frame: 1,
            reason,
        };
        let encoded = toml::to_string(&NativeCommandEnvelope {
            version: 1,
            source_before: "0000000000000000".into(),
            source_after: "0000000000000000".into(),
            command: (&reset).try_into().unwrap(),
        })
        .unwrap();
        assert_eq!(
            AppCommand::from(NativeCommandEnvelope::decode(&encoded).unwrap().command),
            reset
        );
        assert_eq!(
            outputs(app.dispatch(reset).unwrap()),
            vec![LiveVoiceInput::Gate(false)]
        );
        assert_eq!(app.keyboard().held_keys, 0);
        assert!(app.dispatch(stale).is_err());
        assert!(outputs(app.dispatch(key(&app, 1, 0, 48, false)).unwrap()).is_empty());
        assert_eq!(
            outputs(app.dispatch(key(&app, 1, 0, 60, true)).unwrap()),
            vec![
                LiveVoiceInput::Pitch(60),
                LiveVoiceInput::Velocity(127),
                LiveVoiceInput::Gate(true)
            ]
        );
        let current = app.keyboard().clone();
        app.dispatch(AppCommand::SetCockpitMode(CockpitMode::Performance))
            .unwrap();
        app.dispatch(AppCommand::SelectPatternVoice("bass-instance".into()))
            .unwrap();
        assert_eq!(app.keyboard(), &current);
        // Rebinding emits release to the previous target and clears held state.
        assert_eq!(
            outputs(
                app.dispatch(AppCommand::BindKeyboard {
                    voice: None,
                    frame: 2
                })
                .unwrap()
            ),
            vec![LiveVoiceInput::Gate(false)]
        );
        bind(&mut app, 2);
        app.dispatch(key(&app, 2, 0, 60, true)).unwrap();
        let stale = key(&app, 3, 0, 60, false);
        app.dispatch(AppCommand::CreatePalettePattern {
            id: "new-pattern".into(),
            length_ticks: 3840,
            looped: true,
        })
        .unwrap();
        assert_eq!(app.keyboard().voice, None);
        assert!(!app.keyboard().gate);
        assert!(app.dispatch(stale).is_err());
    }
}

#[test]
fn midi_runtime_lowering_preserves_channel_release_epoch_and_frame() {
    use meldritch_control::devices::keyboard_key_command;
    let directory = Directory::new();
    let mut app = directory.app();
    bind(&mut app, 5_000_000_000);
    let epoch = app.keyboard().epoch;
    for (bytes, pressed) in [
        ([0x9f, 60, 127], true),
        ([0x9f, 60, 0], false),
        ([0x8f, 60, 127], false),
    ] {
        let command = keyboard_key_command(&bytes, epoch, 5_000_000_001).unwrap();
        assert_eq!(
            command,
            AppCommand::KeyboardKey {
                epoch,
                frame: 5_000_000_001,
                channel: 15,
                note: 60,
                velocity: bytes[2],
                pressed
            }
        );
        app.dispatch(command).unwrap();
    }
    for bytes in [
        &[0x90, 128, 10][..],
        &[0x90, 60][..],
        &[0xb0, 64, 127][..],
        &[0xb0, 1, 10][..],
        &[0xe0, 0, 64][..],
        &[0xf8][..],
    ] {
        assert!(keyboard_key_command(bytes, epoch, 5_000_000_001).is_none());
    }
    let stale = keyboard_key_command(&[0x90, 60, 127], epoch, 5_000_000_002).unwrap();
    app.dispatch(AppCommand::ResetKeyboard {
        epoch,
        frame: 5_000_000_002,
        reason: KeyboardResetReason::Overflow,
    })
    .unwrap();
    assert!(app.dispatch(stale).is_err());
}

#[test]
fn equal_pitch_on_distinct_channels_preserves_gate_until_both_release() {
    let directory = Directory::new();
    let mut app = directory.app();
    bind(&mut app, 0);
    app.dispatch(key(&app, 0, 0, 48, true)).unwrap();
    app.dispatch(key(&app, 0, 1, 48, true)).unwrap();
    assert_eq!(app.keyboard().held_keys, 2);
    assert_eq!(
        outputs(app.dispatch(key(&app, 1, 1, 48, false)).unwrap()),
        vec![LiveVoiceInput::Pitch(48), LiveVoiceInput::Velocity(127)]
    );
    assert!(app.keyboard().gate);
    assert_eq!(
        outputs(app.dispatch(key(&app, 2, 0, 48, false)).unwrap()),
        vec![LiveVoiceInput::Gate(false)]
    );
}

#[test]
fn explicit_velocity_patch_restores_held_expression_captures_and_reopens() {
    let directory = Directory::new();
    let fixture = include_str!("../../../songs/examples/36-patch-lfo/velocity.project.lua");
    let mut app = directory.app();
    app.enable_native_project_source(load_native_project(&directory.0, fixture).unwrap())
        .unwrap();
    let unwired = app.authored_source().unwrap();
    let reference =
        compile_pattern_voice(&unwired, &unwired.pattern_voices()["bass-instance"]).unwrap();
    app.dispatch(AppCommand::SelectInstrument("bass".into()))
        .unwrap();
    app.dispatch(AppCommand::ToggleInstrumentEditor).unwrap();
    app.dispatch(AppCommand::TogglePatchEditor).unwrap();
    for line in include_str!("../../../songs/examples/36-patch-lfo/velocity-patch.txt").lines() {
        for character in line.chars() {
            app.dispatch(AppCommand::PatchCharacter(character)).unwrap();
        }
        app.dispatch(AppCommand::PatchSubmit).unwrap();
    }
    let source = app.authored_source().unwrap();
    assert_ne!(source.fingerprint(), unwired.fingerprint());
    assert_eq!(
        source.synths()["bass"].inputs()[0].control_unit(),
        Some(meldritch_dsl::ControlUnit::UnitInterval)
    );
    let encoded = encode_native_project(&source).unwrap();
    let patch = compile_pattern_voice(&source, &source.pattern_voices()["bass-instance"]).unwrap();
    let trace: toml::Value = toml::from_str(include_str!(
        "../../../songs/examples/36-patch-lfo/keyboard-velocity.toml"
    ))
    .unwrap();
    let origin = trace["origin_frame"].as_integer().unwrap() as u64;
    // New capture begins after construction; runtime expression never changes source.
    let mut capture = NativeProjectCapture::new(source.clone(), 96_000);
    let history_start = app.history().len();
    bind(&mut app, origin);
    let mut emitted = Vec::new();
    for entry in trace["keys"].as_array().unwrap() {
        let frame = origin + entry["offset"].as_integer().unwrap() as u64;
        let note = entry["note"].as_integer().unwrap() as u8;
        let velocity = entry["velocity"].as_integer().unwrap() as u8;
        let pressed = entry["pressed"].as_bool().unwrap();
        let command = meldritch_control::devices::keyboard_key_command(
            &[if pressed { 0x90 } else { 0x80 }, note, velocity],
            app.keyboard().epoch,
            frame,
        )
        .unwrap();
        let actual = outputs(app.dispatch(command).unwrap());
        let expected = match frame - origin {
            0 => vec![
                LiveVoiceInput::Pitch(48),
                LiveVoiceInput::Velocity(32),
                LiveVoiceInput::Gate(true),
            ],
            256 => vec![
                LiveVoiceInput::Pitch(55),
                LiveVoiceInput::Velocity(96),
                LiveVoiceInput::Gate(true),
            ],
            512 => vec![LiveVoiceInput::Pitch(48), LiveVoiceInput::Velocity(32)],
            768 => vec![
                LiveVoiceInput::Pitch(48),
                LiveVoiceInput::Velocity(64),
                LiveVoiceInput::Gate(true),
            ],
            1024 => vec![LiveVoiceInput::Gate(false)],
            _ => unreachable!(),
        };
        assert_eq!(actual, expected);
        emitted.extend(
            actual
                .into_iter()
                .map(|input| LiveVoiceEvent { frame, input }),
        );
        assert!(std::sync::Arc::ptr_eq(
            &source,
            &app.authored_source().unwrap()
        ));
    }
    assert_eq!(app.keyboard().velocity, Some(64));
    let range = FrameRange::new(origin, origin + 1280).unwrap();
    let audio = patch
        .live_renderer(48, origin)
        .unwrap()
        .render(range, &emitted)
        .unwrap();
    let unscaled = reference
        .live_renderer(48, origin)
        .unwrap()
        .render(range, &emitted)
        .unwrap();
    for (i, (actual, raw)) in audio.samples().iter().zip(unscaled.samples()).enumerate() {
        let velocity = [32.0, 96.0, 32.0, 64.0, 64.0][i / 256] / 127.0;
        assert!((actual - raw * velocity).abs() < 1e-14);
    }
    assert!(audio.samples()[..1024].iter().any(|s| s.abs() > 0.01));
    assert!(audio.samples()[1024..].iter().all(|s| *s == 0.0));
    // An unwired external velocity cannot affect gain, even at zero.
    let zero: Vec<_> = emitted
        .iter()
        .map(|event| LiveVoiceEvent {
            frame: event.frame,
            input: match event.input {
                LiveVoiceInput::Velocity(_) => LiveVoiceInput::Velocity(0),
                input => input,
            },
        })
        .collect();
    assert_eq!(
        unscaled.samples(),
        reference
            .live_renderer(48, origin)
            .unwrap()
            .render(range, &zero)
            .unwrap()
            .samples()
    );
    // Old version-1 captures without velocity still mean unity.
    for velocity in [0, 64, 127] {
        let inputs = [
            LiveVoiceEvent {
                frame: origin,
                input: LiveVoiceInput::Velocity(velocity),
            },
            LiveVoiceEvent {
                frame: origin,
                input: LiveVoiceInput::Gate(true),
            },
        ];
        let scaled = patch
            .live_renderer(48, origin)
            .unwrap()
            .render(range, &inputs)
            .unwrap();
        let raw = reference
            .live_renderer(48, origin)
            .unwrap()
            .render(range, &inputs)
            .unwrap();
        for (actual, raw) in scaled.samples().iter().zip(raw.samples()) {
            assert!((actual - raw * f64::from(velocity) / 127.0).abs() < 1e-14);
        }
    }
    let old: meldritch_control::native_capture::NativeProjectCommand = serde_json::from_str(r#"{"kind":"keyboard_key","value":{"epoch":1,"frame":0,"channel":0,"note":48,"pressed":true}}"#).unwrap();
    assert!(matches!(
        AppCommand::from(old),
        AppCommand::KeyboardKey { velocity: 127, .. }
    ));
    // Capture the runtime suffix through the actual journal persistence boundary.
    // Start a fresh app so the capture's history cursor contains no construction.
    let mut replay = directory.app();
    replay
        .enable_native_project_source((*source).clone())
        .unwrap();
    for entry in app.history().iter().skip(history_start) {
        replay.dispatch(entry.command.clone()).unwrap();
    }
    capture.finish(&replay).unwrap();
    let text = fs::read_to_string(capture.session.as_ref().unwrap().path()).unwrap();
    let baseline = load_native_capture_baseline(&directory.0, &text).unwrap();
    let mut restored = directory.app();
    restored.enable_native_project_source(baseline).unwrap();
    let document: toml::Value = toml::from_str(&text).unwrap();
    let mut replayed = Vec::new();
    for event in document["events"].as_array().unwrap() {
        let envelope =
            NativeCommandEnvelope::decode(&toml::to_string(&event["native"]).unwrap()).unwrap();
        let command: AppCommand = envelope.command.into();
        let frame = match &command {
            AppCommand::BindKeyboard { frame, .. } | AppCommand::KeyboardKey { frame, .. } => {
                *frame
            }
            _ => panic!("unexpected capture"),
        };
        replayed.extend(
            outputs(restored.dispatch(command).unwrap())
                .into_iter()
                .map(|input| LiveVoiceEvent { frame, input }),
        );
    }
    assert_eq!(replayed, emitted);
    assert_eq!(
        audio.samples(),
        patch
            .live_renderer(48, origin)
            .unwrap()
            .render(range, &replayed)
            .unwrap()
            .samples()
    );
    fs::write(directory.0.join("main.project.lua"), &encoded).unwrap();
    let reopened = load_native_project(
        &directory.0,
        &fs::read_to_string(directory.0.join("main.project.lua")).unwrap(),
    )
    .unwrap();
    assert_eq!(reopened.fingerprint(), source.fingerprint());
    assert_eq!(encode_native_project(&reopened).unwrap(), encoded);
    restored.enable_native_project_source(reopened).unwrap();
    assert_eq!(restored.keyboard().voice, None);
}

#[test]
fn invalid_velocity_inputs_and_graphs_are_atomic() {
    let directory = Directory::new();
    let cases: toml::Value = toml::from_str(include_str!(
        "../../../songs/examples/36-patch-lfo/invalid/keyboard-velocity.toml"
    ))
    .unwrap();
    let fixture = include_str!("../../../songs/examples/36-patch-lfo/velocity.project.lua");
    let wired = fixture.replace("{ id = \"shape\", kind = \"control_multiply\" },", "{ id = \"shape\", kind = \"control_multiply\" }, { id = \"touch\", kind = \"control_multiply\" },")
        .replace("{ from = \"shape.control\", to = \"amp.level\", signal = \"control\" },", "{ from = \"shape.control\", to = \"touch.left\", signal = \"control\" }, { from = \"input.velocity\", to = \"touch.right\", signal = \"control\" }, { from = \"touch.control\", to = \"amp.level\", signal = \"control\" },");
    let valid = load_native_project(&directory.0, &wired).unwrap();
    let patch = compile_pattern_voice(&valid, &valid.pattern_voices()["bass-instance"]).unwrap();
    for case in cases["cases"].as_array().unwrap() {
        let mut app = directory.app();
        bind(&mut app, 100);
        app.dispatch(key(&app, 100, 0, 48, true)).unwrap();
        let view = app.keyboard().clone();
        let source = app.authored_source().unwrap();
        let history = app.history().len();
        match case.as_str().unwrap() {
            name @ ("velocity-128" | "velocity-255") => {
                let velocity = if name == "velocity-128" { 128 } else { 255 };
                assert!(
                    app.dispatch(AppCommand::KeyboardKey {
                        epoch: view.epoch,
                        frame: 101,
                        channel: 0,
                        note: 48,
                        velocity,
                        pressed: true
                    })
                    .is_err()
                );
                let mut renderer = patch.live_renderer(48, 100).unwrap();
                let range = FrameRange::new(100, 356).unwrap();
                let good = LiveVoiceEvent {
                    frame: 100,
                    input: LiveVoiceInput::Gate(true),
                };
                assert!(
                    renderer
                        .render(
                            range,
                            &[
                                good,
                                LiveVoiceEvent {
                                    frame: 101,
                                    input: LiveVoiceInput::Velocity(velocity)
                                }
                            ]
                        )
                        .is_err()
                );
                assert_eq!(renderer.cursor(), 100);
                assert_eq!(
                    renderer.render(range, &[good]).unwrap().samples(),
                    patch
                        .live_renderer(48, 100)
                        .unwrap()
                        .render(range, &[good])
                        .unwrap()
                        .samples()
                );
            }
            name => {
                let invalid = match name {
                    "undeclared-input" => wired.replace("{ id = \"velocity\", signal = \"control\" },", ""),
                    "wrong-signal" => wired.replace("id = \"velocity\", signal = \"control\"", "id = \"velocity\", signal = \"gate\""),
                    "bipolar-destination" => wired.replace("from = \"slow.control\", to = \"unipolar.control\"", "from = \"input.velocity\", to = \"unipolar.control\""),
                    "duplicate-cable" => wired.replace("cables = {", "cables = { { from = \"input.velocity\", to = \"touch.right\", signal = \"control\" },"),
                    _=>unreachable!(),
                };
                assert!(
                    load_native_project(&directory.0, &invalid).is_err(),
                    "{name}"
                );
            }
        }
        assert_eq!(app.keyboard(), &view);
        assert_eq!(app.history().len(), history);
        assert!(std::sync::Arc::ptr_eq(
            &source,
            &app.authored_source().unwrap()
        ));
    }
    // The same mismatch through a typed atomic edit cannot replace accepted source.
    use meldritch_dsl::{PatchEdit, PatchOperation};
    let mut app = directory.app();
    app.enable_native_project_source(valid).unwrap();
    let before = app.authored_source().unwrap();
    let history = app.history().len();
    assert!(
        app.dispatch(AppCommand::EditPatch(PatchEdit {
            synth: "bass".into(),
            operations: vec![
                PatchOperation::Disconnect {
                    from: "slow.control".into(),
                    to: "unipolar.control".into()
                },
                PatchOperation::Connect {
                    from: "input.velocity".into(),
                    to: "unipolar.control".into(),
                },
            ]
        }))
        .is_err()
    );
    assert!(std::sync::Arc::ptr_eq(
        &before,
        &app.authored_source().unwrap()
    ));
    assert_eq!(app.history().len(), history);
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 0);
}
