use crate::contract::{Contract, Domain, FanIn, Port, Role};

fn process(_: &mut [f64], input: &[f64], _: &[f64], _: f64, outputs: &mut [f64]) {
    outputs[0] = input[0];
    outputs[1] = -input[0];
}

pub const CONTRACT: Contract = Contract {
    name: "mirror",
    role: Role::Processor,
    process,
    state_len: 0,
    inputs: &[Port {
        name: "input",
        domain: Domain::MonoAudio,
        fan_in: FanIn::One,
        unconnected: None,
    }],
    outputs: &[
        Port {
            name: "out",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: None,
        },
        Port {
            name: "inverted",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: None,
        },
    ],
    params: &[],
};
