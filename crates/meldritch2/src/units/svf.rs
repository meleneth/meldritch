use crate::contract::{Contract, Domain, FanIn, InputPort, OutputPort, Param, Role};

fn process(
    state: &mut [f64],
    input: &[f64],
    params: &[f64],
    sample_rate: f64,
    outputs: &mut [f64],
) {
    let signal = if input[0].is_finite() { input[0] } else { 0.0 };
    let cutoff_mod = if input[1].is_finite() { input[1] } else { 0.0 };
    let resonance_mod = if input[2].is_finite() { input[2] } else { 0.0 };
    let cutoff = (params[0] + cutoff_mod).clamp(1.0, sample_rate * 0.45);
    let resonance = (params[1] + resonance_mod).clamp(0.0, 0.99);
    let q = 0.5 + resonance * 19.5;
    let k = 1.0 / q;
    let g = (std::f64::consts::PI * cutoff / sample_rate).tan();
    let a1 = 1.0 / (1.0 + g * (g + k));
    let a2 = g * a1;
    let a3 = g * a2;
    let v3 = signal - state[1];
    let band = a1 * state[0] + a2 * v3;
    let low = state[1] + a2 * state[0] + a3 * v3;
    state[0] = 2.0 * band - state[0];
    state[1] = 2.0 * low - state[1];
    outputs[0] = low;
    outputs[1] = signal - k * band - low;
    outputs[2] = band;
    if !outputs.iter().all(|v| v.is_finite()) {
        state.fill(0.0);
        outputs.fill(0.0);
    }
}

pub const CONTRACT: Contract = Contract {
    name: "svf",
    role: Role::Processor,
    process,
    state_len: 2,
    inputs: &[
        InputPort {
            name: "input",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: None,
        },
        InputPort {
            name: "cutoff_mod",
            domain: Domain::MonoAudio,
            fan_in: FanIn::Sum,
            unconnected: Some(0.0),
        },
        InputPort {
            name: "resonance_mod",
            domain: Domain::MonoAudio,
            fan_in: FanIn::Sum,
            unconnected: Some(0.0),
        },
    ],
    outputs: &[
        OutputPort {
            name: "lowpass",
            domain: Domain::MonoAudio,
        },
        OutputPort {
            name: "highpass",
            domain: Domain::MonoAudio,
        },
        OutputPort {
            name: "bandpass",
            domain: Domain::MonoAudio,
        },
    ],
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

#[cfg(test)]
mod tests {
    use super::process;
    fn energy(frequency: f64, cutoff: f64, resonance: f64) -> f64 {
        let mut state = [0.0; 2];
        let mut sum = 0.0;
        for n in 0..48_000 {
            let x = (std::f64::consts::TAU * frequency * n as f64 / 48_000.0).sin();
            let mut out = [0.0; 3];
            process(
                &mut state,
                &[x, 0.0, 0.0],
                &[cutoff, resonance],
                48_000.0,
                &mut out,
            );
            if n > 4_000 {
                sum += out[0] * out[0];
            }
            assert!(out.iter().all(|v| v.is_finite()));
        }
        sum
    }
    #[test]
    fn lowpass_response_and_resonance_are_audible_and_stable() {
        assert!(energy(100.0, 1000.0, 0.0) > energy(8000.0, 1000.0, 0.0) * 100.0);
        assert!(energy(1000.0, 1000.0, 0.9) > energy(1000.0, 1000.0, 0.0) * 2.0);
        for rate in [8_000.0, 44_100.0, 48_000.0, 96_000.0] {
            assert!(energy(440.0, rate * 0.49, 0.99).is_finite());
        }
    }
    #[test]
    fn cutoff_and_resonance_modulation_change_rendering() {
        let mut a = [0.0; 2];
        let mut b = [0.0; 2];
        let mut different = false;
        for n in 0..2000 {
            let input = f64::from(n == 0);
            let mut x = [0.0; 3];
            let mut y = [0.0; 3];
            process(&mut a, &[input, 0.0, 0.0], &[200.0, 0.1], 48_000.0, &mut x);
            process(
                &mut b,
                &[input, 4000.0, 0.7],
                &[200.0, 0.1],
                48_000.0,
                &mut y,
            );
            different |= (x[0] - y[0]).abs() > 1e-8;
        }
        assert!(different);
    }
}
