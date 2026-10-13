use meldritch_app::{
    AppCommand, AppCommandResult, AppController, CockpitMode, KeyboardResetReason,
};
use meldritch_control::{
    devices::{keyboard_input_command, keyboard_step_command},
    native_capture::{NativeCommandEnvelope, NativeProjectCapture, load_native_capture_baseline},
    native_project::{NativeProjectStore, encode_native_project, load_native_project},
};
use meldritch_core::FrameRange;
use meldritch_dsl::{ControlUnit, PatchEdit, PatchOperation, ValidatedSong};
use meldritch_render::{
    dsp::{Oscillator, Waveform},
    song_render::{LiveVoiceEvent, LiveVoiceInput, compile_pattern_voice},
};
use std::{fs, path::PathBuf, sync::Arc};

const FIXTURE: &str = include_str!("../../../songs/examples/36-patch-lfo/bend.project.lua");
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "meldritch-bend-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
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
    fn constructed(&self) -> ValidatedSong {
        let mut app = self.app(load_native_project(&self.0, FIXTURE).unwrap());
        app.dispatch(AppCommand::SelectInstrument("bass".into()))
            .unwrap();
        app.dispatch(AppCommand::ToggleInstrumentEditor).unwrap();
        app.dispatch(AppCommand::TogglePatchEditor).unwrap();
        for draft in [
            include_str!("../../../songs/examples/36-patch-lfo/velocity-patch.txt"),
            include_str!("../../../songs/examples/36-patch-lfo/bend-patch.txt"),
        ] {
            for line in draft.lines() {
                for character in line.chars() {
                    app.dispatch(AppCommand::PatchCharacter(character)).unwrap();
                }
                app.dispatch(AppCommand::PatchSubmit).unwrap();
            }
        }
        let source = app.authored_source().unwrap();
        assert!(
            source.synths()["bass"]
                .modules()
                .iter()
                .any(|m| m.id() == "wheel")
        );
        (*source).clone()
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn outputs(result: AppCommandResult) -> Vec<LiveVoiceInput> {
    match result {
        AppCommandResult::KeyboardUpdated { outputs, .. } => outputs,
        other => panic!("{other:?}"),
    }
}
fn bind(app: &mut AppController, frame: u64) {
    app.dispatch(AppCommand::BindKeyboard {
        voice: Some("bass-instance".into()),
        frame,
    })
    .unwrap();
}
fn bend(app: &AppController, frame: u64, value: u16) -> AppCommand {
    AppCommand::KeyboardBend {
        epoch: app.keyboard().epoch,
        frame,
        channel: 15,
        value,
    }
}

#[test]
fn explicit_bend_composes_with_velocity_and_matches_independent_phase_reference() {
    let directory = Directory::new();
    let source = directory.constructed();
    assert_eq!(
        source.synths()["bass"].inputs()[0].control_unit(),
        Some(ControlUnit::Bipolar)
    );
    let patch = compile_pattern_voice(&source, &source.pattern_voices()["bass-instance"]).unwrap();
    let mut app = directory.app(source);
    let source = app.authored_source().unwrap();
    let encoded = encode_native_project(&source).unwrap();
    let mut capture = NativeProjectCapture::new(source.clone(), 96000);
    let trace: toml::Value = toml::from_str(include_str!(
        "../../../songs/examples/36-patch-lfo/keyboard-bend.toml"
    ))
    .unwrap();
    let origin = trace["origin_frame"].as_integer().unwrap() as u64;
    let length = trace["length_frames"].as_integer().unwrap() as u64;
    let depth = trace["depth_semitones"].as_float().unwrap();
    let note = trace["note"].as_integer().unwrap() as u8;
    let velocity = trace["velocity"].as_integer().unwrap() as u8;
    bind(&mut app, origin);
    let mut events: Vec<_> = outputs(
        app.dispatch(
            keyboard_input_command(&[0x90, note, velocity], app.keyboard().epoch, origin).unwrap(),
        )
        .unwrap(),
    )
    .into_iter()
    .map(|input| LiveVoiceEvent {
        frame: origin,
        input,
    })
    .collect();
    for entry in trace["bends"].as_array().unwrap() {
        let value = entry["value"].as_integer().unwrap() as u16;
        let frame = origin + entry["offset"].as_integer().unwrap() as u64;
        let packet = [0xef, (value & 127) as u8, (value >> 7) as u8];
        let command = keyboard_input_command(&packet, app.keyboard().epoch, frame).unwrap();
        assert_eq!(command, bend(&app, frame, value));
        let before = app.keyboard().clone();
        assert_eq!(
            outputs(app.dispatch(command).unwrap()),
            vec![LiveVoiceInput::Bend(value)]
        );
        assert_eq!(app.keyboard().pitch, before.pitch);
        assert_eq!(app.keyboard().velocity, before.velocity);
        assert_eq!(app.keyboard().gate, before.gate);
        assert_eq!(app.keyboard().held_keys, before.held_keys);
        events.push(LiveVoiceEvent {
            frame,
            input: LiveVoiceInput::Bend(value),
        });
        assert!(Arc::ptr_eq(&source, &app.authored_source().unwrap()));
    }
    app.dispatch(AppCommand::SetCockpitMode(CockpitMode::Performance))
        .unwrap();
    app.dispatch(AppCommand::SelectPatternVoice("bass-instance".into()))
        .unwrap();
    assert_eq!(app.keyboard().bend, Some(16383));
    let reset_frame = origin + 1792;
    let reset = outputs(
        app.dispatch(AppCommand::ResetKeyboard {
            epoch: app.keyboard().epoch,
            frame: reset_frame,
            reason: KeyboardResetReason::Panic,
        })
        .unwrap(),
    );
    assert_eq!(
        reset,
        vec![LiveVoiceInput::Gate(false), LiveVoiceInput::Bend(8192)]
    );
    events.extend(reset.into_iter().map(|input| LiveVoiceEvent {
        frame: reset_frame,
        input,
    }));
    let range = FrameRange::new(origin, origin + length).unwrap();
    let expected = patch
        .live_renderer(note, origin)
        .unwrap()
        .render(range, &events)
        .unwrap();
    let values = [8192, 0, 16383, 8191, 12288, 8192, 16383, 8192];
    let mut oscillator = Oscillator::new(Waveform::Sine);
    for (frame, actual) in expected.samples().iter().enumerate() {
        let value = values[frame / 256];
        // Independent endpoint/midpoint reference and continuous oscillator phase.
        let normalized = match value {
            0 => -1.0,
            8191 => -1.0 / 8192.0,
            8192 => 0.0,
            12288 => 4096.0 / 8191.0,
            16383 => 1.0,
            _ => unreachable!(),
        };
        let frequency = 440.0 * 2_f64.powf((f64::from(note) - 69.0 + depth * normalized) / 12.0);
        let wave = oscillator.next(frequency, 48000);
        let shape = (std::f64::consts::TAU * (frame as f64 / 24000.0))
            .sin()
            .mul_add(0.5, 0.5);
        let reference = if frame < 1792 {
            wave * shape * f64::from(velocity) / 127.0
        } else {
            0.0
        };
        assert!((actual - reference).abs() < 1e-12, "frame {frame}");
    }
    // Partitioning and every same-frame event order must preserve exact PCM.
    let mut split = patch.live_renderer(note, origin).unwrap();
    let mut start = origin;
    let mut pcm = Vec::new();
    for size in [1, 17, 238, 511, 257, 768, 256] {
        let end = start + size;
        let block: Vec<_> = events
            .iter()
            .filter(|e| e.frame >= start && e.frame < end)
            .copied()
            .collect();
        pcm.extend_from_slice(
            split
                .render(FrameRange::new(start, end).unwrap(), &block)
                .unwrap()
                .samples(),
        );
        start = end;
    }
    assert_eq!(start, origin + length);
    assert_eq!(pcm, expected.samples());
    capture.finish(&app).unwrap();
    let text = fs::read_to_string(capture.session.as_ref().unwrap().path()).unwrap();
    let mut replay = directory.app(load_native_capture_baseline(&directory.0, &text).unwrap());
    let document: toml::Value = toml::from_str(&text).unwrap();
    let mut replayed = Vec::new();
    for event in document["events"].as_array().unwrap() {
        let envelope =
            NativeCommandEnvelope::decode(&toml::to_string(&event["native"]).unwrap()).unwrap();
        assert_eq!(envelope.source_before, envelope.source_after);
        let command: AppCommand = envelope.command.into();
        let frame = match &command {
            AppCommand::BindKeyboard { frame, .. }
            | AppCommand::KeyboardKey { frame, .. }
            | AppCommand::KeyboardBend { frame, .. }
            | AppCommand::ResetKeyboard { frame, .. } => Some(*frame),
            _ => None,
        };
        let result = replay.dispatch(command).unwrap();
        if let Some(frame) = frame {
            replayed.extend(
                outputs(result)
                    .into_iter()
                    .map(|input| LiveVoiceEvent { frame, input }),
            );
        }
    }
    assert_eq!(replayed, events);
    assert_eq!(replay.keyboard(), app.keyboard());
    assert_eq!(
        expected.samples(),
        patch
            .live_renderer(note, origin)
            .unwrap()
            .render(range, &replayed)
            .unwrap()
            .samples()
    );
    // The real atomic authored store never includes wheel values or held keys.
    let mut store =
        NativeProjectStore::open(&meldritch_dsl::load_project_directory(&directory.0).unwrap())
            .unwrap();
    store.save(&source).unwrap();
    assert_eq!(
        fs::read_to_string(directory.0.join("main.project.lua")).unwrap(),
        encoded
    );
    let reopened =
        NativeProjectStore::open(&meldritch_dsl::load_project_directory(&directory.0).unwrap())
            .unwrap();
    assert_eq!(
        reopened.source().unwrap().fingerprint(),
        source.fingerprint()
    );
    let fresh = directory.app(reopened.source().unwrap().clone());
    assert_eq!(fresh.keyboard().voice, None);
    assert_eq!(fresh.keyboard().bend, None);
    // Merely declaring the port or receiving wheel values cannot modulate pitch.
    let unwired = load_native_project(&directory.0, FIXTURE).unwrap();
    let raw = compile_pattern_voice(&unwired, &unwired.pattern_voices()["bass-instance"]).unwrap();
    let centered: Vec<_> = events
        .iter()
        .map(|event| LiveVoiceEvent {
            frame: event.frame,
            input: match event.input {
                LiveVoiceInput::Bend(_) => LiveVoiceInput::Bend(8192),
                input => input,
            },
        })
        .collect();
    assert_eq!(
        raw.live_renderer(note, origin)
            .unwrap()
            .render(range, &events)
            .unwrap()
            .samples(),
        raw.live_renderer(note, origin)
            .unwrap()
            .render(range, &centered)
            .unwrap()
            .samples()
    );
}

#[test]
fn invalid_bend_commands_blocks_and_cables_preserve_accepted_state() {
    let directory = Directory::new();
    let source = directory.constructed();
    let encoded = encode_native_project(&source).unwrap();
    let patch = compile_pattern_voice(&source, &source.pattern_voices()["bass-instance"]).unwrap();
    let cases: toml::Value = toml::from_str(include_str!(
        "../../../songs/examples/36-patch-lfo/invalid/keyboard-bend.toml"
    ))
    .unwrap();
    for case in cases["cases"].as_array().unwrap() {
        let mut app = directory.app(source.clone());
        bind(&mut app, 100);
        let case = case.as_str().unwrap();
        if case == "unbound" {
            app.dispatch(AppCommand::BindKeyboard {
                voice: None,
                frame: 100,
            })
            .unwrap();
        }
        let before = app.keyboard().clone();
        let old_source = app.authored_source().unwrap();
        let history = app.history().len();
        match case {
            "undeclared-input" | "wrong-signal" => {
                // The readable fixture declaration is still validated before wiring.
                let invalid = if case == "undeclared-input" {
                    FIXTURE.replace("{ id = \"bend\", signal = \"control\" },", "")
                } else {
                    FIXTURE.replace(
                        "id = \"bend\", signal = \"control\"",
                        "id = \"bend\", signal = \"pitch\"",
                    )
                };
                let candidate = load_native_project(&directory.0, &invalid).unwrap();
                let operations = vec![
                    PatchOperation::Instantiate {
                        id: "wheel".into(),
                        kind: meldritch_dsl::ModuleKind::PitchModulate,
                    },
                    PatchOperation::Disconnect {
                        from: "input.pitch".into(),
                        to: "osc.pitch".into(),
                    },
                    PatchOperation::Connect {
                        from: "input.pitch".into(),
                        to: "wheel.pitch".into(),
                    },
                    PatchOperation::Connect {
                        from: "input.bend".into(),
                        to: "wheel.control".into(),
                    },
                    PatchOperation::Connect {
                        from: "wheel.pitch".into(),
                        to: "osc.pitch".into(),
                    },
                ];
                assert!(
                    candidate
                        .edit_patch(&PatchEdit {
                            synth: "bass".into(),
                            operations
                        })
                        .is_err()
                );
            }
            "unit-mismatch" | "duplicate-cable" => {
                let mut operations = vec![];
                if case == "unit-mismatch" {
                    operations.push(PatchOperation::Disconnect {
                        from: "input.velocity".into(),
                        to: "touch.right".into(),
                    });
                }
                operations.push(PatchOperation::Connect {
                    from: "input.bend".into(),
                    to: "touch.right".into(),
                });
                assert!(
                    app.dispatch(AppCommand::EditPatch(PatchEdit {
                        synth: "bass".into(),
                        operations
                    }))
                    .is_err()
                );
            }
            _ => {
                let value = match case {
                    "value-16384" => 16384,
                    "value-65535" => 65535,
                    _ => 8192,
                };
                let command = AppCommand::KeyboardBend {
                    epoch: if case == "stale-epoch" {
                        before.epoch - 1
                    } else {
                        before.epoch
                    },
                    frame: if case == "backwards-frame" { 99 } else { 100 },
                    channel: if case == "channel-16" { 16 } else { 0 },
                    value,
                };
                assert!(app.dispatch(command).is_err(), "{case}");
                if value > 16383 {
                    let mut renderer = patch.live_renderer(48, 100).unwrap();
                    let range = FrameRange::new(100, 356).unwrap();
                    let gate = LiveVoiceEvent {
                        frame: 100,
                        input: LiveVoiceInput::Gate(true),
                    };
                    assert!(
                        renderer
                            .render(
                                range,
                                &[
                                    gate,
                                    LiveVoiceEvent {
                                        frame: 101,
                                        input: LiveVoiceInput::Bend(value)
                                    }
                                ]
                            )
                            .is_err()
                    );
                    assert_eq!(renderer.cursor(), 100);
                    assert_eq!(
                        renderer.render(range, &[gate]).unwrap().samples(),
                        patch
                            .live_renderer(48, 100)
                            .unwrap()
                            .render(range, &[gate])
                            .unwrap()
                            .samples()
                    );
                }
            }
        }
        assert_eq!(app.keyboard(), &before);
        assert_eq!(app.history().len(), history);
        assert!(Arc::ptr_eq(&old_source, &app.authored_source().unwrap()));
        assert_eq!(encode_native_project(&old_source).unwrap(), encoded);
    }
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 0);
    let app = directory.app(source);
    for packet in [
        &[0xe0, 128, 0][..],
        &[0xe0, 0, 128][..],
        &[0xe0, 0][..],
        &[0xb0, 64, 127][..],
        &[0xb0, 1, 127][..],
    ] {
        assert!(keyboard_input_command(packet, 0, 100).is_none());
        assert!(keyboard_step_command(packet, &app.view_model()).is_none());
    }
}

#[test]
fn wheel_ownership_survives_keys_and_resets_center_without_implicit_articulation() {
    let directory = Directory::new();
    for reason in [
        KeyboardResetReason::Disconnect,
        KeyboardResetReason::Overflow,
        KeyboardResetReason::Panic,
    ] {
        let mut app = directory.app(directory.constructed());
        bind(&mut app, 0);
        let epoch = app.keyboard().epoch;
        // Wheel before any key must never articulate the voice.
        let command = bend(&app, 0, 0);
        assert_eq!(
            outputs(app.dispatch(command).unwrap()),
            vec![LiveVoiceInput::Bend(0)]
        );
        assert!(!app.keyboard().gate);
        assert_eq!(app.keyboard().held_keys, 0);
        for packet in [[0x90, 48, 32], [0x91, 55, 96], [0x81, 55, 0], [0x80, 48, 0]] {
            let command = keyboard_input_command(&packet, epoch, 1).unwrap();
            app.dispatch(command).unwrap();
            assert_eq!(app.keyboard().bend, Some(0));
        }
        assert!(!app.keyboard().gate);
        let command = AppCommand::ResetKeyboard {
            epoch,
            frame: 2,
            reason,
        };
        assert_eq!(
            outputs(app.dispatch(command).unwrap()),
            vec![LiveVoiceInput::Bend(8192)]
        );
        assert_eq!(app.keyboard().bend, Some(8192));
        let stale = AppCommand::KeyboardBend {
            epoch,
            frame: 3,
            channel: 0,
            value: 16383,
        };
        assert!(app.dispatch(stale).is_err());
        let command = bend(&app, 3, 16383);
        app.dispatch(command).unwrap();
        let result = app
            .dispatch(AppCommand::BindKeyboard {
                voice: None,
                frame: 4,
            })
            .unwrap();
        match result {
            AppCommandResult::KeyboardUpdated {
                output_voice,
                outputs,
                ..
            } => {
                assert_eq!(output_voice.as_deref(), Some("bass-instance"));
                assert_eq!(outputs, vec![LiveVoiceInput::Bend(8192)]);
            }
            _ => unreachable!(),
        }
        bind(&mut app, 5);
        assert_eq!(app.keyboard().bend, None); // Fresh DSP starts centered.
        let command = bend(&app, 5, 16383);
        app.dispatch(command).unwrap();
        app.dispatch(AppCommand::CreatePalettePattern {
            id: "next".into(),
            length_ticks: 3840,
            looped: true,
        })
        .unwrap();
        assert_eq!(app.keyboard().voice, None);
        assert_eq!(app.keyboard().bend, None);
    }
}
