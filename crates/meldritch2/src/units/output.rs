use crate::contract::{Contract, Domain, FanIn, Port, Role};

fn process(_: &mut [f64], input: &[f64], _: &[f64], _: f64) -> f64 {
    input[0]
}

pub const CONTRACT: Contract = Contract {
    name: "output",
    role: Role::Output,
    process,
    state_len: 0,
    inputs: &[Port {
        name: "input",
        domain: Domain::MonoAudio,
        fan_in: FanIn::One,
        unconnected: None,
    }],
    outputs: &[],
    params: &[],
};
