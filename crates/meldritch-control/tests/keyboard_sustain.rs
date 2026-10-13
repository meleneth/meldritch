use meldritch_app::{
    AppCommand, AppCommandResult, AppController, CockpitMode, KeyboardResetReason,
};
use meldritch_control::{
    devices::{keyboard_performance_command, keyboard_step_command},
    native_capture::{NativeCommandEnvelope, NativeProjectCapture, load_native_capture_baseline},
    native_project::{NativeProjectStore, encode_native_project, load_native_project},
};
use meldritch_core::FrameRange;
use meldritch_dsl::{DependentCables, ModuleKind, PatchEdit, PatchOperation, ValidatedSong};
use meldritch_render::song_render::{LiveVoiceEvent, LiveVoiceInput, compile_pattern_voice};
use std::{fs, path::PathBuf, sync::Arc};

const FIXTURE: &str = include_str!("../../../songs/examples/36-patch-lfo/sustain.project.lua");
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "meldritch-hold-{}-{}",
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
            include_str!("../../../songs/examples/36-patch-lfo/sustain-patch.txt"),
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
                .any(|m| m.kind() == ModuleKind::GateHold)
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
fn unheld(source: &ValidatedSong) -> ValidatedSong {
    source
        .edit_patch(&PatchEdit {
            synth: "bass".into(),
            operations: vec![
                PatchOperation::Remove {
                    id: "hold".into(),
                    cables: DependentCables::Disconnect,
                },
                PatchOperation::Connect {
                    from: "input.gate".into(),
                    to: "env.gate".into(),
                },
            ],
        })
        .unwrap()
}

#[test]
fn authored_gate_hold_composes_pedal_keys_velocity_bend_and_release_tail() {
    let directory = Directory::new();
    let source = directory.constructed();
    let bare = unheld(&source);
    let patch = compile_pattern_voice(&source, &source.pattern_voices()["bass-instance"]).unwrap();
    let raw = compile_pattern_voice(&bare, &bare.pattern_voices()["bass-instance"]).unwrap();
    let trace: toml::Value = toml::from_str(include_str!(
        "../../../songs/examples/36-patch-lfo/keyboard-sustain.toml"
    ))
    .unwrap();
    let origin = trace["origin_frame"].as_integer().unwrap() as u64;
    let length = trace["length_frames"].as_integer().unwrap() as u64;
    let cc = trace["sustain_cc"].as_integer().unwrap() as u8;
    let mut app = directory.app(source);
    let source = app.authored_source().unwrap();
    let mut capture = NativeProjectCapture::new(source.clone(), 96000);
    bind(&mut app, origin);
    let mut emitted = Vec::new();
    for entry in trace["events"].as_array().unwrap() {
        let frame = origin + entry["offset"].as_integer().unwrap() as u64;
        let bytes: Vec<_> = entry["bytes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_integer().unwrap() as u8)
            .collect();
        let command =
            keyboard_performance_command(&bytes, app.keyboard().epoch, frame, Some(cc)).unwrap();
        let prior = app.keyboard().clone();
        let actual = outputs(app.dispatch(command.clone()).unwrap());
        match command {
            AppCommand::KeyboardSustain { high, .. } => {
                assert_eq!(actual, vec![LiveVoiceInput::Sustain(high)]);
                assert_eq!(app.keyboard().gate, prior.gate);
                assert_eq!(app.keyboard().pitch, prior.pitch);
                assert_eq!(app.keyboard().velocity, prior.velocity);
                assert_eq!(app.keyboard().held_keys, prior.held_keys);
            }
            AppCommand::KeyboardBend { .. } => assert_eq!(app.keyboard().gate, prior.gate),
            _ => {}
        }
        emitted.extend(
            actual
                .into_iter()
                .map(|input| LiveVoiceEvent { frame, input }),
        );
        assert!(Arc::ptr_eq(&source, &app.authored_source().unwrap()));
    }
    assert_eq!(app.keyboard().held_keys, 0);
    assert!(!app.keyboard().gate);
    assert!(app.keyboard().sustain);
    assert_eq!(app.keyboard().bend, Some(16383));
    app.dispatch(AppCommand::SetCockpitMode(CockpitMode::Performance))
        .unwrap();
    assert!(app.keyboard().sustain);
    // Physical Gate is already low: reset must still clear the resolved hold.
    let reset_frame = origin + 3968;
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
        vec![LiveVoiceInput::Sustain(false), LiveVoiceInput::Bend(8192)]
    );
    emitted.extend(reset.into_iter().map(|input| LiveVoiceEvent {
        frame: reset_frame,
        input,
    }));
    assert!(!app.keyboard().sustain);
    assert_eq!(app.keyboard().bend, Some(8192));
    let range = FrameRange::new(origin, origin + length).unwrap();
    let held = patch
        .live_renderer(48, origin)
        .unwrap()
        .render(range, &emitted)
        .unwrap();
    // Independent resolved-Gate reference. Only these raw low events defer;
    // pedal up at 1536 and panic at 3968 release the already-low physical Gate.
    let mut resolved = Vec::new();
    for event in &emitted {
        let offset = event.frame - origin;
        match event.input {
            LiveVoiceInput::Gate(false) if matches!(offset, 512 | 1280 | 3840) => {}
            LiveVoiceInput::Sustain(false) if matches!(offset, 1536 | 3968) => {
                resolved.push(LiveVoiceEvent {
                    frame: event.frame,
                    input: LiveVoiceInput::Gate(false),
                })
            }
            LiveVoiceInput::Sustain(_) => {}
            _ => resolved.push(*event),
        }
    }
    assert_eq!(
        held.samples(),
        raw.live_renderer(48, origin)
            .unwrap()
            .render(range, &resolved)
            .unwrap()
            .samples()
    );
    assert!(held.samples()[512..1024].iter().any(|v| v.abs() > 0.01));
    assert!(held.samples()[1280..1536].iter().any(|v| v.abs() > 0.01));
    assert!(held.samples()[2048..2304].iter().all(|v| v.abs() < 1e-12)); // Pedal alone cannot revive.
    assert!(held.samples()[2560..2816].iter().any(|v| v.abs() > 0.01)); // Held physical key survives pedal up.
    assert!(held.samples()[3968..].iter().any(|v| v.abs() > 0.001)); // Release tail, not cut PCM.
    let mut live = patch.live_renderer(48, origin).unwrap();
    let mut pcm = Vec::new();
    let mut start = origin;
    for size in [1, 17, 238, 256, 512, 768, 1024, 1280] {
        let end = start + size;
        let block: Vec<_> = emitted
            .iter()
            .filter(|e| e.frame >= start && e.frame < end)
            .copied()
            .collect();
        pcm.extend_from_slice(
            live.render(FrameRange::new(start, end).unwrap(), &block)
                .unwrap()
                .samples(),
        );
        start = end;
    }
    assert_eq!(start, origin + length);
    assert_eq!(pcm, held.samples());
    // An unwired pedal input cannot alter the original note-off timing.
    let bare_events: Vec<_> = emitted
        .iter()
        .map(|e| LiveVoiceEvent {
            frame: e.frame,
            input: match e.input {
                LiveVoiceInput::Sustain(_) => LiveVoiceInput::Sustain(false),
                input => input,
            },
        })
        .collect();
    assert_eq!(
        raw.live_renderer(48, origin)
            .unwrap()
            .render(range, &emitted)
            .unwrap()
            .samples(),
        raw.live_renderer(48, origin)
            .unwrap()
            .render(range, &bare_events)
            .unwrap()
            .samples()
    );
    capture.finish(&app).unwrap();
    let text = fs::read_to_string(capture.session.as_ref().unwrap().path()).unwrap();
    let mut replay = directory.app(load_native_capture_baseline(&directory.0, &text).unwrap());
    let document: toml::Value = toml::from_str(&text).unwrap();
    let mut reconstructed = Vec::new();
    for event in document["events"].as_array().unwrap() {
        let envelope =
            NativeCommandEnvelope::decode(&toml::to_string(&event["native"]).unwrap()).unwrap();
        assert_eq!(envelope.source_before, envelope.source_after);
        let command: AppCommand = envelope.command.into();
        let frame = match &command {
            AppCommand::BindKeyboard { frame, .. }
            | AppCommand::KeyboardKey { frame, .. }
            | AppCommand::KeyboardBend { frame, .. }
            | AppCommand::KeyboardSustain { frame, .. }
            | AppCommand::ResetKeyboard { frame, .. } => Some(*frame),
            _ => None,
        };
        let result = replay.dispatch(command).unwrap();
        if let Some(frame) = frame {
            reconstructed.extend(
                outputs(result)
                    .into_iter()
                    .map(|input| LiveVoiceEvent { frame, input }),
            );
        }
    }
    assert_eq!(replay.keyboard(), app.keyboard());
    assert_eq!(reconstructed, emitted);
    assert_eq!(
        patch
            .live_renderer(48, origin)
            .unwrap()
            .render(range, &reconstructed)
            .unwrap()
            .samples(),
        held.samples()
    );
    let encoded = encode_native_project(&source).unwrap();
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
    assert!(!fresh.keyboard().sustain);
}

#[test]
fn invalid_sustain_inputs_and_graphs_preserve_state_and_source() {
    let directory = Directory::new();
    let source = directory.constructed();
    let cases: toml::Value = toml::from_str(include_str!(
        "../../../songs/examples/36-patch-lfo/invalid/keyboard-sustain.toml"
    ))
    .unwrap();
    for case in cases["cases"].as_array().unwrap() {
        let case = case.as_str().unwrap();
        let mut app = directory.app(source.clone());
        bind(&mut app, 100);
        if case == "unbound" {
            app.dispatch(AppCommand::BindKeyboard {
                voice: None,
                frame: 100,
            })
            .unwrap();
        }
        let view = app.keyboard().clone();
        let before = app.authored_source().unwrap();
        let history = app.history().len();
        match case {
            "undeclared-input" | "wrong-signal" | "plaits-processor" => {
                let candidate=match case {
                    "undeclared-input"=>FIXTURE.replace("{ id = \"sustain\", signal = \"gate\" },",""),
                    "wrong-signal"=>FIXTURE.replace("id = \"sustain\", signal = \"gate\"","id = \"sustain\", signal = \"control\""),
                    _=>FIXTURE.replace("kind = \"oscillator\", waveform = \"sine\"","kind = \"plaits\", plaits = { model = \"fm\", harmonics = 0.5, timbre = 0.5, morph = 0.5 }"),
                };
                let source = load_native_project(&directory.0, &candidate).unwrap();
                let edit = PatchEdit {
                    synth: "bass".into(),
                    operations: vec![
                        PatchOperation::Instantiate {
                            id: "hold".into(),
                            kind: ModuleKind::GateHold,
                        },
                        PatchOperation::Disconnect {
                            from: "input.gate".into(),
                            to: "env.gate".into(),
                        },
                        PatchOperation::Connect {
                            from: "input.gate".into(),
                            to: "hold.gate".into(),
                        },
                        PatchOperation::Connect {
                            from: "input.sustain".into(),
                            to: "hold.hold".into(),
                        },
                        PatchOperation::Connect {
                            from: "hold.gate".into(),
                            to: "env.gate".into(),
                        },
                    ],
                };
                let mut candidate_app = directory.app(source);
                assert!(
                    candidate_app.dispatch(AppCommand::EditPatch(edit)).is_err(),
                    "{case}"
                );
            }
            "missing-hold" | "duplicate-input" | "multiple-processors" | "cycle" => {
                let operations = match case {
                    "missing-hold" => vec![PatchOperation::Disconnect {
                        from: "input.sustain".into(),
                        to: "hold.hold".into(),
                    }],
                    "duplicate-input" => vec![PatchOperation::Connect {
                        from: "input.gate".into(),
                        to: "hold.hold".into(),
                    }],
                    "multiple-processors" => vec![PatchOperation::Instantiate {
                        id: "another".into(),
                        kind: ModuleKind::GateHold,
                    }],
                    "cycle" => vec![
                        PatchOperation::Disconnect {
                            from: "input.gate".into(),
                            to: "hold.gate".into(),
                        },
                        PatchOperation::Connect {
                            from: "hold.gate".into(),
                            to: "hold.gate".into(),
                        },
                    ],
                    _ => unreachable!(),
                };
                assert!(
                    app.dispatch(AppCommand::EditPatch(PatchEdit {
                        synth: "bass".into(),
                        operations
                    }))
                    .is_err(),
                    "{case}"
                );
            }
            _ => {
                let command = AppCommand::KeyboardSustain {
                    epoch: if case == "stale-epoch" {
                        view.epoch - 1
                    } else {
                        view.epoch
                    },
                    frame: if case == "backwards-frame" { 99 } else { 100 },
                    channel: if case == "channel-16" { 16 } else { 0 },
                    high: true,
                };
                assert!(app.dispatch(command).is_err(), "{case}");
            }
        }
        assert_eq!(app.keyboard(), &view);
        assert_eq!(app.history().len(), history);
        assert!(Arc::ptr_eq(&before, &app.authored_source().unwrap()));
    }
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 0);
    let app = directory.app(source);
    for packet in [
        &[0xb0, 64, 127][..],
        &[0xb0, 1, 127][..],
        &[0xb0, 64][..],
        &[0xb0, 64, 128][..],
    ] {
        assert!(keyboard_performance_command(packet, 1, 100, None).is_none());
        assert!(keyboard_step_command(packet, &app.view_model()).is_none());
    }
    assert!(matches!(
        keyboard_performance_command(&[0xbf, 64, 63], 1, 100, Some(64)),
        Some(AppCommand::KeyboardSustain {
            channel: 15,
            high: false,
            ..
        })
    ));
    assert!(matches!(
        keyboard_performance_command(&[0xbf, 64, 64], 1, 100, Some(64)),
        Some(AppCommand::KeyboardSustain {
            channel: 15,
            high: true,
            ..
        })
    ));
    for packet in [&[0xb0, 1, 127][..], &[0xb0, 64][..], &[0xb0, 64, 128][..]] {
        assert!(keyboard_performance_command(packet, 1, 100, Some(64)).is_none());
    }
}

#[test]
fn typed_input_draft_builds_expression_from_plain_pitch_gate_patch_and_reopens() {
    let directory = Directory::new();
    let bare = load_native_project(
        &directory.0,
        include_str!("../../../songs/examples/36-patch-lfo/ports.project.lua"),
    )
    .unwrap();
    let mut app = directory.app(bare.clone());
    app.dispatch(AppCommand::SelectInstrument("bass".into()))
        .unwrap();
    app.dispatch(AppCommand::ToggleInstrumentEditor).unwrap();
    app.dispatch(AppCommand::TogglePatchEditor).unwrap();
    let base = app.authored_source().unwrap();
    for line in include_str!("../../../songs/examples/36-patch-lfo/input-ports.txt")
        .lines()
        .filter(|line| !line.starts_with('#'))
    {
        for character in line.chars() {
            app.dispatch(AppCommand::PatchCharacter(character)).unwrap();
        }
        app.dispatch(AppCommand::PatchSubmit).unwrap();
        if line != "commit" {
            assert!(Arc::ptr_eq(&base, &app.authored_source().unwrap()));
        }
    }
    let source = app.authored_source().unwrap();
    let inputs: Vec<_> = source.synths()["bass"]
        .inputs()
        .iter()
        .map(|input| (input.id(), input.signal()))
        .collect();
    assert_eq!(
        inputs,
        vec![
            ("pitch", meldritch_dsl::SignalType::Pitch),
            ("gate", meldritch_dsl::SignalType::Gate),
            ("velocity", meldritch_dsl::SignalType::Control),
            ("bend", meldritch_dsl::SignalType::Control),
            ("sustain", meldritch_dsl::SignalType::Gate),
        ]
    );
    let patch = compile_pattern_voice(&source, &source.pattern_voices()["bass-instance"]).unwrap();
    let origin = 5_000_000_000;
    let events = [
        (0, LiveVoiceInput::Pitch(48)),
        (0, LiveVoiceInput::Velocity(64)),
        (0, LiveVoiceInput::Gate(true)),
        (256, LiveVoiceInput::Sustain(true)),
        (512, LiveVoiceInput::Gate(false)),
        (768, LiveVoiceInput::Bend(16383)),
        (1024, LiveVoiceInput::Sustain(false)),
    ]
    .map(|(offset, input)| LiveVoiceEvent {
        frame: origin + offset,
        input,
    });
    let range = FrameRange::new(origin, origin + 1152).unwrap();
    let pcm = patch
        .live_renderer(48, origin)
        .unwrap()
        .render(range, &events)
        .unwrap();
    assert!(
        pcm.samples()[512..1024]
            .iter()
            .any(|value| value.abs() > 0.01)
    );
    assert!(
        pcm.samples()[1024..]
            .iter()
            .any(|value| value.abs() > 0.001)
    );
    let neutral: Vec<_> = events
        .iter()
        .map(|event| LiveVoiceEvent {
            frame: event.frame,
            input: match event.input {
                LiveVoiceInput::Velocity(_) => LiveVoiceInput::Velocity(127),
                LiveVoiceInput::Bend(_) => LiveVoiceInput::Bend(8192),
                input => input,
            },
        })
        .collect();
    let neutral_pcm = patch
        .live_renderer(48, origin)
        .unwrap()
        .render(range, &neutral)
        .unwrap();
    assert_ne!(pcm.samples()[0..256], neutral_pcm.samples()[0..256]);
    assert_ne!(pcm.samples()[768..1024], neutral_pcm.samples()[768..1024]);
    let mut store =
        NativeProjectStore::open(&meldritch_dsl::load_project_directory(&directory.0).unwrap())
            .unwrap();
    store.save(&source).unwrap();
    let reopened =
        NativeProjectStore::open(&meldritch_dsl::load_project_directory(&directory.0).unwrap())
            .unwrap();
    let loaded = reopened.source().unwrap();
    assert_eq!(loaded.fingerprint(), source.fingerprint());
    let loaded_patch =
        compile_pattern_voice(loaded, &loaded.pattern_voices()["bass-instance"]).unwrap();
    assert_eq!(
        loaded_patch
            .live_renderer(48, origin)
            .unwrap()
            .render(range, &events)
            .unwrap()
            .samples(),
        pcm.samples()
    );
    let fresh = directory.app(loaded.clone());
    assert_eq!(fresh.keyboard().voice, None);
    assert!(!fresh.keyboard().sustain);
}

#[test]
fn invalid_input_drafts_and_dependent_removal_preserve_accepted_graph() {
    let directory = Directory::new();
    let bare = load_native_project(
        &directory.0,
        include_str!("../../../songs/examples/36-patch-lfo/ports.project.lua"),
    )
    .unwrap();
    let source = bare
        .edit_patch(&PatchEdit {
            synth: "bass".into(),
            operations: vec![PatchOperation::AddInput {
                id: "velocity".into(),
                signal: meldritch_dsl::SignalType::Control,
            }],
        })
        .unwrap();
    let fixture: toml::Value = toml::from_str(include_str!(
        "../../../songs/examples/36-patch-lfo/invalid/input-ports.toml"
    ))
    .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let case = case.as_str().unwrap();
        let candidate_source =
            if matches!(case, "connected-reject" | "connected-disconnect-incomplete") {
                source
                    .edit_patch(&PatchEdit {
                        synth: "bass".into(),
                        operations: vec![
                            PatchOperation::Disconnect {
                                from: "env.control".into(),
                                to: "amp.level".into(),
                            },
                            PatchOperation::Connect {
                                from: "input.velocity".into(),
                                to: "amp.level".into(),
                            },
                        ],
                    })
                    .unwrap()
            } else {
                source.clone()
            };
        let mut app = directory.app(candidate_source);
        let before = app.authored_source().unwrap();
        let history = app.history().len();
        let operations = match case {
            "duplicate" => vec![PatchOperation::AddInput {
                id: "velocity".into(),
                signal: meldritch_dsl::SignalType::Control,
            }],
            "unknown-name" => vec![PatchOperation::AddInput {
                id: "custom".into(),
                signal: meldritch_dsl::SignalType::Control,
            }],
            "wrong-signal" => vec![PatchOperation::AddInput {
                id: "sustain".into(),
                signal: meldritch_dsl::SignalType::Control,
            }],
            "reserved-base" => vec![PatchOperation::RemoveInput {
                id: "pitch".into(),
                cables: DependentCables::Disconnect,
            }],
            "connected-reject" => vec![PatchOperation::RemoveInput {
                id: "velocity".into(),
                cables: DependentCables::Reject,
            }],
            "connected-disconnect-incomplete" => vec![PatchOperation::RemoveInput {
                id: "velocity".into(),
                cables: DependentCables::Disconnect,
            }],
            "missing-cable" => vec![PatchOperation::Disconnect {
                from: "input.velocity".into(),
                to: "amp.level".into(),
            }],
            "unknown-removal" => vec![PatchOperation::RemoveInput {
                id: "sustain".into(),
                cables: DependentCables::Disconnect,
            }],
            "stale-draft" => {
                app.dispatch(AppCommand::SelectInstrument("bass".into()))
                    .unwrap();
                app.dispatch(AppCommand::ToggleInstrumentEditor).unwrap();
                app.dispatch(AppCommand::TogglePatchEditor).unwrap();
                for character in "input add sustain gate".chars() {
                    app.dispatch(AppCommand::PatchCharacter(character)).unwrap();
                }
                app.dispatch(AppCommand::PatchSubmit).unwrap();
                app.dispatch(AppCommand::EditPatch(PatchEdit {
                    synth: "bass".into(),
                    operations: vec![PatchOperation::AddInput {
                        id: "bend".into(),
                        signal: meldritch_dsl::SignalType::Control,
                    }],
                }))
                .unwrap();
                for character in "commit".chars() {
                    app.dispatch(AppCommand::PatchCharacter(character)).unwrap();
                }
                let accepted = app.authored_source().unwrap();
                assert!(app.dispatch(AppCommand::PatchSubmit).is_err());
                assert!(Arc::ptr_eq(&accepted, &app.authored_source().unwrap()));
                continue;
            }
            _ => panic!("unknown case {case}"),
        };
        assert!(
            app.dispatch(AppCommand::EditPatch(PatchEdit {
                synth: "bass".into(),
                operations,
            }))
            .is_err(),
            "{case}"
        );
        assert!(Arc::ptr_eq(&before, &app.authored_source().unwrap()));
        assert_eq!(history, app.history().len());
    }
    let mut app = directory.app(source);
    app.dispatch(AppCommand::EditPatch(PatchEdit {
        synth: "bass".into(),
        operations: vec![PatchOperation::RemoveInput {
            id: "velocity".into(),
            cables: DependentCables::Reject,
        }],
    }))
    .unwrap();
    assert_eq!(
        app.authored_source().unwrap().synths()["bass"]
            .inputs()
            .len(),
        2
    );

    // A dependent source can be removed in the same batch that repairs its consumer.
    let connected = bare
        .edit_patch(&PatchEdit {
            synth: "bass".into(),
            operations: vec![
                PatchOperation::AddInput {
                    id: "velocity".into(),
                    signal: meldritch_dsl::SignalType::Control,
                },
                PatchOperation::Disconnect {
                    from: "env.control".into(),
                    to: "amp.level".into(),
                },
                PatchOperation::Connect {
                    from: "input.velocity".into(),
                    to: "amp.level".into(),
                },
            ],
        })
        .unwrap();
    let mut app = directory.app(connected);
    app.dispatch(AppCommand::EditPatch(PatchEdit {
        synth: "bass".into(),
        operations: vec![
            PatchOperation::RemoveInput {
                id: "velocity".into(),
                cables: DependentCables::Disconnect,
            },
            PatchOperation::Connect {
                from: "env.control".into(),
                to: "amp.level".into(),
            },
        ],
    }))
    .unwrap();
    assert_eq!(
        app.authored_source().unwrap().synths()["bass"]
            .inputs()
            .len(),
        2
    );

    // The additive typed operation survives the existing native capture vocabulary.
    let mut app = directory.app(bare.clone());
    let mut capture = NativeProjectCapture::new(Arc::new(bare.clone()), 96000);
    app.dispatch(AppCommand::EditPatch(PatchEdit {
        synth: "bass".into(),
        operations: vec![PatchOperation::AddInput {
            id: "velocity".into(),
            signal: meldritch_dsl::SignalType::Control,
        }],
    }))
    .unwrap();
    capture.finish(&app).unwrap();
    let text = fs::read_to_string(capture.session.as_ref().unwrap().path()).unwrap();
    let mut replay = directory.app(load_native_capture_baseline(&directory.0, &text).unwrap());
    let document: toml::Value = toml::from_str(&text).unwrap();
    for event in document["events"].as_array().unwrap() {
        let envelope =
            NativeCommandEnvelope::decode(&toml::to_string(&event["native"]).unwrap()).unwrap();
        replay.dispatch(envelope.command.into()).unwrap();
    }
    assert_eq!(
        replay.authored_source().unwrap().fingerprint(),
        app.authored_source().unwrap().fingerprint()
    );
}
