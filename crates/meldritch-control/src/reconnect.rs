//! Paced, single-owner transport lifecycle. Never run from an audio callback.
use crate::Result;
use std::time::Duration;

pub const RETRY_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Debug, PartialEq, Eq)]
pub enum Change {
    Unchanged,
    Connected,
    Disconnected,
    Unavailable,
    Failed(String),
}

/// At most one owned connection and one probe/open attempt per interval.
/// Dropping a connection is transport cleanup, not an application command.
pub struct Connection<T> {
    live: Option<T>,
    next_poll: Duration,
}
impl<T> Default for Connection<T> {
    fn default() -> Self {
        Self {
            live: None,
            next_poll: Duration::ZERO,
        }
    }
}
impl<T> Connection<T> {
    pub fn get_mut(&mut self) -> Option<&mut T> {
        self.live.as_mut()
    }

    /// `now` is monotonic elapsed time supplied by the control owner/test clock.
    pub fn poll(
        &mut self,
        now: Duration,
        mut probe: impl FnMut(&T) -> Result<bool>,
        mut open: impl FnMut() -> Result<Option<T>>,
    ) -> Change {
        if now < self.next_poll {
            return Change::Unchanged;
        }
        self.next_poll = now.saturating_add(RETRY_INTERVAL);
        if let Some(live) = self.live.as_ref() {
            return match probe(live) {
                Ok(true) => Change::Unchanged,
                Ok(false) => {
                    self.live = None;
                    Change::Disconnected
                }
                Err(error) => {
                    self.live = None;
                    Change::Failed(error)
                }
            };
        }
        match open() {
            Ok(Some(live)) => {
                self.live = Some(live);
                Change::Connected
            }
            Ok(None) => Change::Unavailable,
            Err(error) => Change::Failed(error),
        }
    }

    /// A write failure closes the transport immediately and delays reopening.
    pub fn fail(&mut self, now: Duration) {
        self.live = None;
        self.next_poll = now.saturating_add(RETRY_INTERVAL);
    }
}
