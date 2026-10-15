use crate::contract::{Contract, Domain, FanIn, InputPort, OutputPort, Param, Role};

fn process(_: &mut [f64], input: &[f64], params: &[f64], _: f64, outputs: &mut [f64]) {
    outputs[0] = input[0] * params[0] + params[1];
}

pub const CONTRACT: Contract = Contract {
    name: "affine",
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
    params: &[
        Param {
            name: "scale",
            default: 1.0,
            range: None,
        },
        Param {
            name: "offset",
            default: 0.0,
            range: None,
        },
    ],
};
