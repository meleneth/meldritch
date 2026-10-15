use crate::contract::{Contract, Domain, FanIn, InputPort, Role};

fn process(_: &mut [f64], input: &[f64], _: &[f64], _: f64, outputs: &mut [f64]) {
    outputs[0] = input[0];
}

pub const CONTRACT: Contract = Contract {
    name: "output",
    role: Role::Output,
    process,
    state_len: 0,
    inputs: &[InputPort {
        name: "input",
        domain: Domain::MonoAudio,
        fan_in: FanIn::One,
        unconnected: None,
    }],
    outputs: &[],
    params: &[],
};
