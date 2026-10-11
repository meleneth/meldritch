//! Detached project-root presentation, derived from the existing source model.
use meldritch_dsl::ProjectDirectory;
use std::path::PathBuf;

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
