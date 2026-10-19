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
    } else if !gate {
        state[1] = 0.0; // release
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
        state[0] = (state[0] - 1.0 / (params[3] * sample_rate)).max(0.0);
    }
    state[2] = f64::from(gate);
    outputs[0] = state[0];
}

pub const CONTRACT: Contract = Contract {
    name: "adsr",
    role: Role::Processor,
    process,
    state_len: 3,
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
