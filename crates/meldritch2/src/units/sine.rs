use crate::contract::{Contract, Domain, FanIn, InputPort, OutputPort, Param, Role};

fn process(
    state: &mut [f64],
    input: &[f64],
    params: &[f64],
    sample_rate: f64,
    outputs: &mut [f64],
) {
    outputs[0] = (std::f64::consts::TAU * state[0]).sin();
    state[0] = (state[0] + (params[0] + input[0]) / sample_rate).rem_euclid(1.0);
}

pub const CONTRACT: Contract = Contract {
    name: "sine",
    role: Role::Processor,
    process,
    state_len: 1,
    inputs: &[InputPort {
        name: "fm_hz",
        domain: Domain::MonoAudio,
        fan_in: FanIn::One,
        unconnected: Some(0.0),
    }],
    outputs: &[OutputPort {
        name: "out",
        domain: Domain::MonoAudio,
    }],
    params: &[Param {
        name: "frequency",
        default: 440.0,
        range: None,
    }],
};
