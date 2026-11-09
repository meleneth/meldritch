//! Bounded single-producer/single-consumer note handoff at absolute audio frames.

use std::array;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU8, AtomicU64, AtomicUsize, Ordering},
};

use meldritch_interface::{MidiEvent, MidiMessage, MidiProcessor, Tempo};

use crate::compiler::{MAX_POLY_VOICES, Machine, PolyVoiceTarget, TimedNoteTarget};
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
    arp_route: Option<usize>,
    key_lock_route: Option<usize>,
    key_lock_key: bool,
    midi_note: u8,
    midi_velocity: u8,
    midi_on: bool,
    midi_all_off: bool,
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
    arp_route: AtomicUsize,
    key_lock_route: AtomicUsize,
    key_lock_key: AtomicBool,
    midi_note: AtomicU8,
    midi_velocity: AtomicU8,
    midi_on: AtomicBool,
    midi_all_off: AtomicBool,
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
            arp_route: AtomicUsize::new(usize::MAX),
            key_lock_route: AtomicUsize::new(usize::MAX),
            key_lock_key: AtomicBool::new(false),
            midi_note: AtomicU8::new(0),
            midi_velocity: AtomicU8::new(0),
            midi_on: AtomicBool::new(false),
            midi_all_off: AtomicBool::new(false),
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
        self.arp_route
            .store(event.arp_route.unwrap_or(usize::MAX), Ordering::Relaxed);
        self.key_lock_route.store(
            event.key_lock_route.unwrap_or(usize::MAX),
            Ordering::Relaxed,
        );
        self.key_lock_key
            .store(event.key_lock_key, Ordering::Relaxed);
        self.midi_note.store(event.midi_note, Ordering::Relaxed);
        self.midi_velocity
            .store(event.midi_velocity, Ordering::Relaxed);
        self.midi_on.store(event.midi_on, Ordering::Relaxed);
        self.midi_all_off
            .store(event.midi_all_off, Ordering::Relaxed);
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
            arp_route: match self.arp_route.load(Ordering::Relaxed) {
                usize::MAX => None,
                route => Some(route),
            },
            key_lock_route: match self.key_lock_route.load(Ordering::Relaxed) {
                usize::MAX => None,
                route => Some(route),
            },
            key_lock_key: self.key_lock_key.load(Ordering::Relaxed),
            midi_note: self.midi_note.load(Ordering::Relaxed),
            midi_velocity: self.midi_velocity.load(Ordering::Relaxed),
            midi_on: self.midi_on.load(Ordering::Relaxed),
            midi_all_off: self.midi_all_off.load(Ordering::Relaxed),
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
    poly_targets: [Option<[Option<PolyVoiceTarget>; MAX_POLY_VOICES]>; 16],
    last_frame: [u64; 16],
    arp_inputs: [Option<usize>; 16],
    key_lock_inputs: [Option<(usize, bool)>; 16],
}

struct LiveArpRuntime {
    processor: meldritch_arp::Arpeggiator,
    input_channel: u8,
    output_channel: u8,
    tempo: Arc<crate::sequencer::SequencerControl>,
    target: LiveArpTarget,
}

struct LiveArpTarget {
    pitch_index: usize,
    gate_index: usize,
    velocity_index: Option<usize>,
    voice_slot: usize,
}

struct LiveKeyLockRuntime {
    processor: meldritch_midi_utils::KeyLock,
    notes_channel: u8,
    output_channel: u8,
}

/// The sole reader; owns the sequential DSP cursor and held note state.
pub struct LiveNoteConsumer {
    queue: Arc<Queue>,
    revision: Arc<()>,
    cursor: u64,
    active: [Option<Event>; 16 * (MAX_POLY_VOICES + 1)],
    seen_reset: [u64; 16],
    pending: [Option<PendingEvent>; MAX_EVENTS],
    arpeggiators: Vec<LiveArpRuntime>,
    key_locks: Vec<LiveKeyLockRuntime>,
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
    let mut arp_inputs = [None; 16];
    let arpeggiators = machine
        .live_arpeggiators()
        .iter()
        .enumerate()
        .map(|(index, route)| {
            arp_inputs[usize::from(route.input_channel - 1)] = Some(index);
            let target = if let Ok(target) = machine.timed_note_target(route.output_channel) {
                LiveArpTarget {
                    pitch_index: target.pitch_index,
                    gate_index: target.gate_index,
                    velocity_index: target.velocity_index,
                    voice_slot: usize::from(route.output_channel - 1),
                }
            } else {
                let target = machine
                    .poly_note_targets(route.output_channel)
                    .expect("validated live arpeggiator output")
                    .into_iter()
                    .flatten()
                    .next()
                    .expect("validated polyphonic MIDI target");
                LiveArpTarget {
                    pitch_index: target.pitch_index,
                    gate_index: target.gate_index,
                    velocity_index: Some(target.velocity_index),
                    voice_slot: 16 + usize::from(route.output_channel - 1) * MAX_POLY_VOICES,
                }
            };
            LiveArpRuntime {
                processor: meldritch_arp::Arpeggiator::new(route.config, route.output_channel)
                    .expect("validated live arpeggiator"),
                input_channel: route.input_channel,
                output_channel: route.output_channel,
                tempo: Arc::clone(&route.tempo),
                target,
            }
        })
        .collect();
    let mut key_lock_inputs = [None; 16];
    let key_locks = machine
        .live_key_locks()
        .iter()
        .enumerate()
        .map(|(index, route)| {
            key_lock_inputs[usize::from(route.key_channel - 1)] = Some((index, true));
            key_lock_inputs[usize::from(route.notes_channel - 1)] = Some((index, false));
            LiveKeyLockRuntime {
                processor: meldritch_midi_utils::KeyLock::new(route.config, route.output_channel)
                    .expect("validated live key lock"),
                notes_channel: route.notes_channel,
                output_channel: route.output_channel,
            }
        })
        .collect();
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
            arp_inputs,
            key_lock_inputs,
        },
        LiveNoteConsumer {
            queue,
            revision: machine.revision_token(),
            cursor: start_frame,
            active: [None; 16 * (MAX_POLY_VOICES + 1)],
            seen_reset: [0; 16],
            pending: [None; MAX_EVENTS],
            arpeggiators,
            key_locks,
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
            && self
                .arp_inputs
                .get(index)
                .and_then(|route| *route)
                .is_none()
            && self
                .key_lock_inputs
                .get(index)
                .and_then(|route| *route)
                .is_none()
        {
            return Err(PushError::UnmappedChannel);
        }
        self.queue.reset_epoch[index].fetch_add(1, Ordering::Release);
        self.last_frame[index] = self.first_unrendered_frame();
        Ok(())
    }

    pub(crate) fn has_arp_input(&self, channel: u8) -> bool {
        self.arp_inputs
            .get(usize::from(channel).wrapping_sub(1))
            .and_then(|route| *route)
            .is_some()
    }

    pub(crate) fn has_key_lock_input(&self, channel: u8) -> bool {
        self.key_lock_inputs
            .get(usize::from(channel).wrapping_sub(1))
            .and_then(|route| *route)
            .is_some()
    }

    pub(crate) fn has_processor_input(&self, channel: u8) -> bool {
        self.has_arp_input(channel) || self.has_key_lock_input(channel)
    }

    pub(crate) fn try_push_arp(&mut self, event: MidiEvent) -> Result<PushOutcome, PushError> {
        let channel = event.message.channel();
        let channel_index = usize::from(channel).wrapping_sub(1);
        let route = self
            .arp_inputs
            .get(channel_index)
            .and_then(|route| *route)
            .ok_or(PushError::UnmappedChannel)?;
        self.push_midi_processor(event, Some(route), None)
    }

    pub(crate) fn try_push_key_lock(&mut self, event: MidiEvent) -> Result<PushOutcome, PushError> {
        let channel = event.message.channel();
        let route = self
            .key_lock_inputs
            .get(usize::from(channel).wrapping_sub(1))
            .and_then(|route| *route)
            .ok_or(PushError::UnmappedChannel)?;
        self.push_midi_processor(event, None, Some(route))
    }

    fn push_midi_processor(
        &mut self,
        event: MidiEvent,
        arp_route: Option<usize>,
        key_lock_route: Option<(usize, bool)>,
    ) -> Result<PushOutcome, PushError> {
        let channel = event.message.channel();
        let channel_index = usize::from(channel).wrapping_sub(1);
        let tail = self.queue.tail.load(Ordering::Relaxed);
        let head = self.queue.head.load(Ordering::Acquire);
        if self.queue.pending_count.load(Ordering::Acquire) >= MAX_EVENTS
            || tail.wrapping_sub(head) >= MAX_EVENTS as u64
        {
            return Err(PushError::Full);
        }
        let frame = event
            .frame
            .max(self.last_frame[channel_index])
            .max(self.first_unrendered_frame());
        let (note, velocity, on, all_off) = match event.message {
            MidiMessage::NoteOn { note, velocity, .. } => (note, velocity, velocity > 0, false),
            MidiMessage::NoteOff { note, velocity, .. } => (note, velocity, false, false),
            MidiMessage::AllNotesOff { .. } => (0, 0, false, true),
        };
        self.queue.pending_count.fetch_add(1, Ordering::AcqRel);
        self.queue.slots[(tail as usize) % MAX_EVENTS].write(Event {
            frame,
            channel,
            voice_slot: 0,
            pitch_index: 0,
            gate_index: 0,
            velocity_index: None,
            pitch_hz: 0.0,
            gate: false,
            velocity: 0.0,
            reset_epoch: self.queue.reset_epoch[channel_index].load(Ordering::Relaxed),
            arp_route,
            key_lock_route: key_lock_route.map(|(route, _)| route),
            key_lock_key: key_lock_route.is_some_and(|(_, key)| key),
            midi_note: note,
            midi_velocity: velocity,
            midi_on: on,
            midi_all_off: all_off,
        });
        self.queue
            .tail
            .store(tail.wrapping_add(1), Ordering::Release);
        self.last_frame[channel_index] = frame;
        Ok(PushOutcome {
            frame,
            clamped: frame != event.frame,
        })
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
            .and_then(|targets| targets.get(voice_index).copied().flatten())
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
            16 + channel * MAX_POLY_VOICES + voice_index,
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
            arp_route: None,
            key_lock_route: None,
            key_lock_key: false,
            midi_note: 0,
            midi_velocity: 0,
            midi_on: false,
            midi_all_off: false,
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

    fn tempo(&self, route: usize, sample_rate: f64) -> Tempo {
        let route = &self.arpeggiators[route];
        Tempo {
            bpm: route.tempo.bpm(),
            sample_rate: sample_rate.round().clamp(1.0, u32::MAX as f64) as u32,
            playing: route.tempo.playing(),
        }
    }

    fn apply_arp_output(&mut self, machine: &mut Machine, route: usize, event: MidiEvent) {
        let route = &self.arpeggiators[route];
        let (note, velocity, gate) = match event.message {
            MidiMessage::NoteOn { note, velocity, .. } if velocity > 0 => {
                (note, f64::from(velocity) / 127.0, true)
            }
            MidiMessage::NoteOn { note, .. } | MidiMessage::NoteOff { note, .. } => {
                (note, 0.0, false)
            }
            MidiMessage::AllNotesOff { .. } => {
                let slot = route.target.voice_slot;
                if let Some(active) = self.active[slot] {
                    machine.apply_timed_note(
                        active.pitch_index,
                        active.gate_index,
                        active.pitch_hz,
                        false,
                    );
                    self.active[slot] = Some(Event {
                        gate: false,
                        ..active
                    });
                }
                return;
            }
        };
        let pitch_hz = 440.0 * 2.0_f64.powf((f64::from(note) - 69.0) / 12.0);
        let resolved = Event {
            frame: event.frame,
            channel: route.output_channel,
            voice_slot: route.target.voice_slot,
            pitch_index: route.target.pitch_index,
            gate_index: route.target.gate_index,
            velocity_index: route.target.velocity_index,
            pitch_hz,
            gate,
            velocity,
            reset_epoch: 0,
            arp_route: None,
            key_lock_route: None,
            key_lock_key: false,
            midi_note: 0,
            midi_velocity: 0,
            midi_on: false,
            midi_all_off: false,
        };
        machine.apply_timed_note(
            resolved.pitch_index,
            resolved.gate_index,
            resolved.pitch_hz,
            resolved.gate,
        );
        machine.apply_timed_velocity(resolved.velocity_index, resolved.velocity);
        self.active[resolved.voice_slot] = Some(resolved);
    }

    fn advance_arp(&mut self, machine: &mut Machine, route: usize, end: u64, tempo: Tempo) {
        let mut generated = [None; 4];
        let mut count = 0;
        self.arpeggiators[route]
            .processor
            .advance(end, tempo, &mut |event| {
                if count < generated.len() {
                    generated[count] = Some(event);
                    count += 1;
                }
            });
        for event in generated.into_iter().flatten() {
            self.apply_arp_output(machine, route, event);
        }
    }

    fn input_arp(&mut self, machine: &mut Machine, event: Event) {
        let route = event.arp_route.expect("arpeggiator input event");
        let message = if event.midi_all_off {
            MidiMessage::AllNotesOff {
                channel: event.channel,
            }
        } else if event.midi_on {
            MidiMessage::NoteOn {
                channel: event.channel,
                note: event.midi_note,
                velocity: event.midi_velocity,
            }
        } else {
            MidiMessage::NoteOff {
                channel: event.channel,
                note: event.midi_note,
                velocity: event.midi_velocity,
            }
        };
        self.input_arp_message(
            machine,
            route,
            MidiEvent {
                frame: event.frame,
                message,
            },
        );
    }

    fn input_arp_message(&mut self, machine: &mut Machine, route: usize, event: MidiEvent) {
        let mut generated = [None; 2];
        let mut count = 0;
        self.arpeggiators[route]
            .processor
            .input(event, &mut |event| {
                if count < generated.len() {
                    generated[count] = Some(event);
                    count += 1;
                }
            });
        for event in generated.into_iter().flatten() {
            self.apply_arp_output(machine, route, event);
        }
    }

    fn input_key_lock(&mut self, machine: &mut Machine, event: Event) {
        let route = event.key_lock_route.expect("key-lock input event");
        let message = if event.midi_all_off {
            MidiMessage::AllNotesOff {
                channel: event.channel,
            }
        } else if event.midi_on {
            MidiMessage::NoteOn {
                channel: event.channel,
                note: event.midi_note,
                velocity: event.midi_velocity,
            }
        } else {
            MidiMessage::NoteOff {
                channel: event.channel,
                note: event.midi_note,
                velocity: event.midi_velocity,
            }
        };
        let input = MidiEvent {
            frame: event.frame,
            message,
        };
        // A key change can turn every retained note off and back on. Keep the
        // collection bounded and allocation-free on the audio thread.
        let mut generated = [None; 256];
        let mut count = 0;
        let processor = &mut self.key_locks[route].processor;
        let mut output = |event| {
            if count < generated.len() {
                generated[count] = Some(event);
                count += 1;
            }
        };
        if event.key_lock_key {
            processor.key_input(input, &mut output);
        } else {
            processor.note_input(input, &mut output);
        }
        let output_channel = self.key_locks[route].output_channel;
        let arp_route = self
            .arpeggiators
            .iter()
            .position(|arp| arp.input_channel == output_channel)
            .expect("validated key-lock output route");
        for event in generated.into_iter().flatten() {
            self.input_arp_message(machine, arp_route, event);
        }
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
                for route in 0..self.arpeggiators.len() {
                    if usize::from(self.arpeggiators[route].input_channel - 1) == channel {
                        self.input_arp(
                            machine,
                            Event {
                                frame: self.cursor,
                                channel: channel as u8 + 1,
                                voice_slot: 0,
                                pitch_index: 0,
                                gate_index: 0,
                                velocity_index: None,
                                pitch_hz: 0.0,
                                gate: false,
                                velocity: 0.0,
                                reset_epoch: epoch,
                                arp_route: Some(route),
                                key_lock_route: None,
                                key_lock_key: false,
                                midi_note: 0,
                                midi_velocity: 0,
                                midi_on: false,
                                midi_all_off: true,
                            },
                        );
                    }
                }
                for route in 0..self.key_locks.len() {
                    if usize::from(self.key_locks[route].notes_channel - 1) == channel {
                        self.input_key_lock(
                            machine,
                            Event {
                                frame: self.cursor,
                                channel: channel as u8 + 1,
                                voice_slot: 0,
                                pitch_index: 0,
                                gate_index: 0,
                                velocity_index: None,
                                pitch_hz: 0.0,
                                gate: false,
                                velocity: 0.0,
                                reset_epoch: epoch,
                                arp_route: None,
                                key_lock_route: Some(route),
                                key_lock_key: false,
                                midi_note: 0,
                                midi_velocity: 0,
                                midi_on: false,
                                midi_all_off: true,
                            },
                        );
                    }
                }
            }
        }
        for route in 0..self.arpeggiators.len() {
            let tempo = self.tempo(route, machine.sample_rate());
            self.advance_arp(machine, route, self.cursor, tempo);
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
            let pending = self
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
            let scheduled = self
                .arpeggiators
                .iter()
                .enumerate()
                .filter_map(|(route, arp)| {
                    let tempo = Tempo {
                        bpm: arp.tempo.bpm(),
                        sample_rate: machine.sample_rate().round() as u32,
                        playing: arp.tempo.playing(),
                    };
                    arp.processor
                        .next_frame(tempo)
                        .filter(|frame| *frame < end)
                        .map(|frame| (route, frame, tempo))
                })
                .min_by_key(|(route, frame, _)| (*frame, *route));
            let pending_first = match (pending, scheduled) {
                (Some((_, pending)), Some((_, frame, _))) => pending.event.frame <= frame,
                (Some(_), None) => true,
                (None, Some(_)) => false,
                (None, None) => break,
            };
            let (frame, pending_index, scheduled_route) = if pending_first {
                let (index, pending) = pending.expect("selected pending event");
                (pending.event.frame, Some(index), None)
            } else {
                let (route, frame, tempo) = scheduled.expect("selected scheduled event");
                (frame, None, Some((route, tempo)))
            };
            let requested = frame.saturating_sub(self.cursor) as usize;
            let offset = requested.max(rendered);
            machine.process_prepared(&mut buffer[rendered..offset]);
            if let Some(index) = pending_index {
                let event = self.pending[index].expect("selected pending event").event;
                if event.key_lock_route.is_some() {
                    self.input_key_lock(machine, event);
                } else if event.arp_route.is_some() {
                    self.input_arp(machine, event);
                } else {
                    machine.apply_timed_note(
                        event.pitch_index,
                        event.gate_index,
                        event.pitch_hz,
                        event.gate,
                    );
                    machine.apply_timed_velocity(event.velocity_index, event.velocity);
                    self.active[event.voice_slot] = Some(event);
                }
                self.pending[index] = None;
                self.queue.pending_count.fetch_sub(1, Ordering::AcqRel);
            } else if let Some((route, tempo)) = scheduled_route {
                self.advance_arp(machine, route, frame.saturating_add(1), tempo);
            }
            report.applied += 1;
            report.late += usize::from(frame < self.cursor || offset > requested);
            rendered = offset;
        }
        machine.process_prepared(&mut buffer[rendered..]);
        self.cursor = end;
        self.queue.cursor.store(end, Ordering::Release);
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::live_note_queue;
    use crate::{compiler::compile, lua, midi::TimedMidiRouter};

    const LIVE_ARP: &str = r#"
local instrument=meldritch.instrument("Live arp test")
local sequence=instrument:sequencer {bpm=120,steps=16,steps_per_beat=4}
local input=instrument:midi_input {channel=1}
local arp=instrument:arpeggiator {notes=input.notes,tempo=sequence,output_channel=2,
  mode="up",octaves=1,rate=4,gate=.5}
instrument:note_source(arp.notes)
local synth=instrument:plaits {model="virtual_analog"}
instrument:connect(instrument.note.pitch,synth.pitch)
instrument:connect(instrument.note.gate,synth.gate)
instrument:connect(instrument.note.velocity,synth.velocity)
instrument:connect(synth.main,instrument.output.left)
instrument:connect(synth.main,instrument.output.right)
return instrument
"#;

    const KEY_LOCKED_LIVE_ARP: &str = r#"
local instrument=meldritch.instrument("Key-locked live arp test")
local sequence=instrument:sequencer {bpm=120,steps=16,steps_per_beat=4}
local notes=instrument:midi_input {channel=1}
local key=instrument:midi_input {channel=2}
local locked=instrument:key_lock {key=key.notes,notes=notes.notes,output_channel=3,
  scale="major",wrong_notes="nearest",tie_break="down",initial_key=60}
local arp=instrument:arpeggiator {notes=locked.notes,tempo=sequence,output_channel=4,
  mode="up",octaves=1,rate=4,gate=.5}
instrument:note_source(arp.notes)
local synth=instrument:plaits {model="virtual_analog"}
instrument:connect(instrument.note.pitch,synth.pitch)
instrument:connect(instrument.note.gate,synth.gate)
instrument:connect(instrument.note.velocity,synth.velocity)
instrument:connect(synth.main,instrument.output.left)
instrument:connect(synth.main,instrument.output.right)
return instrument
"#;

    #[test]
    fn lua_wired_live_arp_transforms_held_midi_before_note_resolution() {
        let graph = lua::read(LIVE_ARP).unwrap();
        assert_eq!(graph.live_arpeggiators.len(), 1);
        assert_eq!(graph.live_arpeggiators[0].input_channel, 1);
        assert_eq!(graph.live_arpeggiators[0].output_channel, 2);
        let mut machine = compile(&graph).unwrap();
        let pitch = machine.control_handle("__voice_1_pitch").unwrap();
        let gate = machine.control_handle("__voice_1_gate").unwrap();
        let (producer, mut consumer) = live_note_queue(&machine, 0);
        let mut midi = TimedMidiRouter::new(&machine, producer, 48_000, 3).unwrap();
        midi.dispatch(1_000_000, &[0x90, 60, 100]).unwrap();
        midi.dispatch(1_000_000, &[0x90, 64, 90]).unwrap();
        let mut pcm = [0.0; 1024];
        for _ in 0..7 {
            consumer.render_next(&mut machine, &mut pcm).unwrap();
        }
        let actual_pitch = machine.runtime_value(pitch.target()).unwrap();
        assert!(
            (actual_pitch - 329.627_556).abs() < 0.01,
            "expected E4 after the second tick, got {actual_pitch}"
        );
        assert!(machine.runtime_value(gate.target()).unwrap() > 0.5);
        assert!(pcm.iter().any(|sample| sample.abs() > 1e-6));
        midi.dispatch(1_150_000, &[0x80, 60, 0]).unwrap();
        midi.dispatch(1_150_000, &[0x80, 64, 0]).unwrap();
        consumer.render_next(&mut machine, &mut pcm).unwrap();
        assert!(machine.runtime_value(gate.target()).unwrap() < 0.5);
    }

    #[test]
    fn lua_key_lock_routes_two_midi_inputs_into_live_arp() {
        let graph = lua::read(KEY_LOCKED_LIVE_ARP).unwrap();
        assert_eq!(graph.live_key_locks.len(), 1);
        assert_eq!(graph.live_key_locks[0].key_channel, 2);
        assert_eq!(graph.live_key_locks[0].notes_channel, 1);
        assert_eq!(graph.live_key_locks[0].output_channel, 3);
        let mut machine = compile(&graph).unwrap();
        let pitch = machine.control_handle("__voice_1_pitch").unwrap();
        let (producer, mut consumer) = live_note_queue(&machine, 0);
        let mut midi = TimedMidiRouter::new(&machine, producer, 48_000, 3).unwrap();

        // D selects D major. C4 is outside that scale and an equidistant tie;
        // the configured downward convergence maps it to B3.
        midi.dispatch(1_000_000, &[0x91, 62, 127]).unwrap();
        midi.dispatch(1_000_000, &[0x90, 60, 100]).unwrap();
        let mut pcm = [0.0; 1024];
        consumer.render_next(&mut machine, &mut pcm).unwrap();
        let actual_pitch = machine.runtime_value(pitch.target()).unwrap();
        assert!(
            (actual_pitch - 246.941_651).abs() < 0.01,
            "expected B3 from D-major key lock, got {actual_pitch}"
        );
    }
}
