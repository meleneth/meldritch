use crate::contract::{Contract, Domain, FanIn, Param, Port, Role};

fn process(_: &mut [f64], input: &[f64], params: &[f64], _: f64) -> f64 {
    input[0] * params[0] + params[1]
}

pub const CONTRACT: Contract = Contract {
    name: "affine",
    role: Role::Processor,
    process,
    state_len: 0,
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
