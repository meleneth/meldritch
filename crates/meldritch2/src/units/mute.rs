use crate::contract::{Contract, Domain, FanIn, InputPort, OutputPort, Param, Role};

fn process(_: &mut [f64], input: &[f64], params: &[f64], _: f64, outputs: &mut [f64]) {
    outputs[0] = input[0] * (1.0 - params[0].clamp(0.0, 1.0));
}

pub const CONTRACT: Contract = Contract {
    name: "mute",
    role: Role::Processor,
    process,
    state_len: 0,
    inputs: &[InputPort {
        name: "input",
        domain: Domain::MonoAudio,
        fan_in: FanIn::One,
        unconnected: None,
    }],
    outputs: &[OutputPort {
        name: "out",
        domain: Domain::MonoAudio,
    }],
    params: &[Param {
        name: "muted",
        default: 0.0,
        range: Some((0.0, 1.0)),
    }],
};

#[cfg(test)]
mod tests {
    use super::process;

    #[test]
    fn mute_is_a_bounded_control_multiplier() {
        let mut output = [0.0];
        process(&mut [], &[0.75], &[0.0], 48_000.0, &mut output);
        assert_eq!(output[0], 0.75);
        process(&mut [], &[0.75], &[1.0], 48_000.0, &mut output);
        assert_eq!(output[0], 0.0);
        process(&mut [], &[0.75], &[0.25], 48_000.0, &mut output);
        assert_eq!(output[0], 0.5625);
    }
}
