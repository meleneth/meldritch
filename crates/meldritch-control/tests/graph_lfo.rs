use meldritch_app::{AppCommand, AppController};
use meldritch_control::native_project::{NativeProjectStore, load_native_project};
use meldritch_core::FrameRange;
use meldritch_render::{
    dsp::{Oscillator, Waveform},
    song_host::CompiledSongPatch,
};
use std::{fs, path::PathBuf, sync::Arc};
const FIXTURE: &str = include_str!("../../../songs/examples/36-patch-lfo/main.project.lua");

#[test]
fn live_mono_worker_blocks_preserve_existing_pcm_and_u64_event_timing() {
    use meldritch_render::song_render::{LiveVoiceEvent, LiveVoiceInput, compile_pattern_voice};
    let directory = Directory::new();
    let trace: toml::Value = toml::from_str(include_str!(
        "../../../songs/examples/36-patch-lfo/live-keyboard.toml"
    ))
    .unwrap();
    let origin = trace["origin_frame"].as_integer().unwrap() as u64;
    let length = trace["length_frames"].as_integer().unwrap() as u64;
    let events: Vec<_> = trace["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|event| LiveVoiceEvent {
            frame: event["frame"].as_integer().unwrap() as u64,
            input: LiveVoiceInput::Gate(event["high"].as_bool().unwrap()),
        })
        .collect();
    let reset = FIXTURE
        .replace("reset = \"none\"", "reset = \"gate_rise\"")
        .replace(
            "cables = {",
            "cables = { { from = \"input.gate\", to = \"slow.reset\", signal = \"gate\" },",
        );
    let release = FIXTURE.replace(
        "attack = 0, decay = 0, sustain = 1, release = 0",
        "attack = 0.01, decay = 0.03, sustain = 0.7, release = 0.03",
    );
    let plaits_fm = FIXTURE.replace("kind = \"oscillator\", waveform = \"sine\"",
        "kind = \"plaits\", plaits = { model = \"fm\", harmonics = 0.5, timbre = 0.5, morph = 0.5 }");
    let plaits_harmonic = plaits_fm
        .replace("model = \"fm\"", "model = \"harmonic\"")
        .replace(
            "cables = {",
            "cables = { { from = \"input.gate\", to = \"osc.gate\", signal = \"gate\" },",
        )
        .replace("from = \"osc.audio\"", "from = \"osc.aux\"");
    for (fixture, dynamic_filter) in [
        (FIXTURE, false),
        (reset.as_str(), false),
        (release.as_str(), true),
        (plaits_fm.as_str(), false),
        (plaits_harmonic.as_str(), false),
    ] {
        let mut app = directory.app();
        app.enable_native_project_source(load_native_project(&directory.0, fixture).unwrap())
            .unwrap();
        if dynamic_filter {
            app.dispatch(AppCommand::SelectInstrument("bass".into()))
                .unwrap();
            app.dispatch(AppCommand::ToggleInstrumentEditor).unwrap();
            app.dispatch(AppCommand::TogglePatchEditor).unwrap();
            for script in [
                include_str!("../../../songs/examples/36-patch-lfo/filter-patch.txt"),
                include_str!("../../../songs/examples/36-patch-lfo/rate-patch.txt"),
            ] {
                for line in script.lines() {
                    for c in line.chars() {
                        app.dispatch(AppCommand::PatchCharacter(c)).unwrap();
                    }
                    app.dispatch(AppCommand::PatchSubmit).unwrap();
                }
            }
        }
        let source = app.authored_source().unwrap();
        let history = app.history().len();
        let patch =
            compile_pattern_voice(&source, &source.pattern_voices()["bass-instance"]).unwrap();
        let expected = patch.render(FrameRange::new(0, length).unwrap()).unwrap();
        if dynamic_filter {
            assert!(
                expected.samples()[84000..]
                    .iter()
                    .any(|sample| sample.abs() > 1e-6)
            );
        }
        for partition in [vec![4096], vec![1, 7, 31, 509, 2048], vec![3601, 17, 256]] {
            let mut live = patch.live_renderer(48, origin).unwrap();
            let mut actual = Vec::new();
            let mut index = 0;
            while live.cursor() < origin + length {
                let start = live.cursor();
                let end = (start + partition[index % partition.len()]).min(origin + length);
                let input: Vec<_> = events
                    .iter()
                    .copied()
                    .filter(|e| e.frame >= start && e.frame < end)
                    .collect();
                actual.extend_from_slice(
                    live.render(FrameRange::new(start, end).unwrap(), &input)
                        .unwrap()
                        .samples(),
                );
                index += 1;
            }
            assert_eq!(actual, expected.samples());
            assert_eq!(live.cursor(), origin + length);
        }
        assert!(Arc::ptr_eq(&source, &app.authored_source().unwrap()));
        assert_eq!(history, app.history().len());
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 0);
    }
}

#[test]
fn live_pitch_gate_and_atomic_invalid_block_recovery_follow_explicit_cables() {
    use meldritch_render::dsp::{AdsrEnvelope, AdsrSettings};
    use meldritch_render::song_render::{
        LiveMonoRenderer, LiveVoiceEvent, LiveVoiceInput, compile_pattern_voice,
    };
    let directory = Directory::new();
    let source = load_native_project(
        &directory.0,
        &FIXTURE.replace(
            "attack = 0, decay = 0, sustain = 1, release = 0",
            "attack = 0.01, decay = 0.03, sustain = 0.7, release = 0.03",
        ),
    )
    .unwrap();
    let patch = compile_pattern_voice(&source, &source.pattern_voices()["bass-instance"]).unwrap();
    let events = [
        LiveVoiceEvent {
            frame: 50,
            input: LiveVoiceInput::Pitch(60),
        },
        LiveVoiceEvent {
            frame: 100,
            input: LiveVoiceInput::Gate(true),
        },
        LiveVoiceEvent {
            frame: 700,
            input: LiveVoiceInput::Pitch(67),
        },
        LiveVoiceEvent {
            frame: 900,
            input: LiveVoiceInput::Gate(true),
        },
        LiveVoiceEvent {
            frame: 1500,
            input: LiveVoiceInput::Gate(false),
        },
        LiveVoiceEvent {
            frame: 1600,
            input: LiveVoiceInput::Gate(false),
        },
    ];
    let mut live = patch.live_renderer(48, 0).unwrap();
    let audio = live
        .render(FrameRange::new(0, 4096).unwrap(), &events)
        .unwrap();
    let mut oscillator = Oscillator::new(Waveform::Sine);
    let mut env = AdsrEnvelope::new(
        AdsrSettings {
            attack_seconds: 0.01,
            decay_seconds: 0.03,
            sustain_level: 0.7,
            release_seconds: 0.03,
        },
        48000,
    );
    let mut pitch = 48;
    for (frame, actual) in audio.samples().iter().enumerate() {
        if frame == 50 {
            pitch = 60;
        }
        if frame == 100 || frame == 900 {
            env.note_on();
        }
        if frame == 700 {
            pitch = 67;
        }
        if frame == 1500 {
            env.note_off();
        } // Duplicate low must not restart release.
        let scale = (std::f64::consts::TAU * frame as f64 / 24000.0)
            .sin()
            .mul_add(0.5, 0.5);
        let expected = oscillator.next(440.0 * 2_f64.powf((f64::from(pitch) - 69.0) / 12.0), 48000)
            * env.next_value()
            * scale;
        assert!((actual - expected).abs() < 1e-12, "frame {frame}");
    }
    assert!(audio.samples()[50..100].iter().all(|s| *s == 0.0));
    assert!(audio.samples()[1500..2000].iter().any(|s| s.abs() > 1e-6));
    assert!(audio.samples()[3000..].iter().all(|s| *s == 0.0));
    // Held gate crosses authored retriggers/loop boundaries without consuming them.
    let mut held = patch.live_renderer(48, 0).unwrap();
    let mut oscillator = Oscillator::new(Waveform::Sine);
    let mut env = AdsrEnvelope::new(
        AdsrSettings {
            attack_seconds: 0.01,
            decay_seconds: 0.03,
            sustain_level: 0.7,
            release_seconds: 0.03,
        },
        48000,
    );
    env.note_on();
    for block in 0..25 {
        let start = block * 4096;
        let events = if block == 0 {
            vec![LiveVoiceEvent {
                frame: 0,
                input: LiveVoiceInput::Gate(true),
            }]
        } else {
            vec![]
        };
        let audio = held
            .render(FrameRange::new(start, start + 4096).unwrap(), &events)
            .unwrap();
        for (relative, actual) in audio.samples().iter().enumerate() {
            let frame = start + relative as u64;
            let scale = (std::f64::consts::TAU * frame as f64 / 24000.0)
                .sin()
                .mul_add(0.5, 0.5);
            let expected = oscillator.next(440.0 * 2_f64.powf((48.0 - 69.0) / 12.0), 48000)
                * env.next_value()
                * scale;
            assert!((actual - expected).abs() < 1e-12, "held frame {frame}");
        }
    }
    let mut pristine = patch.live_renderer(48, 4096).unwrap();
    let mut invalid = patch.live_renderer(48, 4096).unwrap();
    let pitch = |frame, value| LiveVoiceEvent {
        frame,
        input: LiveVoiceInput::Pitch(value),
    };
    let cases: toml::Value = toml::from_str(include_str!(
        "../../../songs/examples/36-patch-lfo/invalid/live-keyboard.toml"
    ))
    .unwrap();
    for case in cases["cases"].as_array().unwrap() {
        let normal = FrameRange::new(4096, 4200).unwrap();
        let (range, input) = match case["kind"].as_str().unwrap() {
            "stale_block" => (FrameRange::new(4095, 4100).unwrap(), vec![]),
            "empty_block" => (FrameRange::new(4096, 4096).unwrap(), vec![]),
            "too_long" => (
                FrameRange::new(4096, 4096 + LiveMonoRenderer::MAX_FRAMES + 1).unwrap(),
                vec![],
            ),
            "late_invalid_pitch" => (normal, vec![pitch(4096, 60), pitch(4097, 128)]),
            "unordered" => (normal, vec![pitch(4100, 60), pitch(4099, 64)]),
            "past_event" => (normal, vec![pitch(4095, 60)]),
            "end_event" => (normal, vec![pitch(4200, 60)]),
            "event_budget" => (
                normal,
                vec![pitch(4096, 60); LiveMonoRenderer::MAX_EVENTS + 1],
            ),
            kind => panic!("unexecuted live negative {kind}"),
        };
        assert!(invalid.render(range, &input).is_err());
        assert_eq!(invalid.cursor(), 4096);
    }
    let valid = [LiveVoiceEvent {
        frame: 4096,
        input: LiveVoiceInput::Gate(true),
    }];
    let range = FrameRange::new(4096, 8192).unwrap();
    assert_eq!(
        invalid.render(range, &valid).unwrap().samples(),
        pristine.render(range, &valid).unwrap().samples()
    );
    assert!(patch.live_renderer(128, 0).is_err());
    let poly = load_native_project(
        &directory.0,
        &FIXTURE.replace("polyphony = 1", "polyphony = 2"),
    )
    .unwrap_err();
    assert!(poly.contains("mono"));
    let legacy = meldritch_dsl::load_song_directory(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../songs/examples/26-apc-note-pattern"),
    )
    .unwrap();
    let poly = meldritch_render::song_render::compile_note_song(&legacy).unwrap();
    assert!(poly.live_renderer(48, 0).is_err());

    // Removing authored envelope/VCA wiring allows oscillation despite a low gate.
    let mut app = directory.app();
    app.enable_native_project_source(source).unwrap();
    app.dispatch(AppCommand::EditPatch(meldritch_dsl::PatchEdit {
        synth: "bass".into(),
        operations: vec![
            meldritch_dsl::PatchOperation::Remove {
                id: "shape".into(),
                cables: meldritch_dsl::DependentCables::Disconnect,
            },
            meldritch_dsl::PatchOperation::Connect {
                from: "unipolar.control".into(),
                to: "amp.level".into(),
            },
        ],
    }))
    .unwrap();
    let source = app.authored_source().unwrap();
    let free = compile_pattern_voice(&source, &source.pattern_voices()["bass-instance"]).unwrap();
    let mut low = free.live_renderer(48, 0).unwrap();
    let block = low
        .render(
            FrameRange::new(0, 4096).unwrap(),
            &[
                LiveVoiceEvent {
                    frame: 100,
                    input: LiveVoiceInput::Gate(true),
                },
                LiveVoiceEvent {
                    frame: 200,
                    input: LiveVoiceInput::Gate(false),
                },
            ],
        )
        .unwrap();
    assert!(block.samples()[0..100].iter().any(|s| s.abs() > 0.1));
    assert!(block.samples()[3000..].iter().any(|s| s.abs() > 0.1));
    let mut high_origin = free.live_renderer(48, u64::MAX - 4096).unwrap();
    let mut zero_origin = free.live_renderer(48, 0).unwrap();
    assert_eq!(
        high_origin
            .render(FrameRange::new(u64::MAX - 4096, u64::MAX).unwrap(), &[])
            .unwrap()
            .samples(),
        zero_origin
            .render(FrameRange::new(0, 4096).unwrap(), &[])
            .unwrap()
            .samples()
    );
    assert_eq!(high_origin.cursor(), u64::MAX);
}
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "meldritch-lfo-{}-{}",
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
fn tremolo_matches_independent_phase_gate_and_oscillator_reference_with_split_rendering() {
    let directory = Directory::new();
    for (fixture, cycle_frames, reset) in [
        (FIXTURE.to_owned(), 24_000.0, false),
        (FIXTURE.replace("bpm = 120", "bpm = 60"), 48_000.0, false),
        (
            FIXTURE.replace(
                "kind = \"beats\", value = 1",
                "kind = \"beats\", value = 1.3",
            ),
            31_200.0,
            false,
        ),
        (
            FIXTURE.replace(
                "kind = \"beats\", value = 1",
                "kind = \"hertz\", value = 1.7",
            ),
            48_000.0 / 1.7,
            false,
        ),
        (
            FIXTURE
                .replace("reset = \"none\"", "reset = \"gate_rise\"")
                .replace(
                    "cables = {",
                    "cables = { { from = \"input.gate\", to = \"slow.reset\", signal = \"gate\" },",
                ),
            24_000.0,
            true,
        ),
    ] {
        let source = load_native_project(&directory.0, &fixture).unwrap();
        let patch = CompiledSongPatch::compile_initial(&source).unwrap();
        let audio = patch
            .render_default(FrameRange::new(0, 192_000).unwrap())
            .unwrap();
        let beat_frames = 48_000.0 * 60.0 / source.performance().bpm();
        let mut oscillator = Oscillator::new(Waveform::Sine);
        let mut origin = 0;
        for (frame, actual) in audio.samples().iter().enumerate() {
            let position = frame % (beat_frames * 4.0) as usize;
            if reset && (position == 0 || position == (beat_frames * 1.5) as usize) {
                origin = frame;
            }
            let cycles = (frame - origin) as f64 / cycle_frames;
            let value = (std::f64::consts::TAU * cycles.fract())
                .sin()
                .mul_add(0.5, 0.5);
            let wave = oscillator.next(440.0 * 2_f64.powf((48.0 - 69.0) / 12.0), 48_000);
            let expected =
                wave * if position < (beat_frames * 3.5) as usize {
                    value
                } else {
                    0.0
                } * 0.8;
            assert!(
                (actual - expected).abs() < 1e-12,
                "frame {frame}: {actual} != {expected}"
            );
        }
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
        assert_eq!(split, audio.samples());
    }
}

#[test]
fn invalid_control_contracts_reject_without_accepted_source_or_history_changes() {
    let directory = Directory::new();
    let source = load_native_project(&directory.0, FIXTURE).unwrap();
    let mut app = directory.app();
    app.enable_native_project_source(source).unwrap();
    let before = app.authored_source().unwrap();
    let cases: toml::Value = toml::from_str(include_str!(
        "../../../songs/examples/36-patch-lfo/invalid/control.toml"
    ))
    .unwrap();
    for case in cases["cases"].as_array().unwrap() {
        let kind = case["kind"].as_str().unwrap();
        let fixture = match kind {
            "rate_zero" => FIXTURE.replace("value = 1", "value = 0"),
            "rate_nonfinite" => FIXTURE.replace("value = 1", "value = 1/0"),
            "scale_nonfinite" => FIXTURE.replace("gain = 0.5", "gain = 1/0"),
            "module_limit" => FIXTURE.replace(
                "modules = {",
                &format!(
                    "modules = {{ {}",
                    (0..26)
                        .map(|index| format!(
                            "{{ id = \"extra-{index}\", kind = \"control_multiply\" }},"
                        ))
                        .collect::<String>()
                ),
            ),
            "phase_nonfinite" => FIXTURE.replace("phase = 0", "phase = 0/0"),
            "scale_reversed" => FIXTURE.replace(
                "clamp_min = 0, clamp_max = 1",
                "clamp_min = 1, clamp_max = 0",
            ),
            "missing_input" => FIXTURE.replace(
                "{ from = \"env.control\", to = \"shape.left\", signal = \"control\" },",
                "",
            ),
            "missing_reset" => FIXTURE.replace("reset = \"none\"", "reset = \"gate_rise\""),
            "reset_without_policy" => FIXTURE.replace(
                "cables = {",
                "cables = { { from = \"input.gate\", to = \"slow.reset\", signal = \"gate\" },",
            ),
            "bipolar_to_vca" => FIXTURE.replace(
                "from = \"shape.control\", to = \"amp.level\"",
                "from = \"slow.control\", to = \"amp.level\"",
            ),
            "audio_to_scale" => FIXTURE.replace(
                "from = \"slow.control\", to = \"unipolar.control\", signal = \"control\"",
                "from = \"osc.audio\", to = \"unipolar.control\", signal = \"audio\"",
            ),
            "duplicate_input" => FIXTURE.replace(
                "cables = {",
                "cables = { { from = \"env.control\", to = \"amp.level\", signal = \"control\" },",
            ),
            "unused_module" => FIXTURE.replace(
                "modules = {",
                "modules = { { id = \"unused\", kind = \"control_multiply\" },",
            ),
            "cycle" => FIXTURE.replace(
                "from = \"env.control\", to = \"shape.left\"",
                "from = \"shape.control\", to = \"shape.left\"",
            ),
            "polyphony" => FIXTURE.replace("polyphony = 1", "polyphony = 2"),
            "scope" => FIXTURE.replace("scope = \"per_instance\"", "scope = \"global\""),
            "evaluation" => FIXTURE.replace("evaluation = \"audio\"", "evaluation = \"event\""),
            _ => panic!("unexecuted negative {kind}"),
        };
        assert!(
            load_native_project(&directory.0, &fixture).is_err(),
            "accepted invalid {kind}"
        );
        assert!(Arc::ptr_eq(&before, &app.authored_source().unwrap()));
        assert!(app.history().is_empty());
    }
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 0);
}

#[test]
fn shared_lfo_fanout_and_typed_rate_edits_capture_without_hidden_combination() {
    use meldritch_control::native_capture::{
        NativeCommandEnvelope, NativeProjectCapture, load_native_capture_baseline,
    };
    use meldritch_dsl::{
        ControlScaleSettings, GraphLfoRate, GraphLfoSettings, ModuleKind, PatchEdit, PatchOperation,
    };
    let directory = Directory::new();
    let mut app = directory.app();
    app.enable_native_project_source(load_native_project(&directory.0, FIXTURE).unwrap())
        .unwrap();
    let mut capture = NativeProjectCapture::new(app.authored_source().unwrap(), 96_000);
    app.dispatch(AppCommand::EditPatch(PatchEdit {
        synth: "bass".into(),
        operations: vec![
            PatchOperation::Instantiate {
                id: "inverse".into(),
                kind: ModuleKind::ControlScale,
            },
            PatchOperation::ConfigureControlScale {
                module: "inverse".into(),
                settings: ControlScaleSettings {
                    gain: -0.5,
                    ..ControlScaleSettings::default()
                },
            },
            PatchOperation::Instantiate {
                id: "product".into(),
                kind: ModuleKind::ControlMultiply,
            },
            PatchOperation::Disconnect {
                from: "shape.control".into(),
                to: "amp.level".into(),
            },
            PatchOperation::Connect {
                from: "slow.control".into(),
                to: "inverse.control".into(),
            },
            PatchOperation::Connect {
                from: "shape.control".into(),
                to: "product.left".into(),
            },
            PatchOperation::Connect {
                from: "inverse.control".into(),
                to: "product.right".into(),
            },
            PatchOperation::Connect {
                from: "product.control".into(),
                to: "amp.level".into(),
            },
            PatchOperation::ConfigureLfo {
                module: "slow".into(),
                settings: GraphLfoSettings {
                    rate: GraphLfoRate::Beats(1.3),
                    phase: 0.25,
                    ..GraphLfoSettings::default()
                },
            },
        ],
    }))
    .unwrap();
    capture.checkpoint(&app).unwrap();
    let accepted = app.authored_source().unwrap();
    assert_eq!(
        accepted.synths()["bass"]
            .modules()
            .iter()
            .filter(|m| m.kind() == ModuleKind::Lfo)
            .count(),
        1
    );
    let patch = CompiledSongPatch::compile_initial(&accepted).unwrap();
    let audio = patch
        .render_default(FrameRange::new(0, 192_000).unwrap())
        .unwrap();
    let mut oscillator = Oscillator::new(Waveform::Sine);
    for (frame, actual) in audio.samples().iter().enumerate() {
        let lfo = (std::f64::consts::TAU * (frame as f64 / 31_200.0 + 0.25).fract()).sin();
        let level = if frame % 96_000 < 84_000 {
            lfo.mul_add(0.5, 0.5) * lfo.mul_add(-0.5, 0.5)
        } else {
            0.0
        };
        let wave = oscillator.next(440.0 * 2_f64.powf((48.0 - 69.0) / 12.0), 48_000);
        assert!((actual - wave * level * 0.8).abs() < 1e-12);
    }
    let history = app.history().len();
    assert!(
        app.dispatch(AppCommand::EditPatch(PatchEdit {
            synth: "bass".into(),
            operations: vec![PatchOperation::ConfigureLfo {
                module: "slow".into(),
                settings: GraphLfoSettings {
                    rate: GraphLfoRate::Hertz(f64::INFINITY),
                    ..GraphLfoSettings::default()
                }
            },]
        }))
        .is_err()
    );
    assert_eq!(history, app.history().len());
    assert!(Arc::ptr_eq(&accepted, &app.authored_source().unwrap()));
    capture.finish(&app).unwrap();
    let text = fs::read_to_string(capture.session.as_ref().unwrap().path()).unwrap();
    let mut replay = directory.app();
    replay
        .enable_native_project_source(load_native_capture_baseline(&directory.0, &text).unwrap())
        .unwrap();
    let events: toml::Value = toml::from_str(&text).unwrap();
    for event in events["events"].as_array().unwrap() {
        let envelope =
            NativeCommandEnvelope::decode(&toml::to_string(&event["native"]).unwrap()).unwrap();
        replay.dispatch(envelope.command.into()).unwrap();
        assert_eq!(
            envelope.source_after,
            format!("{:016x}", replay.authored_source().unwrap().fingerprint())
        );
    }
    assert_eq!(
        replay.authored_source().unwrap().fingerprint(),
        accepted.fingerprint()
    );
    assert_eq!(
        CompiledSongPatch::compile_initial(&replay.authored_source().unwrap())
            .unwrap()
            .render_default(FrameRange::new(0, 192_000).unwrap())
            .unwrap()
            .samples(),
        audio.samples()
    );
    let ports = accepted.synths()["bass"]
        .modules()
        .iter()
        .flat_map(|m| m.ports())
        .collect::<Vec<_>>();
    assert_eq!(
        ports
            .iter()
            .find(|p| p.endpoint == "slow.control")
            .unwrap()
            .unit,
        Some(meldritch_dsl::ControlUnit::Bipolar)
    );
    assert_eq!(
        ports
            .iter()
            .find(|p| p.endpoint == "amp.level")
            .unwrap()
            .unit,
        Some(meldritch_dsl::ControlUnit::UnitInterval)
    );
}

#[test]
fn shared_patch_instances_have_independent_explicit_gate_reset_origins() {
    let directory = Directory::new();
    let fixture = FIXTURE
        .replace("reset = \"none\"", "reset = \"gate_rise\"")
        .replace(
            "cables = {",
            "cables = { { from = \"input.gate\", to = \"slow.reset\", signal = \"gate\" },",
        );
    let source = load_native_project(&directory.0, &fixture).unwrap();
    let mut definition = source.native_project_definition().unwrap();
    let mut second = definition.voices[0].clone();
    second.id = "other-instance".into();
    second.level = 0.4;
    second.gate_pattern = Some("melody".into());
    definition.voices.push(second);
    let source =
        meldritch_dsl::ValidatedSong::restore_native_project(&directory.0, &definition).unwrap();
    let audio = CompiledSongPatch::compile_initial(&source)
        .unwrap()
        .render_default(FrameRange::new(0, 192_000).unwrap())
        .unwrap();
    let mut oscillator = Oscillator::new(Waveform::Sine);
    let normalization = 1.0 / 2.0_f64.sqrt();
    for (frame, actual) in audio.samples().iter().enumerate() {
        let position = frame % 96_000;
        let first_phase = (if position >= 36_000 {
            position - 36_000
        } else {
            position
        }) as f64
            / 24_000.0;
        let first = if position < 84_000 {
            (std::f64::consts::TAU * first_phase.fract())
                .sin()
                .mul_add(0.5, 0.5)
        } else {
            0.0
        };
        let second = (std::f64::consts::TAU * (position as f64 / 24_000.0).fract())
            .sin()
            .mul_add(0.5, 0.5);
        let wave = oscillator.next(440.0 * 2_f64.powf((48.0 - 69.0) / 12.0), 48_000);
        // Registry order is bass-instance then other-instance.
        let expected = wave * first * 0.8 * normalization + wave * second * 0.4 * normalization;
        assert!((actual - expected).abs() < 1e-12, "frame {frame}");
    }
}

#[test]
fn explicit_lfo_level_does_not_implicitly_apply_the_disconnected_envelope_or_gate() {
    use meldritch_dsl::{DependentCables, PatchEdit, PatchOperation};
    let directory = Directory::new();
    let source = load_native_project(&directory.0, FIXTURE).unwrap();
    let mut app = directory.app();
    app.enable_native_project_source(source).unwrap();
    app.dispatch(AppCommand::EditPatch(PatchEdit {
        synth: "bass".into(),
        operations: vec![
            PatchOperation::Remove {
                id: "shape".into(),
                cables: DependentCables::Disconnect,
            },
            PatchOperation::Connect {
                from: "unipolar.control".into(),
                to: "amp.level".into(),
            },
        ],
    }))
    .unwrap();
    app.dispatch(AppCommand::BindPatternVoice {
        voice: "bass-instance".into(),
        signal: meldritch_dsl::PatternSignal::Gate,
        pattern: None,
    })
    .unwrap();
    let source = app.authored_source().unwrap();
    let audio = CompiledSongPatch::compile_initial(&source)
        .unwrap()
        .render_default(FrameRange::new(0, 96_000).unwrap())
        .unwrap();
    let mut oscillator = Oscillator::new(Waveform::Sine);
    for (frame, sample) in audio.samples().iter().enumerate() {
        let level = (std::f64::consts::TAU * (frame as f64 / 24_000.0).fract())
            .sin()
            .mul_add(0.5, 0.5);
        let expected =
            oscillator.next(440.0 * 2_f64.powf((48.0 - 69.0) / 12.0), 48_000) * level * 0.8;
        assert!((sample - expected).abs() < 1e-12);
    }
    assert!(audio.samples().iter().any(|sample| sample.abs() > 0.01));
}

#[test]
fn native_atomic_patch_draft_and_fresh_process_persistence_reuse_existing_boundaries() {
    let directory = Directory::new();
    let mut app = directory.app();
    app.dispatch(AppCommand::CreatePatch(
        meldritch_app::PatchCreation::mono_voice("voice-1"),
    ))
    .unwrap();
    app.dispatch(AppCommand::SelectInstrument("voice-1".into()))
        .unwrap();
    app.dispatch(AppCommand::ToggleInstrumentEditor).unwrap();
    app.dispatch(AppCommand::TogglePatchEditor).unwrap();
    let before = app.authored_source().unwrap();
    let lines: Vec<_> = include_str!("../../../songs/examples/36-patch-lfo/keyboard-patch.txt")
        .lines()
        .collect();
    for (index, line) in lines.iter().enumerate() {
        for c in line.chars() {
            app.dispatch(AppCommand::PatchCharacter(c)).unwrap();
        }
        app.dispatch(AppCommand::PatchSubmit).unwrap();
        if index + 1 < lines.len() {
            assert!(Arc::ptr_eq(&before, &app.authored_source().unwrap()));
        }
    }
    let source = app.authored_source().unwrap();
    assert_eq!(source.synths()["voice-1"].modules().len(), 7);
    assert_eq!(source.synths()["voice-1"].cables().len(), 8);
    assert_ne!(source.fingerprint(), before.fingerprint());
    // Snapshot fixture supplies timed independent source material; no second importer.
    let source = load_native_project(&directory.0, FIXTURE).unwrap();
    assert_fresh_process(&directory, &source);
}

fn assert_fresh_process(directory: &Directory, source: &meldritch_dsl::ValidatedSong) {
    let mut store =
        NativeProjectStore::open(&meldritch_dsl::load_project_directory(&directory.0).unwrap())
            .unwrap();
    store.save(source).unwrap();
    let patch = CompiledSongPatch::compile_initial(source).unwrap();
    let pcm: Vec<_> = patch
        .render_default(FrameRange::new(0, 192_000).unwrap())
        .unwrap()
        .samples()
        .iter()
        .flat_map(|sample| sample.to_le_bytes())
        .collect();
    fs::write(directory.0.join("expected.pcm"), pcm).unwrap();
    fs::write(
        directory.0.join("expected.fingerprint"),
        format!("{:016x}", source.fingerprint()),
    )
    .unwrap();
    let child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "graph_lfo_reopen_child",
            "--ignored",
            "--nocapture",
        ])
        .env("MELDRITCH_LFO_ROOT", &directory.0)
        .output()
        .unwrap();
    assert!(
        child.status.success(),
        "{}{}",
        String::from_utf8_lossy(&child.stdout),
        String::from_utf8_lossy(&child.stderr)
    );
}

#[test]
fn explicit_vibrato_draft_fanout_depth_and_fresh_process_match_sample_reference() {
    use meldritch_dsl::{PatchEdit, PatchOperation, PitchModulationSettings};
    let directory = Directory::new();
    let mut app = directory.app();
    let fixture = FIXTURE.replace(
        "notes = { { id = \"note-1\", note = \"C3\", start_tick = 0, duration_ticks = 3840, velocity = 0.8 } }",
        "notes = { { id = \"note-1\", note = \"C3\", start_tick = 0, duration_ticks = 1920, velocity = 0.8 }, { id = \"note-2\", note = \"C4\", start_tick = 1920, duration_ticks = 1920, velocity = 0.8 } }",
    ).replace("id = \"melody\", length_ticks = 3840, looped = true, next_note_id = 2", "id = \"melody\", length_ticks = 3840, looped = true, next_note_id = 3");
    assert_ne!(fixture, FIXTURE);
    app.enable_native_project_source(load_native_project(&directory.0, &fixture).unwrap())
        .unwrap();
    app.dispatch(AppCommand::SelectInstrument("bass".into()))
        .unwrap();
    app.dispatch(AppCommand::ToggleInstrumentEditor).unwrap();
    app.dispatch(AppCommand::TogglePatchEditor).unwrap();
    let before = app.authored_source().unwrap();
    let lines: Vec<_> = include_str!("../../../songs/examples/36-patch-lfo/vibrato-patch.txt")
        .lines()
        .collect();
    for (index, line) in lines.iter().enumerate() {
        for c in line.chars() {
            app.dispatch(AppCommand::PatchCharacter(c)).unwrap();
        }
        app.dispatch(AppCommand::PatchSubmit).unwrap();
        if index + 1 < lines.len() {
            assert!(Arc::ptr_eq(&before, &app.authored_source().unwrap()));
        }
    }
    let accepted = app.authored_source().unwrap();
    assert_ne!(accepted.fingerprint(), before.fingerprint());
    assert_eq!(
        accepted.synths()["bass"]
            .modules()
            .iter()
            .filter(|m| m.kind() == meldritch_dsl::ModuleKind::Lfo)
            .count(),
        1
    );
    for depth in [0.5, 0.0, -2.0] {
        let edit = PatchEdit {
            synth: "bass".into(),
            operations: vec![PatchOperation::ConfigurePitchModulation {
                module: "vibrato".into(),
                settings: PitchModulationSettings {
                    depth_semitones: depth,
                },
            }],
        };
        let envelope = meldritch_control::native_capture::NativeProjectCommand::try_from(
            &AppCommand::EditPatch(edit.clone()),
        )
        .unwrap();
        let text = toml::to_string(&envelope).unwrap();
        let restored: meldritch_control::native_capture::NativeProjectCommand =
            toml::from_str(&text).unwrap();
        app.dispatch(restored.into()).unwrap();
        let source = app.authored_source().unwrap();
        let patch = CompiledSongPatch::compile_initial(&source).unwrap();
        let audio = patch
            .render_default(FrameRange::new(0, 192_000).unwrap())
            .unwrap();
        let mut oscillator = Oscillator::new(Waveform::Sine);
        for (frame, actual) in audio.samples().iter().enumerate() {
            let pitch = if frame % 96_000 < 48_000 { 48.0 } else { 60.0 };
            let base = 440.0 * 2_f64.powf((pitch - 69.0) / 12.0);
            let lfo = (std::f64::consts::TAU * (frame as f64 / 24_000.0).fract()).sin();
            let wave = oscillator.next(base * 2_f64.powf(lfo * depth / 12.0), 48_000);
            let expected =
                wave * if frame % 96_000 < 84_000 {
                    lfo.mul_add(0.5, 0.5)
                } else {
                    0.0
                } * 0.8;
            assert!(
                (actual - expected).abs() < 1e-12,
                "depth {depth}, frame {frame}"
            );
        }
        assert_fresh_process(&directory, &source);
    }
}

#[test]
fn invalid_pitch_adapters_leave_source_and_history_unchanged() {
    use meldritch_dsl::{ModuleKind, PatchEdit, PatchOperation, PitchModulationSettings};
    let directory = Directory::new();
    let mut app = directory.app();
    app.enable_native_project_source(load_native_project(&directory.0, FIXTURE).unwrap())
        .unwrap();
    let connect = |from: &str, to: &str| PatchOperation::Connect {
        from: from.into(),
        to: to.into(),
    };
    let disconnect = |from: &str, to: &str| PatchOperation::Disconnect {
        from: from.into(),
        to: to.into(),
    };
    let base = vec![
        disconnect("input.pitch", "osc.pitch"),
        PatchOperation::Instantiate {
            id: "vibrato".into(),
            kind: ModuleKind::PitchModulate,
        },
        connect("input.pitch", "vibrato.pitch"),
        connect("slow.control", "vibrato.control"),
        connect("vibrato.pitch", "osc.pitch"),
    ];
    let before = app.authored_source().unwrap();
    let cases: toml::Value = toml::from_str(include_str!(
        "../../../songs/examples/36-patch-lfo/invalid/pitch.toml"
    ))
    .unwrap();
    for case in cases["cases"].as_array().unwrap() {
        let kind = case["kind"].as_str().unwrap();
        let mut operations = base.clone();
        match kind {
            "nonfinite_depth" | "depth_range" => {
                operations.push(PatchOperation::ConfigurePitchModulation {
                    module: "vibrato".into(),
                    settings: PitchModulationSettings {
                        depth_semitones: if kind == "depth_range" {
                            25.0
                        } else {
                            f64::INFINITY
                        },
                    },
                })
            }
            "missing_pitch" => operations.push(disconnect("input.pitch", "vibrato.pitch")),
            "missing_control" => operations.push(disconnect("slow.control", "vibrato.control")),
            "duplicate_pitch" => operations.push(connect("input.pitch", "osc.pitch")),
            "unit_interval_control" => {
                operations.push(disconnect("slow.control", "vibrato.control"));
                operations.push(connect("unipolar.control", "vibrato.control"));
            }
            "direct_control_to_pitch" => operations.push(connect("slow.control", "osc.pitch")),
            "second_adapter" => operations.push(PatchOperation::Instantiate {
                id: "extra".into(),
                kind: ModuleKind::PitchModulate,
            }),
            "plaits" => {
                operations.push(PatchOperation::Remove {
                    id: "osc".into(),
                    cables: meldritch_dsl::DependentCables::Disconnect,
                });
                operations.push(PatchOperation::Instantiate {
                    id: "osc".into(),
                    kind: ModuleKind::Plaits,
                });
                operations.push(connect("vibrato.pitch", "osc.pitch"));
                operations.push(connect("osc.audio", "amp.audio"));
            }
            _ => panic!("unexecuted pitch negative {kind}"),
        }
        assert!(
            app.dispatch(AppCommand::EditPatch(PatchEdit {
                synth: "bass".into(),
                operations
            }))
            .is_err(),
            "accepted {kind}"
        );
        assert!(Arc::ptr_eq(&before, &app.authored_source().unwrap()));
        assert!(app.history().is_empty());
    }
}

#[test]
#[ignore = "invoked by isolated graph-LFO parent"]
fn graph_lfo_reopen_child() {
    let root = PathBuf::from(std::env::var_os("MELDRITCH_LFO_ROOT").unwrap());
    let before = fs::read_dir(&root).unwrap().count();
    let store =
        NativeProjectStore::open(&meldritch_dsl::load_project_directory(&root).unwrap()).unwrap();
    let source = store.source().unwrap();
    assert_eq!(
        format!("{:016x}", source.fingerprint()),
        fs::read_to_string(root.join("expected.fingerprint")).unwrap()
    );
    let patch = CompiledSongPatch::compile_initial(source).unwrap();
    let raw = |range| {
        patch
            .render_default(range)
            .unwrap()
            .samples()
            .iter()
            .flat_map(|sample| sample.to_le_bytes())
            .collect::<Vec<_>>()
    };
    let expected = fs::read(root.join("expected.pcm")).unwrap();
    assert_eq!(raw(FrameRange::new(0, 192_000).unwrap()), expected);
    let mut split = raw(FrameRange::new(0, 97_331).unwrap());
    split.extend(raw(FrameRange::new(97_331, 192_000).unwrap()));
    assert_eq!(split, expected);
    assert_eq!(fs::read_dir(&root).unwrap().count(), before);
}

#[test]
fn cabled_lfo_rate_integrates_phase_with_tempo_hz_reset_and_all_waveforms() {
    use meldritch_control::native_capture::{
        NativeCommandEnvelope, NativeProjectCapture, load_native_capture_baseline,
    };
    use meldritch_dsl::{
        GraphLfoRate, GraphLfoSettings, GraphLfoShape, LfoReset, PatchEdit, PatchOperation,
    };
    let directory = Directory::new();
    for (bpm, clock_hz, shape, reset) in [
        (120, false, GraphLfoShape::Sine, false),
        (60, false, GraphLfoShape::Sine, false),
        (60, true, GraphLfoShape::Sine, false),
        (120, false, GraphLfoShape::Triangle, false),
        (120, false, GraphLfoShape::Saw, false),
        (120, false, GraphLfoShape::Square, false),
        (120, false, GraphLfoShape::Sine, true),
    ] {
        let mut app = directory.app();
        app.enable_native_project_source(
            load_native_project(
                &directory.0,
                &FIXTURE.replace("bpm = 120", &format!("bpm = {bpm}")),
            )
            .unwrap(),
        )
        .unwrap();
        let mut capture = (bpm == 120 && !clock_hz && shape == GraphLfoShape::Sine && !reset)
            .then(|| NativeProjectCapture::new(app.authored_source().unwrap(), 192000));
        app.dispatch(AppCommand::SelectInstrument("bass".into()))
            .unwrap();
        app.dispatch(AppCommand::ToggleInstrumentEditor).unwrap();
        app.dispatch(AppCommand::TogglePatchEditor).unwrap();
        let before = app.authored_source().unwrap();
        let lines: Vec<_> = include_str!("../../../songs/examples/36-patch-lfo/rate-patch.txt")
            .lines()
            .collect();
        for (index, line) in lines.iter().enumerate() {
            for c in line.chars() {
                app.dispatch(AppCommand::PatchCharacter(c)).unwrap();
            }
            app.dispatch(AppCommand::PatchSubmit).unwrap();
            if let Some(capture) = &mut capture {
                capture.checkpoint(&app).unwrap();
            }
            if index + 1 < lines.len() {
                assert!(Arc::ptr_eq(&before, &app.authored_source().unwrap()));
            }
        }
        let mut operations = vec![PatchOperation::ConfigureLfo {
            module: "slow".into(),
            settings: GraphLfoSettings {
                shape,
                phase: 0.25,
                reset: if reset {
                    LfoReset::GateRise
                } else {
                    LfoReset::None
                },
                ..GraphLfoSettings::default()
            },
        }];
        if reset {
            operations.push(PatchOperation::Connect {
                from: "input.gate".into(),
                to: "slow.reset".into(),
            });
        }
        if clock_hz {
            operations.push(PatchOperation::ConfigureLfo {
                module: "clock".into(),
                settings: GraphLfoSettings {
                    rate: GraphLfoRate::Hertz(0.5),
                    ..GraphLfoSettings::default()
                },
            });
        }
        app.dispatch(AppCommand::EditPatch(PatchEdit {
            synth: "bass".into(),
            operations,
        }))
        .unwrap();
        if let Some(capture) = &mut capture {
            capture.checkpoint(&app).unwrap();
        }
        let source = app.authored_source().unwrap();
        let patch = CompiledSongPatch::compile_initial(&source).unwrap();
        let audio = patch
            .render_default(FrameRange::new(0, 192000).unwrap())
            .unwrap();
        let mut oscillator = Oscillator::new(Waveform::Sine);
        let beat = 48000 * 60 / bpm;
        let source_period = if clock_hz { 96000.0 } else { (4 * beat) as f64 };
        let mut phase = 0.25;
        for (frame, actual) in audio.samples().iter().enumerate() {
            let position = frame % (4 * beat);
            if reset && (position == 0 || position == beat * 3 / 2) {
                phase = 0.25;
            }
            let value = match shape {
                GraphLfoShape::Sine => (std::f64::consts::TAU * phase).sin(),
                GraphLfoShape::Triangle => 1.0 - 4.0 * (phase - 0.5).abs(),
                GraphLfoShape::Saw => phase.mul_add(2.0, -1.0),
                GraphLfoShape::Square => {
                    if phase < 0.5 {
                        1.0
                    } else {
                        -1.0
                    }
                }
            };
            let wave = oscillator.next(440.0 * 2_f64.powf((48.0 - 69.0) / 12.0), 48000);
            let expected =
                wave * if position < beat * 7 / 2 {
                    value.mul_add(0.5, 0.5)
                } else {
                    0.0
                } * 0.8;
            assert!(
                (actual - expected).abs() < 1e-12,
                "{shape:?}, reset {reset}, frame {frame}"
            );
            let clock = (std::f64::consts::TAU * (frame as f64 / source_period).fract()).sin();
            let hz = clock.mul_add(0.5, 0.5).mul_add(3.0, 1.0);
            phase = (phase + hz / 48000.0).fract();
        }
        assert_fresh_process(&directory, &source);
        if let Some(mut capture) = capture {
            capture.finish(&app).unwrap();
            let text = fs::read_to_string(capture.session.as_ref().unwrap().path()).unwrap();
            let mut replay = directory.app();
            replay
                .enable_native_project_source(
                    load_native_capture_baseline(&directory.0, &text).unwrap(),
                )
                .unwrap();
            let events: toml::Value = toml::from_str(&text).unwrap();
            for event in events["events"].as_array().unwrap() {
                let envelope =
                    NativeCommandEnvelope::decode(&toml::to_string(&event["native"]).unwrap())
                        .unwrap();
                replay.dispatch(envelope.command.into()).unwrap();
                assert_eq!(
                    envelope.source_after,
                    format!("{:016x}", replay.authored_source().unwrap().fingerprint())
                );
            }
            assert_eq!(
                CompiledSongPatch::compile_initial(&replay.authored_source().unwrap())
                    .unwrap()
                    .render_default(FrameRange::new(0, 192000).unwrap())
                    .unwrap()
                    .samples(),
                audio.samples()
            );
        }
    }
}

#[test]
fn invalid_rate_connections_reject_without_source_or_history_changes() {
    use meldritch_dsl::{ControlHertzSettings, ModuleKind, PatchEdit, PatchOperation};
    let directory = Directory::new();
    let mut app = directory.app();
    app.enable_native_project_source(load_native_project(&directory.0, FIXTURE).unwrap())
        .unwrap();
    let connect = |from: &str, to: &str| PatchOperation::Connect {
        from: from.into(),
        to: to.into(),
    };
    let disconnect = |from: &str, to: &str| PatchOperation::Disconnect {
        from: from.into(),
        to: to.into(),
    };
    let base = vec![
        PatchOperation::Instantiate {
            id: "clock".into(),
            kind: ModuleKind::Lfo,
        },
        PatchOperation::Instantiate {
            id: "clock-scale".into(),
            kind: ModuleKind::ControlScale,
        },
        PatchOperation::Instantiate {
            id: "speed".into(),
            kind: ModuleKind::ControlHertz,
        },
        PatchOperation::ConfigureControlHertz {
            module: "speed".into(),
            settings: ControlHertzSettings {
                gain_hz: 3.0,
                offset_hz: 1.0,
                clamp_min_hz: 1.0,
                clamp_max_hz: 4.0,
            },
        },
        connect("clock.control", "clock-scale.control"),
        connect("clock-scale.control", "speed.control"),
        connect("speed.control", "slow.rate"),
    ];
    let before = app.authored_source().unwrap();
    let cases: toml::Value = toml::from_str(include_str!(
        "../../../songs/examples/36-patch-lfo/invalid/rate.toml"
    ))
    .unwrap();
    for case in cases["cases"].as_array().unwrap() {
        let kind = case["kind"].as_str().unwrap();
        let mut operations = base.clone();
        match kind {
            "rate_range" | "zero_clamp" | "nonfinite_mapper" => {
                operations.push(PatchOperation::ConfigureControlHertz {
                    module: "speed".into(),
                    settings: match kind {
                        "rate_range" => ControlHertzSettings {
                            gain_hz: 3.0,
                            offset_hz: 1.0,
                            clamp_min_hz: 1.0,
                            clamp_max_hz: 101.0,
                        },
                        "zero_clamp" => ControlHertzSettings {
                            clamp_min_hz: 0.0,
                            ..ControlHertzSettings::default()
                        },
                        _ => ControlHertzSettings {
                            gain_hz: f64::INFINITY,
                            ..ControlHertzSettings::default()
                        },
                    },
                })
            }
            "bipolar_rate" => {
                operations.push(disconnect("speed.control", "slow.rate"));
                operations.push(connect("clock.control", "slow.rate"));
            }
            "unit_interval_rate" => {
                operations.push(disconnect("speed.control", "slow.rate"));
                operations.push(connect("clock-scale.control", "slow.rate"));
            }
            "missing_input" => operations.push(disconnect("clock-scale.control", "speed.control")),
            "missing_rate_cable" => operations.push(disconnect("speed.control", "slow.rate")),
            "duplicate_rate" => operations.push(connect("speed.control", "slow.rate")),
            "feedback" => {
                operations.push(disconnect("clock.control", "clock-scale.control"));
                operations.push(connect("slow.control", "clock-scale.control"));
            }
            _ => panic!("unexecuted rate negative {kind}"),
        }
        assert!(
            app.dispatch(AppCommand::EditPatch(PatchEdit {
                synth: "bass".into(),
                operations
            }))
            .is_err(),
            "accepted {kind}"
        );
        assert!(Arc::ptr_eq(&before, &app.authored_source().unwrap()));
        assert!(app.history().is_empty());
    }
}

#[test]
fn envelope_depth_draft_fanout_capture_and_reopen_match_explicit_signal_reference() {
    use meldritch_control::native_capture::{
        NativeCommandEnvelope, NativeProjectCapture, load_native_capture_baseline,
    };
    use meldritch_dsl::{DependentCables, PatchEdit, PatchOperation};
    use meldritch_render::dsp::{AdsrEnvelope, AdsrSettings};
    let directory = Directory::new();
    let mut app = directory.app();
    app.enable_native_project_source(load_native_project(&directory.0, FIXTURE).unwrap())
        .unwrap();
    let mut capture = NativeProjectCapture::new(app.authored_source().unwrap(), 192000);
    app.dispatch(AppCommand::SelectInstrument("bass".into()))
        .unwrap();
    app.dispatch(AppCommand::ToggleInstrumentEditor).unwrap();
    app.dispatch(AppCommand::TogglePatchEditor).unwrap();
    for script in [
        include_str!("../../../songs/examples/36-patch-lfo/vibrato-patch.txt"),
        include_str!("../../../songs/examples/36-patch-lfo/depth-patch.txt"),
    ] {
        let before = app.authored_source().unwrap();
        let lines: Vec<_> = script.lines().collect();
        for (index, line) in lines.iter().enumerate() {
            for c in line.chars() {
                app.dispatch(AppCommand::PatchCharacter(c)).unwrap();
            }
            app.dispatch(AppCommand::PatchSubmit).unwrap();
            capture.checkpoint(&app).unwrap();
            if index + 1 < lines.len() {
                assert!(Arc::ptr_eq(&before, &app.authored_source().unwrap()));
            }
        }
        assert_ne!(
            before.fingerprint(),
            app.authored_source().unwrap().fingerprint()
        );
    }
    for envelope_controls_amplitude in [true, false] {
        if !envelope_controls_amplitude {
            app.dispatch(AppCommand::EditPatch(PatchEdit {
                synth: "bass".into(),
                operations: vec![
                    PatchOperation::Remove {
                        id: "shape".into(),
                        cables: DependentCables::Disconnect,
                    },
                    PatchOperation::Connect {
                        from: "unipolar.control".into(),
                        to: "amp.level".into(),
                    },
                ],
            }))
            .unwrap();
            capture.checkpoint(&app).unwrap();
        }
        let source = app.authored_source().unwrap();
        let patch = CompiledSongPatch::compile_initial(&source).unwrap();
        let audio = patch
            .render_default(FrameRange::new(0, 192000).unwrap())
            .unwrap();
        let mut oscillator = Oscillator::new(Waveform::Sine);
        let mut env = AdsrEnvelope::new(
            AdsrSettings {
                attack_seconds: 0.25,
                decay_seconds: 0.5,
                sustain_level: 0.4,
                release_seconds: 0.125,
            },
            48000,
        );
        let base = 440.0 * 2_f64.powf((48.0 - 69.0) / 12.0);
        for (frame, actual) in audio.samples().iter().enumerate() {
            match frame % 96000 {
                0 | 36000 => {
                    env.note_off();
                    env.note_on();
                }
                84000 => env.note_off(),
                _ => {}
            }
            let envelope = env.next_value();
            let lfo = (std::f64::consts::TAU * (frame as f64 / 24000.0).fract()).sin();
            let attenuated = lfo * envelope;
            let wave = oscillator.next(base * 2_f64.powf(attenuated * 0.5 / 12.0), 48000);
            let amp = attenuated.mul_add(0.5, 0.5)
                * if envelope_controls_amplitude {
                    envelope
                } else {
                    1.0
                };
            assert!(
                (actual - wave * amp * 0.8).abs() < 1e-12,
                "frame {frame}, amplitude {envelope_controls_amplitude}"
            );
        }
        if !envelope_controls_amplitude {
            assert!(audio.samples()[92000..96000].iter().any(|s| s.abs() > 0.1));
        }
        assert_fresh_process(&directory, &source);
    }
    let accepted = app.authored_source().unwrap();
    let ports = accepted.synths()["bass"]
        .modules()
        .iter()
        .flat_map(|m| m.ports())
        .collect::<Vec<_>>();
    for (endpoint, unit) in [
        ("depth.control", meldritch_dsl::ControlUnit::Bipolar),
        ("depth.amount", meldritch_dsl::ControlUnit::UnitInterval),
    ] {
        assert_eq!(
            ports.iter().filter(|p| p.endpoint == endpoint).count(),
            if endpoint == "depth.control" { 2 } else { 1 }
        );
        assert!(
            ports
                .iter()
                .filter(|p| p.endpoint == endpoint)
                .all(|p| p.unit == Some(unit)
                    && p.evaluation == Some(meldritch_dsl::ControlEvaluation::Audio))
        );
    }
    capture.finish(&app).unwrap();
    let text = fs::read_to_string(capture.session.as_ref().unwrap().path()).unwrap();
    let mut replay = directory.app();
    replay
        .enable_native_project_source(load_native_capture_baseline(&directory.0, &text).unwrap())
        .unwrap();
    let events: toml::Value = toml::from_str(&text).unwrap();
    for event in events["events"].as_array().unwrap() {
        let envelope =
            NativeCommandEnvelope::decode(&toml::to_string(&event["native"]).unwrap()).unwrap();
        replay.dispatch(envelope.command.into()).unwrap();
        assert_eq!(
            envelope.source_after,
            format!("{:016x}", replay.authored_source().unwrap().fingerprint())
        );
    }
    assert_eq!(
        accepted.fingerprint(),
        replay.authored_source().unwrap().fingerprint()
    );
    assert_eq!(
        CompiledSongPatch::compile_initial(&accepted)
            .unwrap()
            .render_default(FrameRange::new(0, 192000).unwrap())
            .unwrap()
            .samples(),
        CompiledSongPatch::compile_initial(&replay.authored_source().unwrap())
            .unwrap()
            .render_default(FrameRange::new(0, 192000).unwrap())
            .unwrap()
            .samples()
    );
}

#[test]
fn invalid_depth_connections_reject_without_source_or_history_changes() {
    use meldritch_dsl::{ModuleKind, PatchEdit, PatchOperation};
    let directory = Directory::new();
    let mut app = directory.app();
    app.enable_native_project_source(load_native_project(&directory.0, FIXTURE).unwrap())
        .unwrap();
    let connect = |from: &str, to: &str| PatchOperation::Connect {
        from: from.into(),
        to: to.into(),
    };
    let disconnect = |from: &str, to: &str| PatchOperation::Disconnect {
        from: from.into(),
        to: to.into(),
    };
    let base = vec![
        PatchOperation::Instantiate {
            id: "depth".into(),
            kind: ModuleKind::ControlAttenuate,
        },
        disconnect("slow.control", "unipolar.control"),
        connect("slow.control", "depth.control"),
        connect("env.control", "depth.amount"),
        connect("depth.control", "unipolar.control"),
    ];
    let before = app.authored_source().unwrap();
    let cases: toml::Value = toml::from_str(include_str!(
        "../../../songs/examples/36-patch-lfo/invalid/depth.toml"
    ))
    .unwrap();
    for case in cases["cases"].as_array().unwrap() {
        let kind = case["kind"].as_str().unwrap();
        let mut operations = base.clone();
        match kind {
            "missing_signal" => operations.push(disconnect("slow.control", "depth.control")),
            "missing_amount" => operations.push(disconnect("env.control", "depth.amount")),
            "bipolar_amount" => {
                operations.push(disconnect("env.control", "depth.amount"));
                operations.push(connect("slow.control", "depth.amount"));
            }
            "unit_interval_signal" => {
                operations.push(disconnect("slow.control", "depth.control"));
                operations.push(connect("env.control", "depth.control"));
            }
            "bipolar_to_vca" => {
                operations.push(disconnect("shape.control", "amp.level"));
                operations.push(connect("depth.control", "amp.level"));
            }
            "duplicate_amount" => operations.push(connect("unipolar.control", "depth.amount")),
            "signal_cycle" => {
                operations.push(disconnect("slow.control", "depth.control"));
                operations.push(connect("depth.control", "depth.control"));
            }
            "amount_cycle" => {
                operations.push(disconnect("env.control", "depth.amount"));
                operations.push(connect("unipolar.control", "depth.amount"));
            }
            "unused_attenuator" => operations.push(PatchOperation::Instantiate {
                id: "unused".into(),
                kind: ModuleKind::ControlAttenuate,
            }),
            _ => panic!("unexecuted depth negative {kind}"),
        }
        assert!(
            app.dispatch(AppCommand::EditPatch(PatchEdit {
                synth: "bass".into(),
                operations
            }))
            .is_err(),
            "accepted {kind}"
        );
        assert!(Arc::ptr_eq(&before, &app.authored_source().unwrap()));
        assert!(app.history().is_empty());
    }
}

#[test]
fn filter_hertz_draft_matches_independent_recurrence_and_fresh_process() {
    use meldritch_dsl::{ControlHertzSettings, PatchEdit, PatchOperation};
    let directory = Directory::new();
    let mut app = directory.app();
    app.enable_native_project_source(load_native_project(&directory.0, FIXTURE).unwrap())
        .unwrap();
    app.dispatch(AppCommand::SelectInstrument("bass".into()))
        .unwrap();
    app.dispatch(AppCommand::ToggleInstrumentEditor).unwrap();
    app.dispatch(AppCommand::TogglePatchEditor).unwrap();
    let before = app.authored_source().unwrap();
    let lines: Vec<_> = include_str!("../../../songs/examples/36-patch-lfo/filter-patch.txt")
        .lines()
        .collect();
    for (index, line) in lines.iter().enumerate() {
        for c in line.chars() {
            app.dispatch(AppCommand::PatchCharacter(c)).unwrap();
        }
        app.dispatch(AppCommand::PatchSubmit).unwrap();
        if index + 1 < lines.len() {
            assert!(Arc::ptr_eq(&before, &app.authored_source().unwrap()));
        }
    }
    assert_ne!(
        before.fingerprint(),
        app.authored_source().unwrap().fingerprint()
    );
    for settings in [
        ControlHertzSettings::default(),
        ControlHertzSettings {
            gain_hz: -1800.0,
            offset_hz: 2000.0,
            ..ControlHertzSettings::default()
        },
        ControlHertzSettings {
            gain_hz: 0.0,
            offset_hz: 2500.0,
            ..ControlHertzSettings::default()
        },
        ControlHertzSettings {
            gain_hz: 10000.0,
            offset_hz: -1000.0,
            clamp_min_hz: 100.0,
            clamp_max_hz: 1500.0,
        },
    ] {
        let command = AppCommand::EditPatch(PatchEdit {
            synth: "bass".into(),
            operations: vec![PatchOperation::ConfigureControlHertz {
                module: "cutoff".into(),
                settings,
            }],
        });
        let encoded = toml::to_string(
            &meldritch_control::native_capture::NativeProjectCommand::try_from(&command).unwrap(),
        )
        .unwrap();
        let decoded: meldritch_control::native_capture::NativeProjectCommand =
            toml::from_str(&encoded).unwrap();
        app.dispatch(decoded.into()).unwrap();
        let source = app.authored_source().unwrap();
        let patch = CompiledSongPatch::compile_initial(&source).unwrap();
        let audio = patch
            .render_default(FrameRange::new(0, 192000).unwrap())
            .unwrap();
        let mut oscillator = Oscillator::new(Waveform::Sine);
        let mut state = [0.0, 0.0];
        for (frame, actual) in audio.samples().iter().enumerate() {
            let control = (std::f64::consts::TAU * (frame as f64 / 24000.0).fract())
                .sin()
                .mul_add(0.5, 0.5);
            let frequency = control
                .mul_add(settings.gain_hz, settings.offset_hz)
                .clamp(settings.clamp_min_hz, settings.clamp_max_hz);
            let wave = oscillator.next(440.0 * 2_f64.powf((48.0 - 69.0) / 12.0), 48000);
            let input = wave * if frame % 96000 < 84000 { control } else { 0.0 };
            // Independent scalar trapezoidal-integrator recurrence; default Q=0.
            let g = (std::f64::consts::PI * frequency / 48000.0).tan();
            let a = 1.0 / (1.0 + g * (g + 2.0));
            let band = a * (state[0] + g * (input - state[1]));
            let low = state[1] + g * band;
            state = [2.0 * band - state[0], 2.0 * low - state[1]];
            assert!((actual - low * 0.8).abs() < 1e-12, "frame {frame}");
        }
        assert_fresh_process(&directory, &source);
    }
    let source = app.authored_source().unwrap();
    let patch = CompiledSongPatch::compile_initial(&source).unwrap();
    let mut overrides = meldritch_render::song_host::SongLiveOverrides::default();
    overrides.synth_filters.insert(
        meldritch_render::song_host::SynthFilterTarget {
            synth_id: "bass".into(),
            module_id: "tone".into(),
        },
        meldritch_render::song_host::SongFilterLiveOverrides {
            cutoff_hz: Some(1000.0),
            resonance: None,
        },
    );
    assert!(
        patch
            .render(FrameRange::new(0, 96000).unwrap(), &overrides)
            .is_err()
    );
    let ports = source.synths()["bass"]
        .modules()
        .iter()
        .flat_map(|m| m.ports())
        .collect::<Vec<_>>();
    assert_eq!(
        ports
            .iter()
            .find(|p| p.endpoint == "tone.cutoff")
            .unwrap()
            .unit,
        Some(meldritch_dsl::ControlUnit::Hertz)
    );
}

#[test]
fn invalid_filter_adapters_leave_source_and_history_unchanged() {
    use meldritch_dsl::{ControlHertzSettings, ModuleKind, PatchEdit, PatchOperation};
    let directory = Directory::new();
    let mut app = directory.app();
    app.enable_native_project_source(load_native_project(&directory.0, FIXTURE).unwrap())
        .unwrap();
    let connect = |from: &str, to: &str| PatchOperation::Connect {
        from: from.into(),
        to: to.into(),
    };
    let disconnect = |from: &str, to: &str| PatchOperation::Disconnect {
        from: from.into(),
        to: to.into(),
    };
    let base = vec![
        disconnect("amp.audio", "out.audio"),
        PatchOperation::Instantiate {
            id: "tone".into(),
            kind: ModuleKind::LowPass,
        },
        PatchOperation::Instantiate {
            id: "cutoff".into(),
            kind: ModuleKind::ControlHertz,
        },
        connect("amp.audio", "tone.audio"),
        connect("tone.audio", "out.audio"),
        connect("unipolar.control", "cutoff.control"),
        connect("cutoff.control", "tone.cutoff"),
    ];
    let before = app.authored_source().unwrap();
    let cases: toml::Value = toml::from_str(include_str!(
        "../../../songs/examples/36-patch-lfo/invalid/filter.toml"
    ))
    .unwrap();
    for case in cases["cases"].as_array().unwrap() {
        let kind = case["kind"].as_str().unwrap();
        let mut operations = base.clone();
        match kind {
            "nonfinite" | "range" | "clamp_reversed" => {
                operations.push(PatchOperation::ConfigureControlHertz {
                    module: "cutoff".into(),
                    settings: match kind {
                        "nonfinite" => ControlHertzSettings {
                            gain_hz: f64::INFINITY,
                            ..ControlHertzSettings::default()
                        },
                        "range" => ControlHertzSettings {
                            clamp_max_hz: 30000.0,
                            ..ControlHertzSettings::default()
                        },
                        _ => ControlHertzSettings {
                            clamp_min_hz: 1000.0,
                            clamp_max_hz: 100.0,
                            ..ControlHertzSettings::default()
                        },
                    },
                })
            }
            "missing_input" => operations.push(disconnect("unipolar.control", "cutoff.control")),
            "missing_output" => operations.push(disconnect("cutoff.control", "tone.cutoff")),
            "bipolar_input" => {
                operations.push(disconnect("unipolar.control", "cutoff.control"));
                operations.push(connect("slow.control", "cutoff.control"));
            }
            "unit_interval_to_cutoff" => {
                operations.push(disconnect("cutoff.control", "tone.cutoff"));
                operations.push(connect("unipolar.control", "tone.cutoff"));
            }
            "hertz_to_vca" => {
                operations.push(disconnect("shape.control", "amp.level"));
                operations.push(connect("cutoff.control", "amp.level"));
            }
            "duplicate_cutoff" => operations.push(connect("env.control", "tone.cutoff")),
            "second_adapter" => operations.push(PatchOperation::Instantiate {
                id: "extra".into(),
                kind: ModuleKind::ControlHertz,
            }),
            "no_control_modules" => {
                for id in ["slow", "unipolar", "shape", "cutoff"] {
                    operations.push(PatchOperation::Remove {
                        id: id.into(),
                        cables: meldritch_dsl::DependentCables::Disconnect,
                    });
                }
                operations.push(connect("env.control", "amp.level"));
                operations.push(connect("env.control", "tone.cutoff"));
            }
            _ => panic!("unexecuted filter negative {kind}"),
        }
        assert!(
            app.dispatch(AppCommand::EditPatch(PatchEdit {
                synth: "bass".into(),
                operations
            }))
            .is_err(),
            "accepted {kind}"
        );
        assert!(Arc::ptr_eq(&before, &app.authored_source().unwrap()));
        assert!(app.history().is_empty());
    }
}
