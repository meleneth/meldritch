//! Real-time MIDI arpeggiator with no dependency on the Meldritch host.

use meldritch_interface::{
    MidiEvent, MidiMessage, MidiModule, MidiModuleContext, MidiModuleContract, MidiPort,
    MidiProcessor, Tempo,
};

pub const CONTRACT: MidiModuleContract = MidiModuleContract {
    name: "arpeggiator",
    inputs: &[MidiPort { name: "notes" }],
    outputs: &[MidiPort { name: "notes" }],
    needs_tempo: true,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Mode {
    #[default]
    Up,
    Down,
    UpDown,
    Played,
    Random,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Config {
    pub mode: Mode,
    pub octaves: u8,
    /// Arpeggiator notes per quarter-note beat.
    pub rate: u8,
    pub gate: f64,
    pub restart_on_change: bool,
    pub seed: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            mode: Mode::Up,
            octaves: 1,
            rate: 4,
            gate: 0.72,
            restart_on_change: true,
            seed: 1,
        }
    }
}

impl Config {
    pub fn validate(self) -> Result<Self, &'static str> {
        if !(1..=8).contains(&self.octaves) {
            return Err("arpeggiator octaves must be in 1..=8");
        }
        if !(1..=32).contains(&self.rate) {
            return Err("arpeggiator rate must be in 1..=32 notes per beat");
        }
        if !self.gate.is_finite() || !(0.01..=1.0).contains(&self.gate) {
            return Err("arpeggiator gate must be in 0.01..=1");
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug)]
struct Held {
    note: u8,
    velocity: u8,
    age: u64,
}

#[derive(Debug)]
pub struct Arpeggiator {
    config: Config,
    channel: u8,
    held: [Option<Held>; 128],
    next_age: u64,
    cursor: usize,
    direction: i8,
    random: u32,
    next_tick: Option<f64>,
    active: Option<(u8, u64)>,
    was_playing: bool,
    paused: bool,
}

impl Arpeggiator {
    pub fn new(config: Config, output_channel: u8) -> Result<Self, &'static str> {
        let config = config.validate()?;
        if !(1..=16).contains(&output_channel) {
            return Err("arpeggiator output channel must be in 1..=16");
        }
        Ok(Self {
            config,
            channel: output_channel,
            held: [None; 128],
            next_age: 0,
            cursor: 0,
            direction: 1,
            random: config.seed,
            next_tick: None,
            active: None,
            was_playing: false,
            paused: false,
        })
    }

    fn release(&mut self, frame: u64, output: &mut dyn FnMut(MidiEvent)) {
        if let Some((note, _)) = self.active.take() {
            output(MidiEvent {
                frame,
                message: MidiMessage::NoteOff {
                    channel: self.channel,
                    note,
                    velocity: 0,
                },
            });
        }
    }

    fn restart(&mut self, frame: u64) {
        self.cursor = 0;
        self.direction = 1;
        self.random = self.config.seed;
        self.next_tick = Some(frame as f64);
    }

    fn pool(&self) -> ([(u8, u8, u64); 128], usize) {
        let mut expanded = [None; 128];
        for held in self.held.iter().flatten() {
            for octave in 0..self.config.octaves {
                let note = u16::from(held.note) + u16::from(octave) * 12;
                if note <= 127 {
                    expanded[note as usize] = Some((held.velocity, held.age));
                }
            }
        }
        let mut values = [(0, 0, 0); 128];
        let mut len = 0;
        for (note, entry) in expanded.into_iter().enumerate() {
            if let Some((velocity, age)) = entry {
                values[len] = (note as u8, velocity, age);
                len += 1;
            }
        }
        if self.config.mode == Mode::Played {
            values[..len].sort_by_key(|(_, _, age)| *age);
        } else {
            values[..len].sort_by_key(|(note, _, _)| *note);
        }
        (values, len)
    }

    fn select(&mut self) -> Option<(u8, u8)> {
        let (pool, len) = self.pool();
        if len == 0 {
            return None;
        }
        let index = match self.config.mode {
            Mode::Up | Mode::Played => {
                let index = self.cursor % len;
                self.cursor = self.cursor.wrapping_add(1);
                index
            }
            Mode::Down => {
                let index = len - 1 - self.cursor % len;
                self.cursor = self.cursor.wrapping_add(1);
                index
            }
            Mode::UpDown if len == 1 => 0,
            Mode::UpDown => {
                let index = self.cursor.min(len - 1);
                if self.direction > 0 {
                    if self.cursor + 1 >= len {
                        self.direction = -1;
                        self.cursor = len - 2;
                    } else {
                        self.cursor += 1;
                    }
                } else if self.cursor == 0 {
                    self.direction = 1;
                    self.cursor = 1;
                } else {
                    self.cursor -= 1;
                }
                index
            }
            Mode::Random => {
                self.random = self.random.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                self.random as usize % len
            }
        };
        Some((pool[index].0, pool[index].1))
    }

    fn tick(&mut self, frame: u64, interval: f64, output: &mut dyn FnMut(MidiEvent)) {
        self.release(frame, output);
        if let Some((note, velocity)) = self.select() {
            let off = frame.saturating_add((interval * self.config.gate).round().max(1.0) as u64);
            output(MidiEvent {
                frame,
                message: MidiMessage::NoteOn {
                    channel: self.channel,
                    note,
                    velocity,
                },
            });
            self.active = Some((note, off));
        }
    }
}

impl MidiProcessor for Arpeggiator {
    fn reset(&mut self, frame: u64, output: &mut dyn FnMut(MidiEvent)) {
        self.release(frame, output);
        self.held.fill(None);
        self.next_tick = None;
        self.cursor = 0;
        self.was_playing = false;
        self.paused = false;
    }

    fn input(&mut self, event: MidiEvent, output: &mut dyn FnMut(MidiEvent)) {
        let changed = match event.message {
            MidiMessage::NoteOn { note, velocity, .. } => {
                if velocity == 0 {
                    self.held[usize::from(note)].take().is_some()
                } else {
                    self.next_age = self.next_age.wrapping_add(1);
                    self.held[usize::from(note)] = Some(Held {
                        note,
                        velocity,
                        age: self.next_age,
                    });
                    true
                }
            }
            MidiMessage::NoteOff { note, .. } => self.held[usize::from(note)].take().is_some(),
            MidiMessage::AllNotesOff { .. } => {
                let changed = self.held.iter().any(Option::is_some);
                self.held.fill(None);
                changed
            }
        };
        if !self.held.iter().any(Option::is_some) {
            self.release(event.frame, output);
            self.next_tick = None;
        } else if self.next_tick.is_none() || (changed && self.config.restart_on_change) {
            self.release(event.frame, output);
            self.restart(event.frame);
        }
    }

    fn next_frame(&self, tempo: Tempo) -> Option<u64> {
        if !tempo.playing {
            return self.active.map(|(_, off)| off);
        }
        let tick = self.next_tick.map(|frame| frame.round() as u64);
        let off = self.active.map(|(_, frame)| frame);
        match (tick, off) {
            (Some(left), Some(right)) => Some(left.min(right)),
            (Some(frame), None) | (None, Some(frame)) => Some(frame),
            (None, None) => None,
        }
    }

    fn advance(&mut self, end: u64, tempo: Tempo, output: &mut dyn FnMut(MidiEvent)) {
        if !tempo.playing {
            if self.was_playing {
                self.release(end, output);
                self.next_tick = self.held.iter().any(Option::is_some).then_some(end as f64);
            }
            self.was_playing = false;
            self.paused = true;
            return;
        }
        if self.paused && self.held.iter().any(Option::is_some) {
            self.next_tick = Some(end as f64);
        }
        self.paused = false;
        self.was_playing = true;
        let Some(frames_per_beat) = tempo.frames_per_beat() else {
            return;
        };
        let interval = (frames_per_beat / f64::from(self.config.rate)).max(1.0);
        loop {
            let next = self.next_frame(tempo);
            let Some(frame) = next.filter(|frame| *frame < end) else {
                break;
            };
            if self.active.is_some_and(|(_, off)| off == frame) {
                self.release(frame, output);
            }
            if self
                .next_tick
                .is_some_and(|tick| tick.round() as u64 == frame)
            {
                self.tick(frame, interval, output);
                self.next_tick = self.next_tick.map(|tick| tick + interval);
            }
        }
    }
}

impl MidiModule for Arpeggiator {
    fn contract(&self) -> &'static MidiModuleContract {
        &CONTRACT
    }

    fn process_input(
        &mut self,
        port: usize,
        event: MidiEvent,
        _context: MidiModuleContext,
        output: &mut dyn FnMut(MidiEvent),
    ) {
        assert_eq!(port, 0, "validated arpeggiator input port");
        MidiProcessor::input(self, event, output);
    }

    fn scheduled_frame(&self, context: MidiModuleContext) -> Option<u64> {
        MidiProcessor::next_frame(
            self,
            context.tempo.expect("validated arpeggiator tempo input"),
        )
    }

    fn process_until(
        &mut self,
        end: u64,
        context: MidiModuleContext,
        output: &mut dyn FnMut(MidiEvent),
    ) {
        MidiProcessor::advance(
            self,
            end,
            context.tempo.expect("validated arpeggiator tempo input"),
            output,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn held_chord_generates_sample_accurate_note_pairs() {
        let mut arp = Arpeggiator::new(
            Config {
                rate: 2,
                gate: 0.5,
                ..Config::default()
            },
            3,
        )
        .unwrap();
        let mut output = Vec::new();
        arp.input(
            MidiEvent {
                frame: 0,
                message: MidiMessage::NoteOn {
                    channel: 1,
                    note: 60,
                    velocity: 90,
                },
            },
            &mut |event| output.push(event),
        );
        arp.input(
            MidiEvent {
                frame: 0,
                message: MidiMessage::NoteOn {
                    channel: 1,
                    note: 64,
                    velocity: 80,
                },
            },
            &mut |event| output.push(event),
        );
        arp.advance(
            48_001,
            Tempo {
                bpm: 120.0,
                sample_rate: 48_000,
                playing: true,
            },
            &mut |event| output.push(event),
        );
        assert_eq!(
            output[0].message,
            MidiMessage::NoteOn {
                channel: 3,
                note: 60,
                velocity: 90
            }
        );
        assert_eq!(output[1].frame, 6_000);
        assert_eq!(
            output[2].message,
            MidiMessage::NoteOn {
                channel: 3,
                note: 64,
                velocity: 80
            }
        );
        assert!(output.windows(2).all(|pair| pair[0].frame <= pair[1].frame));
    }

    #[test]
    fn release_and_transport_stop_never_leave_a_gate_open() {
        let mut arp = Arpeggiator::new(Config::default(), 1).unwrap();
        let mut output = Vec::new();
        arp.input(
            MidiEvent {
                frame: 10,
                message: MidiMessage::NoteOn {
                    channel: 1,
                    note: 60,
                    velocity: 127,
                },
            },
            &mut |event| output.push(event),
        );
        arp.advance(
            11,
            Tempo {
                bpm: 120.0,
                sample_rate: 48_000,
                playing: true,
            },
            &mut |event| output.push(event),
        );
        arp.advance(
            12,
            Tempo {
                bpm: 120.0,
                sample_rate: 48_000,
                playing: false,
            },
            &mut |event| output.push(event),
        );
        assert!(matches!(
            output.last().unwrap().message,
            MidiMessage::NoteOff { note: 60, .. }
        ));
    }

    fn played_notes(mode: Mode) -> Vec<u8> {
        let mut arp = Arpeggiator::new(
            Config {
                mode,
                rate: 1,
                gate: 0.5,
                seed: 73,
                ..Config::default()
            },
            1,
        )
        .unwrap();
        let mut output = Vec::new();
        for note in [60, 64, 67] {
            arp.input(
                MidiEvent {
                    frame: 0,
                    message: MidiMessage::NoteOn {
                        channel: 1,
                        note,
                        velocity: 100,
                    },
                },
                &mut |_| {},
            );
        }
        arp.advance(
            96_001,
            Tempo {
                bpm: 120.0,
                sample_rate: 48_000,
                playing: true,
            },
            &mut |event| output.push(event),
        );
        output
            .into_iter()
            .filter_map(|event| match event.message {
                MidiMessage::NoteOn { note, .. } => Some(note),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn traversal_modes_and_seeded_random_are_stable() {
        assert_eq!(&played_notes(Mode::Up)[..3], &[60, 64, 67]);
        assert_eq!(&played_notes(Mode::Down)[..3], &[67, 64, 60]);
        assert_eq!(&played_notes(Mode::UpDown)[..5], &[60, 64, 67, 64, 60]);
        assert_eq!(played_notes(Mode::Played), played_notes(Mode::Up));
        assert_eq!(played_notes(Mode::Random), played_notes(Mode::Random));
    }
}
