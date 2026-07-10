//! Headless offline rendering foundations.
//!
//! This crate starts with a deliberately small deterministic event renderer. It
//! is not a sampler yet; it proves that scheduled core events can become finite
//! internal `f64` audio blocks without touching device I/O.

use meldritch_audio::AudioBlock;
use meldritch_core::{FrameRange, Pattern, PatternId, ProbabilitySeed, Sample, SampleRate, Tempo};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RenderSettings {
    channels: u16,
}

impl RenderSettings {
    pub fn new(channels: u16) -> Result<Self, RenderSettingsError> {
        if channels == 0 {
            return Err(RenderSettingsError::InvalidChannelCount(channels));
        }

        Ok(Self { channels })
    }

    #[must_use]
    pub const fn channels(self) -> u16 {
        self.channels
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderSettingsError {
    InvalidChannelCount(u16),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Fingerprint(u64);

impl Fingerprint {
    #[must_use]
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    #[must_use]
    pub const fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactKey {
    pattern: PatternId,
    range: FrameRange,
    sample_rate: SampleRate,
    fingerprint: Fingerprint,
}

impl ArtifactKey {
    #[must_use]
    pub const fn pattern(self) -> PatternId {
        self.pattern
    }

    #[must_use]
    pub const fn range(self) -> FrameRange {
        self.range
    }

    #[must_use]
    pub const fn sample_rate(self) -> SampleRate {
        self.sample_rate
    }

    #[must_use]
    pub const fn fingerprint(self) -> Fingerprint {
        self.fingerprint
    }
}

#[must_use]
pub fn pattern_click_artifact_key(
    pattern: &Pattern,
    tempo: Tempo,
    range: FrameRange,
    probability_seed: ProbabilitySeed,
    settings: RenderSettings,
) -> ArtifactKey {
    let mut state = FingerprintBuilder::new();
    state.write_u64(pattern.id().raw());
    state.write_u64(u64::from(pattern.length_steps()));
    state.write_u64(u64::from(pattern.steps_per_beat()));
    state.write_u64(range.start());
    state.write_u64(range.end());
    state.write_u64(u64::from(tempo.sample_rate()));
    state.write_u64(tempo.bpm().to_bits());
    state.write_u64(probability_seed.raw());
    state.write_u64(u64::from(settings.channels()));
    state.write_u64(0x7061_7474_6572_6e63);

    ArtifactKey {
        pattern: pattern.id(),
        range,
        sample_rate: tempo.sample_rate(),
        fingerprint: state.finish(),
    }
}

pub fn render_pattern_clicks(
    pattern: &Pattern,
    tempo: Tempo,
    range: FrameRange,
    probability_seed: ProbabilitySeed,
    settings: RenderSettings,
) -> AudioBlock {
    let frames = range
        .end()
        .saturating_sub(range.start())
        .min(u64::from(u32::MAX)) as u32;
    let mut block = AudioBlock::silent(settings.channels(), frames);
    let mut events = Vec::new();
    pattern.events_between(tempo, range, probability_seed, &mut events);

    for event in events {
        let Some(relative_frame) = event.range().start().checked_sub(range.start()) else {
            continue;
        };
        if relative_frame >= u64::from(frames) {
            continue;
        }

        let sample = event.velocity().clamp(0.0, 1.0) as Sample;
        write_frame(&mut block, relative_frame as u32, sample);
    }

    block
}

fn write_frame(block: &mut AudioBlock, frame: u32, sample: Sample) {
    let channels = usize::from(block.channels());
    let start = frame as usize * channels;

    for channel_offset in 0..channels {
        block.samples_mut()[start + channel_offset] += sample;
    }
}

struct FingerprintBuilder {
    state: u64,
}

impl FingerprintBuilder {
    fn new() -> Self {
        Self {
            state: 0xcbf2_9ce4_8422_2325,
        }
    }

    fn write_u64(&mut self, value: u64) {
        self.state ^= mix_u64(value);
        self.state = self.state.wrapping_mul(0x0000_0100_0000_01b3);
    }

    fn finish(self) -> Fingerprint {
        Fingerprint::new(mix_u64(self.state))
    }
}

fn mix_u64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;
    use meldritch_core::{PatternId, Step, StepIndex, TrackId};

    fn tempo() -> Tempo {
        Tempo::new(120.0, 48_000).unwrap()
    }

    #[test]
    fn renders_pattern_events_into_audio_block() {
        let mut pattern = Pattern::new(PatternId::new(1), 16, 4).unwrap();
        pattern
            .set_step(
                TrackId::new(1),
                StepIndex::new(0),
                Step::new(36).with_velocity(0.75),
            )
            .unwrap();
        pattern
            .set_step(
                TrackId::new(1),
                StepIndex::new(4),
                Step::new(38).with_velocity(0.5),
            )
            .unwrap();

        let block = render_pattern_clicks(
            &pattern,
            tempo(),
            FrameRange::new(0, 30_000).unwrap(),
            ProbabilitySeed::new(0),
            RenderSettings::new(2).unwrap(),
        );

        assert_eq!(block.channels(), 2);
        assert_eq!(block.frames(), 30_000);
        assert_eq!(block.samples()[0], 0.75);
        assert_eq!(block.samples()[1], 0.75);
        assert_eq!(block.samples()[48_000], 0.5);
        assert_eq!(block.samples()[48_001], 0.5);
        assert!(block.samples().iter().all(|sample| sample.is_finite()));
    }

    #[test]
    fn render_is_deterministic_for_same_seed() {
        let mut pattern = Pattern::new(PatternId::new(1), 4, 4).unwrap();
        pattern
            .set_step(TrackId::new(1), StepIndex::new(0), Step::new(36))
            .unwrap();

        let first = render_pattern_clicks(
            &pattern,
            tempo(),
            FrameRange::new(0, 24_000).unwrap(),
            ProbabilitySeed::new(9),
            RenderSettings::new(1).unwrap(),
        );
        let second = render_pattern_clicks(
            &pattern,
            tempo(),
            FrameRange::new(0, 24_000).unwrap(),
            ProbabilitySeed::new(9),
            RenderSettings::new(1).unwrap(),
        );

        assert_eq!(first, second);
    }

    #[test]
    fn rejects_zero_channels() {
        assert_eq!(
            RenderSettings::new(0),
            Err(RenderSettingsError::InvalidChannelCount(0))
        );
    }

    #[test]
    fn artifact_key_is_stable_for_same_render_inputs() {
        let pattern = Pattern::new(PatternId::new(1), 16, 4).unwrap();
        let key = pattern_click_artifact_key(
            &pattern,
            tempo(),
            FrameRange::new(0, 96_000).unwrap(),
            ProbabilitySeed::new(11),
            RenderSettings::new(2).unwrap(),
        );
        let same = pattern_click_artifact_key(
            &pattern,
            tempo(),
            FrameRange::new(0, 96_000).unwrap(),
            ProbabilitySeed::new(11),
            RenderSettings::new(2).unwrap(),
        );

        assert_eq!(key, same);
    }

    #[test]
    fn artifact_key_changes_for_semantic_render_inputs() {
        let pattern = Pattern::new(PatternId::new(1), 16, 4).unwrap();
        let base = pattern_click_artifact_key(
            &pattern,
            tempo(),
            FrameRange::new(0, 96_000).unwrap(),
            ProbabilitySeed::new(11),
            RenderSettings::new(2).unwrap(),
        );
        let changed_seed = pattern_click_artifact_key(
            &pattern,
            tempo(),
            FrameRange::new(0, 96_000).unwrap(),
            ProbabilitySeed::new(12),
            RenderSettings::new(2).unwrap(),
        );
        let changed_range = pattern_click_artifact_key(
            &pattern,
            tempo(),
            FrameRange::new(0, 48_000).unwrap(),
            ProbabilitySeed::new(11),
            RenderSettings::new(2).unwrap(),
        );

        assert_ne!(base.fingerprint(), changed_seed.fingerprint());
        assert_ne!(base.fingerprint(), changed_range.fingerprint());
    }
}
