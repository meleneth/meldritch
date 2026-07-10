//! Audio-facing type aliases and future audio engine boundary.

pub use meldritch_core::{Coeff, Frame, Frames, Param, Sample, SampleRate};

#[derive(Clone, Debug, PartialEq)]
pub struct AudioBlock {
    channels: u16,
    frames: Frames,
    samples: Vec<Sample>,
}

impl AudioBlock {
    #[must_use]
    pub fn silent(channels: u16, frames: Frames) -> Self {
        let sample_count = usize::from(channels) * frames as usize;
        Self {
            channels,
            frames,
            samples: vec![0.0; sample_count],
        }
    }

    #[must_use]
    pub const fn channels(&self) -> u16 {
        self.channels
    }

    #[must_use]
    pub const fn frames(&self) -> Frames {
        self.frames
    }

    #[must_use]
    pub fn samples(&self) -> &[Sample] {
        &self.samples
    }

    #[must_use]
    pub fn samples_mut(&mut self) -> &mut [Sample] {
        &mut self.samples
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silent_audio_block_uses_f64_samples() {
        let block = AudioBlock::silent(2, 16);

        assert_eq!(block.channels(), 2);
        assert_eq!(block.frames(), 16);
        assert_eq!(block.samples().len(), 32);
        assert!(block.samples().iter().all(|sample| *sample == 0.0));
    }
}
