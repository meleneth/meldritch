use crate::contract::{Contract, Domain, OutputPort, Param, Role};

fn process(
    state: &mut [f64],
    _input: &[f64],
    params: &[f64],
    _sample_rate: f64,
    outputs: &mut [f64],
) {
    let mut bits = state[0].to_bits();
    if bits == 0 {
        bits = 0x9e37_79b9_7f4a_7c15;
    }
    bits ^= bits << 13;
    bits ^= bits >> 7;
    bits ^= bits << 17;
    state[0] = f64::from_bits(bits);
    let unit = (bits >> 11) as f64 * (1.0 / ((1_u64 << 53) as f64));
    outputs[0] = (unit * 2.0 - 1.0) * params[0];
}

pub const CONTRACT: Contract = Contract {
    name: "noise",
    role: Role::Processor,
    process,
    state_len: 1,
    inputs: &[],
    outputs: &[OutputPort {
        name: "out",
        domain: Domain::MonoAudio,
    }],
    params: &[Param {
        name: "amplitude",
        default: 1.0,
        range: Some((0.0, 1.0)),
    }],
};
