//! Hardware-independent playback transport.

use crate::Frames;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransportState {
    Stopped,
    Playing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransportError {
    EmptyLoop,
    ZeroLoops,
    PositionOutsideLoop,
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyLoop => write!(f, "transport loop must contain at least one frame"),
            Self::ZeroLoops => write!(f, "transport loop count must be at least one"),
            Self::PositionOutsideLoop => write!(f, "transport position must be inside the loop"),
        }
    }
}

impl std::error::Error for TransportError {}

/// A finite looping playhead suitable for ownership by an audio callback.
///
/// This type performs no allocation after construction and has no dependency
/// on an audio device. Commands that cross thread boundaries will be applied
/// by the realtime event queue rather than by sharing this value behind a lock.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlaybackTransport {
    state: TransportState,
    loop_start: Frames,
    loop_end: Frames,
    position: Frames,
    total_loops: u32,
    loops_remaining: u32,
}

impl PlaybackTransport {
    pub fn new(loop_start: Frames, loop_end: Frames, loops: u32) -> Result<Self, TransportError> {
        if loop_start >= loop_end {
            return Err(TransportError::EmptyLoop);
        }
        if loops == 0 {
            return Err(TransportError::ZeroLoops);
        }

        Ok(Self {
            state: TransportState::Stopped,
            loop_start,
            loop_end,
            position: loop_start,
            total_loops: loops,
            loops_remaining: loops,
        })
    }

    #[must_use]
    pub const fn state(&self) -> TransportState {
        self.state
    }

    #[must_use]
    pub const fn position(&self) -> Frames {
        self.position
    }

    #[must_use]
    pub const fn loop_range(&self) -> (Frames, Frames) {
        (self.loop_start, self.loop_end)
    }

    #[must_use]
    pub const fn loops_remaining(&self) -> u32 {
        self.loops_remaining
    }

    #[must_use]
    pub const fn is_finished(&self) -> bool {
        self.loops_remaining == 0
    }

    pub fn play(&mut self) {
        if !self.is_finished() {
            self.state = TransportState::Playing;
        }
    }

    pub fn stop(&mut self) {
        self.state = TransportState::Stopped;
    }

    pub fn rewind(&mut self) {
        self.state = TransportState::Stopped;
        self.position = self.loop_start;
        self.loops_remaining = self.total_loops;
    }

    pub fn seek(&mut self, position: Frames) -> Result<(), TransportError> {
        if position < self.loop_start || position >= self.loop_end {
            return Err(TransportError::PositionOutsideLoop);
        }
        self.position = position;
        if self.is_finished() {
            self.loops_remaining = self.total_loops;
        }
        Ok(())
    }

    /// Return the current source frame and advance the playhead by one frame.
    /// A stopped or completed transport returns `None` without advancing.
    pub fn next_frame(&mut self) -> Option<Frames> {
        if self.state != TransportState::Playing || self.is_finished() {
            return None;
        }

        let frame = self.position;
        self.position += 1;
        if self.position == self.loop_end {
            self.loops_remaining -= 1;
            if self.is_finished() {
                self.state = TransportState::Stopped;
            } else {
                self.position = self.loop_start;
            }
        }
        Some(frame)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_starts_stopped_and_advances_only_while_playing() {
        let mut transport = PlaybackTransport::new(4, 6, 1).unwrap();

        assert_eq!(transport.state(), TransportState::Stopped);
        assert_eq!(transport.next_frame(), None);
        transport.play();
        assert_eq!(transport.next_frame(), Some(4));
        transport.stop();
        assert_eq!(transport.next_frame(), None);
        assert_eq!(transport.position(), 5);
    }

    #[test]
    fn transport_loops_and_stops_at_the_end() {
        let mut transport = PlaybackTransport::new(2, 4, 2).unwrap();
        transport.play();

        assert_eq!(
            (0..5).map(|_| transport.next_frame()).collect::<Vec<_>>(),
            vec![Some(2), Some(3), Some(2), Some(3), None]
        );
        assert_eq!(transport.state(), TransportState::Stopped);
        assert_eq!(transport.position(), 4);
        assert_eq!(transport.loops_remaining(), 0);
        assert!(transport.is_finished());
    }

    #[test]
    fn rewind_restores_a_completed_transport() {
        let mut transport = PlaybackTransport::new(0, 1, 1).unwrap();
        transport.play();
        assert_eq!(transport.next_frame(), Some(0));

        transport.rewind();
        assert_eq!(transport.position(), 0);
        assert_eq!(transport.loops_remaining(), 1);
        assert_eq!(transport.state(), TransportState::Stopped);
        transport.play();
        assert_eq!(transport.next_frame(), Some(0));
    }

    #[test]
    fn transport_rejects_empty_ranges_and_zero_loops() {
        assert_eq!(
            PlaybackTransport::new(3, 3, 1),
            Err(TransportError::EmptyLoop)
        );
        assert_eq!(
            PlaybackTransport::new(0, 1, 0),
            Err(TransportError::ZeroLoops)
        );
    }

    #[test]
    fn transport_seeks_within_an_arrangement_loop_range() {
        let mut transport = PlaybackTransport::new(48_000, 96_000, 2).unwrap();
        assert_eq!(transport.loop_range(), (48_000, 96_000));
        transport.seek(72_000).unwrap();
        transport.play();
        assert_eq!(transport.next_frame(), Some(72_000));
        assert_eq!(
            transport.seek(96_000),
            Err(TransportError::PositionOutsideLoop)
        );
    }
}
