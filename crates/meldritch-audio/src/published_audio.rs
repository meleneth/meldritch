//! Immutable, callback-readable snapshots of pre-rendered audio chunks.

use crate::{AudioBlock, Frames, Sample};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublishedAudioError {
    EmptyAudio,
    ZeroChunkFrames,
    InvalidChannelCount,
    WrongChunkCount {
        expected: usize,
        actual: usize,
    },
    WrongChunkSamples {
        index: usize,
        expected: usize,
        actual: usize,
    },
}

#[derive(Clone, Debug)]
pub struct PublishedAudio {
    channels: u16,
    frames: Frames,
    chunk_frames: Frames,
    chunks: Box<[Option<Arc<[Sample]>>]>,
}

impl PublishedAudio {
    pub fn from_block(
        block: &AudioBlock,
        chunk_frames: Frames,
    ) -> Result<Self, PublishedAudioError> {
        if block.channels() == 0 || block.frames() == 0 {
            return Err(PublishedAudioError::EmptyAudio);
        }
        if chunk_frames == 0 {
            return Err(PublishedAudioError::ZeroChunkFrames);
        }

        let channels = usize::from(block.channels());
        let samples_per_chunk = chunk_frames as usize * channels;
        let chunks = block
            .samples()
            .chunks(samples_per_chunk)
            .map(|samples| Some(Arc::from(samples)))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Self::from_chunks(
            block.channels(),
            block.frames(),
            chunk_frames,
            chunks.into_vec(),
        )
    }

    /// Build a snapshot that may contain unpublished chunks. All present
    /// chunks are validated before the snapshot can reach the callback.
    pub fn from_chunks(
        channels: u16,
        frames: Frames,
        chunk_frames: Frames,
        chunks: Vec<Option<Arc<[Sample]>>>,
    ) -> Result<Self, PublishedAudioError> {
        if channels == 0 {
            return Err(PublishedAudioError::InvalidChannelCount);
        }
        if frames == 0 {
            return Err(PublishedAudioError::EmptyAudio);
        }
        if chunk_frames == 0 {
            return Err(PublishedAudioError::ZeroChunkFrames);
        }
        let expected_chunks = frames.div_ceil(chunk_frames) as usize;
        if chunks.len() != expected_chunks {
            return Err(PublishedAudioError::WrongChunkCount {
                expected: expected_chunks,
                actual: chunks.len(),
            });
        }
        for (index, chunk) in chunks.iter().enumerate() {
            let Some(samples) = chunk else {
                continue;
            };
            let start_frame = index as Frames * chunk_frames;
            let frames_in_chunk = chunk_frames.min(frames - start_frame);
            let expected = frames_in_chunk as usize * usize::from(channels);
            if samples.len() != expected {
                return Err(PublishedAudioError::WrongChunkSamples {
                    index,
                    expected,
                    actual: samples.len(),
                });
            }
        }

        Ok(Self {
            channels,
            frames,
            chunk_frames,
            chunks: chunks.into_boxed_slice(),
        })
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
    pub const fn chunk_frames(&self) -> Frames {
        self.chunk_frames
    }

    #[must_use]
    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// Return one source frame, or the missing chunk index when its artifact
    /// was not published. Frame bounds are guaranteed by the transport.
    pub fn frame(&self, frame: Frames) -> Result<&[Sample], usize> {
        let chunk_index = (frame / self.chunk_frames) as usize;
        let frame_in_chunk = (frame % self.chunk_frames) as usize;
        let Some(samples) = self.chunks.get(chunk_index).and_then(Option::as_deref) else {
            return Err(chunk_index);
        };
        let start = frame_in_chunk * usize::from(self.channels);
        Ok(&samples[start..start + usize::from(self.channels)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_is_split_into_addressable_chunks() {
        let mut block = AudioBlock::silent(2, 5);
        block
            .samples_mut()
            .copy_from_slice(&[0.0, 0.1, 1.0, 1.1, 2.0, 2.1, 3.0, 3.1, 4.0, 4.1]);
        let audio = PublishedAudio::from_block(&block, 2).unwrap();

        assert_eq!(audio.chunk_count(), 3);
        assert_eq!(audio.frame(0), Ok([0.0, 0.1].as_slice()));
        assert_eq!(audio.frame(2), Ok([2.0, 2.1].as_slice()));
        assert_eq!(audio.frame(4), Ok([4.0, 4.1].as_slice()));
    }

    #[test]
    fn missing_chunk_is_reported_without_fallback_allocation() {
        let audio =
            PublishedAudio::from_chunks(1, 4, 2, vec![Some(Arc::from([0.0, 0.0])), None]).unwrap();

        assert_eq!(audio.frame(1), Ok([0.0].as_slice()));
        assert_eq!(audio.frame(2), Err(1));
        assert_eq!(audio.frame(3), Err(1));
    }

    #[test]
    fn publishing_rejects_empty_audio_and_zero_sized_chunks() {
        assert_eq!(
            PublishedAudio::from_block(&AudioBlock::silent(1, 0), 1).unwrap_err(),
            PublishedAudioError::EmptyAudio
        );
        assert_eq!(
            PublishedAudio::from_block(&AudioBlock::silent(1, 1), 0).unwrap_err(),
            PublishedAudioError::ZeroChunkFrames
        );
    }

    #[test]
    fn partial_snapshot_validates_present_chunk_lengths() {
        assert_eq!(
            PublishedAudio::from_chunks(2, 2, 2, vec![Some(Arc::from([0.0, 0.0]))]).unwrap_err(),
            PublishedAudioError::WrongChunkSamples {
                index: 0,
                expected: 4,
                actual: 2,
            }
        );
    }
}
