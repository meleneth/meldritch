//! Detached project-root presentation, derived from the existing source model.
use meldritch_dsl::ProjectDirectory;
use meldritch_dsl::ValidatedSong;
use std::path::PathBuf;
use std::sync::Arc;

/// Immutable import provenance and the single accepted authored revision.
/// Editor panels hold selection/draft state, never a competing musical source.
pub(crate) struct ProjectSource {
    pub base: Arc<ValidatedSong>,
    pub source: Arc<ValidatedSong>,
}

impl super::AppController {
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
