use crate::contract::{Contract, Domain, FanIn, Param, Port, Role};

fn process(state: &mut [f64], input: &[f64], params: &[f64], _: f64, outputs: &mut [f64]) {
    if state[0] < params[0] && input[1] >= params[0] {
        state[1] = input[0];
    }
    state[0] = input[1];
    outputs[0] = state[1];
}

pub const CONTRACT: Contract = Contract {
    name: "sample_hold",
    role: Role::Processor,
    process,
    state_len: 2,
    inputs: &[
        Port {
            name: "signal",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: None,
        },
        Port {
            name: "trigger",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: None,
        },
    ],
    outputs: &[Port {
        name: "out",
        domain: Domain::MonoAudio,
        fan_in: FanIn::One,
        unconnected: None,
    }],
    params: &[Param {
        name: "threshold",
        default: 0.5,
        range: None,
    }],
};
