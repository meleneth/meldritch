//! Native command encoding and observation over the shared session writer.
//! No scheduler or alternate musical state: decoded commands use AppController.
use crate::session::{CapturedSessionEvent, PerformanceSessionCapture, session_result_fields};
use meldritch_app::{AppCommand, AppController, CockpitMode};
use meldritch_dsl::{
    InstrumentEdit, NoteEdit, PatchCreation, PatchEdit, PatternSignal, PatternVoiceDefinition,
    ValidatedSong,
};
use serde::{Deserialize, Serialize};
use std::{sync::Arc, time::Instant};

// One bounded serialization boundary. The two conversions are generated from
// the same vocabulary so capture cannot silently discard command arguments.
macro_rules! native_commands {
    (units { $($unit:ident),* } tuples { $($tuple:ident($argument:ident: $ty:ty)),* } structs { $($name:ident { $($field:ident: $field_ty:ty),* }),* }) => {
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(tag = "kind", content = "value", rename_all = "snake_case", deny_unknown_fields)]
        pub enum NativeProjectCommand {
            $($unit,)*
            $($tuple($ty),)*
            $($name { $($field: $field_ty),* },)*
        }
        impl TryFrom<&AppCommand> for NativeProjectCommand {
            type Error = String;
            fn try_from(command: &AppCommand) -> Result<Self, String> {
                match command {
                    $(AppCommand::$unit => Ok(Self::$unit),)*
                    $(AppCommand::$tuple($argument) => Ok(Self::$tuple($argument.clone())),)*
                    $(AppCommand::$name { $($field),* } => Ok(Self::$name { $($field: $field.clone()),* }),)*
                    _ => Err("command is outside the native project capture vocabulary".into()),
                }
            }
        }
        impl From<NativeProjectCommand> for AppCommand {
            fn from(command: NativeProjectCommand) -> Self {
                match command {
                    $(NativeProjectCommand::$unit => Self::$unit,)*
                    $(NativeProjectCommand::$tuple($argument) => Self::$tuple($argument),)*
                    $(NativeProjectCommand::$name { $($field),* } => Self::$name { $($field),* },)*
                }
            }
        }
    }
}
native_commands! {
    units { Play, Stop, Rewind, TogglePatternEditor, ToggleInstrumentEditor, TogglePatchEditor, PatchBackspace, PatchSubmit, SavePattern, SaveInstrument, SaveProject }
    tuples { CreatePatch(creation: PatchCreation), EditPatch(edit: PatchEdit), CreatePatternVoice(voice: PatternVoiceDefinition), EditInstrument(edit: InstrumentEdit), SelectInstrument(id: String), PatchCharacter(character: char), PatchScroll(delta: i32), SetCockpitMode(mode: CockpitMode) }
    structs {
        ExposeOutputLevel { synth: String, control: String },
        SetOutputLevel { voice: String, control: String, value: Option<f64> },
        BindPatternVoice { voice: String, signal: PatternSignal, pattern: Option<String> },
        CreatePalettePattern { id: String, length_ticks: u64, looped: bool },
        MovePatternCursor { tick_delta: i64, pitch_delta: i32 },
        EditNote { pattern: String, edit: NoteEdit },
        SelectNote { pattern: String, id: String },
        NavigateInstrument { synth_delta: i32, parameter_delta: i32 }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeCommandEnvelope {
    pub version: u32,
    pub source_before: String,
    pub source_after: String,
    pub command: NativeProjectCommand,
}

impl NativeCommandEnvelope {
    pub fn decode(source: &str) -> Result<Self, String> {
        if source.len() > 256 * 1024 {
            return Err("native command exceeds capture size bound".into());
        }
        let envelope: Self = toml::from_str(source).map_err(|error| error.to_string())?;
        envelope.validate()?;
        Ok(envelope)
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1
            || [&self.source_before, &self.source_after]
                .iter()
                .any(|value| value.len() != 16 || !value.bytes().all(|c| c.is_ascii_hexdigit()))
        {
            return Err("unsupported native command version or invalid fingerprint".into());
        }
        Ok(())
    }
}

/// Lazy capture of all accepted history, including navigation and no-ops.
/// The host checkpoints before accepting another gesture and on clean quit.
pub struct NativeProjectCapture {
    base: Arc<ValidatedSong>,
    timeline_frames: u32,
    next_sequence: u64,
    current_fingerprint: u64,
    started: Instant,
    pub session: Option<PerformanceSessionCapture>,
}

impl NativeProjectCapture {
    pub fn new(base: Arc<ValidatedSong>, timeline_frames: u32) -> Self {
        let current_fingerprint = base.fingerprint();
        Self {
            base,
            timeline_frames,
            next_sequence: 0,
            current_fingerprint,
            started: Instant::now(),
            session: None,
        }
    }

    pub fn checkpoint(&mut self, app: &AppController) -> Result<(), String> {
        let next_sequence = self.next_sequence;
        for record in app
            .history()
            .iter()
            .filter(|record| record.sequence >= next_sequence)
        {
            if self
                .session
                .as_ref()
                .is_some_and(|session| session.clean_termination)
            {
                return Err("cannot append commands to a cleanly finished native capture".into());
            }
            // Check before creating files or changing the capture cursor.
            if record.sequence != self.next_sequence {
                return Err(
                    "native capture history gap; accepted commands were not checkpointed".into(),
                );
            }
            if record.source_before != Some(self.current_fingerprint) {
                return Err(
                    "native capture source continuity mismatch; load was not recorded".into(),
                );
            }
            let payload = NativeCommandEnvelope {
                version: 1,
                source_before: format!(
                    "{:016x}",
                    record
                        .source_before
                        .ok_or("native capture requires a project source")?
                ),
                source_after: format!(
                    "{:016x}",
                    record
                        .source_after
                        .ok_or("native capture requires a project source")?
                ),
                command: NativeProjectCommand::try_from(&record.command)?,
            };
            let encoded = toml::to_string(&payload).map_err(|error| error.to_string())?;
            if encoded.len() > 256 * 1024 {
                return Err("native command exceeds capture size bound".into());
            }
            let frame = u32::try_from(record.observed_frame)
                .map_err(|_| "legacy session observation exceeds u32 frames")?;
            if self.session.is_none() {
                let mut session =
                    PerformanceSessionCapture::create_native(&self.base, self.timeline_frames)?;
                session.started = self.started;
                self.session = Some(session);
            }
            let session = self.session.as_mut().unwrap();
            // A failed checkpoint retains its pending events. Never enqueue the
            // next event past the bound, and never duplicate an already buffered one.
            if session.events.len() >= session.event_buffer_limit {
                session.checkpoint()?;
            }
            let (kind, target_id, previous, current) =
                session_result_fields(&meldritch_app::AppInput::ToggleCockpitMode, &record.result);
            let event = CapturedSessionEvent {
                sequence: record.sequence,
                wall_offset_ms: self.started.elapsed().as_millis(),
                absolute_frame: frame,
                musical_beat: f64::from(frame)
                    / (60.0 * f64::from(self.base.performance().sample_rate())
                        / self.base.performance().bpm()),
                requested_quantization: "immediate".into(),
                actual_frame: frame,
                provenance: "native:accepted_control_observation".into(),
                input: "native_typed_command".into(),
                command: format!("{:?}", record.command),
                result: format!("{:?}", record.result),
                changed: record.changed,
                kind,
                target_id,
                previous,
                current,
                native_payload: Some(encoded),
            };
            // push_event retains the event even if its automatic checkpoint fails.
            self.next_sequence += 1;
            self.current_fingerprint = record.source_after.unwrap();
            session.push_event(event)?;
        }
        if let Some(session) = &mut self.session {
            if session.uncheckpointed_events != 0 {
                session.checkpoint()?;
            }
        }
        Ok(())
    }

    pub fn finish(&mut self, app: &AppController) -> Result<(), String> {
        self.checkpoint(app)?;
        if let Some(session) = &mut self.session {
            session.native_final_fingerprint =
                app.authored_source().map(|source| source.fingerprint());
            session.finish_clean(app)?;
        }
        Ok(())
    }
}
