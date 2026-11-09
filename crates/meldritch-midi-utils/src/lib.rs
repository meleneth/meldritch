//! Small, allocation-free MIDI stream processors.
//!
//! [`KeyLock`] has two semantic MIDI inputs. [`KeyLock::key_input`] selects
//! the scale root from the pitch class of the latest nonzero-velocity NoteOn;
//! [`KeyLock::note_input`] filters the performed note stream. Out-of-scale
//! notes can be suppressed or moved to the nearest scale tone. Mapped notes
//! are reference-counted, so two held source notes that converge on one pitch
//! produce one NoteOn and one final NoteOff.

use meldritch_interface::{
    MidiEvent, MidiMessage, MidiModule, MidiModuleContext, MidiModuleContract, MidiPort,
};

pub const KEY_LOCK_CONTRACT: MidiModuleContract = MidiModuleContract {
    name: "key_lock",
    inputs: &[MidiPort { name: "key" }, MidiPort { name: "notes" }],
    outputs: &[MidiPort { name: "notes" }],
    needs_tempo: false,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Scale {
    #[default]
    Major,
    NaturalMinor,
    Dorian,
    Mixolydian,
    Pentatonic,
    Chromatic,
}

impl Scale {
    fn contains(self, interval: u8) -> bool {
        const MAJOR: u16 = mask(&[0, 2, 4, 5, 7, 9, 11]);
        const MINOR: u16 = mask(&[0, 2, 3, 5, 7, 8, 10]);
        const DORIAN: u16 = mask(&[0, 2, 3, 5, 7, 9, 10]);
        const MIXOLYDIAN: u16 = mask(&[0, 2, 4, 5, 7, 9, 10]);
        const PENTATONIC: u16 = mask(&[0, 2, 4, 7, 9]);
        let allowed = match self {
            Self::Major => MAJOR,
            Self::NaturalMinor => MINOR,
            Self::Dorian => DORIAN,
            Self::Mixolydian => MIXOLYDIAN,
            Self::Pentatonic => PENTATONIC,
            Self::Chromatic => 0x0fff,
        };
        allowed & (1 << interval) != 0
    }
}

const fn mask(intervals: &[u8]) -> u16 {
    let mut result = 0;
    let mut index = 0;
    while index < intervals.len() {
        result |= 1 << intervals[index];
        index += 1;
    }
    result
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WrongNotePolicy {
    Suppress,
    #[default]
    Nearest,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TieBreak {
    #[default]
    Down,
    Up,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KeyLockConfig {
    pub scale: Scale,
    pub wrong_notes: WrongNotePolicy,
    pub tie_break: TieBreak,
    /// Used until the key input receives its first NoteOn.
    pub initial_key: u8,
}

impl Default for KeyLockConfig {
    fn default() -> Self {
        Self {
            scale: Scale::Major,
            wrong_notes: WrongNotePolicy::Nearest,
            tie_break: TieBreak::Down,
            initial_key: 60,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct SourceNote {
    count: u16,
    mapped: Option<u8>,
    velocity: u8,
}

#[derive(Debug)]
pub struct KeyLock {
    config: KeyLockConfig,
    output_channel: u8,
    root: u8,
    sources: [[SourceNote; 128]; 16],
    output_refs: [u32; 128],
}

impl KeyLock {
    pub fn new(config: KeyLockConfig, output_channel: u8) -> Result<Self, &'static str> {
        if config.initial_key > 127 {
            return Err("key lock initial key must be a MIDI note in 0..=127");
        }
        if !(1..=16).contains(&output_channel) {
            return Err("key lock output channel must be in 1..=16");
        }
        Ok(Self {
            config,
            output_channel,
            root: config.initial_key % 12,
            sources: [[SourceNote::default(); 128]; 16],
            output_refs: [0; 128],
        })
    }

    #[must_use]
    pub const fn root(&self) -> u8 {
        self.root
    }

    fn allowed(&self, note: u8) -> bool {
        self.config.scale.contains((note + 12 - self.root) % 12)
    }

    fn mapped_note(&self, note: u8) -> Option<u8> {
        if self.allowed(note) {
            return Some(note);
        }
        if self.config.wrong_notes == WrongNotePolicy::Suppress {
            return None;
        }
        for distance in 1..=12 {
            let down = note
                .checked_sub(distance)
                .filter(|note| self.allowed(*note));
            let up = note
                .checked_add(distance)
                .filter(|note| *note <= 127 && self.allowed(*note));
            match (down, up) {
                (Some(down), Some(up)) => {
                    return Some(if self.config.tie_break == TieBreak::Down {
                        down
                    } else {
                        up
                    });
                }
                (Some(note), None) | (None, Some(note)) => return Some(note),
                (None, None) => {}
            }
        }
        None
    }

    fn emit_on(&mut self, frame: u64, note: u8, velocity: u8, output: &mut dyn FnMut(MidiEvent)) {
        let refs = &mut self.output_refs[usize::from(note)];
        if *refs == 0 {
            output(MidiEvent {
                frame,
                message: MidiMessage::NoteOn {
                    channel: self.output_channel,
                    note,
                    velocity,
                },
            });
        }
        *refs = refs.saturating_add(1);
    }

    fn emit_off(&mut self, frame: u64, note: u8, output: &mut dyn FnMut(MidiEvent)) {
        let refs = &mut self.output_refs[usize::from(note)];
        if *refs == 0 {
            return;
        }
        *refs -= 1;
        if *refs == 0 {
            output(MidiEvent {
                frame,
                message: MidiMessage::NoteOff {
                    channel: self.output_channel,
                    note,
                    velocity: 0,
                },
            });
        }
    }

    pub fn note_input(&mut self, event: MidiEvent, output: &mut dyn FnMut(MidiEvent)) {
        let midi_channel = event.message.channel();
        if !(1..=16).contains(&midi_channel) {
            return;
        }
        let channel = usize::from(midi_channel - 1);
        match event.message {
            MidiMessage::NoteOn { note, velocity, .. } if velocity > 0 => {
                let source = self.sources[channel][usize::from(note)];
                if source.count == u16::MAX {
                    return;
                }
                let mapped = if source.count == 0 {
                    self.mapped_note(note)
                } else {
                    source.mapped
                };
                let state = &mut self.sources[channel][usize::from(note)];
                state.count = state.count.saturating_add(1);
                state.mapped = mapped;
                state.velocity = velocity;
                if let Some(mapped) = mapped {
                    self.emit_on(event.frame, mapped, velocity, output);
                }
            }
            MidiMessage::NoteOn { note, .. } | MidiMessage::NoteOff { note, .. } => {
                let state = &mut self.sources[channel][usize::from(note)];
                if state.count == 0 {
                    return;
                }
                state.count -= 1;
                let mapped = state.mapped;
                if state.count == 0 {
                    state.mapped = None;
                }
                if let Some(mapped) = mapped {
                    self.emit_off(event.frame, mapped, output);
                }
            }
            MidiMessage::AllNotesOff { .. } => self.release_channel(channel, event.frame, output),
        }
    }

    pub fn key_input(&mut self, event: MidiEvent, output: &mut dyn FnMut(MidiEvent)) {
        if let MidiMessage::NoteOn { note, velocity, .. } = event.message
            && velocity > 0
            && note % 12 != self.root
        {
            self.root = note % 12;
            self.remap_held(event.frame, output);
        }
    }

    pub fn set_policy(
        &mut self,
        policy: WrongNotePolicy,
        frame: u64,
        output: &mut dyn FnMut(MidiEvent),
    ) {
        if self.config.wrong_notes != policy {
            self.config.wrong_notes = policy;
            self.remap_held(frame, output);
        }
    }

    pub fn set_scale(&mut self, scale: Scale, frame: u64, output: &mut dyn FnMut(MidiEvent)) {
        if self.config.scale != scale {
            self.config.scale = scale;
            self.remap_held(frame, output);
        }
    }

    pub fn set_tie_break(
        &mut self,
        tie_break: TieBreak,
        frame: u64,
        output: &mut dyn FnMut(MidiEvent),
    ) {
        if self.config.tie_break != tie_break {
            self.config.tie_break = tie_break;
            self.remap_held(frame, output);
        }
    }

    pub fn reset(&mut self, frame: u64, output: &mut dyn FnMut(MidiEvent)) {
        for note in 0..128 {
            if self.output_refs[note] > 0 {
                output(MidiEvent {
                    frame,
                    message: MidiMessage::NoteOff {
                        channel: self.output_channel,
                        note: note as u8,
                        velocity: 0,
                    },
                });
            }
        }
        self.sources = [[SourceNote::default(); 128]; 16];
        self.output_refs = [0; 128];
    }

    fn release_channel(&mut self, channel: usize, frame: u64, output: &mut dyn FnMut(MidiEvent)) {
        for note in 0..128 {
            let state = self.sources[channel][note];
            if let Some(mapped) = state.mapped {
                for _ in 0..state.count {
                    self.emit_off(frame, mapped, output);
                }
            }
            self.sources[channel][note] = SourceNote::default();
        }
    }

    fn remap_held(&mut self, frame: u64, output: &mut dyn FnMut(MidiEvent)) {
        let old_refs = self.output_refs;
        let mut new_refs = [0_u32; 128];
        let mut new_velocity = [0_u8; 128];
        for channel in 0..16 {
            for note in 0..128 {
                let source = self.sources[channel][note];
                if source.count == 0 {
                    continue;
                }
                let mapped = self.mapped_note(note as u8);
                self.sources[channel][note].mapped = mapped;
                if let Some(mapped) = mapped {
                    let index = usize::from(mapped);
                    new_refs[index] = new_refs[index].saturating_add(u32::from(source.count));
                    new_velocity[index] = new_velocity[index].max(source.velocity);
                }
            }
        }
        for note in 0..128 {
            if old_refs[note] > 0 && new_refs[note] == 0 {
                output(MidiEvent {
                    frame,
                    message: MidiMessage::NoteOff {
                        channel: self.output_channel,
                        note: note as u8,
                        velocity: 0,
                    },
                });
            }
        }
        for note in 0..128 {
            if old_refs[note] == 0 && new_refs[note] > 0 {
                output(MidiEvent {
                    frame,
                    message: MidiMessage::NoteOn {
                        channel: self.output_channel,
                        note: note as u8,
                        velocity: new_velocity[note],
                    },
                });
            }
        }
        self.output_refs = new_refs;
    }
}

impl MidiModule for KeyLock {
    fn contract(&self) -> &'static MidiModuleContract {
        &KEY_LOCK_CONTRACT
    }

    fn process_input(
        &mut self,
        port: usize,
        event: MidiEvent,
        _context: MidiModuleContext,
        output: &mut dyn FnMut(MidiEvent),
    ) {
        match port {
            0 => self.key_input(event, output),
            1 => self.note_input(event, output),
            _ => panic!("validated key-lock input port"),
        }
    }

    fn scheduled_frame(&self, _context: MidiModuleContext) -> Option<u64> {
        None
    }

    fn process_until(
        &mut self,
        _end: u64,
        _context: MidiModuleContext,
        _output: &mut dyn FnMut(MidiEvent),
    ) {
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(frame: u64, channel: u8, note: u8, velocity: u8) -> MidiEvent {
        MidiEvent {
            frame,
            message: MidiMessage::NoteOn {
                channel,
                note,
                velocity,
            },
        }
    }

    fn off(frame: u64, channel: u8, note: u8) -> MidiEvent {
        MidiEvent {
            frame,
            message: MidiMessage::NoteOff {
                channel,
                note,
                velocity: 0,
            },
        }
    }

    #[test]
    fn suppresses_or_converges_wrong_notes() {
        let mut suppress = KeyLock::new(
            KeyLockConfig {
                wrong_notes: WrongNotePolicy::Suppress,
                ..KeyLockConfig::default()
            },
            3,
        )
        .unwrap();
        let mut output = Vec::new();
        suppress.note_input(note(0, 1, 61, 100), &mut |event| output.push(event));
        assert!(output.is_empty());

        let mut nearest = KeyLock::new(KeyLockConfig::default(), 3).unwrap();
        nearest.note_input(note(0, 1, 61, 100), &mut |event| output.push(event));
        assert_eq!(
            output.last().unwrap().message,
            MidiMessage::NoteOn {
                channel: 3,
                note: 60,
                velocity: 100
            }
        );
    }

    #[test]
    fn converged_notes_are_reference_counted_without_duplicates() {
        let mut lock = KeyLock::new(KeyLockConfig::default(), 2).unwrap();
        let mut output = Vec::new();
        lock.note_input(note(0, 1, 60, 100), &mut |event| output.push(event));
        lock.note_input(note(1, 1, 61, 80), &mut |event| output.push(event));
        assert_eq!(output.len(), 1);
        lock.note_input(off(2, 1, 60), &mut |event| output.push(event));
        assert_eq!(output.len(), 1);
        lock.note_input(off(3, 1, 61), &mut |event| output.push(event));
        assert_eq!(output.len(), 2);
        assert!(matches!(
            output[1].message,
            MidiMessage::NoteOff { note: 60, .. }
        ));
    }

    #[test]
    fn key_changes_remap_held_notes_without_duplicate_edges() {
        let mut lock = KeyLock::new(KeyLockConfig::default(), 4).unwrap();
        let mut output = Vec::new();
        lock.note_input(note(0, 1, 60, 110), &mut |event| output.push(event));
        lock.key_input(note(5, 2, 62, 127), &mut |event| output.push(event));
        assert_eq!(output.len(), 3);
        assert!(matches!(
            output[1].message,
            MidiMessage::NoteOff { note: 60, .. }
        ));
        assert!(matches!(
            output[2].message,
            MidiMessage::NoteOn { note: 59, .. }
        ));
    }

    #[test]
    fn policy_and_tie_break_can_change_while_notes_are_held() {
        let mut lock = KeyLock::new(KeyLockConfig::default(), 4).unwrap();
        let mut output = Vec::new();
        lock.note_input(note(0, 1, 61, 100), &mut |event| output.push(event));
        lock.set_tie_break(TieBreak::Up, 1, &mut |event| output.push(event));
        assert!(matches!(
            output.as_slice(),
            [
                MidiEvent {
                    message: MidiMessage::NoteOn { note: 60, .. },
                    ..
                },
                MidiEvent {
                    message: MidiMessage::NoteOff { note: 60, .. },
                    ..
                },
                MidiEvent {
                    message: MidiMessage::NoteOn { note: 62, .. },
                    ..
                }
            ]
        ));
        lock.set_policy(WrongNotePolicy::Suppress, 2, &mut |event| {
            output.push(event)
        });
        assert!(matches!(
            output.last().unwrap().message,
            MidiMessage::NoteOff { note: 62, .. }
        ));
    }
}
