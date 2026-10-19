use crate::contract::{Contract, Domain, FanIn, InputPort, OutputPort, Param, Role};

fn unreachable_process(_: &mut [f64], _: &[f64], _: &[f64], _: f64, _: &mut [f64]) {
    unreachable!("sample players use their prepared real-time processor")
}

pub const CONTRACT: Contract = Contract {
    name: "sample_player",
    role: Role::Processor,
    process: unreachable_process,
    state_len: 0,
    inputs: &[InputPort {
        name: "trigger",
        domain: Domain::MonoAudio,
        fan_in: FanIn::One,
        unconnected: Some(0.0),
    }],
    outputs: &[
        OutputPort {
            name: "left",
            domain: Domain::MonoAudio,
        },
        OutputPort {
            name: "right",
            domain: Domain::MonoAudio,
        },
    ],
    params: &[Param {
        name: "gain",
        default: 1.0,
        range: Some((0.0, 4.0)),
    }],
};
