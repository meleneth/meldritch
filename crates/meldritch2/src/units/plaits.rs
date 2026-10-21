use crate::contract::{Contract, Domain, FanIn, InputPort, OutputPort, Param, Role};

fn prepared_runtime_only(_: &mut [f64], _: &[f64], _: &[f64], _: f64, _: &mut [f64]) {
    unreachable!("Plaits is lowered to its prepared upstream-engine runtime")
}

pub const CONTRACT: Contract = Contract {
    name: "plaits",
    role: Role::Processor,
    process: prepared_runtime_only,
    state_len: 0,
    inputs: &[
        InputPort {
            name: "pitch",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: Some(0.0),
        },
        InputPort {
            name: "gate",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: Some(0.0),
        },
        InputPort {
            name: "velocity",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: Some(1.0),
        },
        InputPort {
            name: "harmonics_mod",
            domain: Domain::MonoAudio,
            fan_in: FanIn::Sum,
            unconnected: Some(0.0),
        },
        InputPort {
            name: "timbre_mod",
            domain: Domain::MonoAudio,
            fan_in: FanIn::Sum,
            unconnected: Some(0.0),
        },
        InputPort {
            name: "morph_mod",
            domain: Domain::MonoAudio,
            fan_in: FanIn::Sum,
            unconnected: Some(0.0),
        },
    ],
    outputs: &[
        OutputPort {
            name: "main",
            domain: Domain::MonoAudio,
        },
        OutputPort {
            name: "aux",
            domain: Domain::MonoAudio,
        },
    ],
    params: &[
        Param {
            name: "frequency",
            default: 440.0,
            range: Some((1.0, 24_000.0)),
        },
        Param {
            name: "harmonics",
            default: 0.5,
            range: Some((0.0, 1.0)),
        },
        Param {
            name: "timbre",
            default: 0.5,
            range: Some((0.0, 1.0)),
        },
        Param {
            name: "morph",
            default: 0.5,
            range: Some((0.0, 1.0)),
        },
    ],
};
