//! Role-aware sidechain dynamics and multiband ducking.

use crate::{Fingerprint, FingerprintBuilder};
use meldritch_audio::AudioBlock;
use meldritch_core::{RolePriorityTable, SampleRate, SourceRole, TrackId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DuckBands {
    pub low: bool,
    pub high: bool,
}

impl DuckBands {
    pub const ALL: Self = Self {
        low: true,
        high: true,
    };
    pub const LOW_ONLY: Self = Self {
        low: true,
        high: false,
    };
    pub const HIGH_ONLY: Self = Self {
        low: false,
        high: true,
    };
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SidechainSettings {
    pub amount: f64,
    pub attack_seconds: f64,
    pub release_seconds: f64,
    pub crossover_hz: f64,
    pub bands: DuckBands,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SidechainRelation {
    pub control_track: TrackId,
    pub source_role: SourceRole,
    pub target_role: SourceRole,
    pub settings: SidechainSettings,
}

#[must_use]
pub fn sidechain_relation_fingerprint(relation: SidechainRelation) -> Fingerprint {
    let mut state = FingerprintBuilder::new();
    state.write_u64(relation.control_track.raw());
    state.write_u64(relation.source_role as u64);
    state.write_u64(relation.target_role as u64);
    state.write_u64(relation.settings.amount.to_bits());
    state.write_u64(relation.settings.attack_seconds.to_bits());
    state.write_u64(relation.settings.release_seconds.to_bits());
    state.write_u64(relation.settings.crossover_hz.to_bits());
    state.write_u64(u64::from(relation.settings.bands.low));
    state.write_u64(u64::from(relation.settings.bands.high));
    state.finish()
}

impl Default for SidechainSettings {
    fn default() -> Self {
        Self {
            amount: 0.55,
            attack_seconds: 0.002,
            release_seconds: 0.12,
            crossover_hz: 180.0,
            bands: DuckBands::LOW_ONLY,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DuckingExplanation {
    pub source_role: SourceRole,
    pub target_role: SourceRole,
    pub bands: DuckBands,
    pub peak_envelope: f64,
    pub maximum_attenuation: f64,
    pub active: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EnvelopeFollower {
    attack_coefficient: f64,
    release_coefficient: f64,
    value: f64,
}

impl EnvelopeFollower {
    #[must_use]
    pub fn new(attack_seconds: f64, release_seconds: f64, sample_rate: SampleRate) -> Self {
        Self {
            attack_coefficient: time_coefficient(attack_seconds, sample_rate),
            release_coefficient: time_coefficient(release_seconds, sample_rate),
            value: 0.0,
        }
    }

    pub fn next(&mut self, input: f64) -> f64 {
        let input = input.abs().clamp(0.0, 1.0);
        let coefficient = if input > self.value {
            self.attack_coefficient
        } else {
            self.release_coefficient
        };
        self.value = input + coefficient * (self.value - input);
        self.value
    }

    #[must_use]
    pub const fn value(&self) -> f64 {
        self.value
    }
}

#[must_use]
pub fn apply_role_sidechain(
    target: &AudioBlock,
    sidechain: &AudioBlock,
    sample_rate: SampleRate,
    source_role: SourceRole,
    target_role: SourceRole,
    priorities: &RolePriorityTable,
    settings: SidechainSettings,
) -> (AudioBlock, DuckingExplanation) {
    let active = priorities.should_duck(source_role, target_role)
        && (settings.bands.low || settings.bands.high)
        && settings.amount > 0.0;
    let mut output = target.clone();
    if !active || target.frames() == 0 || sidechain.frames() == 0 {
        return (
            output,
            DuckingExplanation {
                source_role,
                target_role,
                bands: settings.bands,
                peak_envelope: 0.0,
                maximum_attenuation: 0.0,
                active: false,
            },
        );
    }
    let mut follower = EnvelopeFollower::new(
        settings.attack_seconds,
        settings.release_seconds,
        sample_rate,
    );
    let target_channels = usize::from(target.channels());
    let sidechain_channels = usize::from(sidechain.channels());
    let frames = target.frames().min(sidechain.frames());
    let coefficient = lowpass_coefficient(settings.crossover_hz, sample_rate);
    let mut low_state = vec![0.0; target_channels];
    let amount = settings.amount.clamp(0.0, 1.0);
    let mut peak_envelope: f64 = 0.0;
    let mut maximum_attenuation: f64 = 0.0;
    for frame in 0..frames {
        let sidechain_start = frame as usize * sidechain_channels;
        let detector = sidechain.samples()[sidechain_start..sidechain_start + sidechain_channels]
            .iter()
            .fold(0.0_f64, |peak, sample| peak.max(sample.abs()));
        let envelope = follower.next(detector);
        peak_envelope = peak_envelope.max(envelope);
        let gain = 1.0 - amount * envelope;
        maximum_attenuation = maximum_attenuation.max(1.0 - gain);
        for (channel, low) in low_state.iter_mut().enumerate() {
            let index = frame as usize * target_channels + channel;
            let input = target.samples()[index];
            *low += coefficient * (input - *low);
            let high = input - *low;
            output.samples_mut()[index] = *low * if settings.bands.low { gain } else { 1.0 }
                + high * if settings.bands.high { gain } else { 1.0 };
        }
    }
    (
        output,
        DuckingExplanation {
            source_role,
            target_role,
            bands: settings.bands,
            peak_envelope,
            maximum_attenuation,
            active,
        },
    )
}

fn time_coefficient(seconds: f64, sample_rate: SampleRate) -> f64 {
    if seconds <= 0.0 {
        0.0
    } else {
        (-1.0 / (seconds * f64::from(sample_rate))).exp()
    }
}

fn lowpass_coefficient(cutoff_hz: f64, sample_rate: SampleRate) -> f64 {
    let cutoff = cutoff_hz.clamp(1.0, f64::from(sample_rate) * 0.45);
    1.0 - (-std::f64::consts::TAU * cutoff / f64::from(sample_rate)).exp()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_follower_attacks_and_releases_deterministically() {
        let mut follower = EnvelopeFollower::new(0.0, 0.01, 1_000);
        assert_eq!(follower.next(1.0), 1.0);
        let released = follower.next(0.0);
        assert!(released > 0.0 && released < 1.0);
        assert_eq!(follower.value(), released);
    }

    #[test]
    fn kick_role_ducks_selected_bass_band_and_explains_attenuation() {
        let mut target = AudioBlock::silent(1, 256);
        for (index, sample) in target.samples_mut().iter_mut().enumerate() {
            *sample = if index % 2 == 0 { 1.0 } else { -0.5 };
        }
        let mut kick = AudioBlock::silent(1, 256);
        kick.samples_mut()[0..32].fill(1.0);
        let priorities = RolePriorityTable::default();
        let (low_ducked, explanation) = apply_role_sidechain(
            &target,
            &kick,
            48_000,
            SourceRole::Kick,
            SourceRole::Bass,
            &priorities,
            SidechainSettings {
                amount: 0.8,
                attack_seconds: 0.0,
                bands: DuckBands::LOW_ONLY,
                ..SidechainSettings::default()
            },
        );
        let (high_ducked, _) = apply_role_sidechain(
            &target,
            &kick,
            48_000,
            SourceRole::Kick,
            SourceRole::Bass,
            &priorities,
            SidechainSettings {
                amount: 0.8,
                attack_seconds: 0.0,
                bands: DuckBands::HIGH_ONLY,
                ..SidechainSettings::default()
            },
        );

        assert_ne!(low_ducked, high_ducked);
        assert!(explanation.active);
        assert!(explanation.peak_envelope > 0.0);
        assert!(explanation.maximum_attenuation > 0.0);
        assert_eq!(explanation.source_role, SourceRole::Kick);
        assert_eq!(explanation.target_role, SourceRole::Bass);
    }

    #[test]
    fn lower_priority_source_does_not_duck_higher_priority_target() {
        let target = AudioBlock::silent(1, 8);
        let mut bass = AudioBlock::silent(1, 8);
        bass.samples_mut().fill(1.0);
        let (output, explanation) = apply_role_sidechain(
            &target,
            &bass,
            48_000,
            SourceRole::Bass,
            SourceRole::Kick,
            &RolePriorityTable::default(),
            SidechainSettings::default(),
        );
        assert_eq!(output, target);
        assert!(!explanation.active);
    }

    #[test]
    fn sidechain_relation_fingerprint_tracks_routing_and_settings() {
        let relation = SidechainRelation {
            control_track: TrackId::new(1),
            source_role: SourceRole::Kick,
            target_role: SourceRole::Bass,
            settings: SidechainSettings::default(),
        };
        let changed = SidechainRelation {
            settings: SidechainSettings {
                bands: DuckBands::ALL,
                ..relation.settings
            },
            ..relation
        };
        assert_eq!(
            sidechain_relation_fingerprint(relation),
            sidechain_relation_fingerprint(relation)
        );
        assert_ne!(
            sidechain_relation_fingerprint(relation),
            sidechain_relation_fingerprint(changed)
        );
    }
}
