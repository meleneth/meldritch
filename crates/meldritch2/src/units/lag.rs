use crate::contract::{Contract, Domain, FanIn, Param, Port, Role};

fn process(state: &mut [f64], input: &[f64], params: &[f64], _: f64, outputs: &mut [f64]) {
    state[0] += params[0] * (input[0] - state[0]);
    outputs[0] = state[0];
}

pub const CONTRACT: Contract = Contract {
    name: "lag",
    role: Role::Processor,
    process,
    state_len: 1,
    inputs: &[Port {
        name: "input",
        domain: Domain::MonoAudio,
        fan_in: FanIn::One,
        unconnected: None,
    }],
    outputs: &[Port {
        name: "out",
        domain: Domain::MonoAudio,
        fan_in: FanIn::One,
        unconnected: None,
    }],
    params: &[Param {
        name: "response",
        default: 0.5,
        range: Some((0.0, 1.0)),
    }],
};
