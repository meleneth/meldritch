pub mod compiler;
pub mod contract;
pub mod live;
pub mod lua;
pub mod model;

#[cfg(test)]
mod tests {
    use crate::{
        compiler::{Host, compile, compile_at_rate},
        contract,
        live::LiveRuntime,
        lua,
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
        assert_eq!(contract::all().len(), 5);
        let gain = contract::find("gain").unwrap();
        assert_eq!(gain.inputs[0].name, "input");
        assert_eq!(gain.outputs[0].name, "out");
        assert_eq!(gain.params[0].name, "gain");
        let mix = contract::find("mix").unwrap();
        assert_eq!(
            mix.inputs.iter().map(|port| port.name).collect::<Vec<_>>(),
            ["left", "right"]
        );
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
    fn invalid_parameters_and_fan_in_are_diagnosed() {
        let mut graph = lua::read(BASIC).unwrap();
        graph.modules[1].values.insert("unknown".into(), 1.0);
        assert!(compile(&graph).unwrap_err().contains("not a parameter"));
        graph.modules[1].values.remove("unknown");
        graph.modules[1].values.insert("gain".into(), f64::NAN);
        assert!(compile(&graph).unwrap_err().contains("must be finite"));
        graph.modules[1].values.insert("gain".into(), 0.5);
        graph.connections.push(graph.connections[0].clone());
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
}
