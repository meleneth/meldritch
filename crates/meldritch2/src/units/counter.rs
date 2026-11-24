//! Sampled-trigger counter: a reusable state primitive, not a sequencer.
//! The advance and reset inputs recognize rising edges; length and looped
//! are ordinary patchable numeric inputs with normalled defaults.
use crate::contract::{Contract, Domain, FanIn, InputPort, OutputPort, Param, Role};

fn process(state: &mut [f64], input: &[f64], _: &[f64], _: f64, output: &mut [f64]) {
    let advance = input[0] >= 0.5;
    let reset = input[1] >= 0.5;
    let length = input[2].round().clamp(1.0, 65536.0) as usize;
    let looped = input[3] >= 0.5;
    let prior_advance = state[1] >= 0.5;
    let prior_reset = state[2] >= 0.5;
    let mut index = (state[0] as usize).min(length - 1);
    if reset && !prior_reset {
        index = 0;
    } else if advance && !prior_advance {
        if index + 1 < length {
            index += 1;
        } else if looped {
            index = 0;
        }
    }
    state[0] = index as f64;
    state[1] = f64::from(advance);
    state[2] = f64::from(reset);
    output[0] = index as f64;
    output[1] = if length == 1 {
        0.0
    } else {
        index as f64 / (length - 1) as f64
    };
}

pub const CONTRACT: Contract = Contract {
    name: "counter",
    role: Role::Processor,
    process,
    state_len: 3,
    inputs: &[
        InputPort {
            name: "advance",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: Some(0.0),
        },
        InputPort {
            name: "reset",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: Some(0.0),
        },
        InputPort {
            name: "length",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: Some(8.0),
        },
        InputPort {
            name: "looped",
            domain: Domain::MonoAudio,
            fan_in: FanIn::One,
            unconnected: Some(1.0),
        },
    ],
    outputs: &[
        OutputPort {
            name: "index",
            domain: Domain::MonoAudio,
        },
        OutputPort {
            name: "position",
            domain: Domain::MonoAudio,
        },
    ],
    params: &[],
};

#[cfg(test)]
mod tests {
    use super::process;

    fn tick(state: &mut [f64; 3], advance: f64, reset: f64, length: f64, looped: f64) -> [f64; 2] {
        let mut output = [0.0; 2];
        process(
            state,
            &[advance, reset, length, looped],
            &[],
            48_000.0,
            &mut output,
        );
        output
    }

    #[test]
    fn normalled_eight_step_loop_and_no_retrigger_on_held_gate() {
        let mut state = [0.0; 3];
        for step in 1..=8 {
            let result = tick(&mut state, 1.0, 0.0, 8.0, 1.0);
            assert_eq!(result[0], (step % 8) as f64);
            assert_eq!(tick(&mut state, 1.0, 0.0, 8.0, 1.0)[0], result[0]);
            tick(&mut state, 0.0, 0.0, 8.0, 1.0);
        }
    }

    #[test]
    fn one_shot_stops_at_last_step_and_reset_has_priority() {
        let mut state = [0.0; 3];
        for _ in 0..20 {
            tick(&mut state, 1.0, 0.0, 4.0, 0.0);
            tick(&mut state, 0.0, 0.0, 4.0, 0.0);
        }
        assert_eq!(state[0], 3.0);
        assert_eq!(tick(&mut state, 1.0, 1.0, 4.0, 0.0), [0.0, 0.0]);
    }

    #[test]
    fn length_changes_clamp_position_and_normalize_output() {
        let mut state = [0.0; 3];
        for _ in 0..6 {
            tick(&mut state, 1.0, 0.0, 8.0, 1.0);
            tick(&mut state, 0.0, 0.0, 8.0, 1.0);
        }
        assert_eq!(tick(&mut state, 0.0, 0.0, 3.0, 1.0), [2.0, 1.0]);
    }
}

/// Pass every Nth input pulse, preserving its high time. Divide-by-one is
/// transparent. A reset rising edge clears the phase and suppresses that pulse.
fn divide_clock(state: &mut [f64], input: &[f64], params: &[f64], rate: f64, out: &mut [f64]) {
    let high = input[0] >= 0.5;
    let reset = input[1] >= 0.5;
    let divisor = if params[0].is_finite() { params[0].round().clamp(1.0, 256.0) } else { 1.0 };
    if reset && state[2] == 0.0 {
        state[0] = 0.0;
        state[3] = 0.0;
    } else if high && state[1] == 0.0 {
        state[3] = f64::from(state[0] == 0.0);
        state[0] = (state[0] + 1.0) % divisor;
    }
    state[1] = f64::from(high);
    state[2] = f64::from(reset);
    out[0] = if high { state[3] } else { 0.0 };
    // Period estimate from rising edges, initialized from the declared source rate.
    if high && state[4] == 0.0 {
        if state[5] > 0.0 && state[6] > 0.0 { state[7] = state[5] / rate; }
        state[5] = 0.0;
        state[6] = 1.0;
    }
    state[4] = f64::from(high);
    state[5] += 1.0;
    if out.len() > 1 { out[1] = divisor * if state[7] > 0.0 { state[7] } else { 1.0 / params[1] }; }
}

pub const CLOCK_DIVIDER: Contract = Contract {
    name: "clock_divider",
    role: Role::Processor,
    process: divide_clock,
    state_len: 8,
    inputs: &[
        InputPort { name: "input", domain: Domain::MonoAudio, fan_in: FanIn::One, unconnected: Some(0.0) },
        InputPort { name: "reset", domain: Domain::MonoAudio, fan_in: FanIn::One, unconnected: Some(0.0) },
    ],
    outputs: &[
        OutputPort { name: "out", domain: Domain::MonoAudio },
        OutputPort { name: "period", domain: Domain::MonoAudio },
    ],
    params: &[
        Param { name: "divisor", default: 1.0, range: Some((1.0, 256.0)) },
        Param { name: "initial_hz", default: 8.0, range: Some((0.01, 1000.0)) },
    ],
};

#[cfg(test)]
mod divider_tests {
    use super::divide_clock;
    #[test]
    fn divides_edges_without_shortening_passed_pulses() {
        let mut state = [0.0; 8];
        let mut out = [0.0; 2];
        for i in 0..8 {
            for high in [1.0, 1.0, 0.0, 0.0] {
                divide_clock(&mut state, &[high, 0.0], &[3.0, 8.0], 48000.0, &mut out);
                assert_eq!(out[0], if i % 3 == 0 { high } else { 0.0 });
            }
        }
        divide_clock(&mut state, &[0.0, 1.0], &[3.0, 8.0], 48000.0, &mut out);
        divide_clock(&mut state, &[1.0, 0.0], &[3.0, 8.0], 48000.0, &mut out);
        assert_eq!(out[0], 1.0);
    }
}


/// Independent note articulation: a rising clock edge starts a gate for a
/// fraction of the supplied step period. A one-sample low interval is reserved
/// before the next expected trigger so repeated pitches can retrigger.
fn step_gate(state: &mut [f64], input: &[f64], params: &[f64], rate: f64, out: &mut [f64]) {
    let high = input[0] >= 0.5;
    if high && state[0] == 0.0 {
        let period = (input[1].max(0.0) * rate).round().max(2.0);
        state[1] = (period * params[0]).round().clamp(1.0, period - 1.0);
    }
    state[0] = f64::from(high);
    out[0] = f64::from(state[1] > 0.0);
    state[1] = (state[1] - 1.0).max(0.0);
}

pub const STEP_GATE: Contract = Contract {
    name: "step_gate",
    role: Role::Processor,
    process: step_gate,
    state_len: 2,
    inputs: &[
        InputPort { name: "trigger", domain: Domain::MonoAudio, fan_in: FanIn::One, unconnected: None },
        InputPort { name: "period", domain: Domain::MonoAudio, fan_in: FanIn::One, unconnected: Some(0.125) },
    ],
    outputs: &[OutputPort { name: "out", domain: Domain::MonoAudio }],
    params: &[Param { name: "fraction", default: 0.8, range: Some((0.01, 0.99)) }],
};

#[cfg(test)]
mod step_gate_tests {
    use super::step_gate;
    #[test]
    fn gate_duration_follows_step_period_not_trigger_width() {
        for period in [0.125, 0.5, 1.0] {
            let mut state = [0.0; 2];
            let mut out = [0.0];
            let samples = (period * 1000.0_f64).round() as usize;
            let mut count = 0;
            for frame in 0..samples {
                step_gate(&mut state, &[f64::from(frame == 0), period], &[0.8], 1000.0, &mut out);
                count += usize::from(out[0] > 0.0);
            }
            assert_eq!(count, (period * 800.0_f64).round() as usize);
        }
    }
}
