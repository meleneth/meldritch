use super::envelope_follower::coefficient;
use crate::contract::{Contract, Domain, FanIn, InputPort, OutputPort, Param, Role};

const DETECTOR: usize = 0;
const THRESHOLD: usize = 1;
const RATIO: usize = 2;
const ATTACK: usize = 3;
const RELEASE: usize = 4;
const KNEE: usize = 5;
const MAKEUP: usize = 6;
const BYPASS: usize = 7;
const INITIALIZED: usize = 8;

pub(crate) fn gain_reduction_db(level_db: f64, threshold: f64, ratio: f64, knee: f64) -> f64 {
    let slope = 1.0 / ratio - 1.0;
    let over = level_db - threshold;
    let gain = if knee <= 1e-9 {
        if over > 0.0 { slope * over } else { 0.0 }
    } else if over <= -knee * 0.5 {
        0.0
    } else if over >= knee * 0.5 {
        slope * over
    } else {
        slope * (over + knee * 0.5).powi(2) / (2.0 * knee)
    };
    -gain
}

fn process(
    state: &mut [f64],
    input: &[f64],
    params: &[f64],
    sample_rate: f64,
    outputs: &mut [f64],
) {
    let left = if input[0].is_finite() { input[0] } else { 0.0 };
    let right = if input[1].is_finite() { input[1] } else { 0.0 };
    if state[INITIALIZED] == 0.0 {
        state[THRESHOLD..=BYPASS].copy_from_slice(params);
        state[INITIALIZED] = 1.0;
    } else {
        let smoothing = coefficient(0.010, sample_rate);
        for (smoothed, target) in state[THRESHOLD..=BYPASS].iter_mut().zip(params) {
            *smoothed = *target + smoothing * (*smoothed - *target);
        }
    }

    // Feed-forward peak detector with maximum-channel stereo linking. Both
    // channels receive exactly the same gain, preserving their balance.
    let target = left.abs().max(right.abs());
    let time = if target > state[DETECTOR] {
        state[ATTACK]
    } else {
        state[RELEASE]
    };
    let detector_smoothing = coefficient(time, sample_rate);
    state[DETECTOR] = target + detector_smoothing * (state[DETECTOR] - target);
    let level_db = 20.0 * state[DETECTOR].max(1e-12).log10();
    let reduction = gain_reduction_db(level_db, state[THRESHOLD], state[RATIO], state[KNEE]);
    let compressed_gain = 10.0_f64.powf((-reduction + state[MAKEUP]) / 20.0);
    let bypass = state[BYPASS].clamp(0.0, 1.0);
    let gain = compressed_gain * (1.0 - bypass) + bypass;
    outputs[0] = left * gain;
    outputs[1] = right * gain;
    outputs[2] = reduction * (1.0 - bypass);
}

pub const CONTRACT: Contract = Contract {
    name: "stereo_compressor",
    role: Role::Processor,
    process,
    state_len: 9,
    inputs: &[
        InputPort {
            name: "input_left",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: None,
        },
        InputPort {
            name: "input_right",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: None,
        },
    ],
    outputs: &[
        OutputPort {
            name: "left",
            domain: Domain::MonoAudio,
        },
        OutputPort {
            name: "right",
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
            default: 2.0,
            range: Some((1.0, 40.0)),
        },
        Param {
            name: "attack",
            default: 0.025,
            range: Some((0.000_01, 1.0)),
        },
        Param {
            name: "release",
            default: 0.180,
            range: Some((0.001, 10.0)),
        },
        Param {
            name: "knee",
            default: 6.0,
            range: Some((0.0, 24.0)),
        },
        Param {
            name: "makeup_gain",
            default: 1.5,
            range: Some((-24.0, 24.0)),
        },
        Param {
            name: "bypass",
            default: 0.0,
            range: Some((0.0, 1.0)),
        },
    ],
};

#[cfg(test)]
mod tests {
    use super::{BYPASS, DETECTOR, RATIO, THRESHOLD, gain_reduction_db, process};

    #[test]
    fn static_curve_has_ratio_and_continuous_soft_knee() {
        assert_eq!(gain_reduction_db(-30.0, -18.0, 2.0, 0.0), 0.0);
        assert!((gain_reduction_db(-6.0, -18.0, 2.0, 0.0) - 6.0).abs() < 1e-12);
        let below = gain_reduction_db(-21.000_001, -18.0, 4.0, 6.0);
        let inside = gain_reduction_db(-21.0, -18.0, 4.0, 6.0);
        let above = gain_reduction_db(-20.999_999, -18.0, 4.0, 6.0);
        assert!((inside - below).abs() < 1e-6 && (above - inside).abs() < 1e-6);
        assert!(gain_reduction_db(-18.0, -18.0, 4.0, 6.0) > 0.0);
    }

    #[test]
    fn linked_detector_preserves_balance_and_reports_reduction() {
        let mut state = [0.0; 9];
        let params = [-24.0, 4.0, 0.001, 0.1, 6.0, 0.0, 0.0];
        let mut out = [0.0; 3];
        for _ in 0..1_000 {
            process(&mut state, &[1.0, 0.25], &params, 48_000.0, &mut out);
        }
        assert!(out[2] > 10.0);
        assert!((out[1] / out[0] - 0.25).abs() < 1e-12);
        assert!(out[0] < 0.4);
    }

    #[test]
    fn attack_release_state_crosses_buffers_and_bypass_is_click_resistant() {
        let mut state = [0.0; 9];
        let active = [-20.0, 3.0, 0.01, 0.1, 4.0, 0.0, 0.0];
        let bypassed = [-20.0, 3.0, 0.01, 0.1, 4.0, 0.0, 1.0];
        let mut out = [0.0; 3];
        for _ in 0..480 {
            process(&mut state, &[1.0, 1.0], &active, 48_000.0, &mut out);
        }
        let attacked = out[0];
        assert!(attacked < 0.8);
        process(&mut state, &[1.0, 1.0], &bypassed, 48_000.0, &mut out);
        assert!(out[0] > attacked && out[0] < 1.0);
        for _ in 0..4_800 {
            process(&mut state, &[0.0, 0.0], &active, 48_000.0, &mut out);
        }
        assert!(
            state[0] > 0.0,
            "release state reset at a buffer-like boundary"
        );
        assert!(out.iter().all(|value| value.is_finite()));
    }

    #[test]
    fn detector_time_constants_and_parameter_smoothing_are_defined() {
        let mut state = [0.0; 9];
        let initial = [-30.0, 4.0, 0.1, 0.1, 0.0, 0.0, 0.0];
        let mut out = [0.0; 3];
        for _ in 0..100 {
            process(&mut state, &[1.0, 1.0], &initial, 1_000.0, &mut out);
        }
        assert!((state[DETECTOR] - (1.0 - (-1.0_f64).exp())).abs() < 0.005);
        for _ in 0..100 {
            process(&mut state, &[0.0, 0.0], &initial, 1_000.0, &mut out);
        }
        assert!((state[DETECTOR] - (1.0 - (-1.0_f64).exp()) * (-1.0_f64).exp()).abs() < 0.005);

        let changed = [-6.0, 10.0, 0.01, 0.5, 12.0, 6.0, 1.0];
        process(&mut state, &[0.5, 0.5], &changed, 1_000.0, &mut out);
        assert!(state[THRESHOLD] > -30.0 && state[THRESHOLD] < -6.0);
        assert!(state[RATIO] > 4.0 && state[RATIO] < 10.0);
        assert!(state[BYPASS] > 0.0 && state[BYPASS] < 1.0);
    }

    #[test]
    fn silence_extremes_and_recreation_are_finite_and_deterministic() {
        let params = [-80.0, 40.0, 0.000_01, 0.001, 24.0, 24.0, 0.0];
        let render = || {
            let mut state = [0.0; 9];
            let mut rendered = Vec::new();
            for frame in 0..2_000 {
                let input = if frame < 500 {
                    0.0
                } else {
                    (frame as f64 * 0.1).sin()
                };
                let mut out = [0.0; 3];
                process(
                    &mut state,
                    &[input, -input * 0.7],
                    &params,
                    96_000.0,
                    &mut out,
                );
                assert!(out.iter().all(|value| value.is_finite()));
                rendered.push(out);
            }
            rendered
        };
        assert_eq!(render(), render());
    }
}
