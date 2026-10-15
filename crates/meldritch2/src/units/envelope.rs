use crate::contract::{Contract, Domain, FanIn, InputPort, OutputPort, Param, Role};

fn process(state: &mut [f64], input: &[f64], params: &[f64], _: f64, outputs: &mut [f64]) {
    let target = if input[0] >= 0.5 { 1.0 } else { 0.0 };
    let response = if target > state[0] {
        params[0]
    } else {
        params[1]
    };
    state[0] += response * (target - state[0]);
    outputs[0] = state[0];
}

pub const CONTRACT: Contract = Contract {
    name: "envelope",
    role: Role::Processor,
    process,
    state_len: 1,
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
            default: 0.5,
            range: Some((0.0, 1.0)),
        },
        Param {
            name: "release",
            default: 0.25,
            range: Some((0.0, 1.0)),
        },
    ],
};
