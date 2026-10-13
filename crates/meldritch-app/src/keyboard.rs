//! Bounded runtime Pitch/Gate ownership, independent of authored note patterns.
use crate::{AppCommand, AppCommandError, AppCommandResult, AppController};
use meldritch_render::song_render::{LiveVoiceInput, compile_pattern_voice};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum KeyboardResetReason {
    Disconnect,
    Overflow,
    Panic,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KeyboardView {
    pub voice: Option<String>,
    pub epoch: u64,
    pub frame: u64,
    pub pitch: Option<u8>,
    pub velocity: Option<u8>,
    pub bend: Option<u16>,
    pub sustain: bool,
    pub gate: bool,
    pub held_keys: usize,
}

#[derive(Default)]
pub(crate) struct KeyboardState {
    pub view: KeyboardView,
    // Ordered oldest to newest; channel is part of identity. Never above 128.
    held: Vec<(u8, u8, u8)>,
}

impl KeyboardState {
    pub const MAX_HELD: usize = 128;

    pub(crate) fn invalidate(&mut self) {
        self.held.clear();
        self.view.voice = None;
        self.view.gate = false;
        self.view.held_keys = 0;
        self.view.bend = None;
        self.view.sustain = false;
        self.view.epoch = self.view.epoch.saturating_add(1);
    }
}

impl AppController {
    pub fn keyboard_transport_stopped(&self) -> bool {
        !self.transport_requested_playing
            && self.playback.command_diagnostics().applied >= self.transport_commands_submitted
            && self.playback.status_monitor().snapshot().state
                == meldritch_audio::transport::TransportState::Stopped
    }
    #[must_use]
    pub fn keyboard(&self) -> &KeyboardView {
        &self.keyboard.view
    }

    pub(crate) fn keyboard_command(
        &mut self,
        command: &AppCommand,
    ) -> Result<AppCommandResult, AppCommandError> {
        let fail = |message: &str| AppCommandError::Instrument(message.into());
        let frame = match command {
            AppCommand::BindKeyboard { frame, .. }
            | AppCommand::KeyboardKey { frame, .. }
            | AppCommand::KeyboardBend { frame, .. }
            | AppCommand::KeyboardSustain { frame, .. }
            | AppCommand::ResetKeyboard { frame, .. } => *frame,
            _ => unreachable!(),
        };
        if frame < self.keyboard.view.frame {
            return Err(fail("keyboard frame precedes accepted runtime frame"));
        }
        let previous = self.keyboard.view.clone();
        let mut outputs = Vec::new();
        match command {
            AppCommand::BindKeyboard { voice, .. } => {
                if let Some(id) = voice {
                    if !self.keyboard_transport_stopped() {
                        return Err(fail("stop transport before keyboard audition"));
                    }
                    let source = self
                        .authored_source()
                        .ok_or_else(|| fail("no project source"))?;
                    source
                        .native_project_definition()
                        .map_err(AppCommandError::Instrument)?;
                    if !source.active_pattern_voices().any(|voice| &voice.id == id) {
                        return Err(fail("keyboard target is not an active pattern voice"));
                    }
                    let patch = compile_pattern_voice(&source, &source.pattern_voices()[id])
                        .map_err(|e| fail(&e.to_string()))?;
                    patch
                        .live_renderer(60, frame)
                        .map_err(|e| fail(&e.to_string()))?;
                }
                let epoch = previous
                    .epoch
                    .checked_add(1)
                    .ok_or_else(|| fail("keyboard epoch exhausted"))?;
                if previous.gate {
                    outputs.push(LiveVoiceInput::Gate(false));
                }
                if previous.sustain {
                    outputs.push(LiveVoiceInput::Sustain(false));
                }
                if previous.voice.is_some() && previous.bend.is_some_and(|value| value != 8192) {
                    outputs.push(LiveVoiceInput::Bend(8192));
                }
                self.keyboard.held.clear();
                self.keyboard.view = KeyboardView {
                    voice: voice.clone(),
                    epoch,
                    frame,
                    ..KeyboardView::default()
                };
            }
            AppCommand::KeyboardKey {
                epoch,
                channel,
                note,
                velocity,
                pressed,
                ..
            } => {
                if *epoch != previous.epoch || previous.voice.is_none() {
                    return Err(fail("keyboard event has no current binding"));
                }
                if *channel > 15 || *note > 127 || *velocity > 127 {
                    return Err(fail("invalid keyboard channel, pitch or velocity"));
                }
                let index = self
                    .keyboard
                    .held
                    .iter()
                    .position(|held| held.0 == *channel && held.1 == *note);
                if *pressed {
                    if index.is_none() && self.keyboard.held.len() == KeyboardState::MAX_HELD {
                        return Err(fail(
                            "keyboard held-key bound exceeded; explicit overflow reset required",
                        ));
                    }
                    if let Some(index) = index {
                        self.keyboard.held.remove(index);
                    }
                    self.keyboard.held.push((*channel, *note, *velocity));
                    self.keyboard.view.pitch = Some(*note);
                    self.keyboard.view.velocity = Some(*velocity);
                    self.keyboard.view.gate = true;
                    outputs.extend([
                        LiveVoiceInput::Pitch(*note),
                        LiveVoiceInput::Velocity(*velocity),
                        LiveVoiceInput::Gate(true),
                    ]);
                } else if let Some(index) = index {
                    let was_winner = index + 1 == self.keyboard.held.len();
                    self.keyboard.held.remove(index);
                    if was_winner {
                        if let Some((_, pitch, velocity)) = self.keyboard.held.last() {
                            self.keyboard.view.pitch = Some(*pitch);
                            self.keyboard.view.velocity = Some(*velocity);
                            outputs.extend([
                                LiveVoiceInput::Pitch(*pitch),
                                LiveVoiceInput::Velocity(*velocity),
                            ]);
                        } else {
                            self.keyboard.view.gate = false;
                            outputs.push(LiveVoiceInput::Gate(false));
                        }
                    }
                }
                self.keyboard.view.frame = frame;
                self.keyboard.view.held_keys = self.keyboard.held.len();
            }
            AppCommand::KeyboardBend {
                epoch,
                channel,
                value,
                ..
            } => {
                if *epoch != previous.epoch || previous.voice.is_none() {
                    return Err(fail("keyboard bend has no current binding"));
                }
                if *channel > 15 || *value > 16383 {
                    return Err(fail("invalid keyboard bend channel or value"));
                }
                self.keyboard.view.bend = Some(*value);
                self.keyboard.view.frame = frame;
                outputs.push(LiveVoiceInput::Bend(*value));
            }
            AppCommand::KeyboardSustain {
                epoch,
                channel,
                high,
                ..
            } => {
                if *epoch != previous.epoch || previous.voice.is_none() {
                    return Err(fail("keyboard sustain has no current binding"));
                }
                if *channel > 15 {
                    return Err(fail("invalid keyboard sustain channel"));
                }
                self.keyboard.view.sustain = *high;
                self.keyboard.view.frame = frame;
                outputs.push(LiveVoiceInput::Sustain(*high));
            }
            AppCommand::ResetKeyboard { epoch, .. } => {
                if *epoch != previous.epoch || previous.voice.is_none() {
                    return Err(fail("keyboard reset has no current binding"));
                }
                let next_epoch = previous
                    .epoch
                    .checked_add(1)
                    .ok_or_else(|| fail("keyboard epoch exhausted"))?;
                if previous.gate {
                    outputs.push(LiveVoiceInput::Gate(false));
                }
                if previous.sustain {
                    outputs.push(LiveVoiceInput::Sustain(false));
                }
                if previous.bend.is_some_and(|value| value != 8192) {
                    outputs.push(LiveVoiceInput::Bend(8192));
                }
                self.keyboard.view.bend = previous.bend.map(|_| 8192);
                self.keyboard.view.sustain = false;
                self.keyboard.held.clear();
                self.keyboard.view.gate = false;
                self.keyboard.view.held_keys = 0;
                self.keyboard.view.epoch = next_epoch;
                self.keyboard.view.frame = frame;
            }
            _ => unreachable!(),
        }
        Ok(AppCommandResult::KeyboardUpdated {
            output_voice: if matches!(command, AppCommand::BindKeyboard { .. }) {
                previous.voice.clone()
            } else {
                self.keyboard.view.voice.clone()
            },
            previous: Box::new(previous),
            current: Box::new(self.keyboard.view.clone()),
            outputs,
        })
    }
}
