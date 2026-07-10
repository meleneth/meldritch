//! Headless offline rendering foundations.
//!
//! This crate starts with a deliberately small deterministic event renderer. It
//! is not a sampler yet; it proves that scheduled core events can become finite
//! internal `f64` audio blocks without touching device I/O.

use meldritch_audio::{AudioBlock, SampleBuffer};
use meldritch_core::{FrameRange, Pattern, PatternId, ProbabilitySeed, Sample, SampleRate, Tempo};
use std::collections::BTreeMap;

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

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ArtifactKey {
    pattern: PatternId,
    range: FrameRange,
    sample_rate: SampleRate,
    fingerprint: Fingerprint,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CacheStatus {
    Hit,
    Miss,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CachedArtifact {
    key: ArtifactKey,
    block: AudioBlock,
    status: CacheStatus,
}

impl CachedArtifact {
    #[must_use]
    pub const fn key(&self) -> ArtifactKey {
        self.key
    }

    #[must_use]
    pub fn block(&self) -> &AudioBlock {
        &self.block
    }

    #[must_use]
    pub const fn status(&self) -> CacheStatus {
        self.status
    }

    #[must_use]
    pub fn into_block(self) -> AudioBlock {
        self.block
    }
}

#[derive(Clone, Debug, Default)]
pub struct ArtifactCache {
    artifacts: BTreeMap<ArtifactKey, AudioBlock>,
}

impl ArtifactCache {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.artifacts.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.artifacts.is_empty()
    }

    #[must_use]
    pub fn get(&self, key: ArtifactKey) -> Option<&AudioBlock> {
        self.artifacts.get(&key)
    }

    pub fn insert(&mut self, key: ArtifactKey, block: AudioBlock) -> Option<AudioBlock> {
        self.artifacts.insert(key, block)
    }
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

#[must_use]
pub fn pattern_sample_artifact_key(
    pattern: &Pattern,
    tempo: Tempo,
    range: FrameRange,
    probability_seed: ProbabilitySeed,
    settings: RenderSettings,
    samples_by_note: &BTreeMap<u8, SampleBuffer>,
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
    state.write_u64(0x7361_6d70_6c65_7265);

    for (note, sample) in samples_by_note {
        state.write_u64(u64::from(*note));
        state.write_u64(u64::from(sample.channels()));
        state.write_u64(u64::from(sample.sample_rate()));
        state.write_u64(u64::from(sample.frames()));
        state.write_u64(sample_signature(sample));
    }

    ArtifactKey {
        pattern: pattern.id(),
        range,
        sample_rate: tempo.sample_rate(),
        fingerprint: state.finish(),
    }
}

pub fn render_pattern_clicks_cached(
    cache: &mut ArtifactCache,
    pattern: &Pattern,
    tempo: Tempo,
    range: FrameRange,
    probability_seed: ProbabilitySeed,
    settings: RenderSettings,
) -> CachedArtifact {
    let key = pattern_click_artifact_key(pattern, tempo, range, probability_seed, settings);
    if let Some(block) = cache.get(key) {
        return CachedArtifact {
            key,
            block: block.clone(),
            status: CacheStatus::Hit,
        };
    }

    let block = render_pattern_clicks(pattern, tempo, range, probability_seed, settings);
    cache.insert(key, block.clone());
    CachedArtifact {
        key,
        block,
        status: CacheStatus::Miss,
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

pub fn render_pattern_samples(
    pattern: &Pattern,
    tempo: Tempo,
    range: FrameRange,
    probability_seed: ProbabilitySeed,
    settings: RenderSettings,
    samples_by_note: &BTreeMap<u8, SampleBuffer>,
) -> AudioBlock {
    let frames = range
        .end()
        .saturating_sub(range.start())
        .min(u64::from(u32::MAX)) as u32;
    let mut block = AudioBlock::silent(settings.channels(), frames);
    let mut events = Vec::new();
    pattern.events_between(tempo, range, probability_seed, &mut events);

    for event in events {
        let Some(sample) = samples_by_note.get(&event.note()) else {
            continue;
        };
        let Some(relative_frame) = event.range().start().checked_sub(range.start()) else {
            continue;
        };
        if relative_frame >= u64::from(frames) {
            continue;
        }

        mix_sample(&mut block, relative_frame as u32, sample, event.velocity());
    }

    block
}

pub fn render_pattern_samples_cached(
    cache: &mut ArtifactCache,
    pattern: &Pattern,
    tempo: Tempo,
    range: FrameRange,
    probability_seed: ProbabilitySeed,
    settings: RenderSettings,
    samples_by_note: &BTreeMap<u8, SampleBuffer>,
) -> CachedArtifact {
    let key = pattern_sample_artifact_key(
        pattern,
        tempo,
        range,
        probability_seed,
        settings,
        samples_by_note,
    );
    if let Some(block) = cache.get(key) {
        return CachedArtifact {
            key,
            block: block.clone(),
            status: CacheStatus::Hit,
        };
    }

    let block = render_pattern_samples(
        pattern,
        tempo,
        range,
        probability_seed,
        settings,
        samples_by_note,
    );
    cache.insert(key, block.clone());
    CachedArtifact {
        key,
        block,
        status: CacheStatus::Miss,
    }
}

fn write_frame(block: &mut AudioBlock, frame: u32, sample: Sample) {
    let channels = usize::from(block.channels());
    let start = frame as usize * channels;

    for channel_offset in 0..channels {
        block.samples_mut()[start + channel_offset] += sample;
    }
}

fn mix_sample(block: &mut AudioBlock, start_frame: u32, sample: &SampleBuffer, gain: Sample) {
    let out_channels = usize::from(block.channels());
    let sample_channels = usize::from(sample.channels());
    let available_frames = block.frames().saturating_sub(start_frame);
    let frames_to_mix = available_frames.min(sample.frames());

    for frame in 0..frames_to_mix {
        for out_channel in 0..out_channels {
            let source_channel = out_channel.min(sample_channels.saturating_sub(1));
            let source_index = frame as usize * sample_channels + source_channel;
            let target_index = (start_frame + frame) as usize * out_channels + out_channel;
            block.samples_mut()[target_index] += sample.samples()[source_index] * gain;
        }
    }
}

fn sample_signature(sample: &SampleBuffer) -> u64 {
    let mut state = FingerprintBuilder::new();
    for sample in sample.samples() {
        state.write_u64(sample.to_bits());
    }

    state.finish().raw()
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
    fn renders_pattern_events_with_sample_lookup() {
        let mut pattern = Pattern::new(PatternId::new(1), 16, 4).unwrap();
        pattern
            .set_step(
                TrackId::new(1),
                StepIndex::new(0),
                Step::new(36).with_velocity(0.5),
            )
            .unwrap();
        let mut samples_by_note = BTreeMap::new();
        samples_by_note.insert(36, SampleBuffer::new(1, 48_000, vec![1.0, 0.5]));

        let block = render_pattern_samples(
            &pattern,
            tempo(),
            FrameRange::new(0, 4).unwrap(),
            ProbabilitySeed::new(0),
            RenderSettings::new(2).unwrap(),
            &samples_by_note,
        );

        assert_eq!(block.samples(), &[0.5, 0.5, 0.25, 0.25, 0.0, 0.0, 0.0, 0.0]);
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

    #[test]
    fn cached_click_render_reports_miss_then_hit() {
        let pattern = Pattern::new(PatternId::new(1), 16, 4).unwrap();
        let mut cache = ArtifactCache::new();

        let first = render_pattern_clicks_cached(
            &mut cache,
            &pattern,
            tempo(),
            FrameRange::new(0, 96_000).unwrap(),
            ProbabilitySeed::new(11),
            RenderSettings::new(2).unwrap(),
        );
        let second = render_pattern_clicks_cached(
            &mut cache,
            &pattern,
            tempo(),
            FrameRange::new(0, 96_000).unwrap(),
            ProbabilitySeed::new(11),
            RenderSettings::new(2).unwrap(),
        );

        assert_eq!(first.status(), CacheStatus::Miss);
        assert_eq!(second.status(), CacheStatus::Hit);
        assert_eq!(cache.len(), 1);
        assert_eq!(first.block(), second.block());
    }

    #[test]
    fn sample_artifact_key_changes_when_sample_content_changes() {
        let pattern = Pattern::new(PatternId::new(1), 16, 4).unwrap();
        let mut first_samples = BTreeMap::new();
        first_samples.insert(36, SampleBuffer::new(1, 48_000, vec![1.0, 0.5]));
        let mut changed_samples = BTreeMap::new();
        changed_samples.insert(36, SampleBuffer::new(1, 48_000, vec![1.0, 0.25]));

        let first = pattern_sample_artifact_key(
            &pattern,
            tempo(),
            FrameRange::new(0, 96_000).unwrap(),
            ProbabilitySeed::new(11),
            RenderSettings::new(2).unwrap(),
            &first_samples,
        );
        let changed = pattern_sample_artifact_key(
            &pattern,
            tempo(),
            FrameRange::new(0, 96_000).unwrap(),
            ProbabilitySeed::new(11),
            RenderSettings::new(2).unwrap(),
            &changed_samples,
        );

        assert_ne!(first.fingerprint(), changed.fingerprint());
    }
}
