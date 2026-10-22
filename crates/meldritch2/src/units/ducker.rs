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
    let detector = if input[1].is_finite() {
        input[1].abs()
    } else {
        0.0
    };
    let time = if detector > state[0] {
        params[2]
    } else {
        params[3]
    };
    let smoothing = coefficient(time, sample_rate);
    state[0] = detector + smoothing * (state[0] - detector);

    let threshold = params[0];
    let activity = ((state[0] - threshold) / (1.0 - threshold).max(1e-9)).clamp(0.0, 1.0);
    let gain = (1.0 - params[1] * activity).clamp(0.0, 1.0);
    outputs[0] = signal * gain;
    outputs[1] = gain;
}

pub const CONTRACT: Contract = Contract {
    name: "ducker",
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
            unconnected: None,
        },
    ],
    outputs: &[
        OutputPort {
            name: "out",
            domain: Domain::MonoAudio,
        },
        OutputPort {
            name: "gain",
            domain: Domain::MonoAudio,
        },
    ],
    params: &[
        Param {
            name: "threshold",
            default: 0.08,
            range: Some((0.0, 0.99)),
        },
        Param {
            name: "depth",
            default: 0.75,
            range: Some((0.0, 1.0)),
        },
        Param {
            name: "attack",
            default: 0.003,
            range: Some((0.000_01, 1.0)),
        },
        Param {
            name: "release",
            default: 0.180,
            range: Some((0.001, 3.0)),
        },
    ],
};

#[cfg(test)]
mod tests {
    use super::process;

    #[test]
    fn kick_envelope_ducks_retriggers_and_recovers_smoothly() {
        let mut state = [0.0];
        let params = [0.05, 0.85, 0.001, 0.08];
        let mut out = [0.0; 2];
        for _ in 0..480 {
            process(&mut state, &[1.0, 0.0], &params, 48_000.0, &mut out);
        }
        assert_eq!(out, [1.0, 1.0]);

        let mut minimum: f64 = 1.0;
        for frame in 0..2_000 {
            let kick = f64::from(frame < 160 || (900..1060).contains(&frame));
            process(&mut state, &[1.0, kick], &params, 48_000.0, &mut out);
            minimum = minimum.min(out[1]);
            assert!(out.iter().all(|value| value.is_finite()));
        }
        assert!(
            minimum < 0.3,
            "kick did not create substantial ducking: {minimum}"
        );
        let after_second_kick = out[1];
        for _ in 0..40_000 {
            process(&mut state, &[1.0, 0.0], &params, 48_000.0, &mut out);
        }
        assert!(out[1] > after_second_kick && out[1] > 0.99);
    }

    #[test]
    fn parameter_updates_preserve_detector_state_and_nonfinite_audio_is_safe() {
        let mut state = [0.0];
        let mut out = [0.0; 2];
        for _ in 0..500 {
            process(
                &mut state,
                &[0.5, 1.0],
                &[0.1, 0.6, 0.002, 0.2],
                48_000.0,
                &mut out,
            );
        }
        let envelope = state[0];
        process(
            &mut state,
            &[f64::NAN, f64::NAN],
            &[0.2, 0.9, 0.02, 1.0],
            48_000.0,
            &mut out,
        );
        assert!(state[0] > 0.0 && state[0] < envelope);
        assert!(out.iter().all(|value| value.is_finite()));
    }
}
