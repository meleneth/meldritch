use crate::contract::{Contract, Domain, FanIn, Param, Port, Role};

fn process(state: &mut [f64], input: &[f64], params: &[f64], sample_rate: f64) -> f64 {
    let sample = (std::f64::consts::TAU * state[0]).sin();
    state[0] = (state[0] + (params[0] + input[0]) / sample_rate).rem_euclid(1.0);
    sample
}

pub const CONTRACT: Contract = Contract {
    name: "sine",
    role: Role::Processor,
    process,
    state_len: 1,
    inputs: &[Port {
        name: "fm_hz",
        domain: Domain::MonoAudio,
        fan_in: FanIn::One,
        unconnected: Some(0.0),
    }],
    outputs: &[Port {
        name: "out",
        domain: Domain::MonoAudio,
        fan_in: FanIn::One,
        unconnected: None,
    }],
    params: &[Param {
        name: "frequency",
        default: 440.0,
        range: None,
    }],
};
