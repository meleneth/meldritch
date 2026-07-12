//! Atomic publication of immutable audio snapshots.

use crate::published_audio::PublishedAudio;
use arc_swap::ArcSwap;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AudioPublicationError {
    IncompatibleChannels { expected: u16, actual: u16 },
    IncompatibleFrames { expected: u32, actual: u32 },
    IncompatibleChunkFrames { expected: u32, actual: u32 },
}

#[derive(Clone)]
pub struct AudioSnapshotPublisher {
    shared: Arc<ArcSwap<PublishedAudio>>,
    channels: u16,
    frames: u32,
    chunk_frames: u32,
}

#[derive(Clone)]
pub struct AudioSnapshotReader {
    shared: Arc<ArcSwap<PublishedAudio>>,
}

#[must_use]
pub fn audio_publication(initial: PublishedAudio) -> (AudioSnapshotPublisher, AudioSnapshotReader) {
    let channels = initial.channels();
    let frames = initial.frames();
    let chunk_frames = initial.chunk_frames();
    let shared = Arc::new(ArcSwap::from_pointee(initial));
    (
        AudioSnapshotPublisher {
            shared: Arc::clone(&shared),
            channels,
            frames,
            chunk_frames,
        },
        AudioSnapshotReader { shared },
    )
}

impl AudioSnapshotPublisher {
    /// Publish a complete or partial snapshot for acquisition by the next
    /// callback. Timeline layout remains fixed for the lifetime of a stream.
    pub fn publish(&self, snapshot: PublishedAudio) -> Result<(), AudioPublicationError> {
        if snapshot.channels() != self.channels {
            return Err(AudioPublicationError::IncompatibleChannels {
                expected: self.channels,
                actual: snapshot.channels(),
            });
        }
        if snapshot.frames() != self.frames {
            return Err(AudioPublicationError::IncompatibleFrames {
                expected: self.frames,
                actual: snapshot.frames(),
            });
        }
        if snapshot.chunk_frames() != self.chunk_frames {
            return Err(AudioPublicationError::IncompatibleChunkFrames {
                expected: self.chunk_frames,
                actual: snapshot.chunk_frames(),
            });
        }
        self.shared.store(Arc::new(snapshot));
        Ok(())
    }
}

impl AudioSnapshotReader {
    /// Acquire one immutable snapshot. Callbacks should retain this `Arc` for
    /// the complete output buffer so publication cannot split a callback.
    #[must_use]
    pub fn snapshot(&self) -> Arc<PublishedAudio> {
        self.shared.load_full()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AudioBlock;

    #[test]
    fn reader_observes_atomically_replaced_snapshot() {
        let first = PublishedAudio::from_block(&AudioBlock::silent(1, 2), 1).unwrap();
        let (publisher, reader) = audio_publication(first);
        let mut block = AudioBlock::silent(1, 2);
        block.samples_mut().fill(0.75);
        let second = PublishedAudio::from_block(&block, 1).unwrap();

        let retained = reader.snapshot();
        publisher.publish(second).unwrap();
        assert_eq!(retained.frame(0), Ok([0.0].as_slice()));
        assert_eq!(reader.snapshot().frame(0), Ok([0.75].as_slice()));
    }

    #[test]
    fn publication_rejects_layout_changes() {
        let initial = PublishedAudio::from_block(&AudioBlock::silent(1, 4), 2).unwrap();
        let (publisher, _) = audio_publication(initial);
        let wrong_channels = PublishedAudio::from_block(&AudioBlock::silent(2, 4), 2).unwrap();

        assert_eq!(
            publisher.publish(wrong_channels),
            Err(AudioPublicationError::IncompatibleChannels {
                expected: 1,
                actual: 2,
            })
        );
    }
}
