use meldritch_app::{AppCommand, AppController};
use meldritch_control::native_project::{NativeProjectStore, load_native_project};
use meldritch_core::FrameRange;
use meldritch_render::{
    dsp::{Oscillator, Waveform},
    song_host::CompiledSongPatch,
};
use std::{fs, path::PathBuf, sync::Arc};
const FIXTURE: &str = include_str!("../../../songs/examples/36-patch-lfo/main.project.lua");
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
    let mut store =
        NativeProjectStore::open(&meldritch_dsl::load_project_directory(&directory.0).unwrap())
            .unwrap();
    store.save(&source).unwrap();
    let patch = CompiledSongPatch::compile_initial(&source).unwrap();
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
