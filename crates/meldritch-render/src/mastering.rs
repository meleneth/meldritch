//! Deterministic linked-channel master bus dynamics.

use meldritch_audio::AudioBlock;
use meldritch_core::SampleRate;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BusCompressorSettings {
    pub threshold_db: f64,
    pub ratio: f64,
    pub attack_seconds: f64,
    pub release_seconds: f64,
    pub makeup_db: f64,
}

impl Default for BusCompressorSettings {
    fn default() -> Self {
        Self {
            threshold_db: -12.0,
            ratio: 3.0,
            attack_seconds: 0.012,
            release_seconds: 0.18,
            makeup_db: 2.5,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MasteringSettings {
    pub compressor: BusCompressorSettings,
    pub clip_drive: f64,
    pub limiter_ceiling: f64,
    pub limiter_release_seconds: f64,
}

impl Default for MasteringSettings {
    fn default() -> Self {
        Self {
            compressor: BusCompressorSettings::default(),
            clip_drive: 1.6,
            limiter_ceiling: 0.95,
            limiter_release_seconds: 0.08,
        }
    }
}

#[must_use]
pub fn master_bus(
    input: &AudioBlock,
    sample_rate: SampleRate,
    settings: MasteringSettings,
) -> AudioBlock {
    let compressed = compress_linked(input, sample_rate, settings.compressor);
    let mut clipped = compressed;
    let drive = settings.clip_drive.clamp(0.01, 20.0);
    let normalization = drive.tanh().max(f64::EPSILON);
    for sample in clipped.samples_mut() {
        *sample = (*sample * drive).tanh() / normalization;
    }
    limit_linked(
        &clipped,
        sample_rate,
        settings.limiter_ceiling,
        settings.limiter_release_seconds,
    )
}

#[must_use]
pub fn compress_linked(
    input: &AudioBlock,
    sample_rate: SampleRate,
    settings: BusCompressorSettings,
) -> AudioBlock {
    let mut output = input.clone();
    let channels = usize::from(input.channels());
    let threshold = settings.threshold_db.clamp(-60.0, 0.0);
    let ratio = settings.ratio.clamp(1.0, 40.0);
    let attack = smoothing_coefficient(settings.attack_seconds, sample_rate);
    let release = smoothing_coefficient(settings.release_seconds, sample_rate);
    let makeup = db_to_gain(settings.makeup_db.clamp(-24.0, 24.0));
    let mut envelope = 0.0;
    for frame in 0..input.frames() as usize {
        let start = frame * channels;
        let peak = input.samples()[start..start + channels]
            .iter()
            .fold(0.0_f64, |peak, sample| peak.max(sample.abs()));
        let coefficient = if peak > envelope { attack } else { release };
        envelope = coefficient * envelope + (1.0 - coefficient) * peak;
        let input_db = gain_to_db(envelope);
        let reduction_db = if input_db > threshold {
            threshold + (input_db - threshold) / ratio - input_db
        } else {
            0.0
        };
        let gain = db_to_gain(reduction_db) * makeup;
        for channel in 0..channels {
            output.samples_mut()[start + channel] = input.samples()[start + channel] * gain;
        }
    }
    output
}

fn limit_linked(
    input: &AudioBlock,
    sample_rate: SampleRate,
    ceiling: f64,
    release_seconds: f64,
) -> AudioBlock {
    let mut output = input.clone();
    let channels = usize::from(input.channels());
    let ceiling = ceiling.clamp(0.01, 1.0);
    let release = smoothing_coefficient(release_seconds, sample_rate);
    let mut gain = 1.0;
    for frame in 0..input.frames() as usize {
        let start = frame * channels;
        let peak = input.samples()[start..start + channels]
            .iter()
            .fold(0.0_f64, |peak, sample| peak.max(sample.abs()));
        let wanted = if peak > ceiling { ceiling / peak } else { 1.0 };
        gain = if wanted < gain {
            wanted
        } else {
            release * gain + (1.0 - release) * wanted
        };
        for channel in 0..channels {
            output.samples_mut()[start + channel] =
                (input.samples()[start + channel] * gain).clamp(-ceiling, ceiling);
        }
    }
    output
}

fn smoothing_coefficient(seconds: f64, sample_rate: SampleRate) -> f64 {
    (-1.0 / (seconds.clamp(0.000_01, 10.0) * f64::from(sample_rate))).exp()
}

fn gain_to_db(gain: f64) -> f64 {
    20.0 * gain.max(1e-12).log10()
}

fn db_to_gain(db: f64) -> f64 {
    10.0_f64.powf(db / 20.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compressor_reduces_loud_material_and_links_stereo_gain() {
        let mut input = AudioBlock::silent(2, 4_800);
        for frame in 0..input.frames() as usize {
            input.samples_mut()[frame * 2] = 1.2;
            input.samples_mut()[frame * 2 + 1] = 0.3;
        }
        let output = compress_linked(
            &input,
            48_000,
            BusCompressorSettings {
                makeup_db: 0.0,
                ..BusCompressorSettings::default()
            },
        );
        let end = output.samples().len() - 2;
        assert!(output.samples()[end] < input.samples()[end]);
        assert_eq!(
            output.samples()[end] / input.samples()[end],
            output.samples()[end + 1] / input.samples()[end + 1]
        );
    }

    #[test]
    fn master_chain_is_finite_and_honors_ceiling() {
        let mut input = AudioBlock::silent(2, 2_000);
        for (index, sample) in input.samples_mut().iter_mut().enumerate() {
            *sample = ((index as f64 * 0.17).sin() * 3.0) + 0.5;
        }
        let output = master_bus(
            &input,
            48_000,
            MasteringSettings {
                limiter_ceiling: 0.9,
                ..MasteringSettings::default()
            },
        );
        assert!(output.samples().iter().all(|sample| sample.is_finite()));
        assert!(output.peak_abs() <= 0.9);
        assert_ne!(output.samples(), input.samples());
    }

    #[test]
    fn quiet_signal_remains_nonzero() {
        let mut input = AudioBlock::silent(1, 64);
        input.samples_mut().fill(0.01);
        let output = master_bus(&input, 48_000, MasteringSettings::default());
        assert!(output.peak_abs() > 0.0);
        assert!(output.peak_abs() < 0.1);
    }
}
