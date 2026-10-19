use crate::contract::{Contract, Domain, FanIn, InputPort, OutputPort, Param, Role};
fn process(_: &mut [f64], input: &[f64], params: &[f64], _: f64, out: &mut [f64]) {
    out[0] = input.iter().step_by(2).sum::<f64>() * params[0];
    out[1] = input.iter().skip(1).step_by(2).sum::<f64>() * params[0];
}
const fn input(name: &'static str) -> InputPort {
    InputPort {
        name,
        domain: Domain::MonoAudio,
        fan_in: FanIn::One,
        unconnected: Some(0.0),
    }
}
pub const CONTRACT: Contract = Contract {
    name: "stereo_mixer8",
    role: Role::Processor,
    process,
    state_len: 0,
    inputs: &[
        input("left_1"),
        input("right_1"),
        input("left_2"),
        input("right_2"),
        input("left_3"),
        input("right_3"),
        input("left_4"),
        input("right_4"),
        input("left_5"),
        input("right_5"),
        input("left_6"),
        input("right_6"),
        input("left_7"),
        input("right_7"),
        input("left_8"),
        input("right_8"),
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
    params: &[Param {
        name: "headroom",
        default: 0.25,
        range: Some((0.01, 1.0)),
    }],
};
