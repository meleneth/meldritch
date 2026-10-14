use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use meldritch_app::{AppCommand, AppCommandResult, AppController, NoteEdit};
use meldritch_control::{
    native_capture::{NativeCommandEnvelope, NativeProjectCapture, load_native_capture_baseline},
    native_project::{NativeProjectStore, load_native_project},
};
use meldritch_core::FrameRange;
use meldritch_dsl::{PatternSignal, ValidatedSong};
use meldritch_render::dsp::{Oscillator, Waveform};
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
    fn app_with_source(&self, fixture: &str) -> AppController {
        let source = load_native_project(&self.0, fixture).unwrap();
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

fn render(source: &ValidatedSong, start: u64, end: u64) -> Vec<f64> {
    CompiledSongPatch::compile_initial(source)
        .unwrap()
        .render_default(FrameRange::new(start, end).unwrap())
        .unwrap()
        .samples()
        .to_vec()
}

#[test]
fn sustain_gate_samples_lfo_independently_of_note_gate() {
    use meldritch_dsl::{LfoReset, PatchEdit, PatchOperation};
    use meldritch_render::song_render::{LiveVoiceEvent, LiveVoiceInput, compile_pattern_voice};
    let directory = Directory::new();
    let fixture = FIXTURE.replace("release = 0.005", "release = 0");
    let mut app = directory.app_with_source(&fixture);
    let mut capture = NativeProjectCapture::new(app.authored_source().unwrap(), 48_000);
    app.dispatch(AppCommand::SelectInstrument("bass".into()))
        .unwrap();
    app.dispatch(AppCommand::ToggleInstrumentEditor).unwrap();
    app.dispatch(AppCommand::TogglePatchEditor).unwrap();
    for line in include_str!("../../../songs/examples/36-patch-lfo/sustain-event-patch.txt").lines()
    {
        if line.starts_with('#') {
            continue;
        }
        for character in line.chars() {
            app.dispatch(AppCommand::PatchCharacter(character)).unwrap();
        }
        app.dispatch(AppCommand::PatchSubmit).unwrap();
        capture.checkpoint(&app).unwrap();
    }
    let source = app.authored_source().unwrap();
    let audio = render(&source, 0, 48_000);
    let mut oscillator = Oscillator::new(Waveform::Sine);
    let sampled = (std::f64::consts::TAU * (12_000.0 / 31_200.0))
        .sin()
        .mul_add(0.5, 0.5);
    for (frame, &actual) in audio.iter().enumerate() {
        let note = if frame < 24_000 { 48.0 } else { 55.0 };
        let frequency = 440.0 * 2_f64.powf((note - 69.0) / 12.0);
        let wave = oscillator.next(frequency, 48_000);
        let expected =
            wave * if frame < 36_000 {
                if frame < 12_000 { 0.5 } else { sampled }
            } else {
                0.0
            } * 0.8;
        assert!(
            (actual - expected).abs() < 1e-12,
            "frame {frame}: {actual} != {expected}"
        );
    }
    let mut split = render(&source, 0, 12_001);
    split.extend(render(&source, 12_001, 48_000));
    assert_eq!(split, audio);

    let voice = compile_pattern_voice(&source, &source.pattern_voices()["bass-instance"]).unwrap();
    let events = [
        LiveVoiceEvent {
            frame: 0,
            input: LiveVoiceInput::Gate(true),
        },
        LiveVoiceEvent {
            frame: 1031,
            input: LiveVoiceInput::Sustain(true),
        },
        LiveVoiceEvent {
            frame: 2048,
            input: LiveVoiceInput::Sustain(false),
        },
    ];
    let full = voice
        .live_renderer(48, 0)
        .unwrap()
        .render(FrameRange::new(0, 4096).unwrap(), &events)
        .unwrap();
    let mut live = voice.live_renderer(48, 0).unwrap();
    let mut partitioned = live
        .render(FrameRange::new(0, 1000).unwrap(), &events[..1])
        .unwrap()
        .samples()
        .to_vec();
    partitioned.extend_from_slice(
        live.render(FrameRange::new(1000, 2000).unwrap(), &events[1..2])
            .unwrap()
            .samples(),
    );
    partitioned.extend_from_slice(
        live.render(FrameRange::new(2000, 4096).unwrap(), &events[2..])
            .unwrap()
            .samples(),
    );
    assert_eq!(partitioned, full.samples());
    let note_only = voice
        .live_renderer(48, 0)
        .unwrap()
        .render(FrameRange::new(0, 4096).unwrap(), &events[..1])
        .unwrap();
    assert_eq!(&full.samples()[..1031], &note_only.samples()[..1031]);
    assert!(
        full.samples()[1031..]
            .iter()
            .zip(&note_only.samples()[1031..])
            .any(|(a, b)| (a - b).abs() > 1e-6)
    );
    let duplicate_high = [
        events[0],
        events[1],
        LiveVoiceEvent {
            frame: 1500,
            input: LiveVoiceInput::Sustain(true),
        },
        events[2],
    ];
    let repeated = voice
        .live_renderer(48, 0)
        .unwrap()
        .render(FrameRange::new(0, 4096).unwrap(), &duplicate_high)
        .unwrap();
    assert_eq!(repeated.samples(), full.samples());
    let retrigger = [
        events[0],
        events[1],
        LiveVoiceEvent {
            frame: 1500,
            input: LiveVoiceInput::Gate(true),
        },
        events[2],
    ];
    let retriggered = voice
        .live_renderer(48, 0)
        .unwrap()
        .render(FrameRange::new(0, 4096).unwrap(), &retrigger)
        .unwrap();
    assert_eq!(retriggered.samples(), full.samples());

    let mut settings = source.synths()["bass"]
        .modules()
        .iter()
        .find(|module| module.id() == "slow")
        .unwrap()
        .lfo()
        .unwrap();
    settings.reset = LfoReset::GateRise;
    settings.phase = 0.25;
    let reset_source = source
        .edit_patch(&PatchEdit {
            synth: "bass".into(),
            operations: vec![
                PatchOperation::ConfigureLfo {
                    module: "slow".into(),
                    settings,
                },
                PatchOperation::Connect {
                    from: "input.sustain".into(),
                    to: "slow.reset".into(),
                },
            ],
        })
        .unwrap();
    let reset_audio = render(&reset_source, 0, 48_000);
    let mut reset_oscillator = Oscillator::new(Waveform::Sine);
    for (frame, &actual) in reset_audio.iter().enumerate() {
        let note = if frame < 24_000 { 48.0 } else { 55.0 };
        let wave = reset_oscillator.next(440.0 * 2_f64.powf((note - 69.0) / 12.0), 48_000);
        let expected =
            wave * if frame < 36_000 {
                if frame < 12_000 { 0.5 } else { 1.0 }
            } else {
                0.0
            } * 0.8;
        assert!((actual - expected).abs() < 1e-12, "reset frame {frame}");
    }
    let invalid: toml::Value = toml::from_str(include_str!(
        "../../../songs/examples/36-patch-lfo/invalid/sustain-event.toml"
    ))
    .unwrap();
    for case in invalid["case"].as_array().unwrap() {
        let operations = match case["kind"].as_str().unwrap() {
            "duplicate_sample_driver" => vec![PatchOperation::Connect {
                from: "input.gate".into(),
                to: "slow.sample".into(),
            }],
            kind => panic!("unknown invalid case {kind}"),
        };
        let edited = source.edit_patch(&PatchEdit {
            synth: "bass".into(),
            operations,
        });
        match case["kind"].as_str().unwrap() {
            "duplicate_sample_driver" => assert!(edited.is_err()),
            _ => unreachable!(),
        }
    }

    capture.finish(&app).unwrap();
    let mut store =
        NativeProjectStore::open(&meldritch_dsl::load_project_directory(&directory.0).unwrap())
            .unwrap();
    store.save(&source).unwrap();
    let reopened =
        NativeProjectStore::open(&meldritch_dsl::load_project_directory(&directory.0).unwrap())
            .unwrap();
    assert_eq!(
        reopened.source().unwrap().fingerprint(),
        source.fingerprint()
    );
    assert_eq!(render(reopened.source().unwrap(), 0, 48_000), audio);
}

#[test]
fn declared_sustain_gate_samples_without_gate_hold() {
    use meldritch_dsl::{PatchEdit, PatchOperation};
    use meldritch_render::song_render::{LiveVoiceEvent, LiveVoiceInput, compile_pattern_voice};
    let directory = Directory::new();
    let fixture = FIXTURE.replace("release = 0.005", "release = 0").replace(
        "start_tick = 480, duration_ticks = 960",
        "start_tick = 240, duration_ticks = 960",
    );
    let mut app = directory.app_with_source(&fixture);
    let mut capture = NativeProjectCapture::new(app.authored_source().unwrap(), 24_000);
    app.dispatch(AppCommand::SelectInstrument("bass".into()))
        .unwrap();
    app.dispatch(AppCommand::ToggleInstrumentEditor).unwrap();
    app.dispatch(AppCommand::TogglePatchEditor).unwrap();
    for line in
        include_str!("../../../songs/examples/36-patch-lfo/sustain-direct-patch.txt").lines()
    {
        if line.starts_with('#') {
            continue;
        }
        for character in line.chars() {
            app.dispatch(AppCommand::PatchCharacter(character)).unwrap();
        }
        app.dispatch(AppCommand::PatchSubmit).unwrap();
        capture.checkpoint(&app).unwrap();
    }
    let source = app.authored_source().unwrap();
    assert!(
        !source.synths()["bass"]
            .modules()
            .iter()
            .any(|module| module.id() == "hold")
    );
    let undeclared = load_native_project(
        &directory.0,
        include_str!("../../../songs/examples/36-patch-lfo/main.project.lua"),
    )
    .unwrap();
    let invalid: toml::Value = toml::from_str(include_str!(
        "../../../songs/examples/36-patch-lfo/invalid/sustain-direct.toml"
    ))
    .unwrap();
    for case in invalid["case"].as_array().unwrap() {
        assert_eq!(case["kind"].as_str(), Some("undeclared_sustain_source"));
        assert!(
            undeclared
                .edit_patch(&PatchEdit {
                    synth: "bass".into(),
                    operations: vec![PatchOperation::Connect {
                        from: "input.sustain".into(),
                        to: "slow.sample".into()
                    },]
                })
                .is_err()
        );
    }
    let audio = render(&source, 0, 24_000);
    let mut oscillator = Oscillator::new(Waveform::Sine);
    let sampled = (std::f64::consts::TAU * (6_000.0 / 31_200.0))
        .sin()
        .mul_add(0.5, 0.5);
    for (frame, &actual) in audio.iter().enumerate() {
        let wave = oscillator.next(440.0 * 2_f64.powf((48.0 - 69.0) / 12.0), 48_000);
        let expected =
            wave * if frame < 12_000 {
                if frame < 6_000 { 0.5 } else { sampled }
            } else {
                0.0
            } * 0.8;
        assert!((actual - expected).abs() < 1e-12, "frame {frame}");
    }
    let mut split = render(&source, 0, 6_001);
    split.extend(render(&source, 6_001, 24_000));
    assert_eq!(split, audio);

    let voice = compile_pattern_voice(&source, &source.pattern_voices()["bass-instance"]).unwrap();
    let events = [
        LiveVoiceEvent {
            frame: 0,
            input: LiveVoiceInput::Gate(true),
        },
        LiveVoiceEvent {
            frame: 1031,
            input: LiveVoiceInput::Sustain(true),
        },
        LiveVoiceEvent {
            frame: 2048,
            input: LiveVoiceInput::Gate(false),
        },
    ];
    let full = voice
        .live_renderer(48, 0)
        .unwrap()
        .render(FrameRange::new(0, 4096).unwrap(), &events)
        .unwrap();
    let mut live = voice.live_renderer(48, 0).unwrap();
    let mut parts = live
        .render(FrameRange::new(0, 1000).unwrap(), &events[..1])
        .unwrap()
        .samples()
        .to_vec();
    parts.extend_from_slice(
        live.render(FrameRange::new(1000, 2000).unwrap(), &events[1..2])
            .unwrap()
            .samples(),
    );
    parts.extend_from_slice(
        live.render(FrameRange::new(2000, 4096).unwrap(), &events[2..])
            .unwrap()
            .samples(),
    );
    assert_eq!(parts, full.samples());
    assert!(
        full.samples()[2048..]
            .iter()
            .all(|value| value.abs() < 1e-12)
    );
    let no_pedal = voice
        .live_renderer(48, 0)
        .unwrap()
        .render(FrameRange::new(0, 4096).unwrap(), &[events[0], events[2]])
        .unwrap();
    assert!(
        full.samples()[1031..2048]
            .iter()
            .zip(&no_pedal.samples()[1031..2048])
            .any(|(a, b)| (a - b).abs() > 1e-6)
    );

    capture.finish(&app).unwrap();
    let mut store =
        NativeProjectStore::open(&meldritch_dsl::load_project_directory(&directory.0).unwrap())
            .unwrap();
    store.save(&source).unwrap();
    let reopened =
        NativeProjectStore::open(&meldritch_dsl::load_project_directory(&directory.0).unwrap())
            .unwrap();
    assert_eq!(
        reopened.source().unwrap().fingerprint(),
        source.fingerprint()
    );
    assert_eq!(render(reopened.source().unwrap(), 0, 24_000), audio);
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
