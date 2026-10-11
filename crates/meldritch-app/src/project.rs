//! Detached project-root presentation, derived from the existing source model.
pub use meldritch_dsl::ProjectDirectory;
use meldritch_dsl::ValidatedSong;
pub use meldritch_dsl::{PatternSignal, PatternVoiceDefinition};
use std::path::PathBuf;
use std::sync::Arc;

/// Immutable import provenance and the single accepted authored revision.
/// Editor panels hold selection/draft state, never a competing musical source.
pub(crate) struct ProjectSource {
    pub base: Arc<ValidatedSong>,
    pub source: Arc<ValidatedSong>,
}

impl super::AppController {
    pub(crate) fn synth_dirty_tracks(
        &self,
        synth: &str,
        include_legacy: bool,
    ) -> Vec<super::DirtyRange> {
        use meldritch_core::{DirtyRange, EntityId, FrameRange, TrackId};
        let source = &self.project_source.as_ref().unwrap().source;
        let mut indices = Vec::new();
        if include_legacy {
            indices.extend(
                source
                    .performance()
                    .tracks()
                    .iter()
                    .enumerate()
                    .filter(|(_, track)| track.synth_id() == synth)
                    .map(|(index, _)| index),
            );
        }
        let offset = source.performance().tracks().len();
        indices.extend(
            source
                .pattern_voices()
                .values()
                .enumerate()
                .filter(|(_, voice)| voice.synth == synth)
                .map(|(index, _)| offset + index),
        );
        indices
            .into_iter()
            .map(|index| {
                DirtyRange::new(
                    EntityId::Track(TrackId::new(index as u64 + 1)),
                    FrameRange::new(0, self.editor.timeline_frames()).unwrap(),
                )
            })
            .collect()
    }

    pub(crate) fn pattern_voice_command(
        &mut self,
        command: &super::AppCommand,
    ) -> Result<super::AppCommandResult, super::AppCommandError> {
        use super::{AppCommand, AppCommandError, AppCommandResult};
        let owner = self
            .project_source
            .as_ref()
            .ok_or_else(|| AppCommandError::Instrument("no authored project source".into()))?;
        let (id, next) = match command {
            AppCommand::CreatePatternVoice(definition) => (
                definition.id.clone(),
                owner.source.create_pattern_voice(definition.clone()),
            ),
            AppCommand::BindPatternVoice {
                voice,
                signal,
                pattern,
            } => (
                voice.clone(),
                owner
                    .source
                    .bind_pattern_voice(voice, *signal, pattern.clone()),
            ),
            _ => unreachable!(),
        };
        let next = next.map_err(AppCommandError::Instrument)?;
        meldritch_render::song_host::CompiledSongPatch::compile_initial(&next)
            .map_err(AppCommandError::Instrument)?;
        let changed = next.fingerprint() != owner.source.fingerprint();
        if changed {
            self.project_source.as_mut().unwrap().source = Arc::new(next);
        }
        Ok(AppCommandResult::PatternVoiceEdited { voice: id, changed })
    }

    /// File-free empty-project cockpit using the existing command dispatcher.
    /// The empty coordinator is scheduling scaffolding, not an authored pattern.
    /// No audio/MIDI device is opened and no samples or seed definitions exist.
    pub fn new_empty_project(
        project: &ProjectDirectory,
    ) -> Result<(Self, meldritch_audio::device_output::PlaybackEngine), String> {
        use meldritch_core::{Pattern, PatternId, ProbabilitySeed, StepIndex, Tempo, TrackId};
        use meldritch_render::{
            RenderSettings,
            coordinator::{RenderCoordinator, RenderCoordinatorConfig, SampleRenderState},
            live_edit::LivePatternEditor,
        };
        if project.composition().is_some() {
            return Err("loaded compositions use the existing song cockpit".into());
        }
        let (playback, engine) = meldritch_audio::device_output::playback_session_parts(32)
            .map_err(|e| e.to_string())?;
        let state = SampleRenderState::new(
            Pattern::new(PatternId::new(1), 4, 4).map_err(|e| e.to_string())?,
            Tempo::new(120.0, 48_000).map_err(|e| e.to_string())?,
            ProbabilitySeed::new(0),
            RenderSettings::new(1).map_err(|e| format!("{e:?}"))?,
            Arc::new(std::collections::BTreeMap::new()),
        );
        let config =
            RenderCoordinatorConfig::new(1, 96_000, 6_000, 1, std::time::Duration::from_millis(2))
                .map_err(|e| format!("{e:?}"))?;
        let coordinator = RenderCoordinator::new(
            config,
            state.pattern().clone(),
            state.tempo(),
            state.probability_seed(),
            state.settings(),
            Arc::clone(state.samples_by_note()),
            playback.status_monitor(),
        )
        .map_err(|e| format!("{e:?}"))?;
        let mut app = Self::new(
            playback,
            coordinator,
            LivePatternEditor::new(state, 96_000),
            super::Selection {
                track: TrackId::new(1),
                step: StepIndex::new(0),
            },
            256,
        );
        app.enable_project_source(project.authoring_source())?;
        app.cockpit_mode = super::CockpitMode::AllParameters;
        Ok((app, engine))
    }

    /// Load accepted authored definitions without requiring an instrument panel.
    /// This is a load boundary, not a recorded musical edit. Failed compilation
    /// leaves the existing source, selections and assignment state intact.
    pub fn enable_project_source(&mut self, song: ValidatedSong) -> Result<(), String> {
        meldritch_render::song_host::CompiledSongPatch::compile_initial(&song)?;
        let source = Arc::new(song);
        self.project_source = Some(ProjectSource {
            base: source.clone(),
            source,
        });
        self.instrument_editor = None;
        self.pattern_selection = None;
        self.assignment_editor = None;
        Ok(())
    }

    /// Detached accepted source, available independently of editor visibility.
    #[must_use]
    pub fn authored_source(&self) -> Option<Arc<ValidatedSong>> {
        self.project_source
            .as_ref()
            .map(|owner| owner.source.clone())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectOverview {
    pub root: PathBuf,
    pub composition: Option<String>,
    pub instruments: usize,
    pub patterns: usize,
}

impl ProjectOverview {
    #[must_use]
    pub fn from_project(project: &ProjectDirectory) -> Self {
        let source = project.composition();
        Self {
            root: project.root().to_owned(),
            composition: source.map(|song| song.performance().title().to_owned()),
            instruments: source.map_or(0, |song| song.synths().len() + song.samplers().len()),
            patterns: source.map_or(0, |song| {
                song.note_patterns().len() + song.parameter_patterns().len()
            }),
        }
    }
}
