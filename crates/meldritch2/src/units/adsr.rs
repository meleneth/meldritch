use crate::contract::{Contract, Domain, FanIn, InputPort, OutputPort, Param, Role};

fn process(
    state: &mut [f64],
    input: &[f64],
    params: &[f64],
    sample_rate: f64,
    outputs: &mut [f64],
) {
    let gate = input[0] >= 0.5;
    let previous_gate = state[2] >= 0.5;
    if gate && !previous_gate {
        state[1] = 1.0; // attack
    } else if !gate && previous_gate {
        state[1] = 0.0; // release
        state[3] = state[0] / (params[3] * sample_rate);
    }
    if gate {
        if state[1] == 1.0 {
            state[0] = (state[0] + 1.0 / (params[0] * sample_rate)).min(1.0);
            if state[0] >= 1.0 {
                state[1] = 2.0; // decay/sustain
            }
        } else {
            let decrement = (1.0 - params[2]) / (params[1] * sample_rate);
            state[0] = (state[0] - decrement).max(params[2]);
        }
    } else {
        state[0] = (state[0] - state[3]).max(0.0);
    }
    state[2] = f64::from(gate);
    outputs[0] = state[0];
}

pub const CONTRACT: Contract = Contract {
    name: "adsr",
    role: Role::Processor,
    process,
    state_len: 4,
    inputs: &[InputPort {
        name: "gate",
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
            default: 0.01,
            range: Some((0.000_01, 60.0)),
        },
        Param {
            name: "decay",
            default: 0.2,
            range: Some((0.000_01, 60.0)),
        },
        Param {
            name: "sustain",
            default: 0.65,
            range: Some((0.0, 1.0)),
        },
        Param {
            name: "release",
            default: 0.4,
            range: Some((0.000_01, 60.0)),
        },
    ],
};

#[cfg(test)]
mod tests {
    use super::process;

    fn tick(state: &mut [f64; 4], gate: f64, params: [f64; 4], rate: f64) -> f64 {
        let mut output = [0.0];
        process(state, &[gate], &params, rate, &mut output);
        output[0]
    }

    #[test]
    fn phases_and_note_off_during_attack_use_explicit_seconds() {
        let mut state = [0.0; 4];
        let params = [0.1, 0.2, 0.5, 0.4];
        for _ in 0..50 {
            tick(&mut state, 1.0, params, 1_000.0);
        }
        assert!((state[0] - 0.5).abs() < 1e-9);
        for _ in 0..400 {
            tick(&mut state, 0.0, params, 1_000.0);
        }
        assert!(state[0] < 1e-12);

        for _ in 0..100 {
            tick(&mut state, 1.0, params, 1_000.0);
        }
        assert_eq!(state[0], 1.0);
        for _ in 0..200 {
            tick(&mut state, 1.0, params, 1_000.0);
        }
        assert!((state[0] - 0.5).abs() < 1e-9);
    }

    #[test]
    fn retrigger_and_live_parameters_are_deterministic() {
        let mut state = [0.0; 4];
        let mut params = [0.2, 0.2, 0.5, 0.2];
        for _ in 0..50 {
            tick(&mut state, 1.0, params, 1_000.0);
        }
        tick(&mut state, 0.0, params, 1_000.0);
        let release_level = state[0];
        tick(&mut state, 1.0, params, 1_000.0);
        assert!(state[0] > release_level);
        params[0] = 0.01;
        for _ in 0..8 {
            tick(&mut state, 1.0, params, 1_000.0);
        }
        assert_eq!(state[0], 1.0);
    }
}
