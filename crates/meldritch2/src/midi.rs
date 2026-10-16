use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};

use crate::compiler::{ControlHandle, Machine};
use crate::{
    live_notes::{LiveNoteProducer, PushError},
    midi_clock::{ClockError, MidiFrameClock},
    timed::TimedNoteState,
};

#[derive(Clone)]
struct NoteRoute {
    pitch: ControlHandle,
    gate: ControlHandle,
    state: Arc<NoteState>,
    frequencies: [f64; 128],
}

#[derive(Clone, Copy, Debug)]
struct HeldNotes {
    keys: [u8; 128],
    len: usize,
}

struct TimedNoteRoute {
    held: HeldNotes,
    frequencies: [f64; 128],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimedMidiError {
    Clock(ClockError),
    Queue(PushError),
}

/// MIDI callback adapter: CCs publish controls, note changes enter the frame queue.
pub struct TimedMidiRouter {
    controls: MidiRouter,
    notes: [Option<TimedNoteRoute>; 16],
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
            });
        }
        Ok(Self {
            controls: MidiRouter::from_machine(machine),
            notes,
            producer,
            clock: MidiFrameClock::new(sample_rate, lead_frames)?,
        })
    }

    pub fn first_unrendered_frame(&self) -> u64 {
        self.producer.first_unrendered_frame()
    }

    pub fn dispatch(&mut self, midi_us: u64, message: &[u8]) -> Result<bool, TimedMidiError> {
        if message.len() != 3 || message[1] > 127 || message[2] > 127 {
            return Ok(false);
        }
        if message[0] & 0xF0 == 0xB0 {
            return Ok(self.controls.dispatch(message));
        }
        if !matches!(message[0] & 0xF0, 0x80 | 0x90) {
            return Ok(false);
        }
        let channel = usize::from(message[0] & 0x0F);
        let Some(route) = &mut self.notes[channel] else {
            return Ok(false);
        };
        let mut proposed = route.held;
        let previous = route.held.latest();
        if message[0] & 0xF0 == 0x90 && message[2] != 0 {
            proposed.press(message[1]);
        } else if !proposed.remove(message[1]) {
            return Ok(false);
        }
        let current = proposed.latest();
        if current != previous {
            let mut clock = self.clock.clone();
            let frame = clock
                .map(midi_us, self.producer.first_unrendered_frame())
                .map_err(TimedMidiError::Clock)?
                .frame;
            let note = current.or(previous).expect("note state changed");
            self.producer
                .try_push(TimedNoteState {
                    frame,
                    channel: channel as u8 + 1,
                    pitch_hz: route.frequencies[usize::from(note)],
                    gate: current.is_some(),
                })
                .map_err(TimedMidiError::Queue)?;
            self.clock = clock;
        }
        route.held = proposed;
        Ok(true)
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
            len: 0,
        }
    }

    fn latest(&self) -> Option<u8> {
        (self.len > 0).then(|| self.keys[self.len.saturating_sub(1)])
    }

    fn remove(&mut self, note: u8) -> bool {
        let Some(index) = self.keys[..self.len].iter().position(|key| *key == note) else {
            return false;
        };
        self.keys.copy_within(index + 1..self.len, index);
        self.len -= 1;
        true
    }

    fn press(&mut self, note: u8) {
        self.remove(note);
        self.keys[self.len] = note;
        self.len += 1;
    }
}

/// Fixed lookup from raw MIDI bytes to instrument-declared controls.
#[derive(Clone)]
pub struct MidiRouter {
    routes: Vec<Option<ControlHandle>>,
    notes: Vec<Option<NoteRoute>>,
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
                state: machine
                    .midi_note_state(binding.channel)
                    .expect("compiled MIDI note state"),
                frequencies: std::array::from_fn(|note| {
                    440.0 * 2.0_f64.powf((note as f64 - 69.0) / 12.0)
                }),
            });
        }
        Self { routes, notes }
    }

    /// Returns true only for a mapped, valid three-byte CC or note message.
    pub fn dispatch(&self, message: &[u8]) -> bool {
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
        let note = message[1];
        if message[0] & 0xF0 == 0x90 && message[2] != 0 {
            held.press(note);
        } else if !held.remove(note) {
            return false;
        }
        let current = held.latest();
        if current != previous {
            route.state.begin_update();
            let result = if let Some(current) = current {
                let hz = route.frequencies[usize::from(current)];
                route.pitch.publish(hz).and_then(|_| {
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
