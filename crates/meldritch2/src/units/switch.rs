use crate::contract::{Contract, Domain, FanIn, InputPort, OutputPort, Param, Role};

fn process(_: &mut [f64], input: &[f64], params: &[f64], _: f64, outputs: &mut [f64]) {
    outputs[0] = if input[2] >= params[0] {
        input[1]
    } else {
        input[0]
    };
}

pub const CONTRACT: Contract = Contract {
    name: "switch",
    role: Role::Processor,
    process,
    state_len: 0,
    inputs: &[
        InputPort {
            name: "left",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: None,
        },
        InputPort {
            name: "right",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: None,
        },
        InputPort {
            name: "select",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: None,
        },
    ],
    outputs: &[OutputPort {
        name: "out",
        domain: Domain::MonoAudio,
    }],
    params: &[Param {
        name: "threshold",
        default: 0.5,
        range: None,
    }],
};
