//! Audio-facing type aliases and future audio engine boundary.

pub use meldritch_core::{Coeff, Frame, Frames, Param, Sample, SampleRate};
use std::fmt;
use std::path::Path;

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WavExportError {
    InvalidSampleCount { channels: u16, samples: usize },
    NonFiniteSample { index: usize },
}

impl fmt::Display for WavExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSampleCount { channels, samples } => write!(
                f,
                "sample count {samples} is not divisible by channel count {channels}"
            ),
            Self::NonFiniteSample { index } => write!(f, "sample {index} is not finite"),
        }
    }
}

impl std::error::Error for WavExportError {}

pub fn write_wav_f32(
    path: impl AsRef<Path>,
    block: &AudioBlock,
    sample_rate: SampleRate,
) -> Result<(), Box<dyn std::error::Error>> {
    validate_export_block(block)?;

    let spec = hound::WavSpec {
        channels: block.channels(),
        sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, spec)?;

    for sample in block.samples() {
        writer.write_sample(sample.clamp(-1.0, 1.0) as f32)?;
    }

    writer.finalize()?;

    Ok(())
}

fn validate_export_block(block: &AudioBlock) -> Result<(), WavExportError> {
    let channels = usize::from(block.channels());
    if channels == 0 || !block.samples().len().is_multiple_of(channels) {
        return Err(WavExportError::InvalidSampleCount {
            channels: block.channels(),
            samples: block.samples().len(),
        });
    }

    for (index, sample) in block.samples().iter().enumerate() {
        if !sample.is_finite() {
            return Err(WavExportError::NonFiniteSample { index });
        }
    }

    Ok(())
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

    #[test]
    fn writes_f32_wav_at_file_boundary() {
        let path = std::env::temp_dir().join(format!("meldritch-test-{}.wav", std::process::id()));
        let mut block = AudioBlock::silent(2, 2);
        block.samples_mut()[0] = 0.5;
        block.samples_mut()[1] = -0.5;
        block.samples_mut()[2] = 2.0;
        block.samples_mut()[3] = -2.0;

        write_wav_f32(&path, &block, 48_000).unwrap();

        let reader = hound::WavReader::open(&path).unwrap();
        let spec = reader.spec();
        assert_eq!(spec.channels, 2);
        assert_eq!(spec.sample_rate, 48_000);
        assert_eq!(spec.bits_per_sample, 32);
        assert_eq!(spec.sample_format, hound::SampleFormat::Float);
        let samples = reader
            .into_samples::<f32>()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(samples, vec![0.5, -0.5, 1.0, -1.0]);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn wav_export_rejects_non_finite_samples() {
        let mut block = AudioBlock::silent(1, 1);
        block.samples_mut()[0] = f64::NAN;

        let err = validate_export_block(&block).unwrap_err();

        assert_eq!(err, WavExportError::NonFiniteSample { index: 0 });
    }
}
