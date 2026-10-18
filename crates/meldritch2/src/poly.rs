//! Prepared bounded note allocation on a single MIDI channel.

use std::sync::Arc;

use crate::compiler::{MAX_POLY_VOICES, Machine, PolyVoiceTarget};
use crate::timed::{MAX_EVENTS, MAX_FRAMES_PER_RENDER, TimedRenderError};

#[derive(Clone, Copy, Debug)]
pub enum PolyNoteEvent {
    On {
        frame: u64,
        channel: u8,
        key: u8,
        velocity: u8,
    },
    Off {
        frame: u64,
        channel: u8,
        key: u8,
    },
}

impl PolyNoteEvent {
    fn frame(self) -> u64 {
        match self {
            Self::On { frame, .. } | Self::Off { frame, .. } => frame,
        }
    }

    fn channel(self) -> u8 {
        match self {
            Self::On { channel, .. } | Self::Off { channel, .. } => channel,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct ResolvedEvent {
    frame: u64,
    channel_index: usize,
    voice_index: usize,
    target: PolyVoiceTarget,
    pitch_hz: f64,
    gate: bool,
    velocity: f64,
}

#[derive(Debug)]
pub struct PolyNotePlan {
    revision: Arc<()>,
    events: Vec<ResolvedEvent>,
    next_event: usize,
    cursor: u64,
    active: [[Option<ResolvedEvent>; MAX_POLY_VOICES]; 16],
}

impl PolyNotePlan {
    pub fn prepare(
        machine: &Machine,
        start_frame: u64,
        events: &[PolyNoteEvent],
    ) -> Result<Self, String> {
        if events.len() > MAX_EVENTS {
            return Err(format!(
                "polyphonic note plan permits at most {MAX_EVENTS} events"
            ));
        }
        let mut held = [[None; MAX_POLY_VOICES]; 16];
        let mut states = [[None; MAX_POLY_VOICES]; 16];
        let mut resolved = Vec::with_capacity(events.len());
        let mut previous = start_frame;
        for &event in events {
            let frame = event.frame();
            if frame < previous {
                return Err(
                    "polyphonic note frames must be nondecreasing and at or after start".into(),
                );
            }
            previous = frame;
            let channel = event.channel();
            let targets = machine.poly_note_targets(channel)?;
            let channel_index = usize::from(channel - 1);
            match event {
                PolyNoteEvent::On { key, velocity, .. } if velocity != 0 => {
                    if key > 127 || velocity > 127 {
                        return Err("polyphonic note key and velocity must be MIDI bytes".into());
                    }
                    let voice_index = held[channel_index]
                        .iter()
                        .position(|held_key| *held_key == Some(key))
                        .or_else(|| {
                            held[channel_index]
                                .iter()
                                .zip(targets.iter())
                                .position(|(held, target)| held.is_none() && target.is_some())
                        })
                        .ok_or_else(|| format!("polyphonic channel {channel} has no free voice"))?;
                    held[channel_index][voice_index] = Some(key);
                    let state = ResolvedEvent {
                        frame,
                        channel_index,
                        voice_index,
                        target: targets[voice_index].expect("allocated mapped voice"),
                        pitch_hz: 440.0 * 2.0_f64.powf((f64::from(key) - 69.0) / 12.0),
                        gate: true,
                        velocity: f64::from(velocity) / 127.0,
                    };
                    states[channel_index][voice_index] = Some(state);
                    resolved.push(state);
                }
                PolyNoteEvent::On { key, .. } | PolyNoteEvent::Off { key, .. } => {
                    if key > 127 {
                        return Err("polyphonic note key must be a MIDI byte".into());
                    }
                    if let Some(voice_index) = held[channel_index]
                        .iter()
                        .position(|held_key| *held_key == Some(key))
                    {
                        held[channel_index][voice_index] = None;
                        let mut state =
                            states[channel_index][voice_index].expect("held voice has state");
                        state.frame = frame;
                        state.gate = false;
                        states[channel_index][voice_index] = Some(state);
                        resolved.push(state);
                    }
                }
            }
        }
        Ok(Self {
            revision: machine.revision_token(),
            events: resolved,
            next_event: 0,
            cursor: start_frame,
            active: [[None; MAX_POLY_VOICES]; 16],
        })
    }

    pub fn cursor(&self) -> u64 {
        self.cursor
    }

    pub fn render_next(
        &mut self,
        machine: &mut Machine,
        buffer: &mut [f64],
    ) -> Result<(), TimedRenderError> {
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
        for state in self.active.iter().flatten().flatten() {
            Self::apply(machine, *state);
        }
        let mut rendered = 0;
        while let Some(state) = self.events.get(self.next_event).copied() {
            if state.frame >= end {
                break;
            }
            let offset = (state.frame - self.cursor) as usize;
            machine.process_prepared(&mut buffer[rendered..offset]);
            Self::apply(machine, state);
            self.active[state.channel_index][state.voice_index] = Some(state);
            rendered = offset;
            self.next_event += 1;
        }
        machine.process_prepared(&mut buffer[rendered..]);
        self.cursor = end;
        Ok(())
    }

    fn apply(machine: &mut Machine, state: ResolvedEvent) {
        machine.apply_timed_note(
            state.target.pitch_index,
            state.target.gate_index,
            state.pitch_hz,
            state.gate,
        );
        machine.apply_timed_velocity(Some(state.target.velocity_index), state.velocity);
    }
}
