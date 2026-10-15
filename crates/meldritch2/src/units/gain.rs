use crate::contract::{Contract, Domain, FanIn, Param, Port, Role};

fn process(_: &mut [f64], input: &[f64], params: &[f64], _: f64, outputs: &mut [f64]) {
    outputs[0] = input[0] * params[0] * input[1];
}

pub const CONTRACT: Contract = Contract {
    name: "gain",
    role: Role::Processor,
    process,
    state_len: 0,
    inputs: &[
        Port {
            name: "input",
            domain: Domain::MonoAudio,
            fan_in: FanIn::Sum,
            unconnected: None,
        },
        Port {
            name: "mod",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: Some(1.0),
        },
    ],
    outputs: &[Port {
        name: "out",
        domain: Domain::MonoAudio,
        fan_in: FanIn::One,
        unconnected: None,
    }],
    params: &[Param {
        name: "gain",
        default: 1.0,
        range: None,
    }],
};
