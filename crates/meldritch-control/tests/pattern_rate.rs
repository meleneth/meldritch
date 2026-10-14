use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use meldritch_app::{AppCommand, AppCommandResult, AppController, NoteEdit};
use meldritch_control::native_project::{
    NativeProjectStore, encode_native_project, load_native_project,
};
use meldritch_core::FrameRange;
use meldritch_dsl::{ControlEvaluation, PatchEdit, PatchOperation, PatternSignal, ValidatedSong};
use meldritch_render::{
    dsp::{Oscillator, Waveform},
    song_host::CompiledSongPatch,
    song_render::{LiveVoiceEvent, LiveVoiceInput, compile_pattern_voice},
};
use meldritch_tui::project_binding_command;
use std::{fs, path::PathBuf, sync::Arc};

const FIXTURE: &str = include_str!("../../../songs/examples/36-patch-lfo/pattern-rate.project.lua");

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "meldritch-pattern-rate-{}-{}",
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

fn render(source: &ValidatedSong, start: u64, end: u64) -> Vec<f64> {
    CompiledSongPatch::compile_initial(source)
        .unwrap()
        .render_default(FrameRange::new(start, end).unwrap())
        .unwrap()
        .samples()
        .to_vec()
}

#[test]
fn pattern_rate_integrates_each_sample_while_lfo_output_latches() {
    let directory = Directory::new();
    let source = directory.source();
    let mut settings = source.synths()["bass"]
        .modules()
        .iter()
        .find(|module| module.id() == "slow")
        .unwrap()
        .lfo()
        .unwrap();
    settings.evaluation = ControlEvaluation::Control { period_frames: 64 };
    let source = source
        .edit_patch(&PatchEdit {
            synth: "bass".into(),
            operations: vec![PatchOperation::ConfigureLfo {
                module: "slow".into(),
                settings,
            }],
        })
        .unwrap();
    let actual = render(&source, 0, 96_000);
    let mut oscillator = Oscillator::new(Waveform::Sine);
    let mut phase = 0.0_f64;
    let mut held = 0.0_f64;
    for (frame, &sample) in actual.iter().enumerate() {
        let position = frame % 48_000;
        let velocity: f64 = if position < 12_000 {
            0.0
        } else if position < 24_000 {
            0.5
        } else {
            1.0
        };
        if frame % 64 == 0 {
            held = (std::f64::consts::TAU * phase).sin().mul_add(0.5, 0.5);
        }
        let expected =
            oscillator.next(440.0 * 2_f64.powf((48.0 - 69.0) / 12.0), 48_000) * held * 0.8;
        assert!(
            (sample - expected).abs() < 1e-12,
            "frame {frame}: {sample} != {expected}"
        );
        phase = (phase + velocity.mul_add(3.0, 1.0) / 48_000.0).fract();
    }
    let mut split = render(&source, 0, 12_001);
    split.extend(render(&source, 12_001, 48_001));
    split.extend(render(&source, 48_001, 96_000));
    assert_eq!(split, actual);
    let encoded = encode_native_project(&source).unwrap();
    let reopened = load_native_project(&directory.0, &encoded).unwrap();
    assert_eq!(source.fingerprint(), reopened.fingerprint());
    assert_eq!(actual, render(&reopened, 0, 96_000));
}

#[test]
fn pattern_rate_integrates_between_explicit_gate_event_samples() {
    let directory = Directory::new();
    let source = directory.source();
    let mut settings = source.synths()["bass"]
        .modules()
        .iter()
        .find(|module| module.id() == "slow")
        .unwrap()
        .lfo()
        .unwrap();
    settings.evaluation = ControlEvaluation::Event;
    let source = source
        .edit_patch(&PatchEdit {
            synth: "bass".into(),
            operations: vec![
                PatchOperation::ConfigureLfo {
                    module: "slow".into(),
                    settings,
                },
                PatchOperation::Connect {
                    from: "input.gate".into(),
                    to: "slow.sample".into(),
                },
            ],
        })
        .unwrap();
    let actual = render(&source, 0, 96_000);
    let mut oscillator = Oscillator::new(Waveform::Sine);
    let mut phase = 0.0_f64;
    let mut held = 0.0_f64;
    for (frame, &sample) in actual.iter().enumerate() {
        let position = frame % 48_000;
        let velocity: f64 = if position < 12_000 {
            0.0
        } else if position < 24_000 {
            0.5
        } else {
            1.0
        };
        if position == 0 {
            held = (std::f64::consts::TAU * phase).sin().mul_add(0.5, 0.5);
        }
        let expected =
            oscillator.next(440.0 * 2_f64.powf((48.0 - 69.0) / 12.0), 48_000) * held * 0.8;
        assert!(
            (sample - expected).abs() < 1e-12,
            "frame {frame}: {sample} != {expected}"
        );
        phase = (phase + velocity.mul_add(3.0, 1.0) / 48_000.0).fract();
    }
    let mut split = render(&source, 0, 48_001);
    split.extend(render(&source, 48_001, 96_000));
    assert_eq!(split, actual);
    let encoded = encode_native_project(&source).unwrap();
    let reopened = load_native_project(&directory.0, &encoded).unwrap();
    assert_eq!(source.fingerprint(), reopened.fingerprint());
    assert_eq!(actual, render(&reopened, 0, 96_000));
}

#[test]
fn pattern_steps_integrate_lfo_phase_without_reset_and_reopen_exactly() {
    let directory = Directory::new();
    let source = directory.source();
    let actual = render(&source, 0, 96_000);
    let alternate_default = load_native_project(
        &directory.0,
        &FIXTURE.replace("kind = \"hertz\", value = 1", "kind = \"hertz\", value = 7"),
    )
    .unwrap();
    assert_eq!(render(&alternate_default, 0, 96_000), actual);
    let mut oscillator = Oscillator::new(Waveform::Sine);
    let mut phase = 0.0_f64;
    for (frame, &sample) in actual.iter().enumerate() {
        let position = frame % 48_000;
        let velocity: f64 = if position < 12_000 {
            0.0
        } else if position < 24_000 {
            0.5
        } else {
            1.0
        };
        let hz = velocity.mul_add(3.0, 1.0);
        let control = (std::f64::consts::TAU * phase).sin().mul_add(0.5, 0.5);
        let expected =
            oscillator.next(440.0 * 2_f64.powf((48.0 - 69.0) / 12.0), 48_000) * control * 0.8;
        assert!(
            (sample - expected).abs() < 1e-12,
            "frame {frame}: {sample} != {expected}"
        );
        phase = (phase + hz / 48_000.0).fract();
    }
    let mut split = Vec::new();
    for (start, end) in [
        (0, 1),
        (1, 12_000),
        (12_000, 24_000),
        (24_000, 48_000),
        (48_000, 96_000),
    ] {
        split.extend(render(&source, start, end));
    }
    assert_eq!(split, actual);
    let mut app = directory.app(source.clone());
    app.dispatch(AppCommand::SelectPatternVoice("bass-instance".into()))
        .unwrap();
    app.dispatch(AppCommand::SelectPalettePattern("speed-steps".into()))
        .unwrap();
    let unbind = project_binding_command(
        KeyEvent::new(KeyCode::Char('u'), KeyModifiers::NONE),
        &app.view_model(),
        &source,
    )
    .unwrap();
    assert!(matches!(
        unbind,
        AppCommand::BindPatternVoice {
            signal: PatternSignal::Velocity,
            pattern: None,
            ..
        }
    ));
    app.dispatch(unbind).unwrap();
    assert_ne!(
        render(&app.authored_source().unwrap(), 0, 48_000),
        actual[..48_000]
    );
    app.dispatch(AppCommand::BindPatternVoice {
        voice: "bass-instance".into(),
        signal: PatternSignal::Velocity,
        pattern: Some("speed-steps".into()),
    })
    .unwrap();
    assert_eq!(
        render(&app.authored_source().unwrap(), 0, 48_000),
        actual[..48_000]
    );
    let changed = app
        .dispatch(AppCommand::EditNote {
            pattern: "speed-steps".into(),
            edit: NoteEdit::Velocity {
                id: "note-2".into(),
                velocity: 0.25,
            },
        })
        .unwrap();
    assert!(matches!(
        changed,
        AppCommandResult::PatternEdited { changed: true, .. }
    ));
    assert_eq!(app.history().back().unwrap().dirty_ranges.len(), 1);
    let edited = app.authored_source().unwrap();
    assert_eq!(render(&edited, 0, 12_000), actual[..12_000]);
    assert_ne!(render(&edited, 12_000, 24_000), actual[12_000..24_000]);
    let mut store =
        NativeProjectStore::open(&meldritch_dsl::load_project_directory(&directory.0).unwrap())
            .unwrap();
    store.save(&edited).unwrap();
    let reopened =
        NativeProjectStore::open(&meldritch_dsl::load_project_directory(&directory.0).unwrap())
            .unwrap();
    assert_eq!(
        reopened.source().unwrap().fingerprint(),
        edited.fingerprint()
    );
    assert_eq!(
        render(reopened.source().unwrap(), 0, 96_000),
        render(&edited, 0, 96_000)
    );
}

#[test]
fn live_keyboard_strike_velocity_uses_the_same_explicit_rate_path() {
    let directory = Directory::new();
    let source = directory.source();
    let patch = compile_pattern_voice(&source, &source.pattern_voices()["bass-instance"]).unwrap();
    let mut live = patch.live_renderer(48, 0).unwrap();
    let events = [
        LiveVoiceEvent {
            frame: 0,
            input: LiveVoiceInput::Pitch(48),
        },
        LiveVoiceEvent {
            frame: 0,
            input: LiveVoiceInput::Velocity(0),
        },
        LiveVoiceEvent {
            frame: 0,
            input: LiveVoiceInput::Gate(true),
        },
        LiveVoiceEvent {
            frame: 12_000,
            input: LiveVoiceInput::Velocity(64),
        },
        LiveVoiceEvent {
            frame: 24_000,
            input: LiveVoiceInput::Velocity(127),
        },
        LiveVoiceEvent {
            frame: 40_000,
            input: LiveVoiceInput::Gate(false),
        },
    ];
    let mut actual = Vec::new();
    while live.cursor() < 48_000 {
        let start = live.cursor();
        let end = (start + 4096).min(48_000);
        let block_events: Vec<_> = events
            .iter()
            .copied()
            .filter(|event| event.frame >= start && event.frame < end)
            .collect();
        actual.extend_from_slice(
            live.render(FrameRange::new(start, end).unwrap(), &block_events)
                .unwrap()
                .samples(),
        );
    }
    let mut oscillator = Oscillator::new(Waveform::Sine);
    let mut phase = 0.0_f64;
    for (frame, &sample) in actual.iter().enumerate() {
        let velocity: f64 = if frame < 12_000 {
            0.0
        } else if frame < 24_000 {
            64.0 / 127.0
        } else {
            1.0
        };
        let control = (std::f64::consts::TAU * phase).sin().mul_add(0.5, 0.5);
        let wave = oscillator.next(440.0 * 2_f64.powf((48.0 - 69.0) / 12.0), 48_000);
        let expected = wave * control * if frame < 40_000 { 1.0 } else { 0.0 };
        assert!((sample - expected).abs() < 1e-12, "frame {frame}");
        phase = (phase + velocity.mul_add(3.0, 1.0) / 48_000.0).fract();
    }
}

#[test]
fn invalid_pattern_rate_graphs_and_steps_leave_accepted_source_unchanged() {
    let directory = Directory::new();
    let source = directory.source();
    let fixture: toml::Value = toml::from_str(include_str!(
        "../../../songs/examples/36-patch-lfo/invalid/pattern-rate.toml"
    ))
    .unwrap();
    let mut app = directory.app(source.clone());
    let before = app.authored_source().unwrap();
    let history = app.history().len();
    assert!(
        app.dispatch(AppCommand::BindPatternVoice {
            voice: "bass-instance".into(),
            signal: PatternSignal::Velocity,
            pattern: Some("missing".into())
        })
        .is_err()
    );
    assert!(Arc::ptr_eq(&before, &app.authored_source().unwrap()));
    assert_eq!(app.history().len(), history);
    for case in fixture["cases"].as_array().unwrap() {
        let case = case.as_str().unwrap();
        let result = match case {
            "unknown-pattern" => source.bind_pattern_voice(
                "bass-instance",
                PatternSignal::Velocity,
                Some("missing".into()),
            ),
            "invalid-edit" => source
                .edit_note(
                    "speed-steps",
                    &NoteEdit::Velocity {
                        id: "note-1".into(),
                        velocity: f64::NAN,
                    },
                )
                .map(|(source, _, _)| source),
            _ => {
                let modified = match case {
                    "missing-adapter" => FIXTURE.replace("{ id = \"speed\", kind = \"control_hertz\", hertz = { gain_hz = 3, offset_hz = 1, clamp_min_hz = 1, clamp_max_hz = 4 } },", ""),
                    "wrong-unit" => FIXTURE.replace("from = \"speed.control\", to = \"slow.rate\"", "from = \"input.velocity\", to = \"slow.rate\""),
                    "missing-input" => FIXTURE.replace("{ id = \"velocity\", signal = \"control\" }", ""),
                    "rate-out-of-bounds" => FIXTURE.replace("clamp_max_hz = 4", "clamp_max_hz = 101"),
                    "simultaneous-step" => FIXTURE.replace("note = \"G4\", start_tick = 480", "note = \"G4\", start_tick = 0"),
                    _ => panic!("unknown case {case}"),
                };
                assert_ne!(modified, FIXTURE, "negative fixture unchanged: {case}");
                load_native_project(&directory.0, &modified)
            }
        };
        assert!(result.is_err(), "{case}");
        assert_eq!(
            render(&source, 0, 48_000),
            render(&directory.source(), 0, 48_000)
        );
    }
}
