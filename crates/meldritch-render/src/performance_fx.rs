//! Typed, deterministic performance FX rack used by live cockpit controls.

use crate::effects::{
    ModulatedReverbSettings, TempoDelaySettings, apply_modulated_reverb,
    apply_tempo_ping_pong_delay,
};
use crate::mastering::{MasteringSettings, master_bus};
use crate::modulation::{Lfo, LfoRate, LfoShape};
use crate::stereo_fx::{PhaserSettings, apply_tempo_stereo_phaser};
use meldritch_audio::AudioBlock;
use meldritch_core::Tempo;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PerformanceFxSettings {
    pub delay_feedback: f64,
    pub phaser_mix: f64,
    pub reverb_freeze: bool,
    pub modulation_depth: f64,
    pub master_drive: f64,
}

impl Default for PerformanceFxSettings {
    fn default() -> Self {
        Self {
            delay_feedback: 0.38,
            phaser_mix: 0.32,
            reverb_freeze: false,
            modulation_depth: 0.12,
            master_drive: 1.35,
        }
    }
}

impl PerformanceFxSettings {
    #[must_use]
    pub fn normalized(self) -> Self {
        Self {
            delay_feedback: self.delay_feedback.clamp(0.0, 0.92),
            phaser_mix: self.phaser_mix.clamp(0.0, 1.0),
            reverb_freeze: self.reverb_freeze,
            modulation_depth: self.modulation_depth.clamp(0.0, 1.0),
            master_drive: self.master_drive.clamp(0.5, 8.0),
        }
    }
}

#[must_use]
pub fn apply_performance_fx(
    input: &AudioBlock,
    tempo: Tempo,
    settings: PerformanceFxSettings,
) -> AudioBlock {
    let settings = settings.normalized();
    let delayed = apply_tempo_ping_pong_delay(
        input,
        tempo,
        TempoDelaySettings {
            feedback: settings.delay_feedback,
            ..TempoDelaySettings::default()
        },
    );
    let phased = apply_tempo_stereo_phaser(
        &delayed,
        tempo,
        PhaserSettings {
            mix: settings.phaser_mix,
            ..PhaserSettings::default()
        },
    );
    let reverberated = apply_modulated_reverb(
        &phased,
        tempo,
        ModulatedReverbSettings {
            freeze: settings.reverb_freeze,
            mix: 0.18,
            ..ModulatedReverbSettings::default()
        },
    );
    let mut modulated = reverberated;
    let lfo = Lfo {
        shape: LfoShape::Sine,
        rate: LfoRate::Beats(4.0),
        phase: 0.0,
        seed: 0,
    };
    let channels = usize::from(modulated.channels());
    for frame in 0..modulated.frames() {
        let gain = 1.0 - settings.modulation_depth * 0.5
            + lfo.value_at(u64::from(frame), tempo) * settings.modulation_depth * 0.5;
        let start = frame as usize * channels;
        for sample in &mut modulated.samples_mut()[start..start + channels] {
            *sample *= gain;
        }
    }
    master_bus(
        &modulated,
        tempo.sample_rate(),
        MasteringSettings {
            clip_drive: settings.master_drive,
            ..MasteringSettings::default()
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rack_controls_are_bounded_finite_and_parameter_sensitive() {
        let tempo = Tempo::new(142.0, 48_000).unwrap();
        let mut input = AudioBlock::silent(2, 48_000);
        input.samples_mut()[0] = 0.8;
        input.samples_mut()[1] = 0.8;
        let base = apply_performance_fx(&input, tempo, PerformanceFxSettings::default());
        let extreme = apply_performance_fx(
            &input,
            tempo,
            PerformanceFxSettings {
                delay_feedback: 99.0,
                phaser_mix: -5.0,
                reverb_freeze: true,
                modulation_depth: 4.0,
                master_drive: 100.0,
            },
        );
        assert_ne!(base.samples(), extreme.samples());
        assert!(extreme.samples().iter().all(|sample| sample.is_finite()));
        assert!(extreme.peak_abs() <= 0.95);
    }

    #[test]
    fn every_macro_changes_the_render() {
        let tempo = Tempo::new(120.0, 48_000).unwrap();
        let mut input = AudioBlock::silent(2, 64_000);
        for frame in 0..input.frames() as usize {
            let sample = (frame as f64 * 0.031).sin() * 0.4;
            input.samples_mut()[frame * 2] = sample;
            input.samples_mut()[frame * 2 + 1] = sample;
        }
        let base_settings = PerformanceFxSettings::default();
        let base = apply_performance_fx(&input, tempo, base_settings);
        let signature = |block: &AudioBlock| {
            block
                .samples()
                .iter()
                .enumerate()
                .map(|(index, sample)| *sample * (index % 97 + 1) as f64)
                .sum::<f64>()
        };
        let base_signature = signature(&base);
        for changed in [
            PerformanceFxSettings {
                delay_feedback: 0.8,
                ..base_settings
            },
            PerformanceFxSettings {
                phaser_mix: 0.9,
                ..base_settings
            },
            PerformanceFxSettings {
                reverb_freeze: true,
                ..base_settings
            },
            PerformanceFxSettings {
                modulation_depth: 0.8,
                ..base_settings
            },
            PerformanceFxSettings {
                master_drive: 4.0,
                ..base_settings
            },
        ] {
            assert_ne!(
                base_signature,
                signature(&apply_performance_fx(&input, tempo, changed))
            );
        }
    }
}
