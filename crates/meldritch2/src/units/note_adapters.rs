//! Explicit numerical adapters at the MIDI / normalized-control boundary.
//!
//! note128 is NOTE_NUMBER / 128.0, with legal discrete notes 0..=127.
//! A MIDI note is an integer in 0..=127. Pitch in VST3 ports is Hz.
//! All outputs are prepared f64 signal slots; no heap allocation or lookup
//! is performed in any audio process callback.
use crate::contract::{Contract, Domain, FanIn, InputPort, OutputPort, Role};

const INPUT: &[InputPort] = &[InputPort {
    name: "input",
    domain: Domain::MonoAudio,
    fan_in: FanIn::One,
    unconnected: None,
}];

const OUTPUT: &[OutputPort] = &[OutputPort {
    name: "out",
    domain: Domain::MonoAudio,
}];

#[inline]
fn midi_number(value: f64) -> f64 {
    if value.is_nan() {
        return 0.0;
    }
    value.round().clamp(0.0, 127.0)
}

#[inline]
fn note128_number(value: f64) -> f64 {
    midi_number(value * 128.0)
}

#[inline]
fn midi_hz(note: f64) -> f64 {
    440.0 * 2.0_f64.powf((note - 69.0) / 12.0)
}

fn note128_to_midi(_: &mut [f64], input: &[f64], _: &[f64], _: f64, out: &mut [f64]) {
    out[0] = note128_number(input[0]);
}

fn midi_to_note128(_: &mut [f64], input: &[f64], _: &[f64], _: f64, out: &mut [f64]) {
    out[0] = midi_number(input[0]) / 128.0;
}

fn note128_to_hz(_: &mut [f64], input: &[f64], _: &[f64], _: f64, out: &mut [f64]) {
    out[0] = midi_hz(note128_number(input[0]));
}

fn hz_to_note128(_: &mut [f64], input: &[f64], _: &[f64], _: f64, out: &mut [f64]) {
    let frequency = input[0];
    let note = if frequency.is_finite() && frequency > 0.0 {
        69.0 + 12.0 * (frequency / 440.0).log2()
    } else {
        0.0
    };
    out[0] = midi_number(note) / 128.0;
}

pub const NOTE128_TO_MIDI: Contract = Contract {
    name: "note128_to_midi",
    role: Role::Processor,
    process: note128_to_midi,
    state_len: 0,
    inputs: INPUT,
    outputs: OUTPUT,
    params: &[],
};
pub const MIDI_TO_NOTE128: Contract = Contract {
    name: "midi_to_note128",
    role: Role::Processor,
    process: midi_to_note128,
    state_len: 0,
    inputs: INPUT,
    outputs: OUTPUT,
    params: &[],
};
pub const NOTE128_TO_HZ: Contract = Contract {
    name: "note128_to_hz",
    role: Role::Processor,
    process: note128_to_hz,
    state_len: 0,
    inputs: INPUT,
    outputs: OUTPUT,
    params: &[],
};
pub const HZ_TO_NOTE128: Contract = Contract {
    name: "hz_to_note128",
    role: Role::Processor,
    process: hz_to_note128,
    state_len: 0,
    inputs: INPUT,
    outputs: OUTPUT,
    params: &[],
};

#[cfg(test)]
mod tests {
    use super::*;

    fn run(process: crate::contract::ProcessFn, value: f64) -> f64 {
        let mut output = [f64::NAN];
        process(&mut [], &[value], &[], 48_000.0, &mut output);
        output[0]
    }

    #[test]
    fn every_midi_note_round_trips_exactly_via_note128() {
        for note in 0..=127 {
            let normalized = run(midi_to_note128, note as f64);
            assert_eq!(normalized, note as f64 / 128.0);
            assert_eq!(run(note128_to_midi, normalized), note as f64);
            assert_eq!(run(hz_to_note128, run(note128_to_hz, normalized)), normalized);
        }
    }

    #[test]
    fn concert_a_and_boundaries() {
        assert_eq!(run(note128_to_hz, 69.0 / 128.0), 440.0);
        assert_eq!(run(note128_to_midi, 127.0 / 128.0), 127.0);
        assert_eq!(run(note128_to_midi, 1.0), 127.0);
        assert_eq!(run(note128_to_midi, -0.2), 0.0);
        assert_eq!(run(midi_to_note128, 200.0), 127.0 / 128.0);
        assert_eq!(run(midi_to_note128, -5.0), 0.0);
        assert_eq!(run(hz_to_note128, 440.0), 69.0 / 128.0);
    }

    #[test]
    fn fractional_notes_quantize_to_nearest_integer() {
        assert_eq!(run(note128_to_midi, 60.49 / 128.0), 60.0);
        assert_eq!(run(note128_to_midi, 60.51 / 128.0), 61.0);
        assert_eq!(run(midi_to_note128, 60.51), 61.0 / 128.0);
    }

    #[test]
    fn bad_inputs_never_poison_the_output() {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            for process in [note128_to_midi, midi_to_note128, note128_to_hz, hz_to_note128] {
                assert!(run(process, value).is_finite());
            }
        }
    }
}
