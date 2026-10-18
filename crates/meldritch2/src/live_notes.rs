//! Bounded single-producer/single-consumer note handoff at absolute audio frames.

use std::array;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU8, AtomicU64, AtomicUsize, Ordering},
};

use crate::compiler::{Machine, PolyVoiceTarget, TimedNoteTarget};
use crate::timed::{MAX_EVENTS, MAX_FRAMES_PER_RENDER, TimedNoteState, TimedRenderError};

#[derive(Clone, Copy, Debug)]
struct Event {
    frame: u64,
    channel: u8,
    voice_slot: usize,
    pitch_index: usize,
    gate_index: usize,
    velocity_index: Option<usize>,
    pitch_hz: f64,
    gate: bool,
    velocity: f64,
    reset_epoch: u64,
}

#[derive(Clone, Copy)]
struct PendingEvent {
    sequence: u64,
    event: Event,
}

fn older_epoch(event: u64, seen: u64) -> bool {
    let distance = seen.wrapping_sub(event);
    distance != 0 && distance < (1_u64 << 63)
}

struct Slot {
    frame: AtomicU64,
    channel: AtomicU8,
    voice_slot: AtomicUsize,
    pitch_index: AtomicUsize,
    gate_index: AtomicUsize,
    velocity_index: AtomicUsize,
    pitch_bits: AtomicU64,
    gate: AtomicBool,
    velocity_bits: AtomicU64,
    reset_epoch: AtomicU64,
}

impl Slot {
    fn new() -> Self {
        Self {
            frame: AtomicU64::new(0),
            channel: AtomicU8::new(0),
            voice_slot: AtomicUsize::new(0),
            pitch_index: AtomicUsize::new(0),
            gate_index: AtomicUsize::new(0),
            velocity_index: AtomicUsize::new(usize::MAX),
            pitch_bits: AtomicU64::new(0),
            gate: AtomicBool::new(false),
            velocity_bits: AtomicU64::new(1.0_f64.to_bits()),
            reset_epoch: AtomicU64::new(0),
        }
    }

    fn write(&self, event: Event) {
        self.frame.store(event.frame, Ordering::Relaxed);
        self.channel.store(event.channel, Ordering::Relaxed);
        self.voice_slot.store(event.voice_slot, Ordering::Relaxed);
        self.pitch_index.store(event.pitch_index, Ordering::Relaxed);
        self.gate_index.store(event.gate_index, Ordering::Relaxed);
        self.velocity_index.store(
            event.velocity_index.unwrap_or(usize::MAX),
            Ordering::Relaxed,
        );
        self.pitch_bits
            .store(event.pitch_hz.to_bits(), Ordering::Relaxed);
        self.gate.store(event.gate, Ordering::Relaxed);
        self.velocity_bits
            .store(event.velocity.to_bits(), Ordering::Relaxed);
        self.reset_epoch.store(event.reset_epoch, Ordering::Relaxed);
    }

    fn read(&self) -> Event {
        Event {
            frame: self.frame.load(Ordering::Relaxed),
            channel: self.channel.load(Ordering::Relaxed),
            voice_slot: self.voice_slot.load(Ordering::Relaxed),
            pitch_index: self.pitch_index.load(Ordering::Relaxed),
            gate_index: self.gate_index.load(Ordering::Relaxed),
            velocity_index: match self.velocity_index.load(Ordering::Relaxed) {
                usize::MAX => None,
                index => Some(index),
            },
            pitch_hz: f64::from_bits(self.pitch_bits.load(Ordering::Relaxed)),
            gate: self.gate.load(Ordering::Relaxed),
            velocity: f64::from_bits(self.velocity_bits.load(Ordering::Relaxed)),
            reset_epoch: self.reset_epoch.load(Ordering::Relaxed),
        }
    }
}

struct Queue {
    slots: [Slot; MAX_EVENTS],
    head: AtomicU64,
    tail: AtomicU64,
    pending_count: AtomicUsize,
    cursor: AtomicU64,
    reset_epoch: [AtomicU64; 16],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PushError {
    UnmappedChannel,
    InvalidPitch,
    InvalidVelocity,
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
    pub discarded: usize,
}

/// The sole writer. Resolve and validate source events before audio processing.
pub struct LiveNoteProducer {
    queue: Arc<Queue>,
    targets: [Option<TimedNoteTarget>; 16],
    poly_targets: [Option<[PolyVoiceTarget; 2]>; 16],
    last_frame: [u64; 16],
}

/// The sole reader; owns the sequential DSP cursor and held note state.
pub struct LiveNoteConsumer {
    queue: Arc<Queue>,
    revision: Arc<()>,
    cursor: u64,
    active: [Option<Event>; 48],
    seen_reset: [u64; 16],
    pending: [Option<PendingEvent>; MAX_EVENTS],
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
    let mut poly_targets = [None; 16];
    for channel in machine.poly_note_channels() {
        poly_targets[usize::from(channel - 1)] = Some(
            machine
                .poly_note_targets(channel)
                .expect("validated polyphonic mapping"),
        );
    }
    let queue = Arc::new(Queue {
        slots: array::from_fn(|_| Slot::new()),
        head: AtomicU64::new(0),
        tail: AtomicU64::new(0),
        pending_count: AtomicUsize::new(0),
        cursor: AtomicU64::new(start_frame),
        reset_epoch: array::from_fn(|_| AtomicU64::new(0)),
    });
    (
        LiveNoteProducer {
            queue: Arc::clone(&queue),
            targets,
            poly_targets,
            last_frame: [start_frame; 16],
        },
        LiveNoteConsumer {
            queue,
            revision: machine.revision_token(),
            cursor: start_frame,
            active: [None; 48],
            seen_reset: [0; 16],
            pending: [None; MAX_EVENTS],
        },
    )
}

impl LiveNoteProducer {
    pub fn first_unrendered_frame(&self) -> u64 {
        self.queue.cursor.load(Ordering::Acquire)
    }

    /// Invalidate pending states and release the active Gate for one channel.
    /// This succeeds even if the ordinary queue is full. The same producer
    /// must own all writes to this queue.
    pub fn emergency_release(&mut self, channel: u8) -> Result<(), PushError> {
        let index = usize::from(channel).wrapping_sub(1);
        if self.targets.get(index).and_then(|target| *target).is_none()
            && self
                .poly_targets
                .get(index)
                .and_then(|target| *target)
                .is_none()
        {
            return Err(PushError::UnmappedChannel);
        }
        self.queue.reset_epoch[index].fetch_add(1, Ordering::Release);
        self.last_frame[index] = self.first_unrendered_frame();
        Ok(())
    }

    /// Enqueue with unity velocity. Stale or reordered frames clamp per channel.
    pub fn try_push(&mut self, state: TimedNoteState) -> Result<PushOutcome, PushError> {
        self.try_push_velocity(state, 1.0)
    }

    /// Velocity is a unit-interval note state; unwired voices ignore it in DSP.
    pub fn try_push_velocity(
        &mut self,
        state: TimedNoteState,
        velocity: f64,
    ) -> Result<PushOutcome, PushError> {
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
        if !velocity.is_finite() || !(0.0..=1.0).contains(&velocity) {
            return Err(PushError::InvalidVelocity);
        }
        self.push_resolved(
            state,
            velocity,
            target.pitch_index,
            target.gate_index,
            target.velocity_index,
            usize::from(state.channel - 1),
        )
    }

    pub(crate) fn try_push_poly(
        &mut self,
        state: TimedNoteState,
        velocity: f64,
        voice_index: usize,
    ) -> Result<PushOutcome, PushError> {
        let channel = usize::from(state.channel).wrapping_sub(1);
        let target = self
            .poly_targets
            .get(channel)
            .and_then(|targets| *targets)
            .and_then(|targets| targets.get(voice_index).copied())
            .ok_or(PushError::UnmappedChannel)?;
        if !state.pitch_hz.is_finite() || state.pitch_hz <= 0.0 {
            return Err(PushError::InvalidPitch);
        }
        if !velocity.is_finite() || !(0.0..=1.0).contains(&velocity) {
            return Err(PushError::InvalidVelocity);
        }
        self.push_resolved(
            state,
            velocity,
            target.pitch_index,
            target.gate_index,
            Some(target.velocity_index),
            16 + channel * 2 + voice_index,
        )
    }

    fn push_resolved(
        &mut self,
        state: TimedNoteState,
        velocity: f64,
        pitch_index: usize,
        gate_index: usize,
        velocity_index: Option<usize>,
        voice_slot: usize,
    ) -> Result<PushOutcome, PushError> {
        let tail = self.queue.tail.load(Ordering::Relaxed);
        let head = self.queue.head.load(Ordering::Acquire);
        if self.queue.pending_count.load(Ordering::Acquire) >= MAX_EVENTS
            || tail.wrapping_sub(head) >= MAX_EVENTS as u64
        {
            return Err(PushError::Full);
        }
        let channel = usize::from(state.channel - 1);
        let frame = state
            .frame
            .max(self.last_frame[channel])
            .max(self.first_unrendered_frame());
        // Count before publishing the slot, so the reader can always retire it.
        self.queue.pending_count.fetch_add(1, Ordering::AcqRel);
        self.queue.slots[(tail as usize) % MAX_EVENTS].write(Event {
            frame,
            channel: state.channel,
            voice_slot,
            pitch_index,
            gate_index,
            velocity_index,
            pitch_hz: state.pitch_hz,
            gate: state.gate,
            velocity,
            reset_epoch: self.queue.reset_epoch[channel].load(Ordering::Relaxed),
        });
        self.queue
            .tail
            .store(tail.wrapping_add(1), Ordering::Release);
        self.last_frame[channel] = frame;
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
        self.render_inner(machine, buffer, true)
    }

    /// Render after the enclosing device callback has prepared its one snapshot.
    pub(crate) fn render_prepared(
        &mut self,
        machine: &mut Machine,
        buffer: &mut [f64],
    ) -> Result<RenderReport, TimedRenderError> {
        self.render_inner(machine, buffer, false)
    }

    fn render_inner(
        &mut self,
        machine: &mut Machine,
        buffer: &mut [f64],
        prepare: bool,
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
        if prepare {
            machine.prepare_buffer();
        }
        for channel in 0..16 {
            let epoch = self.queue.reset_epoch[channel].load(Ordering::Acquire);
            if epoch != self.seen_reset[channel] {
                self.seen_reset[channel] = epoch;
                for event in self.active.iter_mut().flatten() {
                    if usize::from(event.channel - 1) == channel {
                        event.gate = false;
                    }
                }
            }
        }
        for event in self.active.iter().flatten() {
            machine.apply_timed_note(
                event.pitch_index,
                event.gate_index,
                event.pitch_hz,
                event.gate,
            );
            machine.apply_timed_velocity(event.velocity_index, event.velocity);
        }
        let mut rendered = 0;
        let mut report = RenderReport::default();
        // A reset may invalidate events already moved out of the ring.
        for entry in &mut self.pending {
            if let Some(pending) = entry {
                let channel = usize::from(pending.event.channel - 1);
                if older_epoch(pending.event.reset_epoch, self.seen_reset[channel]) {
                    *entry = None;
                    self.queue.pending_count.fetch_sub(1, Ordering::AcqRel);
                    report.discarded += 1;
                }
            }
        }
        // Snapshot the published tail. New arrivals wait for the next block;
        // the fixed pending array bounds both memory and per-block work.
        let tail = self.queue.tail.load(Ordering::Acquire);
        let mut head = self.queue.head.load(Ordering::Relaxed);
        while head != tail {
            let event = self.queue.slots[(head as usize) % MAX_EVENTS].read();
            let channel = usize::from(event.channel - 1);
            if older_epoch(event.reset_epoch, self.seen_reset[channel]) {
                self.queue.pending_count.fetch_sub(1, Ordering::AcqRel);
                report.discarded += 1;
            } else {
                let slot = self
                    .pending
                    .iter_mut()
                    .find(|entry| entry.is_none())
                    .expect("pending count bounds deferred events");
                *slot = Some(PendingEvent {
                    sequence: head,
                    event,
                });
            }
            head = head.wrapping_add(1);
            self.queue.head.store(head, Ordering::Release);
        }
        loop {
            let next = self
                .pending
                .iter()
                .enumerate()
                .filter_map(|(index, entry)| {
                    entry
                        .filter(|pending| {
                            pending.event.frame < end
                                && pending.event.reset_epoch
                                    == self.seen_reset[usize::from(pending.event.channel - 1)]
                        })
                        .map(|pending| (index, pending))
                })
                .min_by_key(|(_, pending)| (pending.event.frame, pending.sequence));
            let Some((index, pending)) = next else { break };
            let event = pending.event;
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
            machine.apply_timed_velocity(event.velocity_index, event.velocity);
            self.active[event.voice_slot] = Some(event);
            report.applied += 1;
            report.late += usize::from(event.frame < self.cursor || offset > requested);
            rendered = offset;
            self.pending[index] = None;
            self.queue.pending_count.fetch_sub(1, Ordering::AcqRel);
        }
        machine.process_prepared(&mut buffer[rendered..]);
        self.cursor = end;
        self.queue.cursor.store(end, Ordering::Release);
        Ok(report)
    }
}
