use crate::contract::{Contract, Domain, OutputPort, Param, Role};

fn process(_: &mut [f64], _: &[f64], params: &[f64], _: f64, outputs: &mut [f64]) {
    outputs[0] = params[0];
}

pub const CONTRACT: Contract = Contract {
    name: "constant",
    role: Role::Processor,
    process,
    state_len: 0,
    inputs: &[],
    outputs: &[OutputPort {
        name: "out",
        domain: Domain::MonoAudio,
    }],
    params: &[Param {
        name: "value",
        default: 0.0,
        range: None,
    }],
};
