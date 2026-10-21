pub mod compiler;
pub mod contract;
pub mod device;
mod expand;
pub mod live;
pub mod live_notes;
pub mod lua;
pub mod midi;
pub mod midi_clock;
pub mod midi_device;
pub mod model;
pub mod panel;
pub mod poly;
pub mod sample;
mod screen_panel;
pub mod sequencer;
pub mod timed;
pub mod tui;
mod units;

#[cfg(test)]
mod tests {
    use crate::{
        compiler::{Host, compile, compile_at_rate},
        contract::{self, FanIn, Role},
        live::LiveRuntime,
        live_notes::{PushError, live_note_queue},
        lua,
        midi::{MidiRouter, TimedMidiError, TimedMidiRouter},
        midi_clock::{ClockError, ClockStatus, FrameStamp, MidiFrameClock},
        model::ParameterId,
        poly::{PolyNoteEvent, PolyNotePlan},
        timed::{
            MAX_EVENTS, MAX_FRAMES_PER_RENDER, TimedNotePlan, TimedNoteState, TimedRenderError,
            TimedVelocityState,
        },
    };

    const BASIC: &str = include_str!("../examples/01-basic.lua");
    const TWO: &str = include_str!("../examples/02-two-gains.lua");
    const REORDERED: &str = include_str!("../examples/03-reordered.lua");
    const CYCLE: &str = include_str!("../examples/04-cycle.lua");
    const MIX: &str = include_str!("../examples/05-mix.lua");
    const MIX_MISSING: &str = include_str!("../examples/06-mix-missing.lua");
    const MIX_REORDERED: &str = include_str!("../examples/07-mix-reordered.lua");
    const SINE: &str = include_str!("../examples/08-sine.lua");
    const CONTROLS: &str = include_str!("../examples/09-controls.lua");
    const MIDI: &str = include_str!("../examples/10-midi-control.lua");
    const MULTIPLY: &str = include_str!("../examples/11-audio-multiply.lua");
    const MULTIPLY_MISSING: &str = include_str!("../examples/12-audio-multiply-missing.lua");
    const TWO_SINES_MULTIPLY: &str = include_str!("../examples/13-two-sines-multiply.lua");
    const GAIN_FAN_IN: &str = include_str!("../examples/14-gain-fan-in.lua");
    const OUTPUT_FAN_IN_INVALID: &str = include_str!("../examples/15-output-fan-in-invalid.lua");
    const AFFINE: &str = include_str!("../examples/16-affine.lua");
    const MODULATED_GAIN: &str = include_str!("../examples/17-gain-modulated.lua");
    const MODULATED_GAIN_MISSING: &str = include_str!("../examples/18-gain-mod-missing-input.lua");
    const AFFINE_CONTROLS: &str = include_str!("../examples/19-affine-controls.lua");
    const SINE_FM: &str = include_str!("../examples/20-sine-fm.lua");
    const SINE_FM_CYCLE: &str = include_str!("../examples/21-sine-fm-cycle.lua");
    const SHARED_MODULATOR: &str = include_str!("../examples/22-shared-modulator.lua");
    const LAG: &str = include_str!("../examples/23-lag.lua");
    const LAG_INVALID: &str = include_str!("../examples/24-lag-invalid.lua");
    const LAG_CONTROL_INVALID: &str = include_str!("../examples/25-lag-control-invalid.lua");
    const SMOOTHED_CONTROL: &str = include_str!("../examples/26-smoothed-control.lua");
    const SWITCH: &str = include_str!("../examples/27-switch.lua");
    const SWITCH_MISSING: &str = include_str!("../examples/28-switch-missing.lua");
    const SWITCH_CONTROL: &str = include_str!("../examples/29-switch-control.lua");
    const MIRROR_OUTPUTS: &str = include_str!("../examples/30-mirror-outputs.lua");
    const SAMPLE_HOLD: &str = include_str!("../examples/31-sample-hold.lua");
    const SAMPLE_HOLD_MISSING: &str = include_str!("../examples/32-sample-hold-missing.lua");
    const HELD_PEAK_SELECTOR: &str = include_str!("../examples/33-held-peak-selector.lua");
    const GATED_ENVELOPE: &str = include_str!("../examples/34-gated-envelope.lua");
    const ENVELOPE_MISSING_GATE: &str = include_str!("../examples/35-envelope-missing-gate.lua");
    const ENVELOPE_INVALID_ATTACK: &str =
        include_str!("../examples/36-envelope-invalid-attack.lua");
    const GATED_SINE_VOICE: &str = include_str!("../examples/37-gated-sine-voice.lua");
    const MIDI_NOTE_VOICE: &str = include_str!("../examples/38-midi-note-voice.lua");
    const MIDI_NOTE_MISSING_CONTROL: &str =
        include_str!("../examples/39-midi-note-missing-control.lua");
    const MIDI_NOTE_NARROW_PITCH: &str = include_str!("../examples/40-midi-note-narrow-pitch.lua");
    const CABLED_MIDI_NOTE_PAIR: &str = include_str!("../examples/41-cabled-midi-note-pair.lua");
    const MIDI_NOTE_SHARED_TARGET: &str =
        include_str!("../examples/42-midi-note-shared-target.lua");
    const TIMED_NOTE_VOICE: &str = include_str!("../examples/43-timed-note-voice.lua");
    const TIMED_NOTE_NO_BINDING: &str = include_str!("../examples/44-timed-note-no-binding.lua");
    const MIDI_TIME_ANCHOR: &str = include_str!("../examples/45-midi-time-anchor.lua");
    const LIVE_NOTE_QUEUE: &str = include_str!("../examples/46-live-note-queue.lua");
    const NORMALIZED_MODULATION: &str = include_str!("../examples/54-normalized-modulation.lua");
    const TWO_SUBPATCH_VOICES: &str = include_str!("../examples/55-two-subpatch-voices.lua");
    const SUBPATCH_INVALID_ENDPOINT: &str =
        include_str!("../examples/56-subpatch-invalid-endpoint.lua");
    const NESTED_SUBPATCH: &str = include_str!("../examples/57-nested-subpatch.lua");
    const SUBPATCH_PARAMETERS: &str = include_str!("../examples/58-subpatch-parameters.lua");
    const SUBPATCH_INSTANCE_VALUES: &str =
        include_str!("../examples/59-subpatch-instance-values.lua");
    const TWO_MIDI_SUBPATCH_VOICES: &str =
        include_str!("../examples/60-two-midi-subpatch-voices.lua");
    const PREPARED_VELOCITY: &str = include_str!("../examples/61-prepared-velocity.lua");
    const SUBPATCH_INPUT_FANOUT: &str = include_str!("../examples/62-subpatch-input-fanout.lua");
    const TWO_VOICE_POLY: &str = include_str!("../examples/63-two-voice-poly.lua");
    const FOUR_VOICE_POLY: &str = include_str!("../examples/64-four-voice-poly.lua");
    const FOUR_VOICE_STEAL: &str = include_str!("../examples/65-four-voice-steal-oldest.lua");

    fn render(script: &str) -> (Vec<String>, [f64; 8]) {
        let graph = lua::read(script).unwrap();
        let mut machine = compile(&graph).unwrap();
        let order = machine.order().to_vec();
        let mut buffer = [f64::NAN; 8];
        machine.process(&mut buffer);
        (order, buffer)
    }

    #[test]
    fn contract_discovery_and_port_resolution() {
        assert_eq!(contract::all().len(), 23);
        let gain = contract::find("gain").unwrap();
        assert_eq!(gain.role, Role::Processor);
        assert_eq!(gain.inputs[0].name, "input");
        assert_eq!(gain.inputs[0].fan_in, FanIn::Sum);
        assert_eq!(gain.inputs[0].unconnected, None);
        assert_eq!(gain.inputs[1].name, "mod");
        assert_eq!(gain.inputs[1].unconnected, Some(1.0));
        assert_eq!(gain.outputs[0].name, "out");
        assert_eq!(gain.params[0].name, "gain");
        let mix = contract::find("mix").unwrap();
        assert_eq!(
            mix.inputs.iter().map(|port| port.name).collect::<Vec<_>>(),
            ["left", "right"]
        );
        let affine = contract::find("affine").unwrap();
        assert_eq!(
            affine
                .params
                .iter()
                .map(|param| param.name)
                .collect::<Vec<_>>(),
            ["scale", "offset"]
        );
        let sine = contract::find("sine").unwrap();
        assert_eq!(sine.inputs[0].name, "fm_hz");
        assert_eq!(sine.inputs[0].unconnected, Some(0.0));
        let lag = contract::find("lag").unwrap();
        assert_eq!(lag.params[0].name, "response");
        assert_eq!(lag.params[0].range, Some((0.0, 1.0)));
        let switch = contract::find("switch").unwrap();
        assert_eq!(
            switch
                .inputs
                .iter()
                .map(|port| port.name)
                .collect::<Vec<_>>(),
            ["left", "right", "select"]
        );
        assert_eq!(switch.params[0].name, "threshold");
        let mirror = contract::find("mirror").unwrap();
        assert_eq!(
            mirror
                .outputs
                .iter()
                .map(|port| port.name)
                .collect::<Vec<_>>(),
            ["out", "inverted"]
        );
        let sample_hold = contract::find("sample_hold").unwrap();
        assert_eq!(
            sample_hold
                .inputs
                .iter()
                .map(|port| port.name)
                .collect::<Vec<_>>(),
            ["signal", "trigger"]
        );
        assert_eq!(sample_hold.params[0].name, "threshold");
        let envelope = contract::find("envelope").unwrap();
        assert_eq!(envelope.inputs[0].name, "gate");
        assert_eq!(envelope.params[0].name, "attack");
        assert_eq!(envelope.params[1].name, "release");
        assert_eq!(contract::find("output").unwrap().role, Role::Output);
        let error = lua::read(BASIC.replace("gain.input", "gain.missing").as_str()).unwrap_err();
        assert!(
            error.contains("destination must be an input port"),
            "{error}"
        );
    }

    #[test]
    fn basic_graph_and_dependency_order() {
        let (order, samples) = render(BASIC);
        assert_eq!(order, ["source", "gain", "output"]);
        assert_eq!(samples, [0.125; 8]);
    }

    #[test]
    fn second_gain_needs_no_topology_case() {
        let (order, samples) = render(TWO);
        assert_eq!(order, ["source", "gain_a", "gain_b", "output"]);
        assert_eq!(samples, [0.0625; 8]);
    }

    #[test]
    fn declaration_order_is_irrelevant() {
        assert_eq!(render(BASIC), render(REORDERED));
    }

    #[test]
    fn live_parameter_identity_and_boundary() {
        let mut machine = compile(&lua::read(BASIC).unwrap()).unwrap();
        let id = ParameterId::new("gain", "gain");
        assert!(machine.parameter_ids().any(|candidate| candidate == &id));
        assert_eq!(machine.authored_value(&id), Some(0.5));
        let mut samples = [0.0; 4];
        machine.process(&mut samples);
        assert_eq!(samples, [0.125; 4]);
        machine.set_override(&id, Some(0.75)).unwrap();
        assert_eq!(machine.authored_value(&id), Some(0.5));
        assert_eq!(machine.effective_value(&id), Some(0.75));
        machine.process(&mut samples);
        assert_eq!(samples, [0.1875; 4]);
        machine.set_override(&id, None).unwrap();
        machine.process(&mut samples);
        assert_eq!(samples, [0.125; 4]);
    }

    #[test]
    fn rejected_candidate_preserves_accepted_machine() {
        let mut host = Host::default();
        host.submit(&lua::read(BASIC).unwrap()).unwrap();
        let error = host.submit(&lua::read(CYCLE).unwrap()).unwrap_err();
        assert!(error.contains("cycle"), "{error}");
        let mut samples = [0.0; 4];
        host.accepted().unwrap().process(&mut samples);
        assert_eq!(samples, [0.125; 4]);
        let mut invalid = lua::read(BASIC).unwrap();
        invalid.connections[0].from.port = "missing".into();
        assert!(
            host.submit(&invalid)
                .unwrap_err()
                .contains("invalid output port")
        );
        host.accepted().unwrap().process(&mut samples);
        assert_eq!(samples, [0.125; 4]);
    }

    #[test]
    fn graph_requires_exactly_one_contract_declared_output() {
        let graph = lua::read(BASIC).unwrap();
        let mut missing = graph.clone();
        missing.modules.retain(|module| module.kind != "output");
        missing
            .connections
            .retain(|edge| edge.to.module != "output");
        assert!(compile(&missing).unwrap_err().contains("found 0"));

        let mut duplicate = graph;
        let mut second = duplicate
            .modules
            .iter()
            .find(|module| module.kind == "output")
            .unwrap()
            .clone();
        second.id = "output_two".into();
        duplicate.modules.push(second);
        let mut extra_cable = duplicate.connections.last().unwrap().clone();
        extra_cable.to.module = "output_two".into();
        duplicate.connections.push(extra_cable);
        assert!(compile(&duplicate).unwrap_err().contains("found 2"));
    }

    #[test]
    fn invalid_parameters_and_fan_in_are_diagnosed() {
        let mut graph = lua::read(BASIC).unwrap();
        graph.modules[1].values.insert("unknown".into(), 1.0);
        assert!(compile(&graph).unwrap_err().contains("not a parameter"));
        graph.modules[1].values.remove("unknown");
        graph.modules[1].values.insert("gain".into(), f64::NAN);
        assert!(compile(&graph).unwrap_err().contains("must be finite"));
        graph.modules[1].values.insert("gain".into(), 0.5);
        graph.connections.push(graph.connections[1].clone());
        assert!(
            compile(&graph)
                .unwrap_err()
                .contains("permits one connection")
        );
    }

    #[test]
    fn stable_parameter_id_across_reordered_construction() {
        let id = ParameterId::new("gain", "gain");
        let first = compile(&lua::read(BASIC).unwrap()).unwrap();
        let second = compile(&lua::read(REORDERED).unwrap()).unwrap();
        assert_eq!(first.authored_value(&id), second.authored_value(&id));
        assert_eq!(first.effective_value(&id), second.effective_value(&id));
    }

    #[test]
    fn independent_sources_compose_through_mix() {
        let (order, samples) = render(MIX);
        assert_eq!(order, ["left", "right", "mix", "gain", "output"]);
        assert_eq!(samples, [0.1875; 8]);
        let (order, samples) = render(MIX_REORDERED);
        assert_eq!(order, ["left", "right", "mix", "gain", "output"]);
        assert_eq!(samples, [0.1875; 8]);
    }

    #[test]
    fn independent_audio_signals_multiply_across_buffers() {
        let graph = lua::read(MULTIPLY).unwrap();
        let mut machine = compile_at_rate(&graph, 48_000.0).unwrap();
        assert_eq!(
            machine.order(),
            ["amplitude", "oscillator", "product", "output"]
        );
        let mut first = [f64::NAN; 3];
        let mut second = [f64::NAN; 5];
        machine.process(&mut first);
        machine.process(&mut second);
        for (actual, expected) in first
            .into_iter()
            .chain(second)
            .zip([0.0, 0.25, 0.0, -0.25, 0.0, 0.25, 0.0, -0.25])
        {
            assert!((actual - expected).abs() < 1e-12, "{actual} != {expected}");
        }
    }

    #[test]
    fn disconnected_multiply_rejects_without_replacing_accepted_machine() {
        let mut host = Host::default();
        host.submit(&lua::read(MULTIPLY).unwrap()).unwrap();
        let error = host
            .submit(&lua::read(MULTIPLY_MISSING).unwrap())
            .unwrap_err();
        assert!(error.contains("product.right' is disconnected"), "{error}");
        let mut samples = [f64::NAN; 4];
        host.accepted().unwrap().process(&mut samples);
        for (actual, expected) in samples.into_iter().zip([0.0, 0.25, 0.0, -0.25]) {
            assert!((actual - expected).abs() < 1e-12, "{actual} != {expected}");
        }
    }

    #[test]
    fn two_time_varying_signals_multiply_per_sample() {
        let graph = lua::read(TWO_SINES_MULTIPLY).unwrap();
        let mut machine = compile_at_rate(&graph, 48_000.0).unwrap();
        let mut samples = [f64::NAN; 4];
        machine.process(&mut samples);
        let peak = std::f64::consts::FRAC_1_SQRT_2;
        for (actual, expected) in samples.into_iter().zip([0.0, peak, 0.0, -peak]) {
            assert!((actual - expected).abs() < 1e-12, "{actual} != {expected}");
        }
    }

    #[test]
    fn declared_gain_fan_in_sums_cables_independent_of_author_order() {
        let graph = lua::read(GAIN_FAN_IN).unwrap();
        let mut reordered = graph.clone();
        reordered.modules.reverse();
        reordered.connections.reverse();
        let mut first = compile(&graph).unwrap();
        let mut second = compile(&reordered).unwrap();
        assert_eq!(first.order(), ["left", "right", "gain", "output"]);
        assert_eq!(first.order(), second.order());
        let mut first_samples = [f64::NAN; 8];
        let mut second_samples = [f64::NAN; 8];
        first.process(&mut first_samples);
        second.process(&mut second_samples);
        assert_eq!(first_samples, [0.1875; 8]);
        assert_eq!(first_samples, second_samples);
    }

    #[test]
    fn authored_normalization_is_overridden_and_restored_by_graph_revision() {
        let normalized = lua::read(NORMALIZED_MODULATION).unwrap();
        let overridden = lua::read(
            &NORMALIZED_MODULATION
                .replace("local use_override = false", "local use_override = true"),
        )
        .unwrap();
        assert_eq!(normalized.normalizations.len(), 1);
        assert_eq!(overridden.normalizations, normalized.normalizations);
        assert_eq!(normalized.connections.len(), 2);
        assert_eq!(overridden.connections.len(), 3);

        let mut host = Host::default();
        host.submit(&normalized).unwrap();
        let machine = host.accepted().unwrap();
        assert_eq!(
            machine.authored_normalizations(),
            &normalized.normalizations
        );
        assert!(
            machine
                .effective_connections()
                .contains(&normalized.normalizations[0])
        );
        let mut samples = [f64::NAN; 4];
        machine.process(&mut samples);
        assert_eq!(samples, [0.25; 4]);

        host.submit(&overridden).unwrap();
        let machine = host.accepted().unwrap();
        assert_eq!(
            machine.authored_normalizations(),
            &normalized.normalizations
        );
        assert_eq!(machine.effective_connections(), &overridden.connections);
        machine.process(&mut samples);
        assert_eq!(samples, [0.125; 4]);

        host.submit(&normalized).unwrap();
        let machine = host.accepted().unwrap();
        assert!(
            machine
                .effective_connections()
                .contains(&normalized.normalizations[0])
        );
        machine.process(&mut samples);
        assert_eq!(samples, [0.25; 4]);

        let mut disconnected = normalized.clone();
        disconnected.normalizations.clear();
        host.submit(&disconnected).unwrap();
        host.accepted().unwrap().process(&mut samples);
        assert_eq!(samples, [0.5; 4]);
    }

    #[test]
    fn normalization_resolution_uses_port_contracts_across_module_types() {
        let script = r#"
local graph = meldritch.graph()
local normal = graph:module("normal", "constant", { value = 0.5 })
local override = graph:module("override", "constant", { value = 0.125 })
local right = graph:module("right", "constant", { value = 0.25 })
local mix = graph:module("mix", "mix")
local output = graph:module("output", "output")
graph:normalize(normal.out, mix.left)
graph:connect(right.out, mix.right)
graph:connect(mix.out, output.input)
return graph
"#;
        let graph = lua::read(script).unwrap();
        let mut normal = compile(&graph).unwrap();
        let mut samples = [f64::NAN; 4];
        normal.process(&mut samples);
        assert_eq!(samples, [0.75; 4]);
        let mut overridden = graph.clone();
        overridden.connections.push(crate::model::Connection {
            from: crate::model::Endpoint {
                module: "override".into(),
                port: "out".into(),
            },
            to: graph.normalizations[0].to.clone(),
        });
        let mut machine = compile(&overridden).unwrap();
        assert_eq!(machine.effective_connections(), &overridden.connections);
        machine.process(&mut samples);
        assert_eq!(samples, [0.375; 4]);

        // A declared Sum input still sums explicit cables; normalization is
        // suppressed for the entire destination, not added as another source.
        let mut summed = lua::read(NORMALIZED_MODULATION).unwrap();
        summed.normalizations[0].to.port = "input".into();
        summed.connections.push(crate::model::Connection {
            from: crate::model::Endpoint {
                module: "override".into(),
                port: "out".into(),
            },
            to: summed.normalizations[0].to.clone(),
        });
        let mut machine = compile(&summed).unwrap();
        assert_eq!(machine.effective_connections(), &summed.connections);
        machine.process(&mut samples);
        assert_eq!(samples, [0.75; 4]);
    }

    #[test]
    fn invalid_normalizations_reject_candidate_and_preserve_machine() {
        let base = lua::read(NORMALIZED_MODULATION).unwrap();
        let overridden = lua::read(
            &NORMALIZED_MODULATION
                .replace("local use_override = false", "local use_override = true"),
        )
        .unwrap();
        let mut cases = Vec::new();

        let mut duplicate = overridden.clone();
        duplicate.normalizations[0].to.port = "input".into();
        duplicate
            .normalizations
            .push(duplicate.normalizations[0].clone());
        cases.push((duplicate, "ambiguous normalization"));

        let mut unknown = overridden.clone();
        unknown.normalizations[0].from.module = "missing".into();
        cases.push((unknown, "unknown source module"));

        let mut unknown_destination = overridden.clone();
        unknown_destination.normalizations[0].to.module = "missing".into();
        cases.push((unknown_destination, "unknown destination module"));

        let mut wrong_source_port = overridden.clone();
        wrong_source_port.normalizations[0].from.port = "input".into();
        cases.push((wrong_source_port, "invalid output port"));

        let mut wrong_destination_port = overridden.clone();
        wrong_destination_port.normalizations[0].to.port = "out".into();
        cases.push((wrong_destination_port, "invalid input port"));

        let mut cycle = base.clone();
        cycle.normalizations[0].from.module = "gain".into();
        cycle.normalizations[0].to.port = "input".into();
        cycle.connections.remove(0);
        cases.push((cycle, "cycle"));

        let mut disconnected = base.clone();
        disconnected.connections.remove(0);
        cases.push((disconnected, "is disconnected"));

        let mut explicit_fan_in = base.clone();
        explicit_fan_in.connections.push(crate::model::Connection {
            from: crate::model::Endpoint {
                module: "override".into(),
                port: "out".into(),
            },
            to: base.normalizations[0].to.clone(),
        });
        explicit_fan_in.connections.push(crate::model::Connection {
            from: crate::model::Endpoint {
                module: "source".into(),
                port: "out".into(),
            },
            to: base.normalizations[0].to.clone(),
        });
        cases.push((explicit_fan_in, "permits one connection"));

        let mut host = Host::default();
        host.submit(&base).unwrap();
        for (candidate, expected) in cases {
            let error = host.submit(&candidate).unwrap_err();
            assert!(error.contains(expected), "{error}");
            let machine = host.accepted().unwrap();
            assert_eq!(machine.authored_normalizations(), &base.normalizations);
            let mut samples = [f64::NAN; 4];
            machine.process(&mut samples);
            assert_eq!(samples, [0.25; 4]);
        }

        let direction_error = lua::read(&NORMALIZED_MODULATION.replace(
            "graph:normalize(normal.out, gain.mod)",
            "graph:normalize(gain.mod, normal.out)",
        ))
        .unwrap_err();
        assert!(direction_error.contains("source must be an output port"));
        let destination_direction_error = lua::read(&NORMALIZED_MODULATION.replace(
            "graph:normalize(normal.out, gain.mod)",
            "graph:normalize(normal.out, gain.out)",
        ))
        .unwrap_err();
        assert!(destination_direction_error.contains("destination must be an input port"));

        // The cycle is harmless while an explicit cable overrides its input.
        let mut dormant_cycle = base.clone();
        dormant_cycle.normalizations[0].from.module = "gain".into();
        dormant_cycle.normalizations[0].to.port = "input".into();
        assert!(compile(&dormant_cycle).is_ok());
    }

    #[test]
    fn two_instances_own_independent_oscillator_and_envelope_state() {
        let graph = lua::read(TWO_SUBPATCH_VOICES).unwrap();
        assert_eq!(graph.subpatches.len(), 1);
        assert_eq!(graph.instances.len(), 2);
        let mut machine = compile_at_rate(&graph, 48_000.0).unwrap();
        assert_eq!(machine.authored_normalizations().len(), 2);
        assert!(
            machine
                .effective_connections()
                .iter()
                .all(|edge| { !(edge.from.module.ends_with("/closed") && edge.to.port == "gate") })
        );
        assert!(
            machine
                .parameter_ids()
                .any(|id| id == &ParameterId::new("left/oscillator", "frequency"))
        );
        assert!(
            machine
                .parameter_ids()
                .any(|id| id == &ParameterId::new("right/oscillator", "frequency"))
        );

        let mut first = [f64::NAN; 4];
        machine.process(&mut first);
        let right_gate = machine
            .parameter_handle(&ParameterId::new("right_gate", "value"))
            .unwrap();
        right_gate.publish(Some(1.0)).unwrap();
        let mut second = [f64::NAN; 4];
        machine.process(&mut second);

        let mut left_phase: f64 = 0.0;
        let mut right_phase: f64 = 0.0;
        let mut left_envelope = 0.0;
        let mut right_envelope = 0.0;
        for (frame, actual) in first.into_iter().chain(second).enumerate() {
            left_envelope += 0.5 * (1.0 - left_envelope);
            let right_target = if frame < 4 { 0.0 } else { 1.0 };
            right_envelope += 0.5 * (right_target - right_envelope);
            let expected = 0.25
                * ((std::f64::consts::TAU * left_phase).sin() * left_envelope
                    + (std::f64::consts::TAU * right_phase).sin() * right_envelope);
            assert!(
                (actual - expected).abs() < 1e-12,
                "frame {frame}: {actual} != {expected}"
            );
            left_phase = (left_phase + 0.25).fract();
            right_phase = (right_phase + 0.125).fract();
        }

        // Omitting the right external Gate exposes that instance's authored
        // closed-Gate normalization without changing the left instance.
        let mut missing_gate = graph.clone();
        missing_gate
            .connections
            .retain(|edge| edge.to.module != "right" || edge.to.port != "gate");
        let mut machine = compile(&missing_gate).unwrap();
        assert!(machine.effective_connections().iter().any(|edge| {
            edge.from.module == "right/closed" && edge.to.module == "right/envelope"
        }));
        machine.process(&mut first);
        assert_eq!(first[1], 0.1875);
    }

    #[test]
    fn nested_subpatches_expand_to_the_same_primitive_compiler() {
        let graph = lua::read(NESTED_SUBPATCH).unwrap();
        let mut machine = compile(&graph).unwrap();
        assert!(machine.order().contains(&"left/inner/gain".to_string()));
        assert!(machine.order().contains(&"right/inner/gain".to_string()));
        let mut samples = [f64::NAN; 4];
        machine.process(&mut samples);
        assert_eq!(samples, [0.375; 4]);
    }

    #[test]
    fn nested_subpatch_parameters_bind_independent_controls() {
        let graph = lua::read(SUBPATCH_PARAMETERS).unwrap();
        let mut machine = compile(&graph).unwrap();
        let left = machine.control_handle("left-level").unwrap();
        let right = machine.control_handle("right-level").unwrap();
        assert_eq!(left.target(), &ParameterId::new("left/inner/gain", "gain"));
        assert_eq!(
            right.target(),
            &ParameterId::new("right/inner/gain", "gain")
        );
        let mut samples = [f64::NAN; 4];
        machine.process(&mut samples);
        assert_eq!(samples, [0.25; 4]);
        left.publish(1.0).unwrap();
        machine.process(&mut samples);
        assert_eq!(samples, [0.375; 4]);
        assert!(MidiRouter::from_machine(&machine).dispatch(&[0xB0, 7, 0]));
        machine.process(&mut samples);
        assert_eq!(samples, [0.25; 4]);
    }

    #[test]
    fn invalid_subpatch_parameter_exports_preserve_accepted_machine() {
        let valid = lua::read(SUBPATCH_PARAMETERS).unwrap();
        let mut host = Host::default();
        host.submit(&valid).unwrap();
        let mut invalid_target = valid.clone();
        invalid_target
            .subpatches
            .get_mut("amplifier")
            .unwrap()
            .parameters[0]
            .target
            .name = "missing".into();
        let mut duplicate = valid.clone();
        let parameters = &mut duplicate.subpatches.get_mut("wrapper").unwrap().parameters;
        parameters.push(parameters[0].clone());
        let mut alias = valid.clone();
        let parameters = &mut alias.subpatches.get_mut("wrapper").unwrap().parameters;
        let mut second = parameters[0].clone();
        second.name = "other-level".into();
        parameters.push(second);
        let mut hidden = valid.clone();
        hidden.controls[0].target.name = "gain".into();
        for (candidate, expected) in [
            (invalid_target, "invalid parameter"),
            (duplicate, "duplicate external parameter"),
            (alias, "duplicate external parameter target"),
            (hidden, "invalid exported parameter"),
        ] {
            let error = host.submit(&candidate).unwrap_err();
            assert!(error.contains(expected), "{error}");
            let mut samples = [f64::NAN; 4];
            host.accepted().unwrap().process(&mut samples);
            assert_eq!(samples, [0.25; 4]);
        }
    }

    #[test]
    fn instance_values_override_nested_defaults_independently() {
        let graph = lua::read(SUBPATCH_INSTANCE_VALUES).unwrap();
        let mut machine = compile(&graph).unwrap();
        let left = machine.control_handle("left-level").unwrap();
        assert_eq!(left.target(), &ParameterId::new("left/inner/gain", "gain"));
        let mut samples = [f64::NAN; 4];
        machine.process(&mut samples);
        assert_eq!(samples, [0.25; 4]);
        left.publish(1.0).unwrap();
        machine.process(&mut samples);
        assert_eq!(samples, [0.4375; 4]);
        left.clear().unwrap();
        machine.process(&mut samples);
        assert_eq!(samples, [0.25; 4]);

        let mut fallback = graph.clone();
        fallback.instances[0].values.clear();
        let mut fallback = compile(&fallback).unwrap();
        fallback.process(&mut samples);
        assert_eq!(samples, [0.2375; 4]);
    }

    #[test]
    fn invalid_instance_values_preserve_accepted_machine() {
        let valid = lua::read(SUBPATCH_INSTANCE_VALUES).unwrap();
        let mut host = Host::default();
        host.submit(&valid).unwrap();
        for (name, value, expected) in [
            ("missing", 1.0, "invalid authored parameter"),
            ("level", f64::NAN, "must be finite"),
            ("level", f64::INFINITY, "must be finite"),
        ] {
            let mut invalid = valid.clone();
            invalid.instances[0].values.insert(name.into(), value);
            let error = host.submit(&invalid).unwrap_err();
            assert!(error.contains(expected), "{error}");
            let mut samples = [f64::NAN; 4];
            host.accepted().unwrap().process(&mut samples);
            assert_eq!(samples, [0.25; 4]);
        }
        let mut masked_default = valid.clone();
        masked_default
            .subpatches
            .get_mut("amplifier")
            .unwrap()
            .graph
            .modules[0]
            .values
            .insert("gain".into(), f64::NAN);
        assert!(
            host.submit(&masked_default)
                .unwrap_err()
                .contains("must be finite")
        );

        let mut bounded = valid.clone();
        let amplifier = bounded.subpatches.get_mut("amplifier").unwrap();
        amplifier.graph.modules[0].kind = "lag".into();
        amplifier.graph.modules[0].values.clear();
        amplifier.parameters[0].target.name = "response".into();
        bounded.instances[0].values.insert("level".into(), 1.5);
        assert!(
            host.submit(&bounded)
                .unwrap_err()
                .contains("must be in 0..1")
        );
        let mut samples = [f64::NAN; 4];
        host.accepted().unwrap().process(&mut samples);
        assert_eq!(samples, [0.25; 4]);
    }

    #[test]
    fn two_subpatch_voices_route_timed_notes_by_channel() {
        let graph = lua::read(TWO_MIDI_SUBPATCH_VOICES).unwrap();
        let mut whole = compile_at_rate(&graph, 48_000.0).unwrap();
        assert_eq!(
            whole.authored_value(&ParameterId::new("left/level", "gain")),
            Some(0.25)
        );
        assert_eq!(
            whole.authored_value(&ParameterId::new("right/level", "gain")),
            Some(0.125)
        );
        let events = [
            TimedNoteState {
                frame: 2,
                channel: 1,
                pitch_hz: 12_000.0,
                gate: true,
            },
            TimedNoteState {
                frame: 4,
                channel: 2,
                pitch_hz: 6_000.0,
                gate: true,
            },
            TimedNoteState {
                frame: 6,
                channel: 1,
                pitch_hz: 12_000.0,
                gate: false,
            },
            TimedNoteState {
                frame: 8,
                channel: 2,
                pitch_hz: 6_000.0,
                gate: false,
            },
        ];
        let mut plan = TimedNotePlan::prepare(&whole, 0, &events).unwrap();
        let mut actual = [f64::NAN; 12];
        plan.render_next(&mut whole, &mut actual).unwrap();

        let mut left_phase = 0.0_f64;
        let mut right_phase = 0.0_f64;
        let mut left_envelope = 0.0_f64;
        let mut right_envelope = 0.0_f64;
        for (frame, sample) in actual.iter().enumerate() {
            let left_frequency = if frame >= 2 { 12_000.0 } else { 0.0 };
            let right_frequency = if frame >= 4 { 6_000.0 } else { 0.0 };
            let left_target = if (2..6).contains(&frame) { 1.0 } else { 0.0 };
            let right_target = if (4..8).contains(&frame) { 1.0 } else { 0.0 };
            let left_response = if left_target == 1.0 { 0.01 } else { 0.005 };
            let right_response = if right_target == 1.0 { 0.01 } else { 0.005 };
            left_envelope += left_response * (left_target - left_envelope);
            right_envelope += right_response * (right_target - right_envelope);
            let expected = 0.25 * (std::f64::consts::TAU * left_phase).sin() * left_envelope
                + 0.125 * (std::f64::consts::TAU * right_phase).sin() * right_envelope;
            assert!(
                (sample - expected).abs() < 1e-12,
                "frame {frame}: {sample} != {expected}"
            );
            left_phase = (left_phase + left_frequency / 48_000.0).fract();
            right_phase = (right_phase + right_frequency / 48_000.0).fract();
        }

        let mut split = compile_at_rate(&graph, 48_000.0).unwrap();
        let mut split_plan = TimedNotePlan::prepare(&split, 0, &events).unwrap();
        let mut partitioned = [f64::NAN; 12];
        split_plan
            .render_next(&mut split, &mut partitioned[..3])
            .unwrap();
        split_plan
            .render_next(&mut split, &mut partitioned[3..7])
            .unwrap();
        split_plan
            .render_next(&mut split, &mut partitioned[7..])
            .unwrap();
        assert_eq!(actual, partitioned);
    }

    #[test]
    fn invalid_subpatch_note_binding_preserves_accepted_voice() {
        let graph = lua::read(TWO_MIDI_SUBPATCH_VOICES).unwrap();
        let mut host = Host::default();
        host.submit(&graph).unwrap();
        let mut invalid = graph.clone();
        invalid.controls[0].target.name = "frequency".into();
        assert!(
            host.submit(&invalid)
                .unwrap_err()
                .contains("invalid exported parameter")
        );
        let router = MidiRouter::from_machine(host.accepted().unwrap());
        assert!(router.dispatch(&[0x90, 69, 100]));
        let mut samples = [f64::NAN; 4];
        host.accepted().unwrap().process(&mut samples);
        assert!(samples.iter().any(|sample| sample.abs() > 0.0));
    }

    #[test]
    fn exported_subpatch_input_fans_out_explicit_and_normalized_edges() {
        let graph = lua::read(SUBPATCH_INPUT_FANOUT).unwrap();
        let mut machine = compile(&graph).unwrap();
        assert_eq!(machine.authored_normalizations().len(), 2);
        for instance in ["left", "right"] {
            for destination in ["a", "b"] {
                assert!(machine.effective_connections().iter().any(|edge| {
                    edge.to.module == format!("{instance}/{destination}")
                        && edge.from.module == if instance == "left" { "high" } else { "low" }
                }));
            }
        }
        let mut samples = [f64::NAN; 4];
        machine.process(&mut samples);
        assert!(samples.iter().all(|sample| (sample - 0.45).abs() < 1e-12));

        let mut fallback_graph = graph.clone();
        fallback_graph
            .connections
            .retain(|edge| edge.to.module != "right");
        let mut fallback = compile(&fallback_graph).unwrap();
        assert!(
            fallback
                .effective_connections()
                .iter()
                .filter(|edge| {
                    edge.from.module == "fallback" && edge.to.module.starts_with("right/")
                })
                .count()
                == 2
        );
        fallback.process(&mut samples);
        assert!(samples.iter().all(|sample| (sample - 0.375).abs() < 1e-12));
    }

    #[test]
    fn invalid_subpatch_input_fanout_preserves_accepted_machine() {
        let valid = lua::read(SUBPATCH_INPUT_FANOUT).unwrap();
        let mut host = Host::default();
        host.submit(&valid).unwrap();
        let mut duplicate_target = valid.clone();
        let input = &mut duplicate_target
            .subpatches
            .get_mut("splitter")
            .unwrap()
            .inputs[0];
        input.destinations.push(input.destinations[0].clone());
        let mut invalid_target = valid.clone();
        invalid_target
            .subpatches
            .get_mut("splitter")
            .unwrap()
            .inputs[0]
            .destinations[1]
            .port = "missing".into();
        let mut empty = valid.clone();
        empty.subpatches.get_mut("splitter").unwrap().inputs[0]
            .destinations
            .clear();
        for (candidate, expected) in [
            (duplicate_target, "duplicate destination"),
            (invalid_target, "invalid input port"),
            (empty, "needs a name and destination"),
        ] {
            let error = host.submit(&candidate).unwrap_err();
            assert!(error.contains(expected), "{error}");
            let mut samples = [f64::NAN; 4];
            host.accepted().unwrap().process(&mut samples);
            assert!(samples.iter().all(|sample| (sample - 0.45).abs() < 1e-12));
        }
    }

    #[test]
    fn two_voice_poly_plan_assigns_releases_and_reuses_voices() {
        let graph = lua::read(TWO_VOICE_POLY).unwrap();
        let events = [
            PolyNoteEvent::Off {
                frame: 1,
                channel: 1,
                key: 60,
            },
            PolyNoteEvent::On {
                frame: 3,
                channel: 1,
                key: 69,
                velocity: 64,
            },
            PolyNoteEvent::On {
                frame: 4,
                channel: 1,
                key: 69,
                velocity: 32,
            },
            PolyNoteEvent::On {
                frame: 5,
                channel: 1,
                key: 81,
                velocity: 127,
            },
            PolyNoteEvent::Off {
                frame: 7,
                channel: 1,
                key: 69,
            },
            PolyNoteEvent::On {
                frame: 9,
                channel: 1,
                key: 72,
                velocity: 32,
            },
            PolyNoteEvent::On {
                frame: 11,
                channel: 1,
                key: 81,
                velocity: 0,
            },
            PolyNoteEvent::Off {
                frame: 13,
                channel: 1,
                key: 72,
            },
        ];
        let mut whole = compile_at_rate(&graph, 48_000.0).unwrap();
        let mut plan = PolyNotePlan::prepare(&whole, 0, &events).unwrap();
        let mut actual = [f64::NAN; 20];
        plan.render_next(&mut whole, &mut actual).unwrap();
        assert_eq!(plan.cursor(), 20);

        let mut left_phase = 0.0_f64;
        let mut right_phase = 0.0_f64;
        let mut left_envelope = 0.0_f64;
        let mut right_envelope = 0.0_f64;
        let note72 = 440.0 * 2.0_f64.powf((72.0 - 69.0) / 12.0);
        for (frame, sample) in actual.iter().enumerate() {
            let left_pitch = if frame >= 9 {
                note72
            } else if frame >= 3 {
                440.0
            } else {
                0.0
            };
            let right_pitch = if frame >= 5 { 880.0 } else { 0.0 };
            let left_gate = (3..7).contains(&frame) || (9..13).contains(&frame);
            let right_gate = (5..11).contains(&frame);
            left_envelope +=
                if left_gate { 0.01 } else { 0.005 } * (f64::from(left_gate) - left_envelope);
            right_envelope +=
                if right_gate { 0.01 } else { 0.005 } * (f64::from(right_gate) - right_envelope);
            let left_velocity = if frame >= 4 {
                32.0 / 127.0
            } else if frame >= 3 {
                64.0 / 127.0
            } else {
                1.0
            };
            let expected =
                0.25 * left_envelope * left_velocity * (std::f64::consts::TAU * left_phase).sin()
                    + 0.125 * right_envelope * (std::f64::consts::TAU * right_phase).sin();
            assert!(
                (sample - expected).abs() < 1e-12,
                "frame {frame}: {sample} != {expected}"
            );
            left_phase = (left_phase + left_pitch / 48_000.0).fract();
            right_phase = (right_phase + right_pitch / 48_000.0).fract();
        }

        let mut split = compile_at_rate(&graph, 48_000.0).unwrap();
        let mut split_plan = PolyNotePlan::prepare(&split, 0, &events).unwrap();
        let mut partitioned = [f64::NAN; 20];
        split_plan
            .render_next(&mut split, &mut partitioned[..6])
            .unwrap();
        split_plan
            .render_next(&mut split, &mut partitioned[6..12])
            .unwrap();
        split_plan
            .render_next(&mut split, &mut partitioned[12..])
            .unwrap();
        assert_eq!(actual, partitioned);
    }

    #[test]
    fn timed_midi_poly_matches_prepared_two_voice_timeline() {
        let graph = lua::read(TWO_VOICE_POLY).unwrap();
        let events = [
            PolyNoteEvent::On {
                frame: 3,
                channel: 1,
                key: 69,
                velocity: 64,
            },
            PolyNoteEvent::On {
                frame: 4,
                channel: 1,
                key: 69,
                velocity: 32,
            },
            PolyNoteEvent::On {
                frame: 5,
                channel: 1,
                key: 81,
                velocity: 127,
            },
            PolyNoteEvent::Off {
                frame: 7,
                channel: 1,
                key: 69,
            },
            PolyNoteEvent::On {
                frame: 9,
                channel: 1,
                key: 72,
                velocity: 32,
            },
            PolyNoteEvent::Off {
                frame: 11,
                channel: 1,
                key: 81,
            },
            PolyNoteEvent::Off {
                frame: 13,
                channel: 1,
                key: 72,
            },
        ];
        let mut reference = compile_at_rate(&graph, 48_000.0).unwrap();
        let mut plan = PolyNotePlan::prepare(&reference, 0, &events).unwrap();
        let mut expected = [f64::NAN; 20];
        plan.render_next(&mut reference, &mut expected).unwrap();

        let mut machine = compile_at_rate(&graph, 48_000.0).unwrap();
        let (producer, mut consumer) = live_note_queue(&machine, 0);
        let mut midi = TimedMidiRouter::new(&machine, producer, 48_000, 3).unwrap();
        assert_eq!(midi.first_mapped_channel(), Some(1));
        for (time, message) in [
            (0, [0x90, 69, 64]),
            (21, [0x90, 69, 32]),
            (42, [0x90, 81, 127]),
            (84, [0x80, 69, 0]),
            (125, [0x90, 72, 32]),
            (167, [0x90, 81, 0]),
            (209, [0x80, 72, 0]),
        ] {
            assert_eq!(midi.dispatch(time, &message), Ok(true));
        }
        let mut actual = [f64::NAN; 20];
        consumer
            .render_next(&mut machine, &mut actual[..6])
            .unwrap();
        consumer
            .render_next(&mut machine, &mut actual[6..12])
            .unwrap();
        consumer
            .render_next(&mut machine, &mut actual[12..])
            .unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn timed_midi_four_voices_hold_and_reuse_lowest_free_voice() {
        let graph = lua::read(FOUR_VOICE_POLY).unwrap();
        let events = [
            PolyNoteEvent::On {
                frame: 3,
                channel: 1,
                key: 60,
                velocity: 100,
            },
            PolyNoteEvent::On {
                frame: 4,
                channel: 1,
                key: 64,
                velocity: 90,
            },
            PolyNoteEvent::On {
                frame: 5,
                channel: 1,
                key: 67,
                velocity: 80,
            },
            PolyNoteEvent::On {
                frame: 6,
                channel: 1,
                key: 72,
                velocity: 70,
            },
            PolyNoteEvent::Off {
                frame: 9,
                channel: 1,
                key: 64,
            },
            PolyNoteEvent::On {
                frame: 11,
                channel: 1,
                key: 76,
                velocity: 60,
            },
            PolyNoteEvent::Off {
                frame: 13,
                channel: 1,
                key: 60,
            },
            PolyNoteEvent::Off {
                frame: 14,
                channel: 1,
                key: 67,
            },
            PolyNoteEvent::Off {
                frame: 15,
                channel: 1,
                key: 72,
            },
            PolyNoteEvent::Off {
                frame: 16,
                channel: 1,
                key: 76,
            },
        ];
        let mut reference = compile_at_rate(&graph, 48_000.0).unwrap();
        let mut plan = PolyNotePlan::prepare(&reference, 0, &events).unwrap();
        let mut expected = [f64::NAN; 24];
        plan.render_next(&mut reference, &mut expected).unwrap();

        let mut machine = compile_at_rate(&graph, 48_000.0).unwrap();
        let (producer, mut consumer) = live_note_queue(&machine, 0);
        let mut midi = TimedMidiRouter::new(&machine, producer, 48_000, 3).unwrap();
        for (time, message) in [
            (0, [0x90, 60, 100]),
            (21, [0x90, 64, 90]),
            (42, [0x90, 67, 80]),
            (63, [0x90, 72, 70]),
        ] {
            assert_eq!(midi.dispatch(time, &message), Ok(true));
        }
        assert_eq!(
            midi.dispatch(84, &[0x90, 76, 60]),
            Err(TimedMidiError::NoFreeVoice)
        );
        for (time, message) in [
            (125, [0x80, 64, 0]),
            (167, [0x90, 76, 60]),
            (209, [0x80, 60, 0]),
            (230, [0x80, 67, 0]),
            (250, [0x80, 72, 0]),
            (271, [0x80, 76, 0]),
        ] {
            assert_eq!(midi.dispatch(time, &message), Ok(true));
        }
        let mut actual = [f64::NAN; 24];
        consumer
            .render_next(&mut machine, &mut actual[..8])
            .unwrap();
        consumer
            .render_next(&mut machine, &mut actual[8..17])
            .unwrap();
        consumer
            .render_next(&mut machine, &mut actual[17..])
            .unwrap();
        assert_eq!(actual, expected);

        let mut three_only = compile_at_rate(&graph, 48_000.0).unwrap();
        let mut three_plan = PolyNotePlan::prepare(&three_only, 0, &events[..3]).unwrap();
        let mut three_samples = [0.0; 8];
        three_plan
            .render_next(&mut three_only, &mut three_samples)
            .unwrap();
        assert_ne!(actual[..8], three_samples);
    }

    #[test]
    fn timed_midi_four_voice_reset_closes_every_gate() {
        let graph = lua::read(FOUR_VOICE_POLY).unwrap();
        let mut reference = compile_at_rate(&graph, 48_000.0).unwrap();
        let events = [
            PolyNoteEvent::On {
                frame: 3,
                channel: 1,
                key: 60,
                velocity: 100,
            },
            PolyNoteEvent::On {
                frame: 4,
                channel: 1,
                key: 64,
                velocity: 90,
            },
            PolyNoteEvent::On {
                frame: 5,
                channel: 1,
                key: 67,
                velocity: 80,
            },
            PolyNoteEvent::On {
                frame: 6,
                channel: 1,
                key: 72,
                velocity: 70,
            },
            PolyNoteEvent::Off {
                frame: 8,
                channel: 1,
                key: 60,
            },
            PolyNoteEvent::Off {
                frame: 8,
                channel: 1,
                key: 64,
            },
            PolyNoteEvent::Off {
                frame: 8,
                channel: 1,
                key: 67,
            },
            PolyNoteEvent::Off {
                frame: 8,
                channel: 1,
                key: 72,
            },
            PolyNoteEvent::On {
                frame: 11,
                channel: 1,
                key: 76,
                velocity: 60,
            },
        ];
        let mut plan = PolyNotePlan::prepare(&reference, 0, &events).unwrap();
        let mut expected = [f64::NAN; 18];
        plan.render_next(&mut reference, &mut expected).unwrap();

        let mut machine = compile_at_rate(&graph, 48_000.0).unwrap();
        let (producer, mut consumer) = live_note_queue(&machine, 0);
        let mut midi = TimedMidiRouter::new(&machine, producer, 48_000, 3).unwrap();
        for (time, key, velocity) in [(0, 60, 100), (21, 64, 90), (42, 67, 80), (63, 72, 70)] {
            midi.dispatch(time, &[0x90, key, velocity]).unwrap();
        }
        let mut actual = [f64::NAN; 18];
        consumer
            .render_next(&mut machine, &mut actual[..8])
            .unwrap();
        assert_eq!(midi.dispatch(84, &[0xB0, 123, 0]), Ok(true));
        for key in [60, 64, 67, 72] {
            assert_eq!(midi.dispatch(85, &[0x80, key, 0]), Ok(false));
        }
        midi.dispatch(1, &[0x90, 76, 60]).unwrap();
        consumer
            .render_next(&mut machine, &mut actual[8..])
            .unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn fifth_poly_binding_rejects_without_replacing_four_voice_machine() {
        let graph = lua::read(FOUR_VOICE_POLY).unwrap();
        let mut host = Host::default();
        host.submit(&graph).unwrap();
        let mut invalid = graph.clone();
        let fifth = invalid.midi_poly[0].voices[0].clone();
        invalid.midi_poly[0].voices.push(fifth);
        assert!(
            host.submit(&invalid)
                .unwrap_err()
                .contains("two to four voices")
        );
        assert!(host.accepted().unwrap().has_poly_notes());
    }

    #[test]
    fn invalid_screens_preserve_accepted_machine() {
        use crate::model::Widget;

        let graph = lua::read(include_str!("../examples/66-screens.lua")).unwrap();
        let mut host = Host::default();
        host.submit(&graph).unwrap();
        let mut unknown_control = graph.clone();
        unknown_control.screens[0].widgets[1] = Widget::Control {
            label: "Missing".into(),
            control_id: "missing".into(),
        };
        let mut unknown_link = graph.clone();
        unknown_link.screens[0].widgets[2] = Widget::Link {
            label: "Missing".into(),
            screen_id: "missing".into(),
        };
        let mut duplicate = graph.clone();
        duplicate.screens[1].id = "home".into();
        let mut unsafe_text = graph.clone();
        unsafe_text.screens[0].title = "Home\x1b[2J".into();
        for (candidate, expected) in [
            (unknown_control, "unknown control"),
            (unknown_link, "unknown screen"),
            (duplicate, "duplicate screen ID"),
            (unsafe_text, "printable text"),
        ] {
            assert!(host.submit(&candidate).unwrap_err().contains(expected));
        }
        assert_eq!(host.accepted().unwrap().screens().len(), 2);
        let mut samples = [0.0; 1];
        host.accepted().unwrap().process(&mut samples);
        assert_eq!(samples, [0.1875]);
    }

    #[test]
    fn invalid_screen_button_preserves_accepted_machine() {
        use crate::model::Widget;

        let graph = lua::read(include_str!("../examples/67-screen-buttons.lua")).unwrap();
        let mut host = Host::default();
        host.submit(&graph).unwrap();
        for (control_id, value, expected) in [
            ("missing", 0.0, "unknown control"),
            ("level", 2.0, "button value"),
            ("level", f64::NAN, "button value"),
        ] {
            let mut candidate = graph.clone();
            candidate.screens[0].widgets[2] = Widget::Button {
                label: "Mute".into(),
                control_id: control_id.into(),
                value,
            };
            assert!(host.submit(&candidate).unwrap_err().contains(expected));
        }
        let mut samples = [0.0; 1];
        host.accepted().unwrap().process(&mut samples);
        assert_eq!(samples, [0.1875]);
    }

    #[test]
    fn oldest_poly_voice_is_stolen_after_repeated_key_refresh() {
        let graph = lua::read(FOUR_VOICE_STEAL).unwrap();
        let events = [
            PolyNoteEvent::On {
                frame: 3,
                channel: 1,
                key: 60,
                velocity: 100,
            },
            PolyNoteEvent::On {
                frame: 4,
                channel: 1,
                key: 64,
                velocity: 90,
            },
            PolyNoteEvent::On {
                frame: 5,
                channel: 1,
                key: 67,
                velocity: 80,
            },
            PolyNoteEvent::On {
                frame: 6,
                channel: 1,
                key: 72,
                velocity: 70,
            },
            PolyNoteEvent::On {
                frame: 7,
                channel: 1,
                key: 60,
                velocity: 60,
            },
            PolyNoteEvent::On {
                frame: 8,
                channel: 1,
                key: 76,
                velocity: 50,
            },
            PolyNoteEvent::Off {
                frame: 9,
                channel: 1,
                key: 64,
            },
            PolyNoteEvent::Off {
                frame: 10,
                channel: 1,
                key: 76,
            },
            PolyNoteEvent::On {
                frame: 11,
                channel: 1,
                key: 79,
                velocity: 40,
            },
        ];
        let mut reference = compile_at_rate(&graph, 48_000.0).unwrap();
        let mut plan = PolyNotePlan::prepare(&reference, 0, &events).unwrap();
        let mut expected = [f64::NAN; 18];
        plan.render_next(&mut reference, &mut expected).unwrap();

        let mut machine = compile_at_rate(&graph, 48_000.0).unwrap();
        let (producer, mut consumer) = live_note_queue(&machine, 0);
        let mut midi = TimedMidiRouter::new(&machine, producer, 48_000, 3).unwrap();
        for (time, message, accepted) in [
            (0, [0x90, 60, 100], true),
            (21, [0x90, 64, 90], true),
            (42, [0x90, 67, 80], true),
            (63, [0x90, 72, 70], true),
            (84, [0x90, 60, 60], true),
            (105, [0x90, 76, 50], true),
            (125, [0x80, 64, 0], false),
            (146, [0x80, 76, 0], true),
            (167, [0x90, 79, 40], true),
        ] {
            assert_eq!(midi.dispatch(time, &message), Ok(accepted));
        }
        let mut actual = [f64::NAN; 18];
        consumer
            .render_next(&mut machine, &mut actual[..9])
            .unwrap();
        consumer
            .render_next(&mut machine, &mut actual[9..])
            .unwrap();
        assert_eq!(actual, expected);

        let invalid = FOUR_VOICE_STEAL.replace("steal_oldest", "steal_newest");
        assert!(
            lua::read(&invalid)
                .unwrap_err()
                .contains("unknown polyphonic overflow policy")
        );
    }

    #[test]
    fn full_queue_does_not_commit_stolen_voice() {
        let graph = lua::read(FOUR_VOICE_STEAL).unwrap();
        let mut machine = compile_at_rate(&graph, 48_000.0).unwrap();
        let (producer, mut consumer) = live_note_queue(&machine, 0);
        let mut midi = TimedMidiRouter::new(&machine, producer, 48_000, 3).unwrap();
        for key in [60, 64, 67, 72] {
            midi.dispatch(0, &[0x90, key, 100]).unwrap();
        }
        let mut opening = [0.0; 4];
        consumer.render_next(&mut machine, &mut opening).unwrap();
        for offset in 0..MAX_EVENTS as u64 {
            midi.dispatch(1_000 + offset, &[0x90, 60, 100]).unwrap();
        }
        assert_eq!(
            midi.dispatch(1_100, &[0x90, 76, 100]),
            Err(TimedMidiError::Queue(PushError::Full))
        );
        assert_eq!(midi.dispatch(1_101, &[0x80, 76, 0]), Ok(false));
        let mut drained = [0.0; 64];
        assert_eq!(
            consumer
                .render_next(&mut machine, &mut drained)
                .unwrap()
                .applied,
            MAX_EVENTS
        );
        assert_eq!(midi.dispatch(1_100, &[0x90, 76, 100]), Ok(true));
        assert_eq!(midi.dispatch(1_101, &[0x80, 64, 0]), Ok(false));
        assert_eq!(midi.dispatch(1_102, &[0x80, 76, 0]), Ok(true));
    }

    #[test]
    fn timed_midi_poly_full_bank_keeps_held_notes_retryable() {
        let graph = lua::read(TWO_VOICE_POLY).unwrap();
        let mut machine = compile_at_rate(&graph, 48_000.0).unwrap();
        let (producer, mut consumer) = live_note_queue(&machine, 0);
        let mut midi = TimedMidiRouter::new(&machine, producer, 48_000, 3).unwrap();
        assert_eq!(midi.dispatch(0, &[0x90, 69, 64]), Ok(true));
        assert_eq!(midi.dispatch(42, &[0x90, 81, 127]), Ok(true));
        assert_eq!(
            midi.dispatch(63, &[0x90, 72, 32]),
            Err(TimedMidiError::NoFreeVoice)
        );
        assert_eq!(midi.dispatch(84, &[0x80, 69, 0]), Ok(true));
        assert_eq!(midi.dispatch(125, &[0x90, 72, 32]), Ok(true));
        let mut samples = [f64::NAN; 16];
        assert_eq!(
            consumer
                .render_next(&mut machine, &mut samples)
                .unwrap()
                .applied,
            4
        );
        assert!(samples.iter().any(|sample| sample.abs() > 0.0));
    }

    #[test]
    fn timed_midi_poly_reset_releases_both_voices_and_discards_future_states() {
        let graph = lua::read(TWO_VOICE_POLY).unwrap();
        let mut reference = compile_at_rate(&graph, 48_000.0).unwrap();
        let events = [
            PolyNoteEvent::On {
                frame: 3,
                channel: 1,
                key: 69,
                velocity: 64,
            },
            PolyNoteEvent::On {
                frame: 5,
                channel: 1,
                key: 81,
                velocity: 127,
            },
            PolyNoteEvent::Off {
                frame: 6,
                channel: 1,
                key: 69,
            },
            PolyNoteEvent::Off {
                frame: 6,
                channel: 1,
                key: 81,
            },
            PolyNoteEvent::On {
                frame: 9,
                channel: 1,
                key: 72,
                velocity: 32,
            },
        ];
        let mut plan = PolyNotePlan::prepare(&reference, 0, &events).unwrap();
        let mut expected = [f64::NAN; 20];
        plan.render_next(&mut reference, &mut expected).unwrap();

        let mut machine = compile_at_rate(&graph, 48_000.0).unwrap();
        let (producer, mut consumer) = live_note_queue(&machine, 0);
        let mut midi = TimedMidiRouter::new(&machine, producer, 48_000, 3).unwrap();
        midi.dispatch(0, &[0x90, 69, 64]).unwrap();
        midi.dispatch(42, &[0x90, 81, 127]).unwrap();
        let mut actual = [f64::NAN; 20];
        consumer
            .render_next(&mut machine, &mut actual[..6])
            .unwrap();
        midi.dispatch(1_000, &[0x90, 69, 100]).unwrap();
        assert_eq!(midi.dispatch(1_001, &[0xB0, 123, 0]), Ok(true));
        midi.dispatch(1, &[0x90, 72, 32]).unwrap();
        let report = consumer
            .render_next(&mut machine, &mut actual[6..])
            .unwrap();
        assert_eq!(report.discarded, 1);
        assert_eq!(report.applied, 1);
        assert_eq!(actual, expected);
    }

    #[test]
    fn timed_midi_poly_overflow_releases_both_voices() {
        let graph = lua::read(TWO_VOICE_POLY).unwrap();
        let mut reference = compile_at_rate(&graph, 48_000.0).unwrap();
        let events = [
            PolyNoteEvent::On {
                frame: 3,
                channel: 1,
                key: 69,
                velocity: 100,
            },
            PolyNoteEvent::On {
                frame: 5,
                channel: 1,
                key: 81,
                velocity: 100,
            },
            PolyNoteEvent::Off {
                frame: 6,
                channel: 1,
                key: 69,
            },
            PolyNoteEvent::Off {
                frame: 6,
                channel: 1,
                key: 81,
            },
            PolyNoteEvent::On {
                frame: 17,
                channel: 1,
                key: 72,
                velocity: 100,
            },
        ];
        let mut plan = PolyNotePlan::prepare(&reference, 0, &events).unwrap();
        let mut expected = [f64::NAN; 18];
        plan.render_next(&mut reference, &mut expected).unwrap();
        let mut machine = compile_at_rate(&graph, 48_000.0).unwrap();
        let (producer, mut consumer) = live_note_queue(&machine, 0);
        let mut midi = TimedMidiRouter::new(&machine, producer, 48_000, 3).unwrap();
        midi.dispatch_recovering(0, &[0x90, 69, 100]).unwrap();
        midi.dispatch_recovering(42, &[0x90, 81, 100]).unwrap();
        let mut opening = [0.0; 6];
        consumer.render_next(&mut machine, &mut opening).unwrap();
        assert_eq!(opening, expected[..6]);

        for offset in 0..MAX_EVENTS as u64 {
            assert_eq!(
                midi.dispatch_recovering(1_000_000 + offset, &[0x90, 69, 100]),
                Ok(true)
            );
        }
        assert_eq!(
            midi.dispatch_recovering(1_000_064, &[0x80, 81, 0]),
            Err(TimedMidiError::Queue(PushError::Full))
        );
        let mut released = [f64::NAN; 8];
        let report = consumer.render_next(&mut machine, &mut released).unwrap();
        assert_eq!(report.discarded, MAX_EVENTS);
        assert_eq!(report.applied, 0);
        assert_eq!(released, expected[6..14]);
        assert_eq!(midi.dispatch_recovering(1, &[0x80, 69, 0]), Ok(false));
        assert_eq!(midi.dispatch_recovering(1, &[0x80, 81, 0]), Ok(false));
        assert_eq!(midi.dispatch_recovering(1, &[0x90, 72, 100]), Ok(true));
        let mut fresh = [0.0; 4];
        assert_eq!(
            consumer
                .render_next(&mut machine, &mut fresh)
                .unwrap()
                .applied,
            1
        );
        assert_eq!(fresh, expected[14..]);
    }

    #[test]
    fn poly_plan_full_bank_and_invalid_bindings_preserve_machine() {
        let graph = lua::read(TWO_VOICE_POLY).unwrap();
        let mut host = Host::default();
        host.submit(&graph).unwrap();
        let mut duplicate = graph.clone();
        duplicate.midi_poly[0].voices[1].gate_control_id = "left-gate".into();
        let mut missing = graph.clone();
        missing.midi_poly[0].voices[1].velocity_control_id = "missing".into();
        let mut narrow = graph.clone();
        narrow
            .controls
            .iter_mut()
            .find(|control| control.id == "right-pitch")
            .unwrap()
            .max = 1000.0;
        let mut wrong_count = graph.clone();
        wrong_count.midi_poly[0].voices.pop();
        for (candidate, expected) in [
            (duplicate, "distinct parameter targets"),
            (missing, "unknown control"),
            (narrow, "cover all MIDI note frequencies"),
            (wrong_count, "two to four voices"),
        ] {
            let error = host.submit(&candidate).unwrap_err();
            assert!(error.contains(expected), "{error}");
        }
        let machine = host.accepted().unwrap();
        let full = [
            PolyNoteEvent::On {
                frame: 1,
                channel: 1,
                key: 60,
                velocity: 100,
            },
            PolyNoteEvent::On {
                frame: 2,
                channel: 1,
                key: 64,
                velocity: 100,
            },
            PolyNoteEvent::On {
                frame: 3,
                channel: 1,
                key: 67,
                velocity: 100,
            },
        ];
        assert!(
            PolyNotePlan::prepare(machine, 0, &full)
                .unwrap_err()
                .contains("no free voice")
        );
        for invalid in [
            PolyNoteEvent::On {
                frame: 1,
                channel: 1,
                key: 128,
                velocity: 100,
            },
            PolyNoteEvent::On {
                frame: 1,
                channel: 1,
                key: 60,
                velocity: 128,
            },
        ] {
            assert!(
                PolyNotePlan::prepare(machine, 0, &[invalid])
                    .unwrap_err()
                    .contains("MIDI byte")
            );
        }
        assert!(
            PolyNotePlan::prepare(
                machine,
                0,
                &[PolyNoteEvent::Off {
                    frame: 0,
                    channel: 2,
                    key: 60
                }]
            )
            .unwrap_err()
            .contains("no polyphonic MIDI mapping")
        );
        let mut plan = PolyNotePlan::prepare(machine, 0, &full[..2]).unwrap();
        let mut other = compile(&graph).unwrap();
        assert_eq!(
            plan.render_next(&mut other, &mut [f64::NAN; 4]),
            Err(TimedRenderError::WrongMachine)
        );
        assert_eq!(plan.cursor(), 0);
        let mut samples = [f64::NAN; 4];
        machine.process(&mut samples);
        assert_eq!(samples, [0.0; 4]);
    }

    #[test]
    fn invalid_subpatch_expansion_preserves_accepted_machine() {
        use crate::model::{Connection, Endpoint, SubpatchInstance};

        let valid = lua::read(TWO_SUBPATCH_VOICES).unwrap();
        let invalid_endpoint = lua::read(SUBPATCH_INVALID_ENDPOINT).unwrap();
        let mut duplicate_id = valid.clone();
        duplicate_id.instances[1].id = "left".into();
        let mut recursive = valid.clone();
        recursive
            .subpatches
            .get_mut("gated_oscillator")
            .unwrap()
            .graph
            .instances
            .push(SubpatchInstance {
                id: "self".into(),
                definition: "gated_oscillator".into(),
                values: Default::default(),
            });
        let mut invalid_cable = valid.clone();
        invalid_cable
            .subpatches
            .get_mut("gated_oscillator")
            .unwrap()
            .graph
            .connections[0]
            .to
            .port = "missing".into();
        let mut missing_definition = valid.clone();
        missing_definition.instances[0].definition = "missing".into();
        let mut duplicate_port = valid.clone();
        let definition = duplicate_port
            .subpatches
            .get_mut("gated_oscillator")
            .unwrap();
        definition.inputs.push(definition.inputs[0].clone());
        let mut invalid_input = valid.clone();
        invalid_input
            .subpatches
            .get_mut("gated_oscillator")
            .unwrap()
            .inputs[0]
            .destinations[0]
            .port = "out".into();
        let mut shared_port_name = valid.clone();
        shared_port_name
            .subpatches
            .get_mut("gated_oscillator")
            .unwrap()
            .outputs[0]
            .name = "pitch".into();
        let mut expanded_cycle = valid.clone();
        expanded_cycle
            .subpatches
            .get_mut("gated_oscillator")
            .unwrap()
            .graph
            .connections
            .push(Connection {
                from: Endpoint {
                    module: "level".into(),
                    port: "out".into(),
                },
                to: Endpoint {
                    module: "oscillator".into(),
                    port: "fm_hz".into(),
                },
            });
        expanded_cycle
            .connections
            .retain(|edge| edge.to.port != "pitch");

        let mut host = Host::default();
        host.submit(&valid).unwrap();
        let mut reference = compile(&valid).unwrap();
        for (candidate, expected) in [
            (invalid_endpoint, "invalid output port"),
            (duplicate_id, "duplicate"),
            (recursive, "recursive subpatch"),
            (invalid_cable, "invalid input port"),
            (missing_definition, "unknown subpatch definition"),
            (duplicate_port, "duplicate external port"),
            (invalid_input, "invalid input port"),
            (shared_port_name, "external input and output share name"),
            (expanded_cycle, "cycle"),
        ] {
            let error = host.submit(&candidate).unwrap_err();
            assert!(error.contains(expected), "{error}");
            let mut samples = [f64::NAN; 4];
            let mut expected_samples = [f64::NAN; 4];
            host.accepted().unwrap().process(&mut samples);
            reference.process(&mut expected_samples);
            assert_eq!(samples, expected_samples);
        }
    }

    #[test]
    fn one_connection_port_rejects_second_cable_and_keeps_machine() {
        let mut host = Host::default();
        host.submit(&lua::read(GAIN_FAN_IN).unwrap()).unwrap();
        let error = host
            .submit(&lua::read(OUTPUT_FAN_IN_INVALID).unwrap())
            .unwrap_err();
        assert!(
            error.contains("output.input' permits one connection"),
            "{error}"
        );
        let mut samples = [f64::NAN; 4];
        host.accepted().unwrap().process(&mut samples);
        assert_eq!(samples, [0.1875; 4]);
    }

    #[test]
    fn two_module_parameters_retain_independent_identities() {
        let graph = lua::read(AFFINE).unwrap();
        let mut machine = compile(&graph).unwrap();
        assert_eq!(machine.order(), ["source", "transform", "output"]);
        let scale = ParameterId::new("transform", "scale");
        let offset = ParameterId::new("transform", "offset");
        assert_eq!(machine.authored_value(&scale), Some(0.5));
        assert_eq!(machine.authored_value(&offset), Some(0.125));
        let scale_handle = machine.parameter_handle(&scale).unwrap();
        let offset_handle = machine.parameter_handle(&offset).unwrap();
        let mut samples = [f64::NAN; 4];
        machine.process(&mut samples);
        assert_eq!(samples, [0.25; 4]);
        scale_handle.publish(Some(0.75)).unwrap();
        assert_eq!(machine.authored_value(&scale), Some(0.5));
        assert_eq!(machine.effective_value(&offset), Some(0.125));
        machine.process(&mut samples);
        assert_eq!(samples, [0.3125; 4]);
        offset_handle.publish(Some(0.25)).unwrap();
        machine.process(&mut samples);
        assert_eq!(samples, [0.4375; 4]);
        scale_handle.publish(None).unwrap();
        offset_handle.publish(None).unwrap();
        machine.process(&mut samples);
        assert_eq!(samples, [0.25; 4]);
    }

    #[test]
    fn optional_modulation_is_a_signal_not_a_parameter_override() {
        let graph = lua::read(MODULATED_GAIN).unwrap();
        let mut machine = compile_at_rate(&graph, 48_000.0).unwrap();
        assert_eq!(machine.order(), ["modulator", "source", "gain", "output"]);
        let id = ParameterId::new("gain", "gain");
        let handle = machine.parameter_handle(&id).unwrap();
        let mut samples = [f64::NAN; 4];
        machine.process(&mut samples);
        for (actual, expected) in samples.into_iter().zip([0.0, 0.125, 0.0, -0.125]) {
            assert!((actual - expected).abs() < 1e-12, "{actual} != {expected}");
        }
        assert_eq!(machine.authored_value(&id), Some(0.5));
        assert_eq!(machine.effective_value(&id), Some(0.5));
        handle.publish(Some(0.75)).unwrap();
        machine.process(&mut samples);
        for (actual, expected) in samples.into_iter().zip([0.0, 0.1875, 0.0, -0.1875]) {
            assert!((actual - expected).abs() < 1e-12, "{actual} != {expected}");
        }
        assert_eq!(machine.authored_value(&id), Some(0.5));
        assert_eq!(machine.effective_value(&id), Some(0.75));
    }

    #[test]
    fn optional_modulation_does_not_make_signal_input_optional() {
        let mut host = Host::default();
        host.submit(&lua::read(BASIC).unwrap()).unwrap();
        let error = host
            .submit(&lua::read(MODULATED_GAIN_MISSING).unwrap())
            .unwrap_err();
        assert!(error.contains("gain.input' is disconnected"), "{error}");
        let mut samples = [f64::NAN; 4];
        host.accepted().unwrap().process(&mut samples);
        assert_eq!(samples, [0.125; 4]);
    }

    #[test]
    fn two_named_controls_route_to_distinct_parameters_of_one_module() {
        let graph = lua::read(AFFINE_CONTROLS).unwrap();
        let mut machine = compile(&graph).unwrap();
        let scale = ParameterId::new("transform", "scale");
        let offset = ParameterId::new("transform", "offset");
        assert_eq!(machine.control_handle("scale").unwrap().target(), &scale);
        assert_eq!(machine.control_handle("offset").unwrap().target(), &offset);
        let router = MidiRouter::from_machine(&machine);
        let mut samples = [f64::NAN; 4];
        machine.process(&mut samples);
        assert_eq!(samples, [0.25; 4]);
        assert!(router.dispatch(&[0xB0, 7, 127]));
        assert_eq!(machine.effective_value(&scale), Some(1.0));
        assert_eq!(machine.effective_value(&offset), Some(0.125));
        machine.process(&mut samples);
        assert_eq!(samples, [0.375; 4]);
        assert!(router.dispatch(&[0xB0, 8, 0]));
        assert_eq!(machine.effective_value(&scale), Some(1.0));
        assert_eq!(machine.effective_value(&offset), Some(-0.5));
        machine.process(&mut samples);
        assert_eq!(samples, [-0.25; 4]);
        assert_eq!(machine.authored_value(&scale), Some(0.5));
        assert_eq!(machine.authored_value(&offset), Some(0.125));
    }

    #[test]
    fn mix_contract_rejects_missing_and_duplicate_inputs() {
        let missing = lua::read(MIX_MISSING).unwrap();
        assert!(
            compile(&missing)
                .unwrap_err()
                .contains("mix.right' is disconnected")
        );
        let mut duplicate = lua::read(MIX).unwrap();
        duplicate.connections.push(duplicate.connections[0].clone());
        assert!(
            compile(&duplicate)
                .unwrap_err()
                .contains("mix.left' permits one connection")
        );
    }

    #[test]
    fn fan_out_and_rejected_mix_preserve_accepted_machine() {
        let mut graph = lua::read(MIX).unwrap();
        graph.connections[1].from.module = "left".into();
        let mut host = Host::default();
        host.submit(&graph).unwrap();
        let mut samples = [0.0; 4];
        host.accepted().unwrap().process(&mut samples);
        assert_eq!(samples, [0.25; 4]);
        assert!(host.submit(&lua::read(MIX_MISSING).unwrap()).is_err());
        host.accepted().unwrap().process(&mut samples);
        assert_eq!(samples, [0.25; 4]);
    }

    #[test]
    fn control_thread_publishes_at_processing_boundary() {
        let mut machine = compile(&lua::read(MIX).unwrap()).unwrap();
        let id = ParameterId::new("gain", "gain");
        let handle = machine.parameter_handle(&id).unwrap();
        assert_eq!(handle.id(), &id);
        let mut samples = [0.0; 16];
        machine.process(&mut samples);
        assert_eq!(samples, [0.1875; 16]);
        let control = std::thread::spawn(move || {
            assert!(handle.publish(Some(f64::INFINITY)).is_err());
            handle.publish(Some(0.75)).unwrap();
            handle
        });
        let handle = control.join().unwrap();
        assert_eq!(machine.authored_value(&id), Some(0.5));
        assert_eq!(machine.effective_value(&id), Some(0.75));
        machine.process(&mut samples);
        assert_eq!(samples, [0.28125; 16]);
        handle.publish(None).unwrap();
        machine.process(&mut samples);
        assert_eq!(samples, [0.1875; 16]);
    }

    #[test]
    fn old_revision_handle_cannot_change_replacement() {
        let id = ParameterId::new("gain", "gain");
        let mut host = Host::default();
        host.submit(&lua::read(MIX).unwrap()).unwrap();
        let old_handle = host.accepted().unwrap().parameter_handle(&id).unwrap();
        let mut replacement = lua::read(MIX).unwrap();
        replacement
            .modules
            .iter_mut()
            .find(|module| module.id == "gain")
            .unwrap()
            .values
            .insert("gain".into(), 0.25);
        host.submit(&replacement).unwrap();
        old_handle.publish(Some(0.9)).unwrap();
        let mut samples = [0.0; 8];
        host.accepted().unwrap().process(&mut samples);
        assert_eq!(samples, [0.09375; 8]);
        assert_eq!(host.accepted().unwrap().effective_value(&id), Some(0.25));
    }

    #[test]
    fn complete_revisions_activate_between_buffers() {
        let initial = compile(&lua::read(BASIC).unwrap()).unwrap();
        let (mut runtime, control) = LiveRuntime::new(initial);
        let mut samples = [0.0; 8];
        assert!(!runtime.process(&mut samples));
        assert_eq!(samples, [0.125; 8]);

        let cycle = lua::read(CYCLE).unwrap();
        assert!(control.submit(&cycle).unwrap_err().contains("cycle"));
        let mix = lua::read(MIX).unwrap();
        let publisher = std::thread::spawn(move || {
            control.submit(&mix).unwrap();
            assert!(
                control
                    .submit(&lua::read(BASIC).unwrap())
                    .unwrap_err()
                    .contains("pending")
            );
            assert!(control.submit(&cycle).unwrap_err().contains("cycle"));
            control
        });
        let control = publisher.join().unwrap();
        assert!(runtime.process(&mut samples));
        assert_eq!(samples, [0.1875; 8]);
        assert!(!runtime.process(&mut samples));
        assert_eq!(samples, [0.1875; 8]);
        assert!(
            control
                .submit(&lua::read(BASIC).unwrap())
                .unwrap_err()
                .contains("reclaimed")
        );
        let retired = control.reclaim().unwrap().unwrap();
        assert_eq!(retired.order(), ["source", "gain", "output"]);
        drop(retired);
        control.submit(&lua::read(BASIC).unwrap()).unwrap();
        assert!(runtime.process(&mut samples));
        assert_eq!(samples, [0.125; 8]);
    }

    #[test]
    fn sine_phase_is_stateful_and_partition_invariant() {
        let graph = lua::read(SINE).unwrap();
        let mut full = compile_at_rate(&graph, 48_000.0).unwrap();
        assert_eq!(full.sample_rate(), 48_000.0);
        let mut expected = [0.0; 16];
        full.process(&mut expected);
        for (index, sample) in expected.iter().enumerate() {
            let reference = [0.0, 0.25, 0.0, -0.25][index % 4];
            assert!(
                (sample - reference).abs() < 1e-12,
                "sample {index}: {sample}"
            );
        }
        let mut split = compile_at_rate(&graph, 48_000.0).unwrap();
        let mut first = [0.0; 3];
        let mut second = [0.0; 13];
        split.process(&mut first);
        split.process(&mut second);
        assert_eq!(&expected[..3], &first);
        assert_eq!(&expected[3..], &second);
    }

    #[test]
    fn live_frequency_update_preserves_phase() {
        let mut machine = compile(&lua::read(SINE).unwrap()).unwrap();
        let handle = machine
            .parameter_handle(&ParameterId::new("oscillator", "frequency"))
            .unwrap();
        let mut first = [0.0; 1];
        machine.process(&mut first);
        assert_eq!(first, [0.0]);
        handle.publish(Some(6_000.0)).unwrap();
        let mut next = [0.0; 2];
        machine.process(&mut next);
        assert!((next[0] - 0.25).abs() < 1e-12);
        assert!((next[1] - 0.25 * std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-12);
        assert!(
            compile_at_rate(&lua::read(SINE).unwrap(), 0.0)
                .unwrap_err()
                .contains("sample rate")
        );
        assert!(compile_at_rate(&lua::read(SINE).unwrap(), f64::NAN).is_err());
    }

    #[test]
    fn explicit_fm_advances_carrier_phase_per_sample_and_across_buffers() {
        let graph = lua::read(SINE_FM).unwrap();
        let mut full = compile_at_rate(&graph, 48_000.0).unwrap();
        assert_eq!(full.order(), ["modulator", "depth", "carrier", "output"]);
        let mut expected = [f64::NAN; 8];
        full.process(&mut expected);
        let peak = std::f64::consts::FRAC_1_SQRT_2;
        for (actual, reference) in expected[..4].iter().copied().zip([0.0, peak, peak, 0.0]) {
            assert!(
                (actual - reference).abs() < 1e-12,
                "{actual} != {reference}"
            );
        }
        let mut split = compile_at_rate(&graph, 48_000.0).unwrap();
        let mut first = [f64::NAN; 3];
        let mut second = [f64::NAN; 5];
        split.process(&mut first);
        split.process(&mut second);
        assert_eq!(expected[..3], first);
        assert_eq!(expected[3..], second);
    }

    #[test]
    fn fm_depth_update_preserves_carrier_state_and_cycle_rejection() {
        let graph = lua::read(SINE_FM).unwrap();
        let mut host = Host::default();
        host.submit(&graph).unwrap();
        let error = host.submit(&lua::read(SINE_FM_CYCLE).unwrap()).unwrap_err();
        assert!(error.contains("cycle"), "{error}");
        let machine = host.accepted().unwrap();
        let depth = ParameterId::new("depth", "gain");
        let handle = machine.parameter_handle(&depth).unwrap();
        let mut samples = [f64::NAN; 4];
        machine.process(&mut samples);
        let peak = std::f64::consts::FRAC_1_SQRT_2;
        for (actual, reference) in samples.into_iter().zip([0.0, peak, peak, 0.0]) {
            assert!(
                (actual - reference).abs() < 1e-12,
                "{actual} != {reference}"
            );
        }
        handle.publish(Some(0.0)).unwrap();
        assert_eq!(machine.authored_value(&depth), Some(6000.0));
        machine.process(&mut samples);
        for (actual, reference) in samples.into_iter().zip([0.0, -peak, -1.0, -peak]) {
            assert!(
                (actual - reference).abs() < 1e-12,
                "{actual} != {reference}"
            );
        }
    }

    #[test]
    fn shared_signal_composes_frequency_and_amplitude_modulation() {
        let graph = lua::read(SHARED_MODULATOR).unwrap();
        let mut reordered = graph.clone();
        reordered.modules.reverse();
        reordered.connections.reverse();
        let mut full = compile_at_rate(&graph, 48_000.0).unwrap();
        let mut alternate = compile_at_rate(&reordered, 48_000.0).unwrap();
        assert_eq!(
            full.order(),
            [
                "modulator",
                "amplitude",
                "depth",
                "carrier",
                "level",
                "output"
            ]
        );
        assert_eq!(full.order(), alternate.order());
        let mut expected = [f64::NAN; 8];
        full.process(&mut expected);
        let peak = std::f64::consts::FRAC_1_SQRT_2;
        for (actual, reference) in expected[..4]
            .iter()
            .copied()
            .zip([0.0, peak, peak / 2.0, 0.0])
        {
            assert!(
                (actual - reference).abs() < 1e-12,
                "{actual} != {reference}"
            );
        }
        let mut first = [f64::NAN; 3];
        let mut second = [f64::NAN; 5];
        alternate.process(&mut first);
        alternate.process(&mut second);
        assert_eq!(expected[..3], first);
        assert_eq!(expected[3..], second);
    }

    #[test]
    fn lag_state_is_continuous_and_response_updates_at_buffer_boundary() {
        let graph = lua::read(LAG).unwrap();
        let mut full = compile(&graph).unwrap();
        assert_eq!(full.order(), ["source", "lag", "output"]);
        let mut expected = [f64::NAN; 8];
        full.process(&mut expected);
        assert_eq!(expected[..4], [0.5, 0.75, 0.875, 0.9375]);
        let mut split = compile(&graph).unwrap();
        let mut first = [f64::NAN; 3];
        let mut second = [f64::NAN; 5];
        split.process(&mut first);
        split.process(&mut second);
        assert_eq!(expected[..3], first);
        assert_eq!(expected[3..], second);

        let mut updated = compile(&graph).unwrap();
        let response = ParameterId::new("lag", "response");
        let handle = updated.parameter_handle(&response).unwrap();
        let control = updated.control_handle("response").unwrap();
        let mut samples = [f64::NAN; 4];
        updated.process(&mut samples);
        assert_eq!(samples, [0.5, 0.75, 0.875, 0.9375]);
        assert!(handle.publish(Some(1.5)).unwrap_err().contains("0..1"));
        assert!(updated.set_override(&response, Some(-0.5)).is_err());
        assert_eq!(updated.effective_value(&response), Some(0.5));
        control.publish(1.0).unwrap();
        assert_eq!(updated.authored_value(&response), Some(0.5));
        updated.process(&mut samples);
        assert_eq!(samples, [1.0; 4]);
    }

    #[test]
    fn invalid_lag_candidates_leave_accepted_state_intact() {
        let mut host = Host::default();
        host.submit(&lua::read(LAG).unwrap()).unwrap();
        let mut samples = [f64::NAN; 4];
        host.accepted().unwrap().process(&mut samples);
        assert_eq!(samples, [0.5, 0.75, 0.875, 0.9375]);
        let error = host.submit(&lua::read(LAG_INVALID).unwrap()).unwrap_err();
        assert!(error.contains("lag.response must be in 0..1"), "{error}");
        let error = host
            .submit(&lua::read(LAG_CONTROL_INVALID).unwrap())
            .unwrap_err();
        assert!(error.contains("control 'response' range"), "{error}");
        let mut next = [f64::NAN; 1];
        host.accepted().unwrap().process(&mut next);
        assert_eq!(next, [0.96875]);
    }

    #[test]
    fn midi_control_smooths_through_independent_graph_modules() {
        let graph = lua::read(SMOOTHED_CONTROL).unwrap();
        let mut machine = compile(&graph).unwrap();
        let motion = ParameterId::new("motion", "value");
        assert_eq!(machine.control_handle("motion").unwrap().target(), &motion);
        let router = MidiRouter::from_machine(&machine);
        let mut samples = [f64::NAN; 4];
        machine.process(&mut samples);
        assert_eq!(samples, [0.0; 4]);

        assert!(router.dispatch(&[0xB0, 7, 127]));
        assert_eq!(machine.authored_value(&motion), Some(0.0));
        assert_eq!(machine.effective_value(&motion), Some(1.0));
        machine.process(&mut samples);
        assert_eq!(samples, [0.125, 0.1875, 0.21875, 0.234375]);

        assert!(router.dispatch(&[0xB0, 7, 0]));
        assert_eq!(machine.effective_value(&motion), Some(0.0));
        machine.process(&mut samples);
        assert_eq!(samples, [0.1171875, 0.05859375, 0.029296875, 0.0146484375]);
    }

    #[test]
    fn audio_rate_switch_compiles_by_dependency_and_preserves_parameter_identity() {
        let graph = lua::read(SWITCH).unwrap();
        let mut machine = compile(&graph).unwrap();
        let order = machine.order();
        assert_eq!(order.last().map(String::as_str), Some("output"));
        assert!(
            order.iter().position(|id| id == "selector")
                < order.iter().position(|id| id == "switch")
        );
        assert!(
            order.iter().position(|id| id == "left") < order.iter().position(|id| id == "switch")
        );
        assert!(
            order.iter().position(|id| id == "right") < order.iter().position(|id| id == "switch")
        );
        let threshold = ParameterId::new("switch", "threshold");
        assert!(machine.parameter_ids().any(|id| id == &threshold));
        assert_eq!(machine.authored_value(&threshold), Some(0.5));

        let mut first = [f64::NAN; 4];
        machine.process(&mut first);
        assert_eq!(first, [0.25, -0.25, 0.25, 0.25]);
        machine.set_override(&threshold, Some(2.0)).unwrap();
        assert_eq!(machine.authored_value(&threshold), Some(0.5));
        assert_eq!(machine.effective_value(&threshold), Some(2.0));
        machine.process(&mut first);
        assert_eq!(first, [0.25; 4]);
        machine.set_override(&threshold, None).unwrap();
        machine.process(&mut first);
        assert_eq!(first, [0.25, -0.25, 0.25, 0.25]);

        let mut split = compile(&graph).unwrap();
        let mut a = [f64::NAN; 3];
        let mut b = [f64::NAN; 5];
        split.process(&mut a);
        split.process(&mut b);
        assert_eq!(a, [0.25, -0.25, 0.25]);
        assert_eq!(b, [0.25, 0.25, -0.25, 0.25, 0.25]);
    }

    #[test]
    fn missing_switch_selector_rejects_candidate_without_corrupting_accepted_machine() {
        let mut host = Host::default();
        host.submit(&lua::read(SWITCH).unwrap()).unwrap();
        let error = host
            .submit(&lua::read(SWITCH_MISSING).unwrap())
            .unwrap_err();
        assert!(error.contains("switch.select"), "{error}");
        let mut samples = [f64::NAN; 4];
        host.accepted().unwrap().process(&mut samples);
        assert_eq!(samples, [0.25, -0.25, 0.25, 0.25]);
    }

    #[test]
    fn new_unit_parameter_uses_existing_control_and_midi_route() {
        let mut machine = compile(&lua::read(SWITCH_CONTROL).unwrap()).unwrap();
        let threshold = ParameterId::new("switch", "threshold");
        let control = machine.control_handle("threshold").unwrap();
        assert_eq!(control.target(), &threshold);
        assert_eq!(control.range(), (0.0, 2.0));
        let router = MidiRouter::from_machine(&machine);
        let mut samples = [f64::NAN; 4];
        machine.process(&mut samples);
        assert_eq!(samples, [0.25, -0.25, 0.25, 0.25]);

        assert!(router.dispatch(&[0xB0, 7, 127]));
        assert_eq!(machine.authored_value(&threshold), Some(0.5));
        assert_eq!(machine.effective_value(&threshold), Some(2.0));
        machine.process(&mut samples);
        assert_eq!(samples, [0.25; 4]);

        control.publish(0.0).unwrap();
        machine.process(&mut samples);
        assert_eq!(samples, [-0.25, -0.25, -0.25, 0.25]);
        control.clear().unwrap();
        machine.process(&mut samples);
        assert_eq!(samples, [0.25, -0.25, 0.25, 0.25]);
    }

    #[test]
    fn two_output_ports_route_distinct_samples_without_topology_cases() {
        let graph = lua::read(MIRROR_OUTPUTS).unwrap();
        let mut machine = compile(&graph).unwrap();
        assert_eq!(
            machine.order(),
            ["source", "mirror", "gain", "mix", "output"]
        );
        let mut samples = [f64::NAN; 8];
        machine.process(&mut samples);
        assert_eq!(samples, [-0.125; 8]);

        let mut reordered = graph.clone();
        reordered.modules.reverse();
        reordered.connections.reverse();
        let mut machine = compile(&reordered).unwrap();
        machine.process(&mut samples);
        assert_eq!(samples, [-0.125; 8]);

        let mut repatched = graph;
        repatched.connections[1].from.port = "out".into();
        let mut machine = compile(&repatched).unwrap();
        machine.process(&mut samples);
        assert_eq!(samples, [0.375; 8]);
    }

    #[test]
    fn invalid_multi_output_port_rejects_without_replacing_accepted_machine() {
        let graph = lua::read(MIRROR_OUTPUTS).unwrap();
        let mut host = Host::default();
        host.submit(&graph).unwrap();
        let mut invalid = graph.clone();
        invalid.connections[1].from.port = "missing".into();
        let error = host.submit(&invalid).unwrap_err();
        assert!(
            error.contains("invalid output port 'mirror.missing'"),
            "{error}"
        );
        let mut samples = [f64::NAN; 4];
        host.accepted().unwrap().process(&mut samples);
        assert_eq!(samples, [-0.125; 4]);
    }

    #[test]
    fn sample_hold_captures_on_audio_crossing_and_keeps_state_across_buffers() {
        let graph = lua::read(SAMPLE_HOLD).unwrap();
        let mut full = compile(&graph).unwrap();
        let mut whole = [f64::NAN; 8];
        full.process(&mut whole);
        let peak = std::f64::consts::FRAC_1_SQRT_2;
        for (actual, expected) in whole
            .iter()
            .zip([0.0, peak, peak, peak, peak, -peak, -peak, -peak])
        {
            assert!((actual - expected).abs() < 1e-12, "{actual} != {expected}");
        }
        let mut split = compile(&graph).unwrap();
        let mut first = [f64::NAN; 3];
        let mut second = [f64::NAN; 5];
        split.process(&mut first);
        split.process(&mut second);
        assert_eq!(whole[..3], first);
        assert_eq!(whole[3..], second);

        let mut updated = compile(&graph).unwrap();
        let threshold = ParameterId::new("hold", "threshold");
        assert!(updated.parameter_ids().any(|id| id == &threshold));
        let mut samples = [f64::NAN; 4];
        updated.process(&mut samples);
        updated.set_override(&threshold, Some(2.0)).unwrap();
        assert_eq!(updated.authored_value(&threshold), Some(0.5));
        updated.process(&mut samples);
        for sample in samples {
            assert!((sample - peak).abs() < 1e-12);
        }
    }

    #[test]
    fn missing_sample_hold_trigger_rejects_without_erasing_state() {
        let mut host = Host::default();
        host.submit(&lua::read(SAMPLE_HOLD).unwrap()).unwrap();
        let mut samples = [f64::NAN; 4];
        host.accepted().unwrap().process(&mut samples);
        let error = host
            .submit(&lua::read(SAMPLE_HOLD_MISSING).unwrap())
            .unwrap_err();
        assert!(error.contains("hold.trigger"), "{error}");
        host.accepted().unwrap().process(&mut samples);
        let peak = std::f64::consts::FRAC_1_SQRT_2;
        for (actual, expected) in samples.iter().zip([peak, -peak, -peak, -peak]) {
            assert!((actual - expected).abs() < 1e-12, "{actual} != {expected}");
        }
    }

    #[test]
    fn held_peak_behavior_emerges_from_existing_units_and_cables() {
        let graph = lua::read(HELD_PEAK_SELECTOR).unwrap();
        let mut machine = compile(&graph).unwrap();
        assert_eq!(
            machine.order(),
            ["signal", "trigger", "hold", "switch", "output"]
        );
        let mut whole = [f64::NAN; 8];
        machine.process(&mut whole);
        let peak = std::f64::consts::FRAC_1_SQRT_2;
        for (actual, expected) in whole
            .iter()
            .zip([0.0, peak, peak, peak, 0.0, -peak, -1.0, -peak])
        {
            assert!((actual - expected).abs() < 1e-12, "{actual} != {expected}");
        }

        let mut split = compile(&graph).unwrap();
        let mut first = [f64::NAN; 3];
        let mut second = [f64::NAN; 5];
        split.process(&mut first);
        split.process(&mut second);
        assert_eq!(whole[..3], first);
        assert_eq!(whole[3..], second);

        let mut reordered = graph.clone();
        reordered.modules.reverse();
        reordered.connections.reverse();
        let mut machine = compile(&reordered).unwrap();
        let mut reordered_samples = [f64::NAN; 8];
        machine.process(&mut reordered_samples);
        assert_eq!(whole, reordered_samples);

        let threshold = ParameterId::new("switch", "threshold");
        machine.set_override(&threshold, Some(2.0)).unwrap();
        let mut next = [f64::NAN; 8];
        machine.process(&mut next);
        for (actual, expected) in next
            .iter()
            .zip([0.0, peak, 1.0, peak, 0.0, -peak, -1.0, -peak])
        {
            assert!((actual - expected).abs() < 1e-12, "{actual} != {expected}");
        }
    }

    #[test]
    fn gated_envelope_has_independent_attack_release_and_continuous_state() {
        let graph = lua::read(GATED_ENVELOPE).unwrap();
        let mut machine = compile(&graph).unwrap();
        assert_eq!(
            machine.order(),
            ["gate", "envelope", "signal", "level", "output"]
        );
        let router = MidiRouter::from_machine(&machine);
        let gate = ParameterId::new("gate", "value");
        let mut silent = [f64::NAN; 2];
        machine.process(&mut silent);
        assert_eq!(silent, [0.0; 2]);

        assert!(router.dispatch(&[0xB0, 7, 127]));
        assert_eq!(machine.authored_value(&gate), Some(0.0));
        let mut attack = [f64::NAN; 4];
        machine.process(&mut attack);
        assert_eq!(attack, [0.125, 0.1875, 0.21875, 0.234375]);

        assert!(router.dispatch(&[0xB0, 7, 0]));
        let mut release = [f64::NAN; 4];
        machine.process(&mut release);
        assert_eq!(
            release,
            [0.17578125, 0.1318359375, 0.098876953125, 0.07415771484375]
        );

        let mut split = compile(&graph).unwrap();
        let split_router = MidiRouter::from_machine(&split);
        split.process(&mut silent);
        assert!(split_router.dispatch(&[0xB0, 7, 127]));
        let mut first = [f64::NAN; 1];
        let mut second = [f64::NAN; 3];
        split.process(&mut first);
        split.process(&mut second);
        assert_eq!(attack, [first[0], second[0], second[1], second[2]]);
        assert!(split_router.dispatch(&[0xB0, 7, 0]));
        split.process(&mut first);
        split.process(&mut second);
        assert_eq!(release, [first[0], second[0], second[1], second[2]]);
    }

    #[test]
    fn envelope_parameter_updates_and_rejected_candidates_preserve_machine() {
        let graph = lua::read(GATED_ENVELOPE).unwrap();
        let mut host = Host::default();
        host.submit(&graph).unwrap();
        let machine = host.accepted().unwrap();
        let router = MidiRouter::from_machine(machine);
        let release_id = ParameterId::new("envelope", "release");
        let mut samples = [f64::NAN; 2];
        assert!(router.dispatch(&[0xB0, 7, 127]));
        machine.process(&mut samples);
        assert_eq!(samples, [0.125, 0.1875]);
        let error = host
            .submit(&lua::read(ENVELOPE_MISSING_GATE).unwrap())
            .unwrap_err();
        assert!(error.contains("envelope.gate"), "{error}");
        let error = host
            .submit(&lua::read(ENVELOPE_INVALID_ATTACK).unwrap())
            .unwrap_err();
        assert!(error.contains("envelope.attack must be in 0..1"), "{error}");
        let machine = host.accepted().unwrap();
        assert_eq!(machine.authored_value(&release_id), Some(0.25));
        assert!(machine.set_override(&release_id, Some(-0.1)).is_err());
        machine.set_override(&release_id, Some(1.0)).unwrap();
        assert!(router.dispatch(&[0xB0, 7, 0]));
        machine.process(&mut samples);
        assert_eq!(samples, [0.0; 2]);
        machine.set_override(&release_id, None).unwrap();
        assert_eq!(machine.effective_value(&release_id), Some(0.25));
    }

    #[test]
    fn sine_voice_is_gated_by_envelope_without_stopping_oscillator_phase() {
        let graph = lua::read(GATED_SINE_VOICE).unwrap();
        let mut machine = compile(&graph).unwrap();
        let router = MidiRouter::from_machine(&machine);
        let mut silent = [f64::NAN; 4];
        machine.process(&mut silent);
        assert_eq!(silent, [0.0; 4]);

        assert!(router.dispatch(&[0xB0, 7, 127]));
        let mut attack = [f64::NAN; 4];
        machine.process(&mut attack);
        let expected_attack = [0.0, 0.1875, 0.0, -0.234375];
        for (actual, expected) in attack.into_iter().zip(expected_attack) {
            assert!((actual - expected).abs() < 1e-12, "{actual} != {expected}");
        }

        assert!(router.dispatch(&[0xB0, 7, 0]));
        let mut release = [f64::NAN; 4];
        machine.process(&mut release);
        let expected_release = [0.0, 0.1318359375, 0.0, -0.07415771484375];
        for (actual, expected) in release.into_iter().zip(expected_release) {
            assert!((actual - expected).abs() < 1e-12, "{actual} != {expected}");
        }

        let mut split = compile(&graph).unwrap();
        let split_router = MidiRouter::from_machine(&split);
        split.process(&mut silent);
        assert!(split_router.dispatch(&[0xB0, 7, 127]));
        let mut first = [f64::NAN; 1];
        let mut second = [f64::NAN; 3];
        split.process(&mut first);
        split.process(&mut second);
        let split_attack = [first[0], second[0], second[1], second[2]];
        for (actual, expected) in split_attack.into_iter().zip(expected_attack) {
            assert!((actual - expected).abs() < 1e-12, "{actual} != {expected}");
        }
        assert!(split_router.dispatch(&[0xB0, 7, 0]));
        split.process(&mut first);
        split.process(&mut second);
        let split_release = [first[0], second[0], second[1], second[2]];
        for (actual, expected) in split_release.into_iter().zip(expected_release) {
            assert!((actual - expected).abs() < 1e-12, "{actual} != {expected}");
        }

        let mut phase_probe = compile(&graph).unwrap();
        let phase_router = MidiRouter::from_machine(&phase_probe);
        let mut one = [f64::NAN; 1];
        phase_probe.process(&mut one);
        assert_eq!(one, [0.0]);
        assert!(phase_router.dispatch(&[0xB0, 7, 127]));
        phase_probe.process(&mut one);
        assert!((one[0] - 0.125).abs() < 1e-12);
    }

    #[test]
    fn midi_notes_drive_pitch_and_gate_with_last_held_priority() {
        let graph = lua::read(MIDI_NOTE_VOICE).unwrap();
        let mut machine = compile(&graph).unwrap();
        let router = MidiRouter::from_machine(&machine);
        let pitch = ParameterId::new("oscillator", "frequency");
        let gate = ParameterId::new("gate", "value");
        let mut silent = [f64::NAN; 4];
        machine.process(&mut silent);
        assert_eq!(silent, [0.0; 4]);
        assert!(!router.dispatch(&[0x91, 69, 100]));
        assert!(!router.dispatch(&[0x90, 69]));
        assert!(!router.dispatch(&[0x80, 69, 0]));
        assert!(router.dispatch(&[0x90, 69, 100]));
        assert_eq!(machine.effective_value(&pitch), Some(440.0));
        assert_eq!(machine.effective_value(&gate), Some(1.0));
        assert_eq!(machine.authored_value(&gate), Some(0.0));
        let mut attack = [f64::NAN; 4];
        machine.process(&mut attack);
        for (i, actual) in attack.iter().enumerate() {
            let level = [0.5, 0.75, 0.875, 0.9375][i];
            let expected =
                0.25 * level * (std::f64::consts::TAU * 440.0 * (i + 4) as f64 / 48_000.0).sin();
            assert!((actual - expected).abs() < 1e-12, "{actual} != {expected}");
        }

        let clone = router.clone();
        assert!(clone.dispatch(&[0x90, 81, 80]));
        assert_eq!(machine.effective_value(&pitch), Some(880.0));
        assert!(router.dispatch(&[0x80, 81, 0]));
        assert_eq!(machine.effective_value(&pitch), Some(440.0));
        assert_eq!(machine.effective_value(&gate), Some(1.0));
        assert!(clone.dispatch(&[0x90, 81, 80]));
        assert!(router.dispatch(&[0x80, 69, 0]));
        assert_eq!(machine.effective_value(&pitch), Some(880.0));
        assert_eq!(machine.effective_value(&gate), Some(1.0));
        assert!(router.dispatch(&[0x90, 69, 70]));
        assert_eq!(machine.effective_value(&pitch), Some(440.0));
        assert!(router.dispatch(&[0x90, 69, 0]));
        assert_eq!(machine.effective_value(&pitch), Some(880.0));
        assert_eq!(machine.effective_value(&gate), Some(1.0));
        assert!(router.dispatch(&[0x80, 81, 0]));
        assert_eq!(machine.effective_value(&gate), Some(0.0));
        assert!(!router.dispatch(&[0x80, 81, 0]));
        let mut release = [f64::NAN; 4];
        machine.process(&mut release);
        assert!(release.iter().all(|sample| sample.is_finite()));
        assert!(release.iter().any(|sample| *sample != 0.0));

        for note in 0u8..=127 {
            assert!(router.dispatch(&[0x90, note, 1]));
        }
        assert_eq!(machine.effective_value(&gate), Some(1.0));
        assert!(router.dispatch(&[0x80, 127, 0]));
        let expected = 440.0 * 2.0_f64.powf((126.0 - 69.0) / 12.0);
        assert_eq!(machine.effective_value(&pitch), Some(expected));
        for note in (0u8..=126).rev() {
            assert!(router.dispatch(&[0x80, note, 0]));
        }
        assert_eq!(machine.effective_value(&gate), Some(0.0));
        let second_router = MidiRouter::from_machine(&machine);
        assert!(router.dispatch(&[0x90, 60, 100]));
        assert!(second_router.dispatch(&[0x80, 60, 0]));
        assert_eq!(machine.effective_value(&gate), Some(0.0));
    }

    #[test]
    fn invalid_midi_note_bindings_preserve_the_accepted_voice() {
        let graph = lua::read(MIDI_NOTE_VOICE).unwrap();
        let mut host = Host::default();
        host.submit(&graph).unwrap();
        let error = host
            .submit(&lua::read(MIDI_NOTE_MISSING_CONTROL).unwrap())
            .unwrap_err();
        assert!(error.contains("unknown gate control 'missing'"), "{error}");
        let error = host
            .submit(&lua::read(MIDI_NOTE_NARROW_PITCH).unwrap())
            .unwrap_err();
        assert!(
            error.contains("must cover all MIDI note frequencies"),
            "{error}"
        );
        let mut invalid = graph.clone();
        invalid.midi_notes[0].channel = 17;
        assert!(host.submit(&invalid).unwrap_err().contains("channel 1..16"));
        invalid = graph.clone();
        invalid.midi_notes.push(invalid.midi_notes[0].clone());
        assert!(
            host.submit(&invalid)
                .unwrap_err()
                .contains("duplicate MIDI note")
        );
        invalid = graph.clone();
        invalid.midi_notes[0].gate_control_id = "pitch".into();
        assert!(
            host.submit(&invalid)
                .unwrap_err()
                .contains("different parameters")
        );
        let machine = host.accepted().unwrap();
        let router = MidiRouter::from_machine(machine);
        assert!(router.dispatch(&[0x90, 69, 100]));
        let mut samples = [f64::NAN; 2];
        machine.process(&mut samples);
        assert!(samples[1] > 0.0);
    }

    #[test]
    fn midi_note_pair_is_published_together_at_audio_buffer_entry() {
        let graph = lua::read(CABLED_MIDI_NOTE_PAIR).unwrap();
        let mut machine = compile(&graph).unwrap();
        let router = MidiRouter::from_machine(&machine);
        let pitch = ParameterId::new("pitch_hz", "value");
        let gate = ParameterId::new("gate", "value");
        let state = machine.midi_note_state(1).unwrap();
        let mut one = [f64::NAN; 1];
        machine.process(&mut one);
        assert_eq!(one, [0.0]);

        state.begin_update();
        machine
            .parameter_handle(&pitch)
            .unwrap()
            .publish(Some(880.0))
            .unwrap();
        machine.process(&mut one);
        assert_eq!(one, [0.0]);
        machine
            .parameter_handle(&gate)
            .unwrap()
            .publish(Some(1.0))
            .unwrap();
        machine.process(&mut one);
        assert_eq!(one, [0.0]);

        state.end_update();
        let mut opened = [f64::NAN; 2];
        machine.process(&mut opened);
        let first = 0.125 * (std::f64::consts::TAU * 440.0 * 3.0 / 48_000.0).sin();
        let second = 0.1875 * (std::f64::consts::TAU * (440.0 * 3.0 + 880.0) / 48_000.0).sin();
        assert!((opened[0] - first).abs() < 1e-12);
        assert!((opened[1] - second).abs() < 1e-12);

        state.begin_update();
        machine
            .parameter_handle(&gate)
            .unwrap()
            .publish(Some(0.0))
            .unwrap();
        machine.process(&mut one);
        assert!(one[0] > 0.0);
        state.end_update();
        machine.process(&mut one);
        assert!(one[0] > 0.0);
        let before = state.epoch();
        assert!(router.dispatch(&[0x90, 69, 100]));
        assert_eq!(state.epoch(), before + 2);
    }

    #[test]
    fn immediate_midi_velocity_shares_note_snapshot_and_held_fallback() {
        let graph = lua::read(include_str!("../examples/50-midi-velocity.lua")).unwrap();
        let mut machine = compile(&graph).unwrap();
        let router = MidiRouter::from_machine(&machine);
        let state = machine.midi_note_state(1).unwrap();
        let pitch = machine
            .parameter_handle(&ParameterId::new("pitch_hz", "value"))
            .unwrap();
        let gate = machine
            .parameter_handle(&ParameterId::new("gate", "value"))
            .unwrap();
        let velocity = machine
            .parameter_handle(&ParameterId::new("velocity", "value"))
            .unwrap();
        state.begin_update();
        pitch.publish(Some(880.0)).unwrap();
        velocity.publish(Some(0.5)).unwrap();
        gate.publish(Some(1.0)).unwrap();
        let mut before = [f64::NAN; 4];
        machine.process(&mut before);
        assert_eq!(before, [0.0; 4]);
        state.end_update();
        let mut after = [f64::NAN; 2];
        machine.process(&mut after);
        let velocity_before = 1.0 - 0.99_f64.powi(4);
        let velocity_after = velocity_before + 0.01 * (0.5 - velocity_before);
        let expected =
            0.25 * 0.01 * velocity_after * (std::f64::consts::TAU * 440.0 * 4.0 / 48_000.0).sin();
        assert!((after[0] - expected).abs() < 1e-12);
        assert!(router.dispatch(&[0x90, 69, 64]));
        machine.process(&mut after);
        assert_eq!(velocity.override_value(), Some(64.0 / 127.0));
        assert!(router.dispatch(&[0x90, 81, 127]));
        machine.process(&mut after);
        assert_eq!(velocity.override_value(), Some(1.0));
        assert!(router.dispatch(&[0x80, 81, 0]));
        machine.process(&mut after);
        assert_eq!(velocity.override_value(), Some(64.0 / 127.0));
        assert!(router.dispatch(&[0x90, 69, 32]));
        machine.process(&mut after);
        assert_eq!(velocity.override_value(), Some(32.0 / 127.0));
    }

    #[test]
    fn shared_midi_note_targets_reject_without_replacing_machine() {
        let mut host = Host::default();
        host.submit(&lua::read(CABLED_MIDI_NOTE_PAIR).unwrap())
            .unwrap();
        let error = host
            .submit(&lua::read(MIDI_NOTE_SHARED_TARGET).unwrap())
            .unwrap_err();
        assert!(error.contains("distinct pitch and gate targets"), "{error}");
        let machine = host.accepted().unwrap();
        assert!(machine.midi_note_state(1).is_some());
        let mut samples = [f64::NAN; 2];
        machine.process(&mut samples);
        assert_eq!(samples, [0.0; 2]);
    }

    #[test]
    fn prepared_velocity_matches_live_queue_across_buffers() {
        let graph = lua::read(PREPARED_VELOCITY).unwrap();
        let events = [
            TimedVelocityState {
                note: TimedNoteState {
                    frame: 2,
                    channel: 1,
                    pitch_hz: 440.0,
                    gate: true,
                },
                velocity: 0.25,
            },
            TimedVelocityState {
                note: TimedNoteState {
                    frame: 4,
                    channel: 1,
                    pitch_hz: 440.0,
                    gate: true,
                },
                velocity: 0.75,
            },
            TimedVelocityState {
                note: TimedNoteState {
                    frame: 6,
                    channel: 1,
                    pitch_hz: 440.0,
                    gate: false,
                },
                velocity: 0.75,
            },
        ];
        let mut machine = compile(&graph).unwrap();
        let mut plan = TimedNotePlan::prepare_with_velocity(&machine, 0, &events).unwrap();
        let mut actual = [f64::NAN; 8];
        plan.render_next(&mut machine, &mut actual).unwrap();
        assert_eq!(actual, [0.0, 0.0, 0.25, 0.25, 0.75, 0.75, 0.0, 0.0]);

        let mut live = compile(&graph).unwrap();
        let (mut producer, mut consumer) = live_note_queue(&live, 0);
        for event in events {
            producer
                .try_push_velocity(event.note, event.velocity)
                .unwrap();
        }
        let mut split = [f64::NAN; 8];
        consumer.render_next(&mut live, &mut split[..3]).unwrap();
        consumer.render_next(&mut live, &mut split[3..5]).unwrap();
        consumer.render_next(&mut live, &mut split[5..]).unwrap();
        assert_eq!(actual, split);

        let mut unity = compile(&graph).unwrap();
        let mut unity_plan = TimedNotePlan::prepare(&unity, 0, &[events[0].note]).unwrap();
        unity_plan.render_next(&mut unity, &mut actual).unwrap();
        assert_eq!(actual, [0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0]);
    }

    #[test]
    fn invalid_prepared_velocity_rejects_without_changing_machine() {
        let graph = lua::read(PREPARED_VELOCITY).unwrap();
        let mut machine = compile(&graph).unwrap();
        let note = TimedNoteState {
            frame: 2,
            channel: 1,
            pitch_hz: 440.0,
            gate: true,
        };
        for velocity in [f64::NAN, -0.1, 1.1] {
            let error = TimedNotePlan::prepare_with_velocity(
                &machine,
                0,
                &[TimedVelocityState { note, velocity }],
            )
            .unwrap_err();
            assert!(
                error.contains("velocity must be finite and within 0..1"),
                "{error}"
            );
        }
        let mut samples = [f64::NAN; 4];
        machine.process(&mut samples);
        assert_eq!(samples, [0.0; 4]);
    }

    #[test]
    fn absolute_frame_note_states_render_identically_across_partitions() {
        let graph = lua::read(TIMED_NOTE_VOICE).unwrap();
        let events = [
            TimedNoteState {
                frame: 3,
                channel: 1,
                pitch_hz: 440.0,
                gate: true,
            },
            TimedNoteState {
                frame: 7,
                channel: 1,
                pitch_hz: 880.0,
                gate: true,
            },
            TimedNoteState {
                frame: 11,
                channel: 1,
                pitch_hz: 880.0,
                gate: false,
            },
        ];
        let mut whole_machine = compile(&graph).unwrap();
        let mut whole_plan = TimedNotePlan::prepare(&whole_machine, 0, &events).unwrap();
        let mut whole = [f64::NAN; 16];
        whole_plan
            .render_next(&mut whole_machine, &mut whole)
            .unwrap();
        assert_eq!(whole_plan.cursor(), 16);
        for (frame, actual) in whole.iter().enumerate() {
            let cycles = if frame < 7 {
                440.0 * frame as f64
            } else {
                440.0 * 7.0 + 880.0 * (frame - 7) as f64
            };
            let expected = if (3..11).contains(&frame) {
                0.25 * (std::f64::consts::TAU * cycles / 48_000.0).sin()
            } else {
                0.0
            };
            assert!(
                (actual - expected).abs() < 1e-12,
                "frame {frame}: {actual} != {expected}"
            );
        }

        let mut split_machine = compile(&graph).unwrap();
        let mut split_plan = TimedNotePlan::prepare(&split_machine, 0, &events).unwrap();
        let mut first = [f64::NAN; 5];
        let mut second = [f64::NAN; 4];
        let mut third = [f64::NAN; 7];
        split_plan
            .render_next(&mut split_machine, &mut first)
            .unwrap();
        split_plan
            .render_next(&mut split_machine, &mut second)
            .unwrap();
        split_plan
            .render_next(&mut split_machine, &mut third)
            .unwrap();
        assert_eq!(whole[..5], first);
        assert_eq!(whole[5..9], second);
        assert_eq!(whole[9..], third);
    }

    #[test]
    fn timed_note_boundary_and_same_frame_order_are_explicit() {
        let graph = lua::read(TIMED_NOTE_VOICE).unwrap();
        let mut machine = compile(&graph).unwrap();
        let events = [
            TimedNoteState {
                frame: 4,
                channel: 1,
                pitch_hz: 440.0,
                gate: true,
            },
            TimedNoteState {
                frame: 4,
                channel: 1,
                pitch_hz: 440.0,
                gate: false,
            },
            TimedNoteState {
                frame: 8,
                channel: 1,
                pitch_hz: 880.0,
                gate: true,
            },
        ];
        let mut plan = TimedNotePlan::prepare(&machine, 0, &events).unwrap();
        let mut before = [f64::NAN; 4];
        plan.render_next(&mut machine, &mut before).unwrap();
        assert_eq!(before, [0.0; 4]);
        let mut middle = [f64::NAN; 4];
        plan.render_next(&mut machine, &mut middle).unwrap();
        assert_eq!(middle, [0.0; 4]);
        let mut after = [f64::NAN; 1];
        plan.render_next(&mut machine, &mut after).unwrap();
        let expected = 0.25 * (std::f64::consts::TAU * 440.0 * 8.0 / 48_000.0).sin();
        assert!((after[0] - expected).abs() < 1e-12);
    }

    #[test]
    fn timed_note_state_survives_control_snapshots_between_blocks() {
        let graph = lua::read(TIMED_NOTE_VOICE).unwrap();
        let mut machine = compile(&graph).unwrap();
        let event = TimedNoteState {
            frame: 0,
            channel: 1,
            pitch_hz: 440.0,
            gate: true,
        };
        let mut plan = TimedNotePlan::prepare(&machine, 0, &[event]).unwrap();
        let mut sample = [f64::NAN; 1];
        plan.render_next(&mut machine, &mut sample).unwrap();
        assert_eq!(sample, [0.0]);
        machine
            .parameter_handle(&ParameterId::new("level", "gain"))
            .unwrap()
            .publish(Some(0.5))
            .unwrap();
        plan.render_next(&mut machine, &mut sample).unwrap();
        let expected = 0.5 * (std::f64::consts::TAU * 440.0 / 48_000.0).sin();
        assert!((sample[0] - expected).abs() < 1e-12);
    }

    #[test]
    fn invalid_timed_note_plans_and_render_calls_do_not_change_machine() {
        let graph = lua::read(TIMED_NOTE_VOICE).unwrap();
        let mut machine = compile(&graph).unwrap();
        let valid = TimedNoteState {
            frame: 3,
            channel: 1,
            pitch_hz: 440.0,
            gate: true,
        };
        assert!(
            TimedNotePlan::prepare(&machine, 0, &[valid, TimedNoteState { frame: 2, ..valid }])
                .unwrap_err()
                .contains("nondecreasing")
        );
        assert!(
            TimedNotePlan::prepare(
                &machine,
                0,
                &[TimedNoteState {
                    channel: 2,
                    ..valid
                }]
            )
            .unwrap_err()
            .contains("no MIDI note mapping")
        );
        assert!(
            TimedNotePlan::prepare(
                &machine,
                0,
                &[TimedNoteState {
                    pitch_hz: f64::NAN,
                    ..valid
                }]
            )
            .unwrap_err()
            .contains("finite")
        );
        assert!(
            TimedNotePlan::prepare(
                &machine,
                0,
                &[TimedNoteState {
                    pitch_hz: 20_000.0,
                    ..valid
                }]
            )
            .unwrap_err()
            .contains("within control")
        );
        assert!(
            TimedNotePlan::prepare(&machine, 0, &vec![valid; MAX_EVENTS + 1])
                .unwrap_err()
                .contains("at most")
        );
        let unbound = compile(&lua::read(TIMED_NOTE_NO_BINDING).unwrap()).unwrap();
        assert!(
            TimedNotePlan::prepare(&unbound, 0, &[valid])
                .unwrap_err()
                .contains("no MIDI note mapping")
        );

        let mut plan = TimedNotePlan::prepare(&machine, 0, &[valid]).unwrap();
        let mut other = compile(&graph).unwrap();
        let mut sample = [f64::NAN; 1];
        assert_eq!(
            plan.render_next(&mut other, &mut sample),
            Err(TimedRenderError::WrongMachine)
        );
        assert!(sample[0].is_nan());
        let mut oversized = vec![f64::NAN; MAX_FRAMES_PER_RENDER + 1];
        assert_eq!(
            plan.render_next(&mut machine, &mut oversized),
            Err(TimedRenderError::InvalidBufferSize)
        );
        assert_eq!(plan.cursor(), 0);
        plan.render_next(&mut machine, &mut sample).unwrap();
        assert_eq!(sample, [0.0]);

        let mut overflow = TimedNotePlan::prepare(&machine, u64::MAX - 1, &[]).unwrap();
        let mut two = [f64::NAN; 2];
        assert_eq!(
            overflow.render_next(&mut machine, &mut two),
            Err(TimedRenderError::FrameOverflow)
        );
        assert!(two.iter().all(|value| value.is_nan()));
    }

    #[test]
    fn midi_microseconds_anchor_to_timed_note_pcm() {
        let graph = lua::read(MIDI_TIME_ANCHOR).unwrap();
        let mut clock = MidiFrameClock::new(48_000, 3).unwrap();
        let first = clock.map(1_000_000, 0).unwrap();
        let second = clock.map(1_000_084, 0).unwrap();
        let third = clock.map(1_000_167, 0).unwrap();
        assert_eq!(
            first,
            FrameStamp {
                frame: 3,
                status: ClockStatus::Anchored
            }
        );
        assert_eq!(
            second,
            FrameStamp {
                frame: 7,
                status: ClockStatus::Mapped
            }
        );
        assert_eq!(
            third,
            FrameStamp {
                frame: 11,
                status: ClockStatus::Mapped
            }
        );
        let events = [
            TimedNoteState {
                frame: first.frame,
                channel: 1,
                pitch_hz: 440.0,
                gate: true,
            },
            TimedNoteState {
                frame: second.frame,
                channel: 1,
                pitch_hz: 880.0,
                gate: true,
            },
            TimedNoteState {
                frame: third.frame,
                channel: 1,
                pitch_hz: 880.0,
                gate: false,
            },
        ];
        let mut machine = compile(&graph).unwrap();
        let mut plan = TimedNotePlan::prepare(&machine, 0, &events).unwrap();
        let mut whole = [f64::NAN; 16];
        plan.render_next(&mut machine, &mut whole).unwrap();
        for (frame, actual) in whole.iter().enumerate() {
            let cycles = if frame < 7 {
                440.0 * frame as f64
            } else {
                440.0 * 7.0 + 880.0 * (frame - 7) as f64
            };
            let expected = if (3..11).contains(&frame) {
                0.5 * (std::f64::consts::TAU * cycles / 48_000.0).sin()
            } else {
                0.0
            };
            assert!(
                (actual - expected).abs() < 1e-12,
                "frame {frame}: {actual} != {expected}"
            );
        }
        let mut split_machine = compile(&graph).unwrap();
        let mut split_plan = TimedNotePlan::prepare(&split_machine, 0, &events).unwrap();
        let mut a = [f64::NAN; 5];
        let mut b = [f64::NAN; 4];
        let mut c = [f64::NAN; 7];
        split_plan.render_next(&mut split_machine, &mut a).unwrap();
        split_plan.render_next(&mut split_machine, &mut b).unwrap();
        split_plan.render_next(&mut split_machine, &mut c).unwrap();
        assert_eq!(whole[..5], a);
        assert_eq!(whole[5..9], b);
        assert_eq!(whole[9..], c);
    }

    #[test]
    fn midi_frame_clock_clamps_late_events_and_resets_on_backward_time() {
        let mut clock = MidiFrameClock::new(48_000, 3).unwrap();
        assert_eq!(clock.map(1_000_000, 0).unwrap().frame, 3);
        assert_eq!(clock.map(1_000_084, 0).unwrap().frame, 7);
        assert_eq!(
            clock.map(1_000_167, 20).unwrap(),
            FrameStamp {
                frame: 20,
                status: ClockStatus::Late
            }
        );
        assert_eq!(
            clock.map(1_000_168, 19),
            Err(ClockError::AudioFrameRegressed)
        );
        assert_eq!(
            clock.map(999_000, 30).unwrap(),
            FrameStamp {
                frame: 33,
                status: ClockStatus::Reset
            }
        );
        assert_eq!(
            clock.map(999_100, 30).unwrap(),
            FrameStamp {
                frame: 37,
                status: ClockStatus::Mapped
            }
        );
        assert_eq!(
            MidiFrameClock::new(0, 0).err(),
            Some(ClockError::ZeroSampleRate)
        );
        assert_eq!(
            MidiFrameClock::new(48_000, 48_001).err(),
            Some(ClockError::LeadTooLong)
        );
        let mut overflow = MidiFrameClock::new(48_000, 3).unwrap();
        assert_eq!(
            overflow.map(1_000_000, u64::MAX - 2),
            Err(ClockError::FrameOverflow)
        );
        assert_eq!(overflow.map(1_000_000, 0).unwrap().frame, 3);
        let mut far = MidiFrameClock::new(u32::MAX, 0).unwrap();
        assert_eq!(far.map(0, 0).unwrap().frame, 0);
        assert_eq!(far.map(u64::MAX, 0), Err(ClockError::FrameOverflow));
    }

    #[test]
    fn live_note_queue_matches_prepared_timeline_across_blocks() {
        let graph = lua::read(LIVE_NOTE_QUEUE).unwrap();
        let events = [
            TimedNoteState {
                frame: 3,
                channel: 1,
                pitch_hz: 440.0,
                gate: true,
            },
            TimedNoteState {
                frame: 7,
                channel: 1,
                pitch_hz: 880.0,
                gate: true,
            },
            TimedNoteState {
                frame: 11,
                channel: 1,
                pitch_hz: 880.0,
                gate: false,
            },
        ];
        let mut reference = compile(&graph).unwrap();
        let mut plan = TimedNotePlan::prepare(&reference, 0, &events).unwrap();
        let mut expected = [0.0; 16];
        plan.render_next(&mut reference, &mut expected).unwrap();

        let mut machine = compile(&graph).unwrap();
        let (mut producer, mut consumer) = live_note_queue(&machine, 0);
        for event in events {
            assert_eq!(producer.try_push(event).unwrap().frame, event.frame);
        }
        let mut a = [0.0; 5];
        let mut b = [0.0; 4];
        let mut c = [0.0; 7];
        assert_eq!(
            consumer.render_next(&mut machine, &mut a).unwrap().applied,
            1
        );
        assert_eq!(producer.first_unrendered_frame(), 5);
        assert_eq!(
            consumer.render_next(&mut machine, &mut b).unwrap().applied,
            1
        );
        assert_eq!(
            consumer.render_next(&mut machine, &mut c).unwrap().applied,
            1
        );
        assert_eq!(producer.first_unrendered_frame(), 16);
        assert_eq!(expected[..5], a);
        assert_eq!(expected[5..9], b);
        assert_eq!(expected[9..], c);
    }

    #[test]
    fn midi_clock_feeds_live_note_queue_using_published_cursor() {
        let graph = lua::read(LIVE_NOTE_QUEUE).unwrap();
        let mut machine = compile(&graph).unwrap();
        let (mut producer, mut consumer) = live_note_queue(&machine, 0);
        let mut clock = MidiFrameClock::new(48_000, 3).unwrap();
        for (time, pitch_hz, gate) in [
            (1_000_000, 440.0, true),
            (1_000_084, 880.0, true),
            (1_000_167, 880.0, false),
        ] {
            let stamp = clock.map(time, producer.first_unrendered_frame()).unwrap();
            assert!(
                !producer
                    .try_push(TimedNoteState {
                        frame: stamp.frame,
                        channel: 1,
                        pitch_hz,
                        gate,
                    })
                    .unwrap()
                    .clamped
            );
        }
        let mut pcm = [0.0; 16];
        assert_eq!(
            consumer
                .render_next(&mut machine, &mut pcm)
                .unwrap()
                .applied,
            3
        );
        for (frame, actual) in pcm.iter().enumerate() {
            let cycles = if frame < 7 {
                440.0 * frame as f64
            } else {
                440.0 * 7.0 + 880.0 * (frame - 7) as f64
            };
            let expected = if (3..11).contains(&frame) {
                0.375 * (std::f64::consts::TAU * cycles / 48_000.0).sin()
            } else {
                0.0
            };
            assert!((actual - expected).abs() < 1e-12, "frame {frame}");
        }
        assert_eq!(producer.first_unrendered_frame(), 16);
        let stamp = clock
            .map(1_000_168, producer.first_unrendered_frame())
            .unwrap();
        assert_eq!(stamp.status, ClockStatus::Late);
        assert_eq!(
            producer
                .try_push(TimedNoteState {
                    frame: stamp.frame,
                    channel: 1,
                    pitch_hz: 440.0,
                    gate: true,
                })
                .unwrap()
                .frame,
            16
        );
    }

    #[test]
    fn timed_midi_overflow_keeps_held_priority_and_clock_retryable() {
        let graph = lua::read(include_str!("../examples/47-device-timed-midi.lua")).unwrap();
        let mut machine = compile(&graph).unwrap();
        let (producer, mut consumer) = live_note_queue(&machine, 0);
        let mut midi = TimedMidiRouter::new(&machine, producer, 48_000, 3).unwrap();
        assert_eq!(midi.dispatch(100, &[0xB0, 7, 127]), Ok(true));
        assert_eq!(midi.dispatch(100, &[0x90, 69]), Ok(false));
        assert_eq!(midi.dispatch(100, &[0x91, 69, 100]), Ok(false));
        assert_eq!(midi.dispatch(100, &[0x80, 69, 0]), Ok(false));
        for time in 0..MAX_EVENTS as u64 {
            let note = if time % 2 == 0 { 69 } else { 81 };
            assert_eq!(midi.dispatch(time, &[0x90, note, 100]), Ok(true));
        }
        assert_eq!(
            midi.dispatch(64, &[0x90, 69, 100]),
            Err(TimedMidiError::Queue(PushError::Full))
        );
        assert_eq!(midi.first_unrendered_frame(), 0);
        let mut drained = [0.0; 8];
        assert_eq!(
            consumer
                .render_next(&mut machine, &mut drained)
                .unwrap()
                .applied,
            MAX_EVENTS
        );
        assert_eq!(midi.dispatch(64, &[0x90, 69, 100]), Ok(true));
        assert_eq!(midi.dispatch(65, &[0x80, 69, 0]), Ok(true));
        let mut next = [0.0; 1];
        assert_eq!(
            consumer
                .render_next(&mut machine, &mut next)
                .unwrap()
                .applied,
            2
        );
        assert_eq!(midi.first_unrendered_frame(), 9);
        assert_eq!(midi.dispatch(66, &[0x80, 69, 0]), Ok(false));
    }

    #[test]
    fn device_midi_overflow_releases_gate_and_allows_fresh_note() {
        let graph = lua::read(include_str!("../examples/52-midi-overflow-release.lua")).unwrap();
        let mut machine = compile(&graph).unwrap();
        let (producer, mut consumer) = live_note_queue(&machine, 0);
        let mut midi = TimedMidiRouter::new(&machine, producer, 48_000, 3).unwrap();
        assert_eq!(midi.dispatch_recovering(0, &[0x90, 69, 100]), Ok(true));
        let mut opening = [0.0; 4];
        consumer.render_next(&mut machine, &mut opening).unwrap();
        assert!(opening[3].abs() > 0.0);

        for offset in 0..MAX_EVENTS as u64 {
            let note = if offset % 2 == 0 { 81 } else { 69 };
            assert_eq!(
                midi.dispatch_recovering(1_000_000 + offset, &[0x90, note, 100]),
                Ok(true)
            );
        }
        assert_eq!(
            midi.dispatch_recovering(1_000_064, &[0x80, 69, 0]),
            Err(TimedMidiError::Queue(PushError::Full))
        );
        let mut released = [f64::NAN; 8];
        let report = consumer.render_next(&mut machine, &mut released).unwrap();
        assert_eq!(report.discarded, MAX_EVENTS);
        assert_eq!(report.applied, 0);
        assert_eq!(released, [0.0; 8]);
        assert_eq!(midi.dispatch_recovering(1, &[0x80, 69, 0]), Ok(false));
        assert_eq!(midi.dispatch_recovering(1, &[0x90, 81, 100]), Ok(true));
        let mut fresh = [0.0; 4];
        assert_eq!(
            consumer
                .render_next(&mut machine, &mut fresh)
                .unwrap()
                .applied,
            1
        );
        assert_eq!(fresh[..3], [0.0; 3]);
        assert!(fresh[3].abs() > 0.0);
    }

    #[test]
    fn device_overflow_recovery_keeps_other_channels_pending_notes() {
        let graph = lua::read(include_str!("../examples/49-channel-reset.lua")).unwrap();
        let mut machine = compile(&graph).unwrap();
        let (producer, mut consumer) = live_note_queue(&machine, 0);
        let mut midi = TimedMidiRouter::new(&machine, producer, 48_000, 3).unwrap();
        midi.dispatch_recovering(0, &[0x90, 69, 100]).unwrap();
        midi.dispatch_recovering(0, &[0x91, 69, 100]).unwrap();
        consumer.render_next(&mut machine, &mut [0.0; 4]).unwrap();
        for offset in 0..MAX_EVENTS as u64 {
            let note = if offset % 2 == 0 { 81 } else { 69 };
            midi.dispatch_recovering(2_000 + offset, &[0x91, note, 100])
                .unwrap();
        }
        assert_eq!(
            midi.dispatch_recovering(2_064, &[0x80, 69, 0]),
            Err(TimedMidiError::Queue(PushError::Full))
        );
        let mut actual = [0.0; 110];
        let report = consumer.render_next(&mut machine, &mut actual).unwrap();
        assert_eq!(report.discarded, 0);
        assert_eq!(report.applied, MAX_EVENTS);
        assert_eq!(midi.dispatch_recovering(1, &[0x80, 69, 0]), Ok(false));

        let mut reference = compile(&graph).unwrap();
        let mut plan = TimedNotePlan::prepare(
            &reference,
            0,
            &[
                TimedNoteState {
                    frame: 3,
                    channel: 1,
                    pitch_hz: 440.0,
                    gate: true,
                },
                TimedNoteState {
                    frame: 3,
                    channel: 2,
                    pitch_hz: 440.0,
                    gate: true,
                },
                TimedNoteState {
                    frame: 4,
                    channel: 1,
                    pitch_hz: 440.0,
                    gate: false,
                },
            ],
        )
        .unwrap();
        let mut expected = [0.0; 99];
        plan.render_next(&mut reference, &mut expected).unwrap();
        assert_eq!(actual[..95], expected[4..]);
    }

    #[test]
    fn invalid_velocity_bindings_and_values_preserve_accepted_voice() {
        let valid = lua::read(include_str!("../examples/50-midi-velocity.lua")).unwrap();
        let invalid = lua::read(include_str!("../examples/51-midi-velocity-invalid.lua")).unwrap();
        let mut host = Host::default();
        host.submit(&valid).unwrap();
        assert!(
            host.submit(&invalid)
                .unwrap_err()
                .contains("must cover 0..1")
        );
        let mut shared = valid.clone();
        shared.midi_notes[0].velocity_control_id = Some("pitch".into());
        assert!(
            host.submit(&shared)
                .unwrap_err()
                .contains("distinct parameter")
        );
        let mut missing = valid.clone();
        missing.midi_notes[0].velocity_control_id = Some("missing".into());
        assert!(
            host.submit(&missing)
                .unwrap_err()
                .contains("unknown velocity control")
        );

        let machine = host.accepted().unwrap();
        assert_eq!(
            machine.midi_notes()[0].velocity_control_id.as_deref(),
            Some("velocity")
        );
        let (mut producer, mut consumer) = live_note_queue(machine, 0);
        let state = TimedNoteState {
            frame: 0,
            channel: 1,
            pitch_hz: 440.0,
            gate: true,
        };
        assert_eq!(
            producer.try_push_velocity(state, f64::NAN),
            Err(PushError::InvalidVelocity)
        );
        assert_eq!(
            producer.try_push_velocity(state, 1.1),
            Err(PushError::InvalidVelocity)
        );
        assert_eq!(producer.try_push_velocity(state, 0.5).unwrap().frame, 0);
        let mut pcm = [0.0; 4];
        assert_eq!(consumer.render_next(machine, &mut pcm).unwrap().applied, 1);
        assert!(pcm[1].abs() > 0.0);
    }

    #[test]
    fn all_notes_off_releases_active_gate_and_discards_full_future_queue() {
        let graph = lua::read(include_str!("../examples/48-midi-reset.lua")).unwrap();
        let mut machine = compile(&graph).unwrap();
        let (producer, mut consumer) = live_note_queue(&machine, 0);
        let mut midi = TimedMidiRouter::new(&machine, producer, 48_000, 3).unwrap();
        assert_eq!(midi.dispatch(0, &[0x90, 69, 100]), Ok(true));
        let mut opening = [0.0; 4];
        consumer.render_next(&mut machine, &mut opening).unwrap();
        assert!(opening[3].abs() > 0.0);
        for offset in 0..MAX_EVENTS as u64 {
            let note = if offset % 2 == 0 { 81 } else { 69 };
            assert_eq!(
                midi.dispatch(1_000_000 + offset, &[0x90, note, 100]),
                Ok(true)
            );
        }
        assert_eq!(
            midi.dispatch(1_000_064, &[0x90, 81, 100]),
            Err(TimedMidiError::Queue(PushError::Full))
        );
        assert_eq!(midi.dispatch(1_000_065, &[0xB0, 123, 0]), Ok(true));
        let mut silence = [f64::NAN; 8];
        let report = consumer.render_next(&mut machine, &mut silence).unwrap();
        assert_eq!(report.discarded, MAX_EVENTS);
        assert_eq!(report.applied, 0);
        assert_eq!(silence, [0.0; 8]);
        assert_eq!(midi.dispatch(1, &[0x80, 69, 0]), Ok(false));
        assert_eq!(midi.dispatch(1, &[0x90, 81, 100]), Ok(true));
        let mut reopened = [0.0; 4];
        assert_eq!(
            consumer
                .render_next(&mut machine, &mut reopened)
                .unwrap()
                .applied,
            1
        );
        assert_eq!(reopened[..3], [0.0; 3]);
        assert!(reopened[3].abs() > 0.0);
        assert_eq!(midi.dispatch(2, &[0xFF]), Ok(true));
        let mut ended = [f64::NAN; 4];
        consumer.render_next(&mut machine, &mut ended).unwrap();
        assert_eq!(ended, [0.0; 4]);
        assert_eq!(midi.dispatch(3, &[0x90, 69, 100]), Ok(true));
        let mut open_again = [0.0; 1];
        consumer.render_next(&mut machine, &mut open_again).unwrap();
        assert!(open_again[0].abs() > 0.0);
        assert_eq!(midi.dispatch(4, &[0xB0, 120, 0]), Ok(true));
        let mut stopped = [f64::NAN; 1];
        consumer.render_next(&mut machine, &mut stopped).unwrap();
        assert_eq!(stopped, [0.0]);
        midi.release_all();
    }

    #[test]
    fn channel_reset_preserves_other_voice_and_its_future_event() {
        let graph = lua::read(include_str!("../examples/49-channel-reset.lua")).unwrap();
        let mut machine = compile(&graph).unwrap();
        let (producer, mut consumer) = live_note_queue(&machine, 0);
        let mut midi = TimedMidiRouter::new(&machine, producer, 48_000, 3).unwrap();
        assert_eq!(midi.dispatch(0, &[0x90, 69, 100]), Ok(true));
        assert_eq!(midi.dispatch(0, &[0x91, 69, 100]), Ok(true));
        let mut first = [0.0; 4];
        consumer.render_next(&mut machine, &mut first).unwrap();
        assert_eq!(midi.dispatch(2_000, &[0x90, 81, 100]), Ok(true));
        assert_eq!(midi.dispatch(2_100, &[0x91, 81, 100]), Ok(true));
        assert_eq!(midi.dispatch(2_101, &[0xB0, 123, 0]), Ok(true));
        let mut rest = [0.0; 110];
        let report = consumer.render_next(&mut machine, &mut rest).unwrap();
        assert_eq!(report.discarded, 1);
        assert_eq!(report.applied, 1);

        let mut reference = compile(&graph).unwrap();
        let mut plan = TimedNotePlan::prepare(
            &reference,
            0,
            &[
                TimedNoteState {
                    frame: 3,
                    channel: 1,
                    pitch_hz: 440.0,
                    gate: true,
                },
                TimedNoteState {
                    frame: 3,
                    channel: 2,
                    pitch_hz: 440.0,
                    gate: true,
                },
                TimedNoteState {
                    frame: 4,
                    channel: 1,
                    pitch_hz: 440.0,
                    gate: false,
                },
                TimedNoteState {
                    frame: 103,
                    channel: 2,
                    pitch_hz: 880.0,
                    gate: true,
                },
            ],
        )
        .unwrap();
        let mut expected = [0.0; 114];
        plan.render_next(&mut reference, &mut expected).unwrap();
        assert_eq!(first, expected[..4]);
        assert_eq!(rest, expected[4..]);
        assert!(rest[20].abs() > 0.0);
    }

    #[test]
    fn channel_reset_can_schedule_before_another_channels_deferred_event() {
        let graph = lua::read(include_str!("../examples/49-channel-reset.lua")).unwrap();
        let mut machine = compile(&graph).unwrap();
        let (mut producer, mut consumer) = live_note_queue(&machine, 0);
        let state = |frame, channel, pitch_hz, gate| TimedNoteState {
            frame,
            channel,
            pitch_hz,
            gate,
        };
        producer.try_push(state(3, 1, 440.0, true)).unwrap();
        producer.try_push(state(3, 2, 440.0, true)).unwrap();
        let mut actual = [0.0; 114];
        consumer
            .render_next(&mut machine, &mut actual[..4])
            .unwrap();
        producer.try_push(state(99, 1, 880.0, true)).unwrap();
        producer.try_push(state(103, 2, 880.0, true)).unwrap();
        producer.emergency_release(1).unwrap();
        let report = consumer
            .render_next(&mut machine, &mut actual[4..12])
            .unwrap();
        assert_eq!(report.discarded, 1);
        assert_eq!(report.applied, 0);
        let outcome = producer.try_push(state(15, 1, 880.0, true)).unwrap();
        assert_eq!(outcome.frame, 15);
        assert!(!outcome.clamped);
        let report = consumer
            .render_next(&mut machine, &mut actual[12..])
            .unwrap();
        assert_eq!(report.applied, 2);

        let mut reference = compile(&graph).unwrap();
        let mut plan = TimedNotePlan::prepare(
            &reference,
            0,
            &[
                state(3, 1, 440.0, true),
                state(3, 2, 440.0, true),
                state(4, 1, 440.0, false),
                state(15, 1, 880.0, true),
                state(103, 2, 880.0, true),
            ],
        )
        .unwrap();
        let mut expected = [0.0; 114];
        plan.render_next(&mut reference, &mut expected).unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn deferred_future_events_still_use_queue_capacity() {
        let graph = lua::read(include_str!("../examples/49-channel-reset.lua")).unwrap();
        let mut machine = compile(&graph).unwrap();
        let (mut producer, mut consumer) = live_note_queue(&machine, 0);
        let state = TimedNoteState {
            frame: 100,
            channel: 1,
            pitch_hz: 440.0,
            gate: true,
        };
        for _ in 0..MAX_EVENTS {
            producer.try_push(state).unwrap();
        }
        assert_eq!(
            consumer
                .render_next(&mut machine, &mut [0.0; 1])
                .unwrap()
                .applied,
            0
        );
        assert_eq!(producer.try_push(state), Err(PushError::Full));
        let mut through_future = [0.0; 100];
        assert_eq!(
            consumer
                .render_next(&mut machine, &mut through_future)
                .unwrap()
                .applied,
            MAX_EVENTS
        );
        assert_eq!(producer.try_push(state).unwrap().frame, 101);
    }

    #[test]
    fn reset_discards_a_deferred_event_after_many_other_channel_events() {
        let graph = lua::read(include_str!("../examples/49-channel-reset.lua")).unwrap();
        let mut machine = compile(&graph).unwrap();
        let (mut producer, mut consumer) = live_note_queue(&machine, 0);
        producer
            .try_push(TimedNoteState {
                frame: 10_000,
                channel: 1,
                pitch_hz: 440.0,
                gate: true,
            })
            .unwrap();
        for frame in 0..130 {
            producer
                .try_push(TimedNoteState {
                    frame,
                    channel: 2,
                    pitch_hz: 440.0,
                    gate: true,
                })
                .unwrap();
            assert_eq!(
                consumer
                    .render_next(&mut machine, &mut [0.0; 1])
                    .unwrap()
                    .applied,
                1
            );
        }
        producer.emergency_release(1).unwrap();
        let report = consumer.render_next(&mut machine, &mut [0.0; 1]).unwrap();
        assert_eq!(report.discarded, 1);
        assert_eq!(report.applied, 0);
        assert_eq!(
            producer
                .try_push(TimedNoteState {
                    frame: 131,
                    channel: 1,
                    pitch_hz: 880.0,
                    gate: true,
                })
                .unwrap()
                .frame,
            131
        );
    }

    #[test]
    fn live_note_queue_clamps_late_events_and_preserves_capacity() {
        let graph = lua::read(LIVE_NOTE_QUEUE).unwrap();
        let mut machine = compile(&graph).unwrap();
        let (mut producer, mut consumer) = live_note_queue(&machine, 0);
        let note = |frame, gate| TimedNoteState {
            frame,
            channel: 1,
            pitch_hz: 440.0,
            gate,
        };
        assert_eq!(producer.try_push(note(0, true)).unwrap().frame, 0);
        let mut initial = [0.0; 4];
        consumer.render_next(&mut machine, &mut initial).unwrap();
        assert_eq!(producer.try_push(note(1, false)).unwrap().frame, 4);
        let mut released = [f64::NAN; 4];
        assert_eq!(
            consumer
                .render_next(&mut machine, &mut released)
                .unwrap()
                .late,
            0
        );
        assert_eq!(released, [0.0; 4]);

        let mut old = compile(&graph).unwrap();
        let (mut producer, mut consumer) = live_note_queue(&machine, 8);
        assert_eq!(
            consumer.render_next(&mut old, &mut [0.0; 1]),
            Err(TimedRenderError::WrongMachine)
        );
        assert_eq!(consumer.cursor(), 8);
        assert_eq!(
            producer.try_push(TimedNoteState {
                channel: 2,
                ..note(8, true)
            }),
            Err(PushError::UnmappedChannel)
        );
        assert_eq!(
            producer.try_push(TimedNoteState {
                pitch_hz: f64::NAN,
                ..note(8, true)
            }),
            Err(PushError::InvalidPitch)
        );
        for _ in 0..MAX_EVENTS {
            producer.try_push(note(8, true)).unwrap();
        }
        assert_eq!(producer.try_push(note(8, false)), Err(PushError::Full));
        let mut one = [0.0; 1];
        assert_eq!(
            consumer
                .render_next(&mut machine, &mut one)
                .unwrap()
                .applied,
            MAX_EVENTS
        );
        assert_eq!(producer.try_push(note(9, false)).unwrap().frame, 9);
        assert_eq!(
            consumer.render_next(&mut machine, &mut []),
            Err(TimedRenderError::InvalidBufferSize)
        );
        assert_eq!(consumer.cursor(), 9);
        let (mut overflow_producer, mut overflow_consumer) = live_note_queue(&machine, u64::MAX);
        assert_eq!(
            overflow_consumer.render_next(&mut machine, &mut [0.0; 1]),
            Err(TimedRenderError::FrameOverflow)
        );
        assert_eq!(overflow_producer.first_unrendered_frame(), u64::MAX);
        assert_eq!(
            overflow_producer.try_push(note(0, true)).unwrap().frame,
            u64::MAX
        );
    }

    #[test]
    fn live_revisions_keep_the_initial_sample_rate() {
        let initial = compile_at_rate(&lua::read(BASIC).unwrap(), 96_000.0).unwrap();
        let (mut runtime, control) = LiveRuntime::new(initial);
        control.submit(&lua::read(SINE).unwrap()).unwrap();
        let mut samples = [0.0; 2];
        assert!(runtime.process(&mut samples));
        assert_eq!(runtime.current().sample_rate(), 96_000.0);
        assert_eq!(samples[0], 0.0);
        assert!((samples[1] - 0.25 * std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-12);
    }

    #[test]
    fn instrument_control_resolves_shared_parameter_identity() {
        let graph = lua::read(CONTROLS).unwrap();
        let mut machine = compile(&graph).unwrap();
        assert_eq!(
            machine.control_ids().collect::<Vec<_>>(),
            ["left-value", "level"]
        );
        let control = machine.control_handle("level").unwrap();
        assert_eq!(control.id(), "level");
        assert_eq!(control.target(), &ParameterId::new("gain", "gain"));
        assert_eq!(control.range(), (0.0, 1.0));
        let mut samples = [0.0; 8];
        machine.process(&mut samples);
        assert_eq!(samples, [0.1875; 8]);
        assert!(control.publish(1.5).is_err());
        assert!(control.publish(f64::NAN).is_err());
        machine.process(&mut samples);
        assert_eq!(samples, [0.1875; 8]);
        control.publish(0.75).unwrap();
        machine.process(&mut samples);
        assert_eq!(samples, [0.28125; 8]);
        control.clear().unwrap();
        machine.process(&mut samples);
        assert_eq!(samples, [0.1875; 8]);
    }

    #[test]
    fn invalid_control_declarations_reject_without_replacing_machine() {
        let mut host = Host::default();
        let graph = lua::read(CONTROLS).unwrap();
        host.submit(&graph).unwrap();
        let mut invalid = graph.clone();
        invalid.controls[0].target.name = "missing".into();
        assert!(
            host.submit(&invalid)
                .unwrap_err()
                .contains("unknown parameter")
        );
        invalid = graph.clone();
        invalid.controls.push(invalid.controls[0].clone());
        assert!(
            host.submit(&invalid)
                .unwrap_err()
                .contains("duplicate control ID")
        );
        invalid = graph.clone();
        invalid.controls[0].max = f64::INFINITY;
        assert!(
            host.submit(&invalid)
                .unwrap_err()
                .contains("finite min < max")
        );
        invalid.controls[0].max = 0.0;
        assert!(
            host.submit(&invalid)
                .unwrap_err()
                .contains("finite min < max")
        );
        let mut samples = [0.0; 4];
        host.accepted().unwrap().process(&mut samples);
        assert_eq!(samples, [0.1875; 4]);
    }

    #[test]
    fn old_control_handle_cannot_affect_new_revision() {
        let graph = lua::read(CONTROLS).unwrap();
        let mut host = Host::default();
        host.submit(&graph).unwrap();
        let old = host.accepted().unwrap().control_handle("level").unwrap();
        let mut replacement = graph.clone();
        replacement
            .modules
            .iter_mut()
            .find(|m| m.id == "gain")
            .unwrap()
            .values
            .insert("gain".into(), 0.25);
        host.submit(&replacement).unwrap();
        old.publish(1.0).unwrap();
        let mut samples = [0.0; 4];
        host.accepted().unwrap().process(&mut samples);
        assert_eq!(samples, [0.09375; 4]);
    }

    #[test]
    fn declared_midi_cc_uses_the_named_control() {
        let mut machine = compile(&lua::read(MIDI).unwrap()).unwrap();
        let router = MidiRouter::from_machine(&machine);
        let mut samples = [0.0; 4];
        machine.process(&mut samples);
        assert_eq!(samples, [0.1875; 4]);
        for ignored in [
            [0x90, 7, 127],
            [0xB1, 7, 127],
            [0xB0, 8, 127],
            [0xB0, 7, 128],
        ] {
            assert!(!router.dispatch(&ignored));
        }
        assert!(!router.dispatch(&[0xB0, 7]));
        machine.process(&mut samples);
        assert_eq!(samples, [0.1875; 4]);
        assert!(router.dispatch(&[0xB0, 7, 127]));
        machine.process(&mut samples);
        assert_eq!(samples, [0.375; 4]);
        assert!(router.dispatch(&[0xB0, 7, 0]));
        machine.process(&mut samples);
        assert_eq!(samples, [0.0; 4]);
        assert!(router.dispatch(&[0xB0, 7, 64]));
        machine.process(&mut samples);
        let expected = 0.375 * 64.0 / 127.0;
        assert!(
            samples
                .iter()
                .all(|sample| (*sample - expected).abs() < 1e-12)
        );
    }

    #[test]
    fn invalid_midi_mappings_reject_without_corrupting_accepted_graph() {
        let graph = lua::read(MIDI).unwrap();
        let mut host = Host::default();
        host.submit(&graph).unwrap();
        let mut invalid = graph.clone();
        invalid.midi_ccs[0].channel = 17;
        assert!(host.submit(&invalid).unwrap_err().contains("channel 1..16"));
        invalid = graph.clone();
        invalid.midi_ccs[0].number = 128;
        assert!(host.submit(&invalid).unwrap_err().contains("number 0..127"));
        invalid = graph.clone();
        invalid.midi_ccs[0].control_id = "missing".into();
        assert!(
            host.submit(&invalid)
                .unwrap_err()
                .contains("unknown control")
        );
        invalid = graph.clone();
        invalid.midi_ccs.push(invalid.midi_ccs[0].clone());
        assert!(
            host.submit(&invalid)
                .unwrap_err()
                .contains("duplicate MIDI CC")
        );
        let mut samples = [0.0; 4];
        host.accepted().unwrap().process(&mut samples);
        assert_eq!(samples, [0.1875; 4]);
    }

    #[test]
    fn old_midi_router_cannot_change_replacement_revision() {
        let graph = lua::read(MIDI).unwrap();
        let mut host = Host::default();
        host.submit(&graph).unwrap();
        let router = MidiRouter::from_machine(host.accepted().unwrap());
        let mut replacement = graph.clone();
        replacement
            .modules
            .iter_mut()
            .find(|module| module.id == "gain")
            .unwrap()
            .values
            .insert("gain".into(), 0.25);
        host.submit(&replacement).unwrap();
        assert!(router.dispatch(&[0xB0, 7, 127]));
        let mut samples = [0.0; 4];
        host.accepted().unwrap().process(&mut samples);
        assert_eq!(samples, [0.09375; 4]);
    }

    #[test]
    fn lua_midi_button_hooks_invoke_authored_actions_and_consume_edges() {
        let graph = lua::read(include_str!("../examples/68-midi-buttons.lua")).unwrap();
        let sequence = graph.actions[0].sequencer.clone();
        let mut machine = compile(&graph).unwrap();
        let router = MidiRouter::from_machine(&machine);
        let mut samples = [0.0; 8];
        machine.process(&mut samples);
        assert!(sequence.playing());

        assert!(router.dispatch(&[0x99, 36, 100]));
        machine.process(&mut samples);
        assert!(!sequence.playing());
        assert!(!router.dispatch(&[0x99, 37, 100]));

        assert!(!router.dispatch(&[0xB0, 64, 127]));
        assert!(router.dispatch(&[0xB0, 64, 0]));
        machine.process(&mut samples);
        assert!(sequence.playing());

        assert!(router.dispatch(&[0xC0, 3]));
        machine.process(&mut samples);
        assert_eq!(sequence.current_step(), 0);

        assert!(router.dispatch(&[0xf0, 0x7d, 0x4d, 0x32, 0x01, 0xf7]));
        machine.process(&mut samples);
        assert!(!sequence.playing());
    }
}
pub mod apc;
