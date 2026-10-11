//! Existing performance_session v1 writer, shared by TOML and Lua hosts.
//! Retains legacy fields and checkpoint behavior; not a format migration.
use meldritch_core::Tempo;
use std::{
    fs::OpenOptions,
    path::{Path, PathBuf},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug, PartialEq)]
pub struct CapturedSessionEvent {
    pub sequence: u64,
    pub wall_offset_ms: u128,
    pub absolute_frame: u32,
    pub musical_beat: f64,
    pub requested_quantization: String,
    pub actual_frame: u32,
    pub provenance: String,
    pub input: String,
    pub command: String,
    pub result: String,
    pub changed: bool,
    pub kind: String,
    pub target_id: Option<String>,
    pub previous: Option<String>,
    pub current: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FinalSessionState {
    pub cockpit_mode: String,
    pub transport_state: String,
    pub transport_position: u32,
    pub selection_track: u64,
    pub selection_step: u32,
    pub history_len: usize,
    pub curated_controls: Vec<FinalSessionControl>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FinalSessionControl {
    pub id: String,
    pub target: String,
    pub value: Option<f64>,
}

#[derive(Debug)]
pub struct PerformanceSessionCapture {
    pub path: PathBuf,
    pub temp_path: PathBuf,
    pub session_id: String,
    pub created_at_utc: String,
    pub source_performance: String,
    pub source_title: String,
    pub song_root: String,
    pub source_fingerprint: u64,
    pub timeline_frames: u32,
    pub started: Instant,
    pub events: Vec<CapturedSessionEvent>,
    pub uncheckpointed_events: usize,
    pub event_buffer_limit: usize,
    pub final_state: Option<FinalSessionState>,
    pub clean_termination: bool,
    pub termination: String,
}

const SESSION_EVENT_BUFFER_LIMIT: usize = 8;

impl PerformanceSessionCapture {
    pub fn create(
        song: &meldritch_dsl::ValidatedSong,
        timeline_frames: u32,
    ) -> Result<Self, String> {
        let timestamp = utc_session_timestamp()?;
        Self::create_at(song.root(), song, timeline_frames, &timestamp)
    }

    pub fn create_at(
        session_root: &Path,
        song: &meldritch_dsl::ValidatedSong,
        timeline_frames: u32,
        timestamp: &str,
    ) -> Result<Self, String> {
        Self::create_at_with_buffer_limit(
            session_root,
            song,
            timeline_frames,
            timestamp,
            SESSION_EVENT_BUFFER_LIMIT,
        )
    }

    pub fn create_at_with_buffer_limit(
        session_root: &Path,
        song: &meldritch_dsl::ValidatedSong,
        timeline_frames: u32,
        timestamp: &str,
        event_buffer_limit: usize,
    ) -> Result<Self, String> {
        if event_buffer_limit == 0 {
            return Err("session event buffer limit must be greater than zero".to_owned());
        }
        let directory = session_root.join("performances");
        std::fs::create_dir_all(&directory)
            .map_err(|error| format!("failed to create {}: {error}", directory.display()))?;
        let (path, session_id) = reserve_session_path(&directory, timestamp)?;
        let temp_path = path.with_extension("mlperformance.tmp");
        let mut capture = Self {
            path,
            temp_path,
            session_id,
            created_at_utc: timestamp.to_owned(),
            source_performance: song.performance().id().to_owned(),
            source_title: song.performance().title().to_owned(),
            song_root: song.root().display().to_string(),
            source_fingerprint: song.fingerprint(),
            timeline_frames,
            started: Instant::now(),
            events: Vec::new(),
            uncheckpointed_events: 0,
            event_buffer_limit,
            final_state: None,
            clean_termination: false,
            termination: "running".to_owned(),
        };
        capture.checkpoint()?;
        Ok(capture)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn record(
        &mut self,
        controller: &meldritch_app::AppController,
        input: &meldritch_app::AppInput,
        result: &meldritch_app::AppCommandResult,
        frame_count: u32,
        tempo: Tempo,
    ) -> Result<(), String> {
        self.record_observed(controller, input, result, frame_count, tempo, None)
    }

    /// Bridge Lua's vocabulary to the existing session categories and writer.
    /// The authoritative command is taken from app history, not synthesized.
    pub fn record_command(
        &mut self,
        controller: &meldritch_app::AppController,
        request: &crate::CommandRequest,
        result: &meldritch_app::AppCommandResult,
        source: &str,
        frame: u64,
        tempo: Tempo,
    ) -> Result<(), String> {
        use crate::CommandRequest as R;
        use meldritch_app::AppInput as I;
        let input = match request {
            R::EditParameter { edit } => I::EditParameter(edit.clone()),
            R::AssignFader { id, target } => I::AssignFader {
                id: id.clone(),
                target: target.clone(),
            },
            R::AssignmentPage { id } => I::AssignmentPage(id.clone()),
            R::SaveAssignments => I::SaveAssignments,
            R::CreateNotePattern { .. } | R::EditNote { .. } | R::SelectNote { .. } => {
                I::TogglePatternEditor
            }
            R::SavePattern => I::SavePattern,
            R::EditInstrument { edit } => I::EditInstrument(edit.clone()),
            R::EditPatch { edit } => I::EditPatch(edit.clone()),
            R::SaveInstrument => I::SaveInstrument,
            R::SaveProject => I::SaveProject,
            R::SetMode { .. } => I::ToggleCockpitMode,
            R::Play => I::TogglePlayback,
            R::Stop => I::Stop,
            R::Rewind => I::Rewind,
            R::SelectPage { id } => I::SelectPerformancePage(id.clone()),
            R::SelectLaneVariation { lane, variation } => I::SelectLaneVariation {
                lane_id: lane.clone(),
                variation_id: variation.clone(),
            },
            R::ToggleLaneMute { lane } => I::ToggleLaneMute(lane.clone()),
            R::ToggleLaneSolo { lane } => I::ToggleLaneSolo(lane.clone()),
            R::SetControl { id, value } => I::SetCuratedControlNormalized {
                id: id.clone(),
                value: *value,
            },
        };
        let frame = u32::try_from(frame).map_err(|_| "legacy session frame exceeds u32::MAX")?;
        self.record_observed(
            controller,
            &input,
            result,
            self.timeline_frames,
            tempo,
            Some((source, frame, format!("{request:?}"))),
        )
    }

    fn record_observed(
        &mut self,
        controller: &meldritch_app::AppController,
        input: &meldritch_app::AppInput,
        result: &meldritch_app::AppCommandResult,
        frame_count: u32,
        tempo: Tempo,
        observed: Option<(&str, u32, String)>,
    ) -> Result<(), String> {
        let view = controller.view_model();
        let latest = controller.history().back();
        let sequence = latest.map_or(self.events.len() as u64, |record| record.sequence);
        let command = latest.map_or_else(
            || "<not-recorded>".to_owned(),
            |record| format!("{:?}", record.command),
        );
        let changed = latest.is_none_or(|record| record.changed);
        let absolute_frame = observed
            .as_ref()
            .map_or(view.transport.position, |(_, frame, _)| *frame);
        let musical_beat = f64::from(absolute_frame) / tempo.frames_per_beat().max(f64::EPSILON);
        let (kind, target_id, previous, current) = session_result_fields(input, result);
        self.push_event(CapturedSessionEvent {
            sequence,
            wall_offset_ms: self.started.elapsed().as_millis(),
            absolute_frame,
            musical_beat,
            requested_quantization: "immediate".to_owned(),
            actual_frame: absolute_frame.min(frame_count.saturating_sub(1)),
            provenance: observed.as_ref().map_or_else(
                || "performer".to_owned(),
                |(source, _, _)| format!("lua:{source}"),
            ),
            input: observed.map_or_else(|| format!("{input:?}"), |(_, _, description)| description),
            command,
            result: format!("{result:?}"),
            changed,
            kind,
            target_id,
            previous,
            current,
        })
    }

    pub fn push_event(&mut self, event: CapturedSessionEvent) -> Result<(), String> {
        self.events.push(event);
        self.uncheckpointed_events += 1;
        if self.uncheckpointed_events >= self.event_buffer_limit {
            self.checkpoint()?;
        }
        Ok(())
    }

    pub fn capture_final_state(&mut self, controller: &meldritch_app::AppController) {
        self.final_state = Some(FinalSessionState::from_view(&controller.view_model()));
    }

    pub fn finish_clean(
        &mut self,
        controller: &meldritch_app::AppController,
    ) -> Result<(), String> {
        self.capture_final_state(controller);
        self.finish_clean_without_controller()
    }

    pub fn finish_clean_without_controller(&mut self) -> Result<(), String> {
        self.clean_termination = true;
        self.termination = "clean".to_owned();
        self.checkpoint()
    }

    pub fn checkpoint(&mut self) -> Result<(), String> {
        let encoded = self.to_toml();
        std::fs::write(&self.temp_path, encoded)
            .map_err(|error| format!("failed to write {}: {error}", self.temp_path.display()))?;
        std::fs::rename(&self.temp_path, &self.path).map_err(|error| {
            format!("failed to publish session {}: {error}", self.path.display())
        })?;
        self.uncheckpointed_events = 0;
        Ok(())
    }

    pub fn to_toml(&self) -> String {
        let mut out = String::new();
        out.push_str("[meldritch]\n");
        out.push_str("kind = \"performance_session\"\n");
        out.push_str("version = 1\n\n");
        out.push_str("[session]\n");
        push_string(&mut out, "id", &self.session_id);
        push_string(&mut out, "created_at_utc", &self.created_at_utc);
        push_string(&mut out, "source_performance", &self.source_performance);
        push_string(&mut out, "source_title", &self.source_title);
        push_string(&mut out, "song_root", &self.song_root);
        push_string(
            &mut out,
            "source_fingerprint",
            &format!("{:016x}", self.source_fingerprint),
        );
        out.push_str(&format!("timeline_frames = {}\n", self.timeline_frames));
        out.push_str(&format!(
            "event_buffer_limit = {}\n",
            self.event_buffer_limit
        ));
        out.push_str(&format!("clean_termination = {}\n", self.clean_termination));
        push_string(&mut out, "termination", &self.termination);
        out.push('\n');
        if let Some(final_state) = &self.final_state {
            out.push_str("[final_state]\n");
            push_string(&mut out, "cockpit_mode", &final_state.cockpit_mode);
            push_string(&mut out, "transport_state", &final_state.transport_state);
            out.push_str(&format!(
                "transport_position = {}\n",
                final_state.transport_position
            ));
            out.push_str(&format!(
                "selection_track = {}\n",
                final_state.selection_track
            ));
            out.push_str(&format!(
                "selection_step = {}\n",
                final_state.selection_step
            ));
            out.push_str(&format!("history_len = {}\n\n", final_state.history_len));
            for control in &final_state.curated_controls {
                out.push_str("[[final_controls]]\n");
                push_string(&mut out, "id", &control.id);
                push_string(&mut out, "target", &control.target);
                if let Some(value) = control.value {
                    out.push_str(&format!("value = {:.12}\n", value));
                }
                out.push('\n');
            }
        }
        for event in &self.events {
            out.push_str("[[events]]\n");
            out.push_str(&format!("sequence = {}\n", event.sequence));
            out.push_str(&format!("wall_offset_ms = {}\n", event.wall_offset_ms));
            out.push_str(&format!("absolute_frame = {}\n", event.absolute_frame));
            out.push_str(&format!("musical_beat = {:.9}\n", event.musical_beat));
            push_string(
                &mut out,
                "requested_quantization",
                &event.requested_quantization,
            );
            out.push_str(&format!("actual_frame = {}\n", event.actual_frame));
            push_string(&mut out, "provenance", &event.provenance);
            push_string(&mut out, "input", &event.input);
            push_string(&mut out, "command", &event.command);
            push_string(&mut out, "result", &event.result);
            out.push_str(&format!("changed = {}\n", event.changed));
            push_string(&mut out, "kind", &event.kind);
            if let Some(target_id) = &event.target_id {
                push_string(&mut out, "target_id", target_id);
            }
            if let Some(previous) = &event.previous {
                push_string(&mut out, "previous", previous);
            }
            if let Some(current) = &event.current {
                push_string(&mut out, "current", current);
            }
            out.push('\n');
        }
        out
    }
}

impl FinalSessionState {
    pub fn from_view(view: &meldritch_app::AppViewModel) -> Self {
        Self {
            cockpit_mode: format!("{:?}", view.cockpit_mode),
            transport_state: format!("{:?}", view.transport.state),
            transport_position: view.transport.position,
            selection_track: view.diagnostics.selection.track.raw(),
            selection_step: view.diagnostics.selection.step.raw(),
            history_len: view.diagnostics.history_len,
            curated_controls: view
                .curated_controls
                .iter()
                .map(|control| FinalSessionControl {
                    id: control.id.clone(),
                    target: control.target.clone(),
                    value: control.value,
                })
                .collect(),
        }
    }
}

pub fn session_result_fields(
    input: &meldritch_app::AppInput,
    result: &meldritch_app::AppCommandResult,
) -> (String, Option<String>, Option<String>, Option<String>) {
    match result {
        meldritch_app::AppCommandResult::ParameterEdited { target, .. } => (
            "authored_parameter".into(),
            Some(target.identity()),
            None,
            None,
        ),
        meldritch_app::AppCommandResult::AssignmentsChanged { .. } => {
            ("control_assignment".into(), None, None, None)
        }
        meldritch_app::AppCommandResult::AssignmentSaveRequested => {
            ("authored_controls_save".into(), None, None, None)
        }
        meldritch_app::AppCommandResult::PatternEdited {
            pattern,
            selected_note,
            ..
        } => (
            "note_pattern".into(),
            Some(pattern.clone()),
            None,
            selected_note.clone(),
        ),
        meldritch_app::AppCommandResult::PatternSelectionChanged => {
            ("note_selection".into(), None, None, None)
        }
        meldritch_app::AppCommandResult::PatternSaveRequested => {
            ("authored_pattern_save".into(), None, None, None)
        }
        meldritch_app::AppCommandResult::InstrumentEdited { edit, previous, .. } => (
            "instrument_definition".to_owned(),
            Some(format!("{:?}", edit.target)),
            Some(format!("{previous:?}")),
            Some(format!("{:?}", edit.value)),
        ),
        meldritch_app::AppCommandResult::PatchEdited {
            synth, operations, ..
        } => (
            "patch_definition".into(),
            Some(synth.clone()),
            None,
            Some(format!("{operations:?}")),
        ),
        meldritch_app::AppCommandResult::InstrumentEditorChanged => {
            ("instrument_selection".into(), None, None, None)
        }
        meldritch_app::AppCommandResult::ProjectSaveRequested => {
            ("authored_project_save".into(), None, None, None)
        }
        meldritch_app::AppCommandResult::InstrumentSaveRequested => {
            ("authored_save".into(), None, None, None)
        }
        meldritch_app::AppCommandResult::CuratedControlAdjusted {
            id,
            previous,
            current,
        } => (
            "curated_control".to_owned(),
            Some(id.clone()),
            Some(format!("{previous:.12}")),
            Some(format!("{current:.12}")),
        ),
        meldritch_app::AppCommandResult::CockpitModeChanged { previous, current } => (
            "cockpit_mode".to_owned(),
            None,
            Some(format!("{previous:?}")),
            Some(format!("{current:?}")),
        ),
        meldritch_app::AppCommandResult::SelectionChanged { previous, current } => (
            "selection".to_owned(),
            None,
            Some(format!("{previous:?}")),
            Some(format!("{current:?}")),
        ),
        meldritch_app::AppCommandResult::TransportQueued => (
            "transport".to_owned(),
            Some(
                match input {
                    meldritch_app::AppInput::TogglePlayback => "toggle_playback",
                    meldritch_app::AppInput::Stop => "stop",
                    meldritch_app::AppInput::Rewind => "rewind",
                    _ => "transport",
                }
                .to_owned(),
            ),
            None,
            None,
        ),
        meldritch_app::AppCommandResult::Edit(_) => (
            "parameter_edit".to_owned(),
            Some(format!("{input:?}")),
            None,
            Some(format!("{result:?}")),
        ),
        meldritch_app::AppCommandResult::SynthUpdated { .. } => (
            "synth_control".to_owned(),
            Some(format!("{input:?}")),
            None,
            Some(format!("{result:?}")),
        ),
        meldritch_app::AppCommandResult::PerformanceFxUpdated(_) => (
            "performance_fx".to_owned(),
            Some(format!("{input:?}")),
            None,
            Some(format!("{result:?}")),
        ),
        meldritch_app::AppCommandResult::TransformCreated { .. } => (
            "transform".to_owned(),
            Some(format!("{input:?}")),
            None,
            Some(format!("{result:?}")),
        ),
        meldritch_app::AppCommandResult::AudioSourceSwitched { transformed } => (
            "audio_source".to_owned(),
            None,
            None,
            Some(if *transformed { "transformed" } else { "live" }.to_owned()),
        ),
        meldritch_app::AppCommandResult::PerformanceQueued(_) => (
            "performance_queue".to_owned(),
            Some(format!("{input:?}")),
            None,
            Some(format!("{result:?}")),
        ),
        meldritch_app::AppCommandResult::PerformancePageSelected { .. } => (
            "performance_page".to_owned(),
            Some(format!("{input:?}")),
            None,
            Some(format!("{result:?}")),
        ),
        meldritch_app::AppCommandResult::LaneVariationSelected { lane_id, .. } => (
            "lane_variation".to_owned(),
            Some(lane_id.clone()),
            None,
            Some(format!("{result:?}")),
        ),
        meldritch_app::AppCommandResult::LanePatternBankSelected { lane_id, .. } => (
            "lane_pattern_bank".to_owned(),
            Some(lane_id.clone()),
            None,
            Some(format!("{result:?}")),
        ),
        meldritch_app::AppCommandResult::PerformanceModifierChanged {
            modifier_id,
            active,
        } => (
            "performance_modifier".to_owned(),
            Some(modifier_id.clone()),
            None,
            Some(active.to_string()),
        ),
        meldritch_app::AppCommandResult::LaneOctaveSet {
            lane_id, current, ..
        } => (
            "lane_octave".to_owned(),
            Some(lane_id.clone()),
            None,
            Some(current.to_string()),
        ),
        meldritch_app::AppCommandResult::LaneMuteToggled { lane_id, muted } => (
            "lane_mute".to_owned(),
            Some(lane_id.clone()),
            None,
            Some(muted.to_string()),
        ),
        meldritch_app::AppCommandResult::LaneSoloToggled { lane_id, soloed } => (
            "lane_solo".to_owned(),
            Some(lane_id.clone()),
            None,
            Some(soloed.to_string()),
        ),
        meldritch_app::AppCommandResult::PerformanceCancelled(_) => (
            "performance_cancel".to_owned(),
            Some(format!("{input:?}")),
            None,
            Some(format!("{result:?}")),
        ),
    }
}

pub fn reserve_session_path(
    directory: &Path,
    timestamp: &str,
) -> Result<(PathBuf, String), String> {
    for suffix in 0..1_000_u16 {
        let session_id = if suffix == 0 {
            format!("session-{timestamp}")
        } else {
            format!("session-{timestamp}-{suffix:03}")
        };
        let path = directory.join(format!("{session_id}.mlperformance"));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(_) => return Ok((path, session_id)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(format!("failed to reserve {}: {error}", path.display()));
            }
        }
    }
    Err(format!(
        "could not reserve a unique session filename in {} for {timestamp}",
        directory.display()
    ))
}

pub fn utc_session_timestamp() -> Result<String, String> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock is before the Unix epoch: {error}"))?;
    Ok(format_unix_timestamp_utc(elapsed.as_secs()))
}

pub fn format_unix_timestamp_utc(seconds: u64) -> String {
    let days = (seconds / 86_400) as i64;
    let seconds_of_day = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    format!("{year:04}{month:02}{day:02}T{hour:02}{minute:02}{second:02}Z")
}

pub fn civil_from_days(days_since_unix_epoch: i64) -> (i64, u64, u64) {
    let days = days_since_unix_epoch + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    let year = year + i64::from(month <= 2);
    (year, month as u64, day as u64)
}

pub fn push_string(out: &mut String, key: &str, value: &str) {
    out.push_str(key);
    out.push_str(" = ");
    out.push_str(&toml_string(value));
    out.push('\n');
}

pub fn toml_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character.is_control() => {
                escaped.push_str(&format!("\\u{:04x}", character as u32));
            }
            character => escaped.push(character),
        }
    }
    escaped.push('"');
    escaped
}
