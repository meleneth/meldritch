//! Bounded SPSC command queue for the realtime audio boundary.

use std::cell::Cell;
use std::cell::UnsafeCell;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransportCommand {
    Play,
    Stop,
    Rewind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QueueFull(pub TransportCommand);

struct CommandRing {
    slots: Box<[UnsafeCell<MaybeUninit<TransportCommand>>]>,
    read: AtomicUsize,
    write: AtomicUsize,
    applied: AtomicU64,
    dropped: AtomicU64,
}

// Each slot has exactly one producer and one consumer. The producer publishes
// a completed write with `write` Release and the consumer observes it with
// Acquire before reading. The consumer similarly publishes freed slots through
// `read`, so the producer never overwrites a slot still being read.
unsafe impl Sync for CommandRing {}

pub struct TransportCommandProducer {
    ring: Arc<CommandRing>,
    not_sync: PhantomData<Cell<()>>,
}

pub struct TransportCommandConsumer {
    ring: Arc<CommandRing>,
}

#[derive(Clone)]
pub struct QueueMonitor {
    ring: Arc<CommandRing>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QueueDiagnostics {
    pub applied: u64,
    pub dropped: u64,
}

/// Create a bounded command queue. Capacity must be at least one.
pub fn transport_command_queue(
    capacity: usize,
) -> Result<(TransportCommandProducer, TransportCommandConsumer), &'static str> {
    if capacity == 0 {
        return Err("transport command queue capacity must be at least one");
    }

    // One slot remains empty so equal indices unambiguously mean empty.
    let slots = (0..=capacity)
        .map(|_| UnsafeCell::new(MaybeUninit::uninit()))
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let ring = Arc::new(CommandRing {
        slots,
        read: AtomicUsize::new(0),
        write: AtomicUsize::new(0),
        applied: AtomicU64::new(0),
        dropped: AtomicU64::new(0),
    });
    Ok((
        TransportCommandProducer {
            ring: Arc::clone(&ring),
            not_sync: PhantomData,
        },
        TransportCommandConsumer { ring },
    ))
}

impl TransportCommandProducer {
    pub fn try_push(&self, command: TransportCommand) -> Result<(), QueueFull> {
        let write = self.ring.write.load(Ordering::Relaxed);
        let next = increment(write, self.ring.slots.len());
        if next == self.ring.read.load(Ordering::Acquire) {
            self.ring.dropped.fetch_add(1, Ordering::Relaxed);
            return Err(QueueFull(command));
        }

        // SAFETY: This is the sole producer. The acquire read above proves the
        // consumer has released this slot before it is reused.
        unsafe {
            (*self.ring.slots[write].get()).write(command);
        }
        self.ring.write.store(next, Ordering::Release);
        Ok(())
    }

    #[must_use]
    pub fn diagnostics(&self) -> QueueDiagnostics {
        diagnostics(&self.ring)
    }

    #[must_use]
    pub fn monitor(&self) -> QueueMonitor {
        QueueMonitor {
            ring: Arc::clone(&self.ring),
        }
    }
}

impl TransportCommandConsumer {
    pub fn try_pop(&mut self) -> Option<TransportCommand> {
        let read = self.ring.read.load(Ordering::Relaxed);
        if read == self.ring.write.load(Ordering::Acquire) {
            return None;
        }

        // SAFETY: This is the sole consumer. The acquire write above proves
        // the producer finished initializing this slot before it is read.
        let command = unsafe { (*self.ring.slots[read].get()).assume_init_read() };
        self.ring
            .read
            .store(increment(read, self.ring.slots.len()), Ordering::Release);
        self.ring.applied.fetch_add(1, Ordering::Relaxed);
        Some(command)
    }

    #[must_use]
    pub fn diagnostics(&self) -> QueueDiagnostics {
        diagnostics(&self.ring)
    }
}

impl QueueMonitor {
    #[must_use]
    pub fn diagnostics(&self) -> QueueDiagnostics {
        diagnostics(&self.ring)
    }
}

fn increment(index: usize, length: usize) -> usize {
    let next = index + 1;
    if next == length { 0 } else { next }
}

fn diagnostics(ring: &CommandRing) -> QueueDiagnostics {
    QueueDiagnostics {
        applied: ring.applied.load(Ordering::Relaxed),
        dropped: ring.dropped.load(Ordering::Relaxed),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_preserves_command_order() {
        let (producer, mut consumer) = transport_command_queue(3).unwrap();
        producer.try_push(TransportCommand::Play).unwrap();
        producer.try_push(TransportCommand::Stop).unwrap();
        producer.try_push(TransportCommand::Rewind).unwrap();

        assert_eq!(consumer.try_pop(), Some(TransportCommand::Play));
        assert_eq!(consumer.try_pop(), Some(TransportCommand::Stop));
        assert_eq!(consumer.try_pop(), Some(TransportCommand::Rewind));
        assert_eq!(consumer.try_pop(), None);
        assert_eq!(
            consumer.diagnostics(),
            QueueDiagnostics {
                applied: 3,
                dropped: 0
            }
        );
    }

    #[test]
    fn full_queue_drops_without_blocking_and_can_be_reused() {
        let (producer, mut consumer) = transport_command_queue(1).unwrap();
        producer.try_push(TransportCommand::Play).unwrap();
        assert_eq!(
            producer.try_push(TransportCommand::Stop),
            Err(QueueFull(TransportCommand::Stop))
        );
        assert_eq!(consumer.try_pop(), Some(TransportCommand::Play));
        producer.try_push(TransportCommand::Rewind).unwrap();
        assert_eq!(consumer.try_pop(), Some(TransportCommand::Rewind));
        assert_eq!(
            producer.diagnostics(),
            QueueDiagnostics {
                applied: 2,
                dropped: 1
            }
        );
    }

    #[test]
    fn queue_rejects_zero_capacity() {
        assert!(transport_command_queue(0).is_err());
    }
}
