//! Sampled-trigger counter: a reusable state primitive, not a sequencer.
//! The advance and reset inputs recognize rising edges; length and looped
//! are ordinary patchable numeric inputs with normalled defaults.
use crate::contract::{Contract, Domain, FanIn, InputPort, OutputPort, Role};

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
    output[1] = if length == 1 { 0.0 } else { index as f64 / (length - 1) as f64 };
}

pub const CONTRACT: Contract = Contract {
    name: "counter",
    role: Role::Processor,
    process,
    state_len: 3,
    inputs: &[
        InputPort { name: "advance", domain: Domain::MonoAudio, fan_in: FanIn::One, unconnected: Some(0.0) },
        InputPort { name: "reset", domain: Domain::MonoAudio, fan_in: FanIn::One, unconnected: Some(0.0) },
        InputPort { name: "length", domain: Domain::MonoAudio, fan_in: FanIn::One, unconnected: Some(8.0) },
        InputPort { name: "looped", domain: Domain::MonoAudio, fan_in: FanIn::One, unconnected: Some(1.0) },
    ],
    outputs: &[
        OutputPort { name: "index", domain: Domain::MonoAudio },
        OutputPort { name: "position", domain: Domain::MonoAudio },
    ],
    params: &[],
};

#[cfg(test)]
mod tests {
    use super::process;

    fn tick(state: &mut [f64; 3], advance: f64, reset: f64, length: f64, looped: f64) -> [f64; 2] {
        let mut output = [0.0; 2];
        process(state, &[advance, reset, length, looped], &[], 48_000.0, &mut output);
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
