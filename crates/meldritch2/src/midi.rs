use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};

use crate::compiler::{ControlHandle, MAX_POLY_VOICES, Machine};
use crate::model::{Action, MidiButtonEdge, MidiButtonMessage, PolyOverflow};
use crate::{
    live_notes::{LiveNoteProducer, PushError},
    midi_clock::{ClockError, MidiFrameClock},
    timed::TimedNoteState,
};

#[derive(Clone)]
struct NoteRoute {
    pitch: ControlHandle,
    gate: ControlHandle,
    velocity: Option<ControlHandle>,
    state: Arc<NoteState>,
    frequencies: [f64; 128],
}

#[derive(Clone, Copy, Debug)]
struct HeldNotes {
    keys: [u8; 128],
    velocities: [u8; 128],
    len: usize,
}

struct TimedNoteRoute {
    held: HeldNotes,
    frequencies: [f64; 128],
    velocity_mapped: bool,
}

#[derive(Clone, Copy, Debug)]
struct PolyHeld {
    key: u8,
    velocity: u8,
}

struct TimedPolyRoute {
    held: [Option<PolyHeld>; MAX_POLY_VOICES],
    ages: [u64; MAX_POLY_VOICES],
    next_age: u64,
    overflow: PolyOverflow,
    voice_count: usize,
    frequencies: [f64; 128],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimedMidiError {
    Clock(ClockError),
    Queue(PushError),
    NoFreeVoice,
}

/// MIDI callback adapter: CCs publish controls, note changes enter the frame queue.
pub struct TimedMidiRouter {
    controls: MidiRouter,
    notes: [Option<TimedNoteRoute>; 16],
    poly: [Option<TimedPolyRoute>; 16],
    producer: LiveNoteProducer,
    clock: MidiFrameClock,
}

impl TimedMidiRouter {
    pub(crate) fn new(
        machine: &Machine,
        producer: LiveNoteProducer,
        sample_rate: u32,
        lead_frames: u32,
    ) -> Result<Self, ClockError> {
        let mut notes = std::array::from_fn(|_| None);
        for mapping in machine.midi_notes() {
            notes[usize::from(mapping.channel - 1)] = Some(TimedNoteRoute {
                held: HeldNotes::new(),
                frequencies: std::array::from_fn(|note| {
                    440.0 * 2.0_f64.powf((note as f64 - 69.0) / 12.0)
                }),
                velocity_mapped: mapping.velocity_control_id.is_some(),
            });
        }
        let mut poly = std::array::from_fn(|_| None);
        for channel in machine.poly_note_channels() {
            let voice_count = machine
                .poly_note_targets(channel)
                .expect("validated polyphonic mapping")
                .iter()
                .flatten()
                .count();
            poly[usize::from(channel - 1)] = Some(TimedPolyRoute {
                held: [None; MAX_POLY_VOICES],
                ages: [0; MAX_POLY_VOICES],
                next_age: 0,
                overflow: machine
                    .poly_note_overflow(channel)
                    .expect("validated polyphonic mapping"),
                voice_count,
                frequencies: std::array::from_fn(|note| {
                    440.0 * 2.0_f64.powf((note as f64 - 69.0) / 12.0)
                }),
            });
        }
        Ok(Self {
            controls: MidiRouter::from_machine(machine),
            notes,
            poly,
            producer,
            clock: MidiFrameClock::new(sample_rate, lead_frames)?,
        })
    }

    pub fn first_unrendered_frame(&self) -> u64 {
        self.producer.first_unrendered_frame()
    }

    pub(crate) fn first_mapped_channel(&self) -> Option<u8> {
        (0..16)
            .find(|&index| self.notes[index].is_some() || self.poly[index].is_some())
            .map(|index| index as u8 + 1)
    }

    fn release_channel(&mut self, channel: usize) -> bool {
        if self.notes[channel].is_none() && self.poly[channel].is_none() {
            return false;
        }
        self.producer
            .emergency_release(channel as u8 + 1)
            .expect("mapped timed note channel");
        if let Some(route) = &mut self.notes[channel] {
            route.held = HeldNotes::new();
        }
        if let Some(route) = &mut self.poly[channel] {
            route.held = [None; MAX_POLY_VOICES];
            route.ages = [0; MAX_POLY_VOICES];
            route.next_age = 0;
        }
        true
    }

    /// Used after the MIDI connection has stopped its callback.
    pub(crate) fn release_all(&mut self) {
        for channel in 0..16 {
            self.release_channel(channel);
        }
    }

    pub fn dispatch(&mut self, midi_us: u64, message: &[u8]) -> Result<bool, TimedMidiError> {
        if message == [0xFF] {
            self.release_all();
            return Ok(true);
        }
        if self.controls.dispatch_button(message) {
            return Ok(true);
        }
        if message.len() != 3 || message[1] > 127 || message[2] > 127 {
            return Ok(false);
        }
        if message[0] & 0xF0 == 0xB0 {
            if matches!(message[1], 120 | 123) && message[2] == 0 {
                return Ok(self.release_channel(usize::from(message[0] & 0x0F)));
            }
            return Ok(self.controls.dispatch(message));
        }
        if !matches!(message[0] & 0xF0, 0x80 | 0x90) {
            return Ok(false);
        }
        let channel = usize::from(message[0] & 0x0F);
        if self.poly[channel].is_some() {
            return self.dispatch_poly(midi_us, message, channel);
        }
        let Some(route) = &mut self.notes[channel] else {
            return Ok(false);
        };
        let mut proposed = route.held;
        let previous = route.held.latest();
        let previous_velocity = route.held.latest_velocity();
        if message[0] & 0xF0 == 0x90 && message[2] != 0 {
            proposed.press(message[1], message[2]);
        } else if !proposed.remove(message[1]) {
            return Ok(false);
        }
        let current = proposed.latest();
        let current_velocity = proposed.latest_velocity();
        if current != previous || (route.velocity_mapped && current_velocity != previous_velocity) {
            let mut clock = self.clock.clone();
            let frame = clock
                .map(midi_us, self.producer.first_unrendered_frame())
                .map_err(TimedMidiError::Clock)?
                .frame;
            let note = current.or(previous).expect("note state changed");
            let velocity = f64::from(current_velocity.or(previous_velocity).unwrap_or(127)) / 127.0;
            self.producer
                .try_push_velocity(
                    TimedNoteState {
                        frame,
                        channel: channel as u8 + 1,
                        pitch_hz: route.frequencies[usize::from(note)],
                        gate: current.is_some(),
                    },
                    velocity,
                )
                .map_err(TimedMidiError::Queue)?;
            self.clock = clock;
        }
        route.held = proposed;
        Ok(true)
    }

    fn dispatch_poly(
        &mut self,
        midi_us: u64,
        message: &[u8],
        channel: usize,
    ) -> Result<bool, TimedMidiError> {
        let route = self.poly[channel]
            .as_mut()
            .expect("mapped polyphonic channel");
        let key = message[1];
        let mut proposed = route.held;
        let (voice, held, gate) = if message[0] & 0xF0 == 0x90 && message[2] != 0 {
            let voice = proposed[..route.voice_count]
                .iter()
                .position(|held| held.is_some_and(|held| held.key == key))
                .or_else(|| {
                    proposed[..route.voice_count]
                        .iter()
                        .position(Option::is_none)
                })
                .or_else(|| {
                    (route.overflow == PolyOverflow::StealOldest).then(|| {
                        (0..route.voice_count)
                            .min_by_key(|&index| route.ages[index])
                            .expect("validated polyphonic voices")
                    })
                })
                .ok_or(TimedMidiError::NoFreeVoice)?;
            let held = PolyHeld {
                key,
                velocity: message[2],
            };
            proposed[voice] = Some(held);
            (voice, held, true)
        } else {
            let Some(voice) = proposed
                .iter()
                .position(|held| held.is_some_and(|held| held.key == key))
            else {
                return Ok(false);
            };
            let held = proposed[voice].expect("matched held voice");
            proposed[voice] = None;
            (voice, held, false)
        };
        let mut clock = self.clock.clone();
        let frame = clock
            .map(midi_us, self.producer.first_unrendered_frame())
            .map_err(TimedMidiError::Clock)?
            .frame;
        self.producer
            .try_push_poly(
                TimedNoteState {
                    frame,
                    channel: channel as u8 + 1,
                    pitch_hz: route.frequencies[usize::from(held.key)],
                    gate,
                },
                f64::from(held.velocity) / 127.0,
                voice,
            )
            .map_err(TimedMidiError::Queue)?;
        route.held = proposed;
        if gate {
            route.next_age = route.next_age.saturating_add(1);
            route.ages[voice] = route.next_age;
        }
        self.clock = clock;
        Ok(true)
    }

    /// Device callbacks cannot retry a lost note-off. A full note queue
    /// releases that channel immediately through the separate reset marker.
    pub(crate) fn dispatch_recovering(
        &mut self,
        midi_us: u64,
        message: &[u8],
    ) -> Result<bool, TimedMidiError> {
        let result = self.dispatch(midi_us, message);
        if result == Err(TimedMidiError::Queue(PushError::Full)) {
            let channel = usize::from(message[0] & 0x0F);
            self.release_channel(channel);
        }
        result
    }
}

#[derive(Debug)]
pub(crate) struct NoteState {
    epoch: AtomicU64,
    held: Mutex<HeldNotes>,
}

impl NoteState {
    pub(crate) fn new() -> Self {
        Self {
            epoch: AtomicU64::new(0),
            held: Mutex::new(HeldNotes::new()),
        }
    }

    pub(crate) fn epoch(&self) -> u64 {
        self.epoch.load(Ordering::SeqCst)
    }

    pub(crate) fn begin_update(&self) {
        self.epoch.fetch_add(1, Ordering::SeqCst);
    }

    pub(crate) fn end_update(&self) {
        self.epoch.fetch_add(1, Ordering::SeqCst);
    }
}

impl HeldNotes {
    fn new() -> Self {
        Self {
            keys: [0; 128],
            velocities: [0; 128],
            len: 0,
        }
    }

    fn latest(&self) -> Option<u8> {
        (self.len > 0).then(|| self.keys[self.len.saturating_sub(1)])
    }

    fn latest_velocity(&self) -> Option<u8> {
        (self.len > 0).then(|| self.velocities[self.len.saturating_sub(1)])
    }

    fn remove(&mut self, note: u8) -> bool {
        let Some(index) = self.keys[..self.len].iter().position(|key| *key == note) else {
            return false;
        };
        self.keys.copy_within(index + 1..self.len, index);
        self.velocities.copy_within(index + 1..self.len, index);
        self.len -= 1;
        true
    }

    fn press(&mut self, note: u8, velocity: u8) {
        self.remove(note);
        self.keys[self.len] = note;
        self.velocities[self.len] = velocity;
        self.len += 1;
    }
}

/// Fixed lookup from raw MIDI bytes to instrument-declared controls.
#[derive(Clone)]
pub struct MidiRouter {
    routes: Vec<Option<ControlHandle>>,
    notes: Vec<Option<NoteRoute>>,
    buttons: Vec<ButtonRoute>,
}

#[derive(Clone)]
struct ButtonRoute {
    message: MidiButtonMessage,
    channel: u8,
    number: u8,
    edge: MidiButtonEdge,
    action: Action,
}

impl MidiRouter {
    pub fn from_machine(machine: &Machine) -> Self {
        let mut routes = vec![None; 16 * 128];
        for binding in machine.midi_ccs() {
            let index = usize::from(binding.channel - 1) * 128 + usize::from(binding.number);
            routes[index] = machine.control_handle(&binding.control_id);
        }
        let mut notes = vec![None; 16];
        for binding in machine.midi_notes() {
            notes[usize::from(binding.channel - 1)] = Some(NoteRoute {
                pitch: machine
                    .control_handle(&binding.pitch_control_id)
                    .expect("compiled MIDI pitch control"),
                gate: machine
                    .control_handle(&binding.gate_control_id)
                    .expect("compiled MIDI gate control"),
                velocity: binding.velocity_control_id.as_ref().map(|id| {
                    machine
                        .control_handle(id)
                        .expect("compiled MIDI velocity control")
                }),
                state: machine
                    .midi_note_state(binding.channel)
                    .expect("compiled MIDI note state"),
                frequencies: std::array::from_fn(|note| {
                    440.0 * 2.0_f64.powf((note as f64 - 69.0) / 12.0)
                }),
            });
        }
        let buttons = machine
            .midi_buttons()
            .iter()
            .map(|button| ButtonRoute {
                message: button.message,
                channel: button.channel,
                number: button.number,
                edge: button.edge,
                action: machine
                    .actions()
                    .iter()
                    .find(|action| action.label == button.action_label)
                    .expect("validated MIDI button action")
                    .clone(),
            })
            .collect();
        Self {
            routes,
            notes,
            buttons,
        }
    }

    /// Returns true only for a mapped, valid three-byte CC or note message.
    pub fn dispatch(&self, message: &[u8]) -> bool {
        if self.dispatch_button(message) {
            return true;
        }
        if message.len() != 3 || message[1] > 127 || message[2] > 127 {
            return false;
        }
        if matches!(message[0] & 0xF0, 0x80 | 0x90) {
            return self.dispatch_note(message);
        }
        if message[0] & 0xF0 != 0xB0 {
            return false;
        }
        let index = usize::from(message[0] & 0x0F) * 128 + usize::from(message[1]);
        let Some(handle) = &self.routes[index] else {
            return false;
        };
        let (min, max) = handle.range();
        let value = if message[2] == 127 {
            max
        } else {
            min + (max - min) * f64::from(message[2]) / 127.0
        };
        handle.publish(value.clamp(min, max)).is_ok()
    }

    fn dispatch_button(&self, message: &[u8]) -> bool {
        let Some(status) = message.first().copied() else {
            return false;
        };
        let kind = match status & 0xF0 {
            0x80 | 0x90 => MidiButtonMessage::Note,
            0xB0 => MidiButtonMessage::ControlChange,
            0xC0 => MidiButtonMessage::ProgramChange,
            _ => return false,
        };
        let channel = (status & 0x0F) + 1;
        let Some(number) = message.get(1).copied().filter(|number| *number <= 127) else {
            return false;
        };
        let valid = match kind {
            MidiButtonMessage::ProgramChange => message.len() == 2,
            MidiButtonMessage::Note | MidiButtonMessage::ControlChange => {
                message.len() == 3 && message[2] <= 127
            }
        };
        if !valid {
            return false;
        }
        let pressed = match kind {
            MidiButtonMessage::ProgramChange => true,
            MidiButtonMessage::Note => status & 0xF0 == 0x90 && message[2] > 0,
            MidiButtonMessage::ControlChange => message[2] > 0,
        };
        let edge = if pressed {
            MidiButtonEdge::Press
        } else {
            MidiButtonEdge::Release
        };
        let Some(route) = self.buttons.iter().find(|route| {
            route.message == kind
                && route.channel == channel
                && route.number == number
                && route.edge == edge
        }) else {
            return false;
        };
        route.action.invoke();
        true
    }

    fn dispatch_note(&self, message: &[u8]) -> bool {
        let Some(route) = &self.notes[usize::from(message[0] & 0x0F)] else {
            return false;
        };
        let mut held = route
            .state
            .held
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let previous = held.latest();
        let previous_velocity = held.latest_velocity();
        let note = message[1];
        if message[0] & 0xF0 == 0x90 && message[2] != 0 {
            held.press(note, message[2]);
        } else if !held.remove(note) {
            return false;
        }
        let current = held.latest();
        let current_velocity = held.latest_velocity();
        if current != previous
            || (route.velocity.is_some() && current_velocity != previous_velocity)
        {
            route.state.begin_update();
            let result = if let Some(current) = current {
                let hz = route.frequencies[usize::from(current)];
                route.pitch.publish(hz).and_then(|_| {
                    if let Some(velocity) = &route.velocity {
                        velocity
                            .publish(f64::from(current_velocity.expect("held note")) / 127.0)?;
                    }
                    if previous.is_none() {
                        route.gate.publish(1.0)
                    } else {
                        Ok(())
                    }
                })
            } else {
                route.gate.publish(0.0)
            };
            route.state.end_update();
            return result.is_ok();
        }
        true
    }
}
