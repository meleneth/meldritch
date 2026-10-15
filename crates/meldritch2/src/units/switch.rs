use crate::contract::{Contract, Domain, FanIn, Param, Port, Role};

fn process(_: &mut [f64], input: &[f64], params: &[f64], _: f64) -> f64 {
    if input[2] >= params[0] {
        input[1]
    } else {
        input[0]
    }
}

pub const CONTRACT: Contract = Contract {
    name: "switch",
    role: Role::Processor,
    process,
    state_len: 0,
    inputs: &[
        Port {
            name: "left",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: None,
        },
        Port {
            name: "right",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: None,
        },
        Port {
            name: "select",
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
