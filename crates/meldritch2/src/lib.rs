pub mod compiler;
pub mod contract;
pub mod device;
pub mod live;
pub mod lua;
pub mod midi;
pub mod midi_device;
pub mod model;
pub mod panel;
pub mod tui;
mod units;

#[cfg(test)]
mod tests {
    use crate::{
        compiler::{Host, compile, compile_at_rate},
        contract::{self, FanIn, Role},
        live::LiveRuntime,
        lua,
        midi::MidiRouter,
        model::ParameterId,
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
        assert_eq!(contract::all().len(), 11);
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
}
