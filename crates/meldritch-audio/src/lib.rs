//! Audio-facing type aliases and future audio engine boundary.

pub use meldritch_core::{Coeff, Frame, Frames, Param, Sample, SampleRate};
use std::fmt;
use std::path::Path;

pub mod audio_publication;
pub mod device_output;
pub mod published_audio;
pub mod realtime_queue;
pub mod realtime_status;
pub mod transport;

#[derive(Clone, Debug, PartialEq)]
pub struct AudioBlock {
    channels: u16,
    frames: Frames,
    samples: Vec<Sample>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SampleBuffer {
    channels: u16,
    sample_rate: SampleRate,
    samples: Vec<Sample>,
}

impl SampleBuffer {
    #[must_use]
    pub fn new(channels: u16, sample_rate: SampleRate, samples: Vec<Sample>) -> Self {
        Self {
            channels,
            sample_rate,
            samples,
        }
    }

    #[must_use]
    pub const fn channels(&self) -> u16 {
        self.channels
    }

    #[must_use]
    pub const fn sample_rate(&self) -> SampleRate {
        self.sample_rate
    }

    #[must_use]
    pub fn samples(&self) -> &[Sample] {
        &self.samples
    }

    #[must_use]
    pub fn frames(&self) -> Frames {
        (self.samples.len() / usize::from(self.channels)) as Frames
    }
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

    #[must_use]
    pub fn peak_abs(&self) -> Sample {
        self.samples
            .iter()
            .fold(0.0, |peak, sample| peak.max(sample.abs()))
    }

    #[must_use]
    pub fn normalized_to_peak(&self, target_peak: Sample) -> Self {
        let peak = self.peak_abs();
        if peak == 0.0 || target_peak <= 0.0 {
            return self.clone();
        }

        let gain = target_peak / peak;
        let mut normalized = self.clone();
        for sample in normalized.samples_mut() {
            *sample *= gain;
        }

        normalized
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

pub fn read_wav(path: impl AsRef<Path>) -> Result<SampleBuffer, Box<dyn std::error::Error>> {
    let mut reader = hound::WavReader::open(path)?;
    let spec = reader.spec();
    let samples = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .map(|sample| sample.map(f64::from))
            .collect::<Result<Vec<_>, _>>()?,
        hound::SampleFormat::Int => read_int_samples(&mut reader, spec.bits_per_sample)?,
    };

    Ok(SampleBuffer::new(spec.channels, spec.sample_rate, samples))
}

fn read_int_samples<R: std::io::Read>(
    reader: &mut hound::WavReader<R>,
    bits_per_sample: u16,
) -> Result<Vec<Sample>, hound::Error> {
    if bits_per_sample <= 16 {
        let scale = f64::from(i16::MAX);
        reader
            .samples::<i16>()
            .map(|sample| sample.map(|sample| f64::from(sample) / scale))
            .collect()
    } else {
        let max_amplitude = ((1_i64 << (bits_per_sample - 1)) - 1) as f64;
        reader
            .samples::<i32>()
            .map(|sample| sample.map(|sample| f64::from(sample) / max_amplitude))
            .collect()
    }
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

    #[test]
    fn reads_wav_into_f64_sample_buffer() {
        let path =
            std::env::temp_dir().join(format!("meldritch-read-test-{}.wav", std::process::id()));
        let mut block = AudioBlock::silent(1, 2);
        block.samples_mut()[0] = 0.25;
        block.samples_mut()[1] = -0.25;
        write_wav_f32(&path, &block, 48_000).unwrap();

        let loaded = read_wav(&path).unwrap();

        assert_eq!(loaded.channels(), 1);
        assert_eq!(loaded.sample_rate(), 48_000);
        assert_eq!(loaded.frames(), 2);
        assert_eq!(loaded.samples(), &[0.25, -0.25]);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn audio_block_reports_and_normalizes_peak() {
        let mut block = AudioBlock::silent(1, 3);
        block.samples_mut()[0] = -2.0;
        block.samples_mut()[1] = 0.5;
        block.samples_mut()[2] = 1.0;

        let normalized = block.normalized_to_peak(1.0);

        assert_eq!(block.peak_abs(), 2.0);
        assert_eq!(normalized.samples(), &[-1.0, 0.25, 0.5]);
    }
}
