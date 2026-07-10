//! Headless offline rendering foundations.
//!
//! This crate starts with a deliberately small deterministic event renderer. It
//! is not a sampler yet; it proves that scheduled core events can become finite
//! internal `f64` audio blocks without touching device I/O.

use meldritch_audio::AudioBlock;
use meldritch_core::{FrameRange, Pattern, ProbabilitySeed, Sample, Tempo};

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
}
