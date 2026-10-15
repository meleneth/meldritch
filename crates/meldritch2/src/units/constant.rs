use crate::contract::{Contract, Domain, FanIn, Param, Port, Role};

fn process(_: &mut [f64], _: &[f64], params: &[f64], _: f64) -> f64 {
    params[0]
}

pub const CONTRACT: Contract = Contract {
    name: "constant",
    role: Role::Processor,
    process,
    state_len: 0,
    inputs: &[],
    outputs: &[Port {
        name: "out",
        domain: Domain::MonoAudio,
        fan_in: FanIn::One,
        unconnected: None,
    }],
    params: &[Param {
        name: "value",
        default: 0.0,
        range: None,
    }],
};
