use super::envelope_follower::coefficient;
use crate::contract::{Contract, Domain, FanIn, InputPort, OutputPort, Param, Role};

fn process(
    state: &mut [f64],
    input: &[f64],
    params: &[f64],
    sample_rate: f64,
    outputs: &mut [f64],
) {
    let signal = if input[0].is_finite() { input[0] } else { 0.0 };
    // NaN is the prepared unconnected sentinel. A patched sidechain fully owns
    // detection, including its quiet intervals; it is not mixed with self-detection.
    let detector = if input[1].is_finite() {
        input[1].abs()
    } else {
        signal.abs()
    };
    let time = if detector > state[0] {
        params[2]
    } else {
        params[3]
    };
    let c = coefficient(time, sample_rate);
    state[0] = detector + c * (state[0] - detector);
    let level_db = 20.0 * state[0].max(1e-12).log10();
    let reduction_db = if level_db > params[0] {
        (params[0] + (level_db - params[0]) / params[1]) - level_db
    } else {
        0.0
    };
    let gain = 10.0_f64.powf((reduction_db + params[4]) / 20.0);
    outputs[0] = signal * gain;
    outputs[1] = -reduction_db;
}

pub const CONTRACT: Contract = Contract {
    name: "compressor",
    role: Role::Processor,
    process,
    state_len: 1,
    inputs: &[
        InputPort {
            name: "input",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: None,
        },
        InputPort {
            name: "sidechain",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: Some(f64::NAN),
        },
    ],
    outputs: &[
        OutputPort {
            name: "out",
            domain: Domain::MonoAudio,
        },
        OutputPort {
            name: "gain_reduction",
            domain: Domain::MonoAudio,
        },
    ],
    params: &[
        Param {
            name: "threshold",
            default: -18.0,
            range: Some((-80.0, 0.0)),
        },
        Param {
            name: "ratio",
            default: 4.0,
            range: Some((1.0, 40.0)),
        },
        Param {
            name: "attack",
            default: 0.01,
            range: Some((0.000_01, 10.0)),
        },
        Param {
            name: "release",
            default: 0.1,
            range: Some((0.000_01, 10.0)),
        },
        Param {
            name: "makeup_gain",
            default: 0.0,
            range: Some((-24.0, 24.0)),
        },
    ],
};

#[cfg(test)]
mod tests {
    use super::process;
    #[test]
    fn gain_reduction_tracks_main_and_explicit_sidechain_then_recovers() {
        let mut state = [0.0];
        let mut out = [0.0; 2];
        for _ in 0..1000 {
            process(
                &mut state,
                &[0.1, 1.0],
                &[-24.0, 8.0, 0.001, 0.02, 0.0],
                48_000.0,
                &mut out,
            );
        }
        assert!(out[0].abs() < 0.05 && out[1] > 6.0);
        let ducked = out[0];
        for _ in 0..10_000 {
            process(
                &mut state,
                &[0.1, 0.0],
                &[-24.0, 8.0, 0.001, 0.02, 0.0],
                48_000.0,
                &mut out,
            );
        }
        assert!(out[0] > ducked * 1.5 && out.iter().all(|v| v.is_finite()));
    }
}
