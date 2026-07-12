//! Tempo-aware stereo movement effects.

use crate::modulation::{Lfo, LfoRate, LfoShape};
use meldritch_audio::AudioBlock;
use meldritch_core::Tempo;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhaserSettings {
    pub cycle_beats: f64,
    pub minimum_hz: f64,
    pub maximum_hz: f64,
    pub feedback: f64,
    pub mix: f64,
    pub stereo_phase: f64,
    pub stages: usize,
}

impl Default for PhaserSettings {
    fn default() -> Self {
        Self {
            cycle_beats: 8.0,
            minimum_hz: 180.0,
            maximum_hz: 2_800.0,
            feedback: 0.35,
            mix: 0.5,
            stereo_phase: 0.25,
            stages: 6,
        }
    }
}

#[must_use]
pub fn apply_tempo_stereo_phaser(
    input: &AudioBlock,
    tempo: Tempo,
    settings: PhaserSettings,
) -> AudioBlock {
    let channels = usize::from(input.channels());
    if channels == 0 || input.frames() == 0 {
        return input.clone();
    }
    let stages = settings.stages.clamp(1, 12);
    let mut states = vec![vec![0.0; stages]; channels];
    let mut feedback_state = vec![0.0; channels];
    let mut output = input.clone();
    let mix = settings.mix.clamp(0.0, 1.0);
    let feedback = settings.feedback.clamp(-0.95, 0.95);
    let minimum = settings.minimum_hz.clamp(20.0, 18_000.0);
    let maximum = settings.maximum_hz.clamp(minimum, 20_000.0);
    for frame in 0..input.frames() {
        for channel in 0..channels {
            let phase_offset = if channel % 2 == 0 {
                0.0
            } else {
                settings.stereo_phase.clamp(-1.0, 1.0)
            };
            let lfo = Lfo {
                shape: LfoShape::Sine,
                rate: LfoRate::Beats(settings.cycle_beats),
                phase: phase_offset.rem_euclid(1.0),
                seed: 0,
            }
            .value_at(u64::from(frame), tempo)
            .mul_add(0.5, 0.5);
            let frequency = minimum * (maximum / minimum).powf(lfo);
            let tangent = (std::f64::consts::PI * frequency / f64::from(tempo.sample_rate())).tan();
            let coefficient = ((tangent - 1.0) / (tangent + 1.0)).clamp(-0.999, 0.999);
            let index = frame as usize * channels + channel;
            let dry = input.samples()[index];
            let mut wet = dry + feedback_state[channel] * feedback;
            for state in &mut states[channel] {
                let next = coefficient * wet + *state;
                *state = wet - coefficient * next;
                wet = next;
            }
            feedback_state[channel] = wet;
            output.samples_mut()[index] = dry * (1.0 - mix) + wet * mix;
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stereo_phaser_is_finite_distinct_and_channel_offset() {
        let tempo = Tempo::new(120.0, 48_000).unwrap();
        let mut input = AudioBlock::silent(2, 24_000);
        for frame in 0..input.frames() {
            let sample = (std::f64::consts::TAU * 220.0 * f64::from(frame) / 48_000.0).sin() * 0.4;
            input.samples_mut()[frame as usize * 2] = sample;
            input.samples_mut()[frame as usize * 2 + 1] = sample;
        }
        let output = apply_tempo_stereo_phaser(&input, tempo, PhaserSettings::default());
        assert_ne!(output.samples(), input.samples());
        assert!(output.samples().iter().all(|sample| sample.is_finite()));
        assert!((0..output.frames()).any(|frame| {
            let index = frame as usize * 2;
            output.samples()[index] != output.samples()[index + 1]
        }));
        assert!(output.peak_abs() < 2.0);
    }

    #[test]
    fn zero_mix_is_bit_identical_to_input() {
        let mut input = AudioBlock::silent(1, 16);
        input.samples_mut().fill(0.25);
        let output = apply_tempo_stereo_phaser(
            &input,
            Tempo::new(142.0, 48_000).unwrap(),
            PhaserSettings {
                mix: 0.0,
                ..PhaserSettings::default()
            },
        );
        assert_eq!(output, input);
    }
}
