use std::{path::PathBuf, sync::OnceLock};

use meldritch2::{
    compiler::{OfflineRenderer, compile_at_rate},
    lua,
    model::ParameterId,
    vst3::{self, Instance},
};

fn fixture_directory() -> &'static PathBuf {
    static FIXTURE: OnceLock<PathBuf> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        assert_eq!(
            meldritch_vst3_fixture::FIXTURE_NAME,
            "Meldritch Test Instrument"
        );
        let executable = std::env::current_exe().expect("test executable");
        let debug = executable
            .parent()
            .and_then(|deps| deps.parent())
            .expect("target debug directory");
        let library = debug.join("libmeldritch_vst3_fixture.so");
        assert!(
            library.is_file(),
            "fixture library missing: {}",
            library.display()
        );
        let root = std::env::temp_dir().join(format!(
            "meldritch-vst3-{}-{}",
            std::process::id(),
            env!("CARGO_PKG_VERSION")
        ));
        let architecture = match std::env::consts::ARCH {
            "x86_64" => "x86_64-linux",
            "aarch64" => "aarch64-linux",
            other => panic!("unsupported VST3 fixture architecture {other}"),
        };
        let bundle = root
            .join("TestSynth.vst3/Contents")
            .join(architecture)
            .join("TestSynth.so");
        std::fs::create_dir_all(bundle.parent().unwrap()).expect("create fixture bundle");
        std::fs::copy(library, bundle).expect("copy fixture library");
        root
    })
}

fn energy(samples: &[f64]) -> f64 {
    samples.iter().map(|sample| sample * sample).sum()
}

fn parameter_id(spec: &vst3::Spec, name: &str) -> u32 {
    spec.parameters
        .iter()
        .find(|parameter| parameter.name == name)
        .unwrap_or_else(|| panic!("missing VST3 parameter '{name}'"))
        .id
}

fn render_effect(spec: &vst3::Spec, input: &[f64]) -> Vec<f64> {
    let values = spec
        .parameters
        .iter()
        .map(|parameter| spec.initial_value(parameter))
        .collect::<Vec<_>>();
    let mut plugin = Instance::new(spec, 48_000.0).expect("instantiate VST3 effect");
    let mut output = vec![0.0; input.len() * 2];
    let right = input.to_vec();
    plugin.process_effect(input, &right, &values, &mut output);
    assert!(!plugin.failed());
    assert!(output.iter().all(|sample| sample.is_finite()));
    output
}

#[test]
fn installed_lsp_bundle_discovers_and_processes_stereo_compressor_and_limiter() {
    const COMPRESSOR_ID: &str = "647370206B31732020202020756E7363";
    const LIMITER_ID: &str = "64737020623173202020202072667563";
    let root = PathBuf::from("/usr/lib/vst3");
    if !root.join("lsp-plugins.vst3").is_dir() {
        eprintln!("LSP VST3 plugins are not installed; skipping conditional integration test");
        return;
    }

    let discovered = vst3::discover(std::slice::from_ref(&root)).expect("discover LSP bundle");
    let compressor = discovered
        .iter()
        .find(|plugin| plugin.class_id == COMPRESSOR_ID)
        .expect("discover Compressor Stereo");
    let limiter = discovered
        .iter()
        .find(|plugin| plugin.class_id == LIMITER_ID)
        .expect("discover Limiter Stereo");
    assert_eq!(compressor.name, "Compressor Stereo");
    assert_eq!(limiter.name, "Limiter Stereo");
    assert!(!compressor.instrument);
    assert!(!limiter.instrument);

    let mut compressor =
        vst3::load(COMPRESSOR_ID, std::slice::from_ref(&root)).expect("inspect Compressor Stereo");
    assert_eq!(compressor.audio_inputs, 2);
    for name in [
        "Bypass",
        "Attack threshold",
        "Attack time",
        "Release time",
        "Ratio",
        "Knee",
        "Makeup gain",
    ] {
        assert!(
            compressor
                .parameters
                .iter()
                .any(|parameter| parameter.name == name),
            "missing compressor parameter {name}"
        );
    }
    compressor
        .initial_values
        .insert(parameter_id(&compressor, "Attack threshold"), 0.45);
    compressor
        .initial_values
        .insert(parameter_id(&compressor, "Ratio"), 0.85);
    compressor
        .initial_values
        .insert(parameter_id(&compressor, "Attack time"), 0.2);
    compressor
        .initial_values
        .insert(parameter_id(&compressor, "Release time"), 0.45);
    let input = (0..1024)
        .map(|frame| ((frame as f64 * std::f64::consts::TAU * 220.0) / 48_000.0).sin() * 0.8)
        .collect::<Vec<_>>();
    let compressed = render_effect(&compressor, &input);
    assert!(energy(&compressed) > 0.001);
    let dry_energy = energy(&input) * 2.0;
    assert!(
        (energy(&compressed) - dry_energy).abs() > 0.01,
        "compressor did not measurably alter the signal"
    );

    let limiter =
        vst3::load(LIMITER_ID, std::slice::from_ref(&root)).expect("inspect Limiter Stereo");
    assert_eq!(limiter.audio_inputs, 2);
    for name in ["Bypass", "Threshold", "Lookahead", "Release time"] {
        assert!(
            limiter
                .parameters
                .iter()
                .any(|parameter| parameter.name == name),
            "missing limiter parameter {name}"
        );
    }
    let limited = render_effect(&limiter, &input);
    assert!(energy(&limited) > 0.001);

    let stateful = Instance::new(&compressor, 48_000.0).expect("instantiate compressor");
    let state = stateful.save_state().expect("save compressor state");
    assert!(!state.is_empty());
    assert!(stateful.latency_samples() < 48_000);
    drop(stateful);
    compressor.state = Some(state);
    drop(Instance::new(&compressor, 48_000.0).expect("restore compressor state"));

    let script = format!(
        r#"
local instrument = meldritch.instrument("LSP effect graph")
local tone = instrument:oscillator {{ waveform="sine", frequency=110 }}
local effect = instrument:vst3_effect {{
  id="master_compressor", plugin="{COMPRESSOR_ID}", paths={{"{}"}}
}}
effect:set_parameter({}, 0.45)
effect:set_parameter({}, 0.85)
instrument:connect(tone.out, effect.input.left)
instrument:connect(tone.out, effect.input.right)
instrument:connect(effect.out.left, instrument.output.left)
instrument:connect(effect.out.right, instrument.output.right)
return instrument
"#,
        root.display(),
        parameter_id(&compressor, "Attack threshold"),
        parameter_id(&compressor, "Ratio"),
    );
    let graph = lua::read_with_root(&script, &root).expect("load LSP effect graph");
    let machine = compile_at_rate(&graph, 48_000.0).expect("compile LSP effect graph");
    assert!(
        machine
            .vst3_latency_samples("instrument/master_compressor")
            .is_ok()
    );
    let mut renderer = OfflineRenderer::new(machine, 1024, 2).expect("effect renderer");
    let mut left = vec![0.0; 4096];
    let mut right = vec![0.0; 4096];
    for (left, right) in left.chunks_mut(1024).zip(right.chunks_mut(1024)) {
        renderer.process_stereo(left, right);
    }
    assert!(energy(&left) > 0.001);
    assert!(energy(&right) > 0.001);
}

#[test]
fn installed_pulse_exposes_automation_routes_gm_drums_and_restores_state() {
    const PULSE_ID: &str = "ABCDEF019182FAEB506C7365506C6473";
    if std::env::var_os("MELDRITCH_TEST_PULSE").is_none() {
        eprintln!("set MELDRITCH_TEST_PULSE=1 to run the installed PULSE test");
        return;
    }
    let root = PathBuf::from(std::env::var_os("HOME").expect("HOME")).join(".vst3");
    let mut spec = vst3::load(PULSE_ID, &[root]).expect("inspect installed PULSE");
    assert_eq!(spec.descriptor.name, "PULSE Drum Synth");
    assert_eq!(spec.descriptor.vendor, "Pulse");
    assert_eq!(spec.descriptor.version, "0.1.0");
    assert_eq!(spec.descriptor.category, "Instrument|Synth");
    assert!(spec.descriptor.instrument);
    assert_eq!(spec.parameters.len(), 2_245);
    assert_eq!(
        spec.parameters
            .iter()
            .filter(|value| value.automatable)
            .count(),
        165
    );
    for (id, name) in [
        (1_709_986_221, "KICK Synth Level"),
        (1_469_782_605, "KICK Synth Freq"),
        (1_387_348_517, "KICK Noise Level"),
        (337_125_320, "KICK Env2 D"),
    ] {
        let parameter = spec
            .parameters
            .iter()
            .find(|parameter| parameter.id == id)
            .unwrap_or_else(|| panic!("missing stable PULSE parameter {id}"));
        assert_eq!(parameter.name, name);
        assert!(parameter.automatable);
        assert!(!parameter.read_only);
        assert!(!parameter.display.is_empty());
    }

    for (name, value) in [
        ("KICK S1 Level", 0.0),
        ("KICK S2 Level", 0.0),
        ("KICK Synth Level", 0.9),
        ("KICK Noise Level", 0.0),
        ("KICK Volume", 0.8),
    ] {
        spec.initial_values.insert(parameter_id(&spec, name), value);
    }
    spec.midi_channels = [10, 10, 10, 10, 5, 6, 7, 8];

    let render = |spec: &vst3::Spec, frequency: f64| {
        let mut configured = spec.clone();
        configured
            .initial_values
            .insert(parameter_id(&configured, "KICK Synth Freq"), frequency);
        let mut values = configured
            .parameters
            .iter()
            .map(|parameter| configured.initial_value(parameter))
            .collect::<Vec<_>>();
        let frames = 1_024;
        let pitch = vec![65.406_391; frames]; // GM kick note 36
        let velocity = vec![0.9; frames];
        let tempo = vec![124.0; frames];
        let extra = vec![0.0; frames * vst3::EXTRA_NOTE_INPUTS];
        let mut output = Vec::with_capacity(frames * 2 * 8);
        let mut plugin = Instance::new(&configured, 48_000.0).expect("instantiate PULSE");
        for block in 0..8 {
            let gate = vec![if block % 2 == 0 { 1.0 } else { 0.0 }; frames];
            let mut block_output = vec![0.0; frames * 2];
            if block == 4 {
                values[configured
                    .parameters
                    .iter()
                    .position(|parameter| parameter.id == parameter_id(&configured, "KICK Env2 D"))
                    .unwrap()] = 0.8;
            }
            plugin.process(
                [&pitch, &gate, &velocity, &tempo, &extra],
                &values,
                &mut block_output,
            );
            assert!(block_output.iter().all(|sample| sample.is_finite()));
            output.extend(block_output);
        }
        assert!(!plugin.failed());
        let state = plugin.save_state().expect("save PULSE state");
        (output, state)
    };

    let (low, state) = render(&spec, 0.12);
    let (high, _) = render(&spec, 0.78);
    assert!(energy(&low) > 0.001, "PULSE GM kick produced silence");
    assert!(
        energy(&high) > 0.001,
        "automated PULSE kick produced silence"
    );
    let difference = low
        .iter()
        .zip(&high)
        .map(|(left, right)| (left - right).abs())
        .sum::<f64>();
    assert!(
        difference > 1.0,
        "PULSE frequency automation had no DSP effect"
    );
    assert!(!state.is_empty());
    spec.state = Some(state);
    drop(Instance::new(&spec, 48_000.0).expect("restore PULSE state"));
}

#[test]
fn installed_geonkick_loads_timekit_processes_all_buses_and_restores_state() {
    const GEONKICK_ID: &str = "E2CC953290E863F06405E13924E34B24";
    if std::env::var_os("MELDRITCH_TEST_GEONKICK").is_none() {
        eprintln!("set MELDRITCH_TEST_GEONKICK=1 to run the installed Geonkick test");
        return;
    }
    let home = PathBuf::from(std::env::var_os("HOME").expect("HOME"));
    let plugins = home.join(".local/lib/vst3");
    let kit = home.join(".local/share/geonkick/presets/TimeKit/TimeKit.gkit");
    let mut spec = vst3::load(GEONKICK_ID, &[plugins]).expect("inspect installed Geonkick");
    assert_eq!(spec.descriptor.name, "Geonkick");
    assert_eq!(spec.descriptor.vendor, "Iurie Nistor");
    assert_eq!(spec.descriptor.version, "3.9.0");
    assert_eq!(spec.descriptor.category, "Instrument|Synth");
    assert!(spec.descriptor.instrument);
    assert!(
        spec.parameters.is_empty(),
        "Geonkick 3.9 exposes kit state, not VST parameters"
    );
    spec.state = Some(std::fs::read(kit).expect("read TimeKit state"));
    spec.activate_all_outputs = true;
    let frames = 1024;
    let pitch = vec![32.703_196; frames]; // MIDI 24, TimeKit kick
    let gate = vec![1.0; frames];
    let velocity = vec![1.0; frames];
    let tempo = vec![124.0; frames];
    let extra = vec![0.0; frames * vst3::EXTRA_NOTE_INPUTS];
    let mut output = vec![0.0; frames * 2];
    let mut plugin = Instance::new(&spec, 48_000.0).expect("instantiate Geonkick");
    let mut total = 0.0;
    for _ in 0..8 {
        plugin.process([&pitch, &gate, &velocity, &tempo, &extra], &[], &mut output);
        assert!(output.iter().all(|sample| sample.is_finite()));
        total += energy(&output);
    }
    assert!(total > 0.001, "TimeKit kick produced silence");
    let state = plugin.save_state().expect("save Geonkick state");
    assert!(!state.is_empty());
    drop(plugin);
    let mut restored = spec.clone();
    restored.state = Some(state);
    drop(Instance::new(&restored, 48_000.0).expect("restore Geonkick state"));
}

#[test]
fn installed_serum_supports_real_notes_parameters_audio_and_teardown() {
    const SERUM_ID: &str = "56534558667350736572756D20320000";
    let Some(home) = std::env::var_os("HOME") else {
        return;
    };
    let root = PathBuf::from(home).join(".vst3");
    if !root.join("Serum2.vst3").is_dir() {
        eprintln!("Serum 2 is not installed; skipping conditional integration test");
        return;
    }
    let mut spec = vst3::load(SERUM_ID, &[root]).expect("inspect installed Serum 2");
    assert_eq!(spec.descriptor.name, "Serum 2");
    assert_eq!(spec.descriptor.vendor, "Xfer Records");
    assert!(spec.descriptor.instrument);
    assert!(spec.parameters.len() >= 2_600);
    for (id, name) in [
        (0, "Main Vol"),
        (1_000_039, "A WT Pos"),
        (2_000_003, "Filter 1 Freq"),
        (2_000_004, "Filter 1 Res"),
        (7_000_000, "Macro 1"),
    ] {
        assert_eq!(
            spec.parameters
                .iter()
                .find(|parameter| parameter.id == id)
                .map(|parameter| parameter.name.as_str()),
            Some(name),
            "Serum parameter metadata changed for stable ID {id}"
        );
    }
    spec.initial_values.insert(0, 0.5);
    spec.initial_values.insert(2_000_000, 1.0);
    spec.initial_values.insert(2_000_001, 1.0);
    spec.initial_values.insert(2_000_003, 0.62);
    let values = spec
        .parameters
        .iter()
        .map(|parameter| spec.initial_value(parameter))
        .collect::<Vec<_>>();
    let frames = 1_024;
    let pitch = vec![261.625_565; frames];
    let gate = vec![1.0; frames];
    let velocity = vec![0.85; frames];
    let tempo = vec![116.0; frames];
    let mut extra = vec![0.0; frames * vst3::EXTRA_NOTE_INPUTS];
    extra[..frames].fill(329.627_557);
    extra[frames..frames * 2].fill(1.0);
    extra[frames * 2..frames * 3].fill(0.72);
    let mut serum = Instance::new(&spec, 48_000.0).expect("instantiate Serum 2");
    let mut output = vec![0.0; frames * 2];
    let mut total_energy = 0.0;
    for _ in 0..6 {
        serum.process(
            [&pitch, &gate, &velocity, &tempo, &extra],
            &values,
            &mut output,
        );
        assert!(output.iter().all(|sample| sample.is_finite()));
        total_energy += energy(&output);
    }
    assert!(!serum.failed());
    assert!(
        total_energy > 0.001,
        "Serum produced silence for a two-note chord"
    );
    drop(serum);
}

#[test]
fn native_plugin_vertical_slice_covers_discovery_lifecycle_events_parameters_and_state() {
    let directory = fixture_directory().clone();
    let paths = [directory];
    let discovered = vst3::discover(&paths).expect("discover fixture");
    assert_eq!(discovered.len(), 1);
    let descriptor = &discovered[0];
    assert_eq!(descriptor.name, "Meldritch Test Instrument");
    assert_eq!(descriptor.vendor, "Meldritch");
    assert!(descriptor.instrument);
    assert_eq!(descriptor.class_id.len(), 32);

    let mut spec = vst3::load(&descriptor.class_id, &paths).expect("inspect fixture");
    assert_eq!(spec.parameters.len(), 1);
    assert_eq!(spec.parameters[0].id, 0);
    assert_eq!(spec.parameters[0].name, "Gain");
    assert!(spec.parameters[0].automatable);
    spec.initial_values.insert(0, 0.5);

    let frames = 128;
    let pitch = vec![440.0; frames];
    let mut gate = vec![0.0; frames];
    gate[17..80].fill(1.0);
    let velocity = vec![0.8; frames];
    let tempo = vec![120.0; frames];
    let extra_notes = vec![0.0; frames * vst3::EXTRA_NOTE_INPUTS];
    let mut output = vec![f64::NAN; frames * 2];
    let mut plugin = Instance::new(&spec, 48_000.0).expect("create fixture instance");
    plugin.process(
        [&pitch, &gate, &velocity, &tempo, &extra_notes],
        &[0.5],
        &mut output,
    );
    assert!(!plugin.failed());
    assert_eq!(energy(&output[..17 * 2]), 0.0);
    assert!(energy(&output[18 * 2..80 * 2]) > 0.001);
    assert_eq!(energy(&output[80 * 2..]), 0.0);

    let state = plugin.save_state().expect("save plugin state");
    assert!(!state.is_empty());
    drop(plugin);
    let mut restored = spec.clone();
    restored.initial_values.clear();
    restored.state = Some(state);
    let mut plugin = Instance::new(&restored, 48_000.0).expect("restore plugin state");
    let full_gate = vec![1.0; frames];
    let mut restored_output = vec![0.0; frames * 2];
    plugin.process(
        [&pitch, &full_gate, &velocity, &tempo, &extra_notes],
        &[restored.parameters[0].normalized],
        &mut restored_output,
    );
    assert!(energy(&restored_output) > 0.001);

    for _ in 0..8 {
        drop(Instance::new(&spec, 48_000.0).expect("repeat instance creation"));
    }
    assert!(vst3::resolve("missing-plugin-id", &paths).is_err());
}

#[test]
fn lua_sequences_plugin_through_existing_graph_effect_and_offline_renderer() {
    let directory = fixture_directory();
    let root = directory.to_string_lossy().replace('\\', "\\\\");
    let script = format!(
        r#"
local instrument = meldritch.instrument("VST3 fixture demo")
local sequence = instrument:sequencer {{ bpm = 120, steps = 8, steps_per_beat = 4 }}
local notes = sequence:track("fixture")
local pattern = sequence:pattern("notes")
pattern:notes("fixture", {{
  {{ step = 1, note = 69, velocity = 0.8, gate = 0.5 }},
  {{ step = 3, note = 72, velocity = 0.7, gate = 0.25 }},
}})
local synth = instrument:vst3 {{
  plugin = "Meldritch Test Instrument",
  paths = {{ "{root}" }},
}}
synth:set_parameter(0, 0.65)
local left = instrument:drive {{ amount = 1.5, output_gain = 0.8 }}
local right = instrument:drive {{ amount = 1.5, output_gain = 0.8 }}
instrument:connect(notes.pitch, synth.pitch)
instrument:connect(notes.gate, synth.gate)
instrument:connect(notes.velocity, synth.velocity)
instrument:connect(sequence.tempo, synth.tempo)
instrument:connect(synth.left, left.input)
instrument:connect(synth.right, right.input)
instrument:connect(left.out, instrument.output.left)
instrument:connect(right.out, instrument.output.right)
return instrument
"#
    );
    let graph = lua::read_with_root(&script, directory).expect("load Lua VST3 composition");
    let machine = compile_at_rate(&graph, 48_000.0).expect("compile VST3 graph");
    assert!(machine.parameter_ids().any(|id| id.name == "parameter_0"));
    let gain = machine
        .parameter_handle(&ParameterId::new("instrument/vst3_1", "parameter_0"))
        .expect("generic plugin parameter handle");
    assert_eq!(gain.effective_value(), 0.65);
    let unsupported = compile_at_rate(&graph, 48_000.0).expect("compile second VST3 graph");
    assert!(
        OfflineRenderer::new(unsupported, 1025, 1)
            .err()
            .expect("oversized VST3 block must be rejected")
            .contains("block sizes up to 1024")
    );

    let mut renderer = OfflineRenderer::new(machine, 1024, 2).expect("offline renderer");
    let mut left = vec![0.0; 48_000];
    let mut right = vec![0.0; 48_000];
    for (left, right) in left.chunks_mut(1024).zip(right.chunks_mut(1024)) {
        renderer.process_stereo(left, right);
    }
    assert!(energy(&left) > 0.01);
    assert!(energy(&right) > 0.01);
    assert!(left.iter().all(|sample| sample.is_finite()));
    assert!(right.iter().all(|sample| sample.is_finite()));
}
