use crate::contract::{Contract, Domain, FanIn, InputPort, OutputPort, Param, Role};

fn prepared_runtime_only(_: &mut [f64], _: &[f64], _: &[f64], _: f64, _: &mut [f64]) {
    unreachable!("delay is lowered to its prepared variable-state runtime")
}

pub const CONTRACT: Contract = Contract {
    name: "delay",
    role: Role::Processor,
    process: prepared_runtime_only,
    state_len: 0,
    inputs: &[
        InputPort {
            name: "input",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: None,
        },
        InputPort {
            name: "time_mod",
            domain: Domain::MonoAudio,
            fan_in: FanIn::Sum,
            unconnected: Some(0.0),
        },
        InputPort {
            name: "feedback_mod",
            domain: Domain::MonoAudio,
            fan_in: FanIn::Sum,
            unconnected: Some(0.0),
        },
        InputPort {
            name: "mix_mod",
            domain: Domain::MonoAudio,
            fan_in: FanIn::Sum,
            unconnected: Some(0.0),
        },
    ],
    outputs: &[OutputPort {
        name: "out",
        domain: Domain::MonoAudio,
    }],
    params: &[
        Param {
            name: "time",
            default: 0.375,
            range: Some((0.000_01, 4.0)),
        },
        Param {
            name: "feedback",
            default: 0.45,
            range: Some((-0.99, 0.99)),
        },
        Param {
            name: "mix",
            default: 0.3,
            range: Some((0.0, 1.0)),
        },
    ],
};
