//! Bounded single-producer/single-consumer note handoff at absolute audio frames.

use std::array;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU8, AtomicU64, AtomicUsize, Ordering},
};

use crate::compiler::{Machine, TimedNoteTarget};
use crate::timed::{MAX_EVENTS, MAX_FRAMES_PER_RENDER, TimedNoteState, TimedRenderError};

#[derive(Clone, Copy, Debug)]
struct Event {
    frame: u64,
    channel: u8,
    pitch_index: usize,
    gate_index: usize,
    pitch_hz: f64,
    gate: bool,
}

struct Slot {
    frame: AtomicU64,
    channel: AtomicU8,
    pitch_index: AtomicUsize,
    gate_index: AtomicUsize,
    pitch_bits: AtomicU64,
    gate: AtomicBool,
}

impl Slot {
    fn new() -> Self {
        Self {
            frame: AtomicU64::new(0),
            channel: AtomicU8::new(0),
            pitch_index: AtomicUsize::new(0),
            gate_index: AtomicUsize::new(0),
            pitch_bits: AtomicU64::new(0),
            gate: AtomicBool::new(false),
        }
    }

    fn write(&self, event: Event) {
        self.frame.store(event.frame, Ordering::Relaxed);
        self.channel.store(event.channel, Ordering::Relaxed);
        self.pitch_index.store(event.pitch_index, Ordering::Relaxed);
        self.gate_index.store(event.gate_index, Ordering::Relaxed);
        self.pitch_bits
            .store(event.pitch_hz.to_bits(), Ordering::Relaxed);
        self.gate.store(event.gate, Ordering::Relaxed);
    }

    fn read(&self) -> Event {
        Event {
            frame: self.frame.load(Ordering::Relaxed),
            channel: self.channel.load(Ordering::Relaxed),
            pitch_index: self.pitch_index.load(Ordering::Relaxed),
            gate_index: self.gate_index.load(Ordering::Relaxed),
            pitch_hz: f64::from_bits(self.pitch_bits.load(Ordering::Relaxed)),
            gate: self.gate.load(Ordering::Relaxed),
        }
    }
}

struct Queue {
    slots: [Slot; MAX_EVENTS],
    head: AtomicU64,
    tail: AtomicU64,
    cursor: AtomicU64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PushError {
    UnmappedChannel,
    InvalidPitch,
    Full,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PushOutcome {
    pub frame: u64,
    pub clamped: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RenderReport {
    pub applied: usize,
    pub late: usize,
}

/// The sole writer. Resolve and validate source events before audio processing.
pub struct LiveNoteProducer {
    queue: Arc<Queue>,
    targets: [Option<TimedNoteTarget>; 16],
    last_frame: u64,
}

/// The sole reader; owns the sequential DSP cursor and held note state.
pub struct LiveNoteConsumer {
    queue: Arc<Queue>,
    revision: Arc<()>,
    cursor: u64,
    active: [Option<Event>; 16],
}

pub fn live_note_queue(
    machine: &Machine,
    start_frame: u64,
) -> (LiveNoteProducer, LiveNoteConsumer) {
    let mut targets = [None; 16];
    for mapping in machine.midi_notes() {
        targets[usize::from(mapping.channel - 1)] = Some(
            machine
                .timed_note_target(mapping.channel)
                .expect("validated MIDI mapping"),
        );
    }
    let queue = Arc::new(Queue {
        slots: array::from_fn(|_| Slot::new()),
        head: AtomicU64::new(0),
        tail: AtomicU64::new(0),
        cursor: AtomicU64::new(start_frame),
    });
    (
        LiveNoteProducer {
            queue: Arc::clone(&queue),
            targets,
            last_frame: start_frame,
        },
        LiveNoteConsumer {
            queue,
            revision: machine.revision_token(),
            cursor: start_frame,
            active: [None; 16],
        },
    )
}

impl LiveNoteProducer {
    pub fn first_unrendered_frame(&self) -> u64 {
        self.queue.cursor.load(Ordering::Acquire)
    }

    /// Enqueue in nondecreasing frame order. Stale or reordered frames clamp forward.
    pub fn try_push(&mut self, state: TimedNoteState) -> Result<PushOutcome, PushError> {
        let target = self
            .targets
            .get(usize::from(state.channel).wrapping_sub(1))
            .and_then(|target| *target)
            .ok_or(PushError::UnmappedChannel)?;
        if !state.pitch_hz.is_finite()
            || state.pitch_hz < target.min_pitch
            || state.pitch_hz > target.max_pitch
        {
            return Err(PushError::InvalidPitch);
        }
        let tail = self.queue.tail.load(Ordering::Relaxed);
        let head = self.queue.head.load(Ordering::Acquire);
        if tail.wrapping_sub(head) >= MAX_EVENTS as u64 {
            return Err(PushError::Full);
        }
        let frame = state
            .frame
            .max(self.last_frame)
            .max(self.first_unrendered_frame());
        self.queue.slots[(tail as usize) % MAX_EVENTS].write(Event {
            frame,
            channel: state.channel,
            pitch_index: target.pitch_index,
            gate_index: target.gate_index,
            pitch_hz: state.pitch_hz,
            gate: state.gate,
        });
        self.queue
            .tail
            .store(tail.wrapping_add(1), Ordering::Release);
        self.last_frame = frame;
        Ok(PushOutcome {
            frame,
            clamped: frame != state.frame,
        })
    }
}

impl LiveNoteConsumer {
    pub fn cursor(&self) -> u64 {
        self.cursor
    }

    /// Applies events before each sample; events at the exclusive end wait.
    pub fn render_next(
        &mut self,
        machine: &mut Machine,
        buffer: &mut [f64],
    ) -> Result<RenderReport, TimedRenderError> {
        if !machine.is_revision(&self.revision) {
            return Err(TimedRenderError::WrongMachine);
        }
        if buffer.is_empty() || buffer.len() > MAX_FRAMES_PER_RENDER {
            return Err(TimedRenderError::InvalidBufferSize);
        }
        let end = self
            .cursor
            .checked_add(buffer.len() as u64)
            .ok_or(TimedRenderError::FrameOverflow)?;
        machine.prepare_buffer();
        for event in self.active.iter().flatten() {
            machine.apply_timed_note(
                event.pitch_index,
                event.gate_index,
                event.pitch_hz,
                event.gate,
            );
        }
        let mut rendered = 0;
        let mut report = RenderReport::default();
        loop {
            let head = self.queue.head.load(Ordering::Relaxed);
            if head == self.queue.tail.load(Ordering::Acquire) {
                break;
            }
            let event = self.queue.slots[(head as usize) % MAX_EVENTS].read();
            if event.frame >= end {
                break;
            }
            // A producer can publish while an earlier segment is rendering.
            // Never revisit samples already written in this block.
            let requested = event.frame.saturating_sub(self.cursor) as usize;
            let offset = requested.max(rendered);
            machine.process_prepared(&mut buffer[rendered..offset]);
            machine.apply_timed_note(
                event.pitch_index,
                event.gate_index,
                event.pitch_hz,
                event.gate,
            );
            self.active[usize::from(event.channel - 1)] = Some(event);
            report.applied += 1;
            report.late += usize::from(event.frame < self.cursor || offset > requested);
            rendered = offset;
            self.queue
                .head
                .store(head.wrapping_add(1), Ordering::Release);
        }
        machine.process_prepared(&mut buffer[rendered..]);
        self.cursor = end;
        self.queue.cursor.store(end, Ordering::Release);
        Ok(report)
    }
}
