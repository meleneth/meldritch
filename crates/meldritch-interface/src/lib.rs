//! Shared, allocation-free interfaces for Meldritch graph modules.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Domain {
    /// Untyped numeric audio/control sample. Use an explicit marker to assign
    /// a musical unit before connecting to a semantically typed input.
    MonoAudio,
    Midi,
    Tempo,
    /// A discrete MIDI note encoded exactly as (note_number / 128.0).
    Note128,
    /// A MIDI note number 0..=127 carried in numeric DSP storage.
    MidiNote,
    /// A pitch measured in cycles per second.
    FrequencyHz,
}

impl Domain {
    /// All of these compile to the same f64 storage; a generic numeric sink
    /// may observe typed values, but generic sources may not enter typed inputs.
    pub fn accepts(self, source: Self) -> bool {
        self == source
            || matches!(self, Self::MonoAudio)
                && matches!(source, Self::Note128 | Self::MidiNote | Self::FrequencyHz)
    }
}

/// A semantic MIDI pitch encoded as an exactly representable note / 128.
/// Not interchangeable with raw normalized control values or frequencies.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Note128(f64);

impl Note128 {
    pub fn new(value: f64) -> Result<Self, &'static str> {
        if !value.is_finite() || !(0.0..=127.0 / 128.0).contains(&value) {
            return Err("note/128 must represent MIDI note 0..127");
        }
        if (value * 128.0).fract() != 0.0 {
            return Err("note/128 must be an exact discrete note");
        }
        Ok(Self(value))
    }

    pub const fn value(self) -> f64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MidiNote(u8);

impl MidiNote {
    pub fn new(value: u8) -> Result<Self, &'static str> {
        if value > 127 {
            return Err("MIDI note must be in 0..127");
        }
        Ok(Self(value))
    }

    pub const fn value(self) -> u8 {
        self.0
    }
}

impl From<MidiNote> for Note128 {
    fn from(value: MidiNote) -> Self {
        Self(f64::from(value.0) / 128.0)
    }
}

impl TryFrom<Note128> for MidiNote {
    type Error = &'static str;

    fn try_from(value: Note128) -> Result<Self, Self::Error> {
        // Note128::new has already established a finite, integral MIDI note.
        Self::new((value.0 * 128.0) as u8)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrequencyHz(f64);

impl FrequencyHz {
    pub fn new(value: f64) -> Result<Self, &'static str> {
        if !value.is_finite() || value <= 0.0 {
            return Err("frequency must be finite and positive");
        }
        Ok(Self(value))
    }

    pub const fn value(self) -> f64 {
        self.0
    }
}

impl From<MidiNote> for FrequencyHz {
    fn from(note: MidiNote) -> Self {
        Self(440.0 * 2.0_f64.powf((f64::from(note.0) - 69.0) / 12.0))
    }
}

#[cfg(test)]
mod semantic_types_tests {
    use super::{FrequencyHz, MidiNote, Note128};

    #[test]
    fn all_128_discrete_notes_preserve_exact_binary_representation() {
        for number in 0..=127 {
            let note = MidiNote::new(number).unwrap();
            let normalized = Note128::from(note);
            assert_eq!(normalized.value(), f64::from(number) / 128.0);
            assert_eq!(MidiNote::try_from(normalized), Ok(note));
            assert_eq!(Note128::new(f64::from(number) / 128.0), Ok(normalized));
        }
    }

    #[test]
    fn invalid_semantic_values_are_rejected() {
        assert!(MidiNote::new(128).is_err());
        assert!(Note128::new(1.0).is_err());
        assert!(Note128::new(60.5 / 128.0).is_err());
        assert!(Note128::new(f64::NAN).is_err());
        assert!(FrequencyHz::new(-440.0).is_err());
        assert!(FrequencyHz::new(f64::INFINITY).is_err());
        let a4 = FrequencyHz::from(MidiNote::new(69).unwrap());
        assert_eq!(a4.value(), 440.0);
    }
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MidiPort {
    pub name: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MidiModuleContract {
    pub name: &'static str,
    pub inputs: &'static [MidiPort],
    pub outputs: &'static [MidiPort],
    pub needs_tempo: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tempo {
    pub bpm: f64,
    pub sample_rate: u32,
    pub playing: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MidiModuleContext {
    pub tempo: Option<Tempo>,
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

/// A named-port MIDI graph node. The host routes events solely from the
/// contract; implementations do not know which modules precede or follow them.
pub trait MidiModule: Send {
    fn contract(&self) -> &'static MidiModuleContract;
    fn process_input(
        &mut self,
        port: usize,
        event: MidiEvent,
        context: MidiModuleContext,
        output: &mut dyn FnMut(MidiEvent),
    );
    fn scheduled_frame(&self, context: MidiModuleContext) -> Option<u64>;
    fn process_until(
        &mut self,
        end: u64,
        context: MidiModuleContext,
        output: &mut dyn FnMut(MidiEvent),
    );
}
