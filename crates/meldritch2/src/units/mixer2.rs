use crate::contract::{Contract, Domain, FanIn, InputPort, OutputPort, Role};

fn process(_: &mut [f64], input: &[f64], _: &[f64], _: f64, outputs: &mut [f64]) {
    outputs[0] = (input[0] + input[1]) * 0.5;
}

pub const CONTRACT: Contract = Contract {
    name: "mixer2",
    role: Role::Processor,
    process,
    state_len: 0,
    inputs: &[
        InputPort {
            name: "input_1",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: None,
        },
        InputPort {
            name: "input_2",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: None,
        },
    ],
    outputs: &[OutputPort {
        name: "out",
        domain: Domain::MonoAudio,
    }],
    params: &[],
};
