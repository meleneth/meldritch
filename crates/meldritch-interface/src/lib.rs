//! Shared, allocation-free interfaces for Meldritch graph modules.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Domain {
    MonoAudio,
    Midi,
    Tempo,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FanIn {
    One,
    Sum,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Role {
    Processor,
    Output,
    StereoOutput,
}

#[derive(Clone, Copy, Debug)]
pub struct InputPort {
    pub name: &'static str,
    pub domain: Domain,
    pub fan_in: FanIn,
    /// Neutral audio/control sample when unconnected; `None` means required.
    pub unconnected: Option<f64>,
}

#[derive(Clone, Copy, Debug)]
pub struct OutputPort {
    pub name: &'static str,
    pub domain: Domain,
}

#[derive(Clone, Copy, Debug)]
pub struct Param {
    pub name: &'static str,
    pub default: f64,
    pub range: Option<(f64, f64)>,
}

pub type ProcessFn = fn(&mut [f64], &[f64], &[f64], f64, &mut [f64]);

#[derive(Clone, Copy, Debug)]
pub struct Contract {
    pub name: &'static str,
    pub role: Role,
    pub process: ProcessFn,
    pub state_len: usize,
    pub inputs: &'static [InputPort],
    pub outputs: &'static [OutputPort],
    pub params: &'static [Param],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MidiMessage {
    NoteOn { channel: u8, note: u8, velocity: u8 },
    NoteOff { channel: u8, note: u8, velocity: u8 },
    AllNotesOff { channel: u8 },
}

impl MidiMessage {
    #[must_use]
    pub const fn channel(self) -> u8 {
        match self {
            Self::NoteOn { channel, .. }
            | Self::NoteOff { channel, .. }
            | Self::AllNotesOff { channel } => channel,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MidiEvent {
    pub frame: u64,
    pub message: MidiMessage,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tempo {
    pub bpm: f64,
    pub sample_rate: u32,
    pub playing: bool,
}

impl Tempo {
    pub fn frames_per_beat(self) -> Option<f64> {
        (self.bpm.is_finite() && self.bpm > 0.0 && self.sample_rate > 0)
            .then(|| f64::from(self.sample_rate) * 60.0 / self.bpm)
    }
}

pub trait MidiProcessor {
    fn reset(&mut self, frame: u64, output: &mut dyn FnMut(MidiEvent));
    fn input(&mut self, event: MidiEvent, output: &mut dyn FnMut(MidiEvent));
    fn next_frame(&self, tempo: Tempo) -> Option<u64>;
    fn advance(&mut self, end: u64, tempo: Tempo, output: &mut dyn FnMut(MidiEvent));
}
