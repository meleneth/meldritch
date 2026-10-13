//! Shared mono voice execution, owned and advanced on a render worker.
use super::*;

pub(super) struct MonoState {
    oscillator: NoteOscillator,
    envelope: AdsrEnvelope,
    filter: StateVariableFilter,
    pub frequency: f64,
    pub velocity: f64,
    control_values: [f64; 35],
    control_origins: [u64; 32],
    control_phases: [f64; 32],
    tempo: meldritch_core::Tempo,
}

impl MonoState {
    pub fn new(patch: &CompiledNotePatch) -> Result<Self, SongRenderError> {
        Ok(Self {
            oscillator: NoteOscillator::new(patch.waveform, patch.plaits),
            envelope: AdsrEnvelope::new(patch.envelope, patch.sample_rate),
            filter: StateVariableFilter::new(),
            frequency: patch
                .independent_signals
                .as_ref()
                .map_or(440.0, |signals| midi_frequency(signals.initial_pitch)),
            velocity: if patch.independent_signals.is_some() {
                1.0
            } else {
                0.0
            },
            control_values: {
                let mut values = [0.0; 35];
                // Independent external Control, separate from legacy note gain.
                values[crate::song_control::VELOCITY_INPUT] = 1.0;
                values
            },
            control_origins: [0; 32],
            control_phases: [0.0; 32],
            tempo: meldritch_core::Tempo::new(patch.bpm, patch.sample_rate)
                .map_err(|e| SongRenderError::InvalidPatternVoice(e.to_string()))?,
        })
    }

    pub fn gate_on(&mut self, velocity: f64) {
        self.velocity = velocity;
        self.envelope.note_on();
        self.oscillator.note_on(velocity);
    }
    pub fn gate_off(&mut self) {
        self.envelope.note_off();
        self.oscillator.note_off();
    }

    pub fn next_sample(
        &mut self,
        patch: &CompiledNotePatch,
        age: u64,
        gate_rise: bool,
        cutoff_override: Option<f64>,
        resonance_override: Option<f64>,
    ) -> f64 {
        let envelope_value = self.envelope.next_value();
        let level = patch.control.as_ref().map_or(envelope_value, |plan| {
            plan.value_at(
                age,
                gate_rise,
                envelope_value,
                self.tempo,
                &mut self.control_values,
                &mut self.control_origins,
                &mut self.control_phases,
            )
        });
        let frequency = patch
            .control
            .as_ref()
            .and_then(|plan| plan.pitch_offset)
            .map_or(self.frequency, |index| {
                self.frequency * 2_f64.powf(self.control_values[index] / 12.0)
            });
        let mut sample = self.oscillator.next(frequency, patch.sample_rate) * level * self.velocity;
        if let Some(settings) = &patch.filter {
            sample = self.filter.process(
                sample,
                patch
                    .control
                    .as_ref()
                    .and_then(|plan| plan.cutoff)
                    .map_or(cutoff_override.unwrap_or(settings.cutoff_hz), |index| {
                        self.control_values[index]
                    }),
                resonance_override.unwrap_or(settings.resonance),
                FilterMode::LowPass,
                patch.sample_rate,
            );
        }
        sample
    }
}

/// Separate typed source inputs: pitch changes never imply gate articulation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LiveVoiceInput {
    Pitch(u8),
    /// MIDI resolution, explicitly normalized to unit interval at input.velocity.
    /// Never changes articulation or implicit note gain.
    Velocity(u8),
    /// 14-bit MIDI value, normalized around exact center 8192 at input.bend.
    /// Pitch changes only through an explicit compatible graph adapter.
    Bend(u16),
    Gate(bool),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LiveVoiceEvent {
    pub frame: u64,
    pub input: LiveVoiceInput,
}

/// Fresh mono instance, preserving its DSP history across bounded worker blocks.
/// This is not a device callback, MIDI scheduler or graph-state migration API.
pub struct LiveMonoRenderer {
    patch: CompiledNotePatch,
    state: MonoState,
    origin: u64,
    cursor: u64,
    gate: bool,
}

impl CompiledNotePatch {
    pub fn live_renderer(
        &self,
        initial_pitch: u8,
        origin: u64,
    ) -> Result<LiveMonoRenderer, SongRenderError> {
        if self.polyphony != 1 || initial_pitch > 127 {
            return Err(SongRenderError::InvalidPatternVoice(
                "live renderer requires mono topology and MIDI pitch in 0..127".into(),
            ));
        }
        let mut state = MonoState::new(self)?;
        state.frequency = midi_frequency(initial_pitch);
        // Independent inputs: only authored cables determine amplitude articulation.
        state.velocity = 1.0;
        Ok(LiveMonoRenderer {
            patch: self.clone(),
            state,
            origin,
            cursor: origin,
            gate: false,
        })
    }
}

impl LiveMonoRenderer {
    pub const MAX_FRAMES: u64 = 4096;
    pub const MAX_EVENTS: usize = 128;

    pub fn cursor(&self) -> u64 {
        self.cursor
    }

    pub fn render(
        &mut self,
        range: FrameRange,
        events: &[LiveVoiceEvent],
    ) -> Result<AudioBlock, SongRenderError> {
        let invalid = |message: &str| SongRenderError::InvalidPatternVoice(message.into());
        let frames = range.end() - range.start();
        if range.start() != self.cursor || !(1..=Self::MAX_FRAMES).contains(&frames) {
            return Err(invalid(
                "live block must start at cursor and contain 1..4096 frames",
            ));
        }
        if events.len() > Self::MAX_EVENTS {
            return Err(invalid("live block exceeds 128-event budget"));
        }
        let mut previous = range.start();
        for event in events {
            if event.frame < previous || event.frame >= range.end() {
                return Err(invalid("live events must be ordered and inside the block"));
            }
            if matches!(event.input, LiveVoiceInput::Pitch(pitch) if pitch > 127) {
                return Err(invalid("live pitch must be in 0..127"));
            }
            if matches!(event.input, LiveVoiceInput::Velocity(value) if value > 127) {
                return Err(invalid("live velocity must be in 0..127"));
            }
            if matches!(event.input, LiveVoiceInput::Bend(value) if value > 16383) {
                return Err(invalid("live bend must be in 0..16383"));
            }
            previous = event.frame;
        }
        // All validation completes before any state advance or event application.
        let mut block = AudioBlock::silent(self.patch.channels, frames as u32);
        let mut events = events.iter().peekable();
        for frame in range.start()..range.end() {
            let mut gate_rise = false;
            while events.peek().is_some_and(|event| event.frame == frame) {
                match events.next().unwrap().input {
                    LiveVoiceInput::Pitch(pitch) => self.state.frequency = midi_frequency(pitch),
                    LiveVoiceInput::Velocity(value) => {
                        self.state.control_values[crate::song_control::VELOCITY_INPUT] =
                            f64::from(value) / 127.0
                    }
                    LiveVoiceInput::Bend(value) => {
                        // Both physical endpoints reach exactly +/-1; center is 0.
                        let offset = f64::from(value) - 8192.0;
                        self.state.control_values[crate::song_control::BEND_INPUT] =
                            offset / if value < 8192 { 8192.0 } else { 8191.0 };
                    }
                    LiveVoiceInput::Gate(true) => {
                        self.state.gate_on(1.0);
                        self.gate = true;
                        gate_rise = true;
                    }
                    LiveVoiceInput::Gate(false) => {
                        if self.gate {
                            self.state.gate_off();
                        }
                        self.gate = false;
                    }
                }
            }
            let sample =
                self.state
                    .next_sample(&self.patch, frame - self.origin, gate_rise, None, None);
            let offset = (frame - range.start()) as usize * usize::from(self.patch.channels);
            block.samples_mut()[offset..offset + usize::from(self.patch.channels)].fill(sample);
        }
        self.cursor = range.end();
        Ok(block)
    }
}
