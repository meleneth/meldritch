//! Atomic publication of callback-owned realtime state.

use crate::Frames;
use crate::transport::{PlaybackTransport, TransportState};
use std::cell::Cell;
use std::marker::PhantomData;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, AtomicU32, AtomicU64, Ordering};

const STOPPED: u8 = 0;
const PLAYING: u8 = 1;

#[derive(Default)]
struct RealtimeStatusInner {
    state: AtomicU8,
    position: AtomicU32,
    callbacks: AtomicU64,
    stream_errors: AtomicU64,
    underruns: AtomicU64,
    missed_artifacts: AtomicU64,
}

pub struct RealtimeStatusPublisher {
    inner: Arc<RealtimeStatusInner>,
    not_sync: PhantomData<Cell<()>>,
}

#[derive(Clone)]
pub struct RealtimeStatusMonitor {
    inner: Arc<RealtimeStatusInner>,
}

#[derive(Clone)]
pub struct StreamErrorReporter {
    inner: Arc<RealtimeStatusInner>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RealtimeStatusSnapshot {
    pub state: TransportState,
    pub position: Frames,
    pub callbacks: u64,
    pub stream_errors: u64,
    pub underruns: u64,
    pub missed_artifacts: u64,
}

#[must_use]
pub fn realtime_status() -> (
    RealtimeStatusPublisher,
    RealtimeStatusMonitor,
    StreamErrorReporter,
) {
    let inner = Arc::new(RealtimeStatusInner::default());
    (
        RealtimeStatusPublisher {
            inner: Arc::clone(&inner),
            not_sync: PhantomData,
        },
        RealtimeStatusMonitor {
            inner: Arc::clone(&inner),
        },
        StreamErrorReporter { inner },
    )
}

impl RealtimeStatusPublisher {
    pub fn publish_transport(&mut self, transport: &PlaybackTransport) {
        self.inner
            .position
            .store(transport.position(), Ordering::Relaxed);
        let state = match transport.state() {
            TransportState::Stopped => STOPPED,
            TransportState::Playing => PLAYING,
        };
        self.inner.state.store(state, Ordering::Release);
    }

    pub fn callback_completed(&mut self) {
        self.inner.callbacks.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_underrun(&mut self) {
        self.inner.underruns.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_missed_artifact(&mut self) {
        self.inner.missed_artifacts.fetch_add(1, Ordering::Relaxed);
    }
}

impl RealtimeStatusMonitor {
    #[must_use]
    pub fn snapshot(&self) -> RealtimeStatusSnapshot {
        let state = match self.inner.state.load(Ordering::Acquire) {
            PLAYING => TransportState::Playing,
            _ => TransportState::Stopped,
        };
        RealtimeStatusSnapshot {
            state,
            position: self.inner.position.load(Ordering::Relaxed),
            callbacks: self.inner.callbacks.load(Ordering::Relaxed),
            stream_errors: self.inner.stream_errors.load(Ordering::Relaxed),
            underruns: self.inner.underruns.load(Ordering::Relaxed),
            missed_artifacts: self.inner.missed_artifacts.load(Ordering::Relaxed),
        }
    }
}

impl StreamErrorReporter {
    pub fn record_error(&self) {
        self.inner.stream_errors.fetch_add(1, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monitor_observes_transport_and_callback_updates() {
        let (mut publisher, monitor, _) = realtime_status();
        let mut transport = PlaybackTransport::new(4, 8, 1).unwrap();
        transport.play();
        assert_eq!(transport.next_frame(), Some(4));

        publisher.publish_transport(&transport);
        publisher.callback_completed();
        assert_eq!(
            monitor.snapshot(),
            RealtimeStatusSnapshot {
                state: TransportState::Playing,
                position: 5,
                callbacks: 1,
                stream_errors: 0,
                underruns: 0,
                missed_artifacts: 0,
            }
        );
    }

    #[test]
    fn diagnostic_counters_are_monotonic() {
        let (mut publisher, monitor, errors) = realtime_status();
        publisher.record_underrun();
        publisher.record_underrun();
        publisher.record_missed_artifact();
        errors.record_error();

        let snapshot = monitor.snapshot();
        assert_eq!(snapshot.underruns, 2);
        assert_eq!(snapshot.missed_artifacts, 1);
        assert_eq!(snapshot.stream_errors, 1);
    }
}
