use crate::contract::{Contract, Domain, FanIn, InputPort, OutputPort, Param, Role};

// Transposed direct-form II low-pass biquad. Resonance maps monotonically to Q.
fn process(
    state: &mut [f64],
    input: &[f64],
    params: &[f64],
    sample_rate: f64,
    outputs: &mut [f64],
) {
    let cutoff = params[0].clamp(1.0, sample_rate * 0.49);
    let q = 0.5 + params[1] * 19.5;
    let omega = std::f64::consts::TAU * cutoff / sample_rate;
    let (sin, cos) = omega.sin_cos();
    let alpha = sin / (2.0 * q);
    let a0 = 1.0 + alpha;
    let b0 = (1.0 - cos) * 0.5 / a0;
    let b1 = (1.0 - cos) / a0;
    let b2 = b0;
    let a1 = -2.0 * cos / a0;
    let a2 = (1.0 - alpha) / a0;
    let output = b0 * input[0] + state[0];
    state[0] = b1 * input[0] - a1 * output + state[1];
    state[1] = b2 * input[0] - a2 * output;
    outputs[0] = output;
}

pub const CONTRACT: Contract = Contract {
    name: "lowpass",
    role: Role::Processor,
    process,
    state_len: 2,
    inputs: &[InputPort {
        name: "input",
        domain: Domain::MonoAudio,
        fan_in: FanIn::One,
        unconnected: None,
    }],
    outputs: &[OutputPort {
        name: "out",
        domain: Domain::MonoAudio,
    }],
    params: &[
        Param {
            name: "cutoff",
            default: 1200.0,
            range: Some((1.0, 24_000.0)),
        },
        Param {
            name: "resonance",
            default: 0.0,
            range: Some((0.0, 0.99)),
        },
    ],
};
