use crate::contract::{Contract, Domain, FanIn, InputPort, OutputPort, Param, Role};

pub(crate) fn coefficient(seconds: f64, sample_rate: f64) -> f64 {
    (-1.0 / (seconds * sample_rate)).exp()
}

fn process(
    state: &mut [f64],
    input: &[f64],
    params: &[f64],
    sample_rate: f64,
    outputs: &mut [f64],
) {
    let target = if input[0].is_finite() {
        input[0].abs()
    } else {
        0.0
    };
    let time = if target > state[0] {
        params[0]
    } else {
        params[1]
    };
    let coefficient = coefficient(time, sample_rate);
    state[0] = target + coefficient * (state[0] - target);
    outputs[0] = state[0];
}

pub const CONTRACT: Contract = Contract {
    name: "envelope_follower",
    role: Role::Processor,
    process,
    state_len: 1,
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
    params: &[
        Param {
            name: "attack",
            default: 0.005,
            range: Some((0.000_01, 10.0)),
        },
        Param {
            name: "release",
            default: 0.150,
            range: Some((0.000_01, 10.0)),
        },
    ],
};

#[cfg(test)]
mod tests {
    use super::process;
    #[test]
    fn attack_and_release_have_distinct_real_response() {
        let mut state = [0.0];
        let mut out = [0.0];
        for _ in 0..480 {
            process(&mut state, &[1.0], &[0.01, 0.2], 48_000.0, &mut out);
        }
        assert!(out[0] > 0.6);
        let peak = out[0];
        for _ in 0..480 {
            process(&mut state, &[0.0], &[0.01, 0.2], 48_000.0, &mut out);
        }
        assert!(out[0] < peak && out[0] > peak * 0.9);
    }
}
