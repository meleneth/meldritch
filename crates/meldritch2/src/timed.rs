use std::sync::Arc;

use crate::compiler::Machine;

pub const MAX_EVENTS: usize = 64;
pub const MAX_FRAMES_PER_RENDER: usize = 1024;

/// A prepared note state change at an absolute audio frame.
#[derive(Clone, Copy, Debug)]
pub struct TimedNoteState {
    pub frame: u64,
    pub channel: u8,
    pub pitch_hz: f64,
    pub gate: bool,
}

/// A prepared note state with an explicit unit-interval velocity.
#[derive(Clone, Copy, Debug)]
pub struct TimedVelocityState {
    pub note: TimedNoteState,
    pub velocity: f64,
}

#[derive(Clone, Copy, Debug)]
struct ResolvedEvent {
    frame: u64,
    channel: u8,
    pitch_index: usize,
    gate_index: usize,
    velocity_index: Option<usize>,
    pitch_hz: f64,
    gate: bool,
    velocity: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimedRenderError {
    WrongMachine,
    InvalidBufferSize,
    FrameOverflow,
}

/// Bounded event plan resolved against one compiled machine revision.
#[derive(Debug)]
pub struct TimedNotePlan {
    revision: Arc<()>,
    events: Vec<ResolvedEvent>,
    next_event: usize,
    cursor: u64,
    active: [Option<ResolvedEvent>; 16],
}

impl TimedNotePlan {
    pub fn prepare(
        machine: &Machine,
        start_frame: u64,
        events: &[TimedNoteState],
    ) -> Result<Self, String> {
        Self::prepare_events(
            machine,
            start_frame,
            events.iter().copied().map(|note| (note, 1.0)),
        )
    }

    pub fn prepare_with_velocity(
        machine: &Machine,
        start_frame: u64,
        events: &[TimedVelocityState],
    ) -> Result<Self, String> {
        Self::prepare_events(
            machine,
            start_frame,
            events.iter().map(|event| (event.note, event.velocity)),
        )
    }

    fn prepare_events(
        machine: &Machine,
        start_frame: u64,
        events: impl ExactSizeIterator<Item = (TimedNoteState, f64)>,
    ) -> Result<Self, String> {
        if events.len() > MAX_EVENTS {
            return Err(format!(
                "timed note plan permits at most {MAX_EVENTS} events"
            ));
        }
        let mut previous = start_frame;
        let mut resolved = Vec::with_capacity(events.len());
        for (event, velocity) in events {
            if event.frame < previous {
                return Err("timed note frames must be nondecreasing and at or after start".into());
            }
            if !velocity.is_finite() || !(0.0..=1.0).contains(&velocity) {
                return Err("timed note velocity must be finite and within 0..1".into());
            }
            let (pitch_index, gate_index) =
                machine.resolve_timed_note(event.channel, event.pitch_hz)?;
            let velocity_index = machine.timed_note_target(event.channel)?.velocity_index;
            resolved.push(ResolvedEvent {
                frame: event.frame,
                channel: event.channel,
                pitch_index,
                gate_index,
                velocity_index,
                pitch_hz: event.pitch_hz,
                gate: event.gate,
                velocity,
            });
            previous = event.frame;
        }
        Ok(Self {
            revision: machine.revision_token(),
            events: resolved,
            next_event: 0,
            cursor: start_frame,
            active: [None; 16],
        })
    }

    pub fn cursor(&self) -> u64 {
        self.cursor
    }

    /// Render one bounded block; events at the exclusive end wait for the next call.
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
        while let Some(event) = self.events.get(self.next_event) {
            if event.frame >= end {
                break;
            }
            let offset = (event.frame - self.cursor) as usize;
            machine.process_prepared(&mut buffer[rendered..offset]);
            machine.apply_timed_note(
                event.pitch_index,
                event.gate_index,
                event.pitch_hz,
                event.gate,
            );
            machine.apply_timed_velocity(event.velocity_index, event.velocity);
            self.active[usize::from(event.channel - 1)] = Some(*event);
            rendered = offset;
            self.next_event += 1;
        }
        machine.process_prepared(&mut buffer[rendered..]);
        self.cursor = end;
        Ok(())
    }
}
