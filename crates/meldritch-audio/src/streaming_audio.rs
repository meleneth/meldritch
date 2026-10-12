//! Bounded, single-producer/single-consumer timed mono PCM lane.
//! DSP stays on the worker. Published sample slots contain no heap owners.
use std::{
    cell::Cell,
    marker::PhantomData,
    sync::{
        Arc,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
};

struct Slot {
    frame: AtomicU64,
    epoch: AtomicU64,
    sample: AtomicU64,
}
struct Ring {
    slots: Box<[Slot]>,
    read: AtomicUsize,
    write: AtomicUsize,
    clock: AtomicU64,
    epoch: AtomicU64,
    active_epoch: AtomicU64,
    underruns: AtomicU64,
}
pub struct StreamingProducer {
    ring: Arc<Ring>,
    last: Option<(u64, u64)>,
    _single: PhantomData<Cell<()>>,
}
pub struct StreamingConsumer {
    ring: Arc<Ring>,
    budget: usize,
    _single: PhantomData<Cell<()>>,
}
#[derive(Clone)]
pub struct StreamingControl {
    ring: Arc<Ring>,
}

pub fn streaming_audio(
    capacity: usize,
    origin: u64,
) -> Result<(StreamingProducer, StreamingConsumer, StreamingControl), String> {
    if !(1..=4096).contains(&capacity) {
        return Err("stream capacity must be 1..4096 frames".into());
    }
    let ring = Arc::new(Ring {
        slots: (0..=capacity)
            .map(|_| Slot {
                frame: AtomicU64::new(0),
                epoch: AtomicU64::new(0),
                sample: AtomicU64::new(0),
            })
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        read: AtomicUsize::new(0),
        write: AtomicUsize::new(0),
        clock: AtomicU64::new(origin),
        epoch: AtomicU64::new(0),
        active_epoch: AtomicU64::new(0),
        underruns: AtomicU64::new(0),
    });
    Ok((
        StreamingProducer {
            ring: ring.clone(),
            last: None,
            _single: PhantomData,
        },
        StreamingConsumer {
            ring: ring.clone(),
            budget: 0,
            _single: PhantomData,
        },
        StreamingControl { ring },
    ))
}
impl StreamingControl {
    pub fn clock(&self) -> u64 {
        self.ring.clock.load(Ordering::Acquire)
    }
    pub fn epoch(&self) -> u64 {
        self.ring.epoch.load(Ordering::Acquire)
    }
    pub fn enabled(&self) -> bool {
        self.ring.active_epoch.load(Ordering::Acquire) != 0
    }
    pub fn underruns(&self) -> u64 {
        self.ring.underruns.load(Ordering::Relaxed)
    }
    /// Invalidate buffered PCM before a new worker instance can become audible.
    /// One control owner activates epochs; worker/callback may only deactivate.
    pub fn activate(&self) -> Result<u64, String> {
        let next = self
            .epoch()
            .checked_add(1)
            .ok_or("stream epoch exhausted")?;
        self.deactivate();
        self.ring.epoch.store(next, Ordering::Release);
        self.ring.active_epoch.store(next, Ordering::Release);
        Ok(next)
    }
    pub fn deactivate(&self) {
        self.ring.active_epoch.store(0, Ordering::Release);
    }
}
impl StreamingProducer {
    pub fn available(&self) -> usize {
        let read = self.ring.read.load(Ordering::Acquire);
        let write = self.ring.write.load(Ordering::Relaxed);
        (read + self.ring.slots.len() - write - 1) % self.ring.slots.len()
    }
    /// A complete finite batch is validated before any slot is published.
    pub fn publish(&mut self, frame: u64, epoch: u64, samples: &[f64]) -> Result<(), String> {
        let end = frame
            .checked_add(samples.len() as u64)
            .ok_or("stream frame overflow")?;
        if samples.is_empty() || samples.iter().any(|s| !s.is_finite()) {
            return Err("stream batch must contain finite samples".into());
        }
        if self
            .last
            .is_some_and(|(previous_epoch, end)| previous_epoch == epoch && frame < end)
        {
            return Err("stream batch moves backwards".into());
        }
        if samples.len() > self.available() {
            return Err("stream ring is full".into());
        }
        let mut write = self.ring.write.load(Ordering::Relaxed);
        for (offset, sample) in samples.iter().enumerate() {
            let slot = &self.ring.slots[write];
            slot.frame.store(frame + offset as u64, Ordering::Relaxed);
            slot.epoch.store(epoch, Ordering::Relaxed);
            slot.sample.store(sample.to_bits(), Ordering::Relaxed);
            write = (write + 1) % self.ring.slots.len();
        }
        // Release publishes all initialized slots. Acquire read above proves
        // consumer released them. Atomic payloads also avoid unsafe storage.
        self.ring.write.store(write, Ordering::Release);
        self.last = Some((epoch, end));
        Ok(())
    }
}
impl StreamingConsumer {
    pub fn begin_callback(&mut self) {
        self.budget = self.ring.slots.len() - 1;
    }
    pub fn deactivate(&self) {
        self.ring.active_epoch.store(0, Ordering::Release);
    }
    /// Clock advances through stopped transport and silence. Stale/late samples
    /// never replay later. Total slot examinations are bounded per callback.
    pub fn next_sample(&mut self) -> f64 {
        let clock = self.ring.clock.load(Ordering::Relaxed);
        self.ring
            .clock
            .store(clock.saturating_add(1), Ordering::Release);
        let epoch = self.ring.active_epoch.load(Ordering::Acquire);
        let enabled = epoch != 0;
        while self.budget != 0 {
            let read = self.ring.read.load(Ordering::Relaxed);
            if read == self.ring.write.load(Ordering::Acquire) {
                break;
            }
            let slot = &self.ring.slots[read];
            let frame = slot.frame.load(Ordering::Relaxed);
            let current = enabled && slot.epoch.load(Ordering::Relaxed) == epoch;
            if current && frame > clock {
                break;
            }
            let sample = f64::from_bits(slot.sample.load(Ordering::Relaxed));
            self.ring
                .read
                .store((read + 1) % self.ring.slots.len(), Ordering::Release);
            self.budget -= 1;
            if current && frame == clock {
                return sample;
            }
        }
        if enabled {
            self.ring.underruns.fetch_add(1, Ordering::Relaxed);
        }
        0.0
    }
}
