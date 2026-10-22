use crate::contract::{Contract, Domain, FanIn, InputPort, OutputPort, Param, Role};

fn prepared_runtime_only(_: &mut [f64], _: &[f64], _: &[f64], _: f64, _: &mut [f64]) {
    unreachable!("stereo_delay is lowered to its prepared variable-state runtime")
}

/// A tempo-aware stereo delay. `left_beats` and `right_beats` are converted
/// from beats to seconds using the `bpm` input; the time modulation inputs are
/// additive seconds after that conversion.
pub const CONTRACT: Contract = Contract {
    name: "stereo_delay",
    role: Role::Processor,
    process: prepared_runtime_only,
    state_len: 0,
    inputs: &[
        InputPort {
            name: "input_left",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: None,
        },
        InputPort {
            name: "input_right",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: None,
        },
        InputPort {
            name: "bpm",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: Some(120.0),
        },
        InputPort {
            name: "left_time_mod",
            domain: Domain::MonoAudio,
            fan_in: FanIn::Sum,
            unconnected: Some(0.0),
        },
        InputPort {
            name: "right_time_mod",
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
        InputPort {
            name: "filter_mod",
            domain: Domain::MonoAudio,
            fan_in: FanIn::Sum,
            unconnected: Some(0.0),
        },
    ],
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
    params: &[
        Param {
            name: "left_beats",
            default: 0.75,
            range: Some((0.015_625, 16.0)),
        },
        Param {
            name: "right_beats",
            default: 1.0,
            range: Some((0.015_625, 16.0)),
        },
        Param {
            name: "feedback",
            default: 0.45,
            range: Some((-0.95, 0.95)),
        },
        Param {
            name: "crossfeed",
            default: 1.0,
            range: Some((0.0, 1.0)),
        },
        Param {
            name: "mix",
            default: 0.35,
            range: Some((0.0, 1.0)),
        },
        Param {
            name: "filter_cutoff",
            default: 6_000.0,
            range: Some((20.0, 20_000.0)),
        },
        Param {
            name: "feedback_drive",
            default: 1.5,
            range: Some((0.1, 8.0)),
        },
    ],
};
