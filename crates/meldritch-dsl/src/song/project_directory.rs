//! Project-root discovery around the existing semantic importer.
//!
//! A root without an entry has no selected composition. This adapter does not
//! manufacture definitions, create files or initialize version control.
use super::*;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub struct ProjectDirectory {
    root: PathBuf,
    composition: Option<ValidatedSong>,
}

impl ProjectDirectory {
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The legacy entrypoint's validated definitions, without another music model.
    #[must_use]
    pub fn composition(&self) -> Option<&ValidatedSong> {
        self.composition.as_ref()
    }

    /// File-free accepted definition storage. Empty scheduling metadata is only
    /// a legacy model projection; it does not create a selected composition.
    #[must_use]
    pub fn authoring_source(&self) -> ValidatedSong {
        if let Some(song) = &self.composition {
            return song.clone();
        }
        empty_project_source(&self.root)
    }
}

pub(super) fn empty_project_source(root: &Path) -> ValidatedSong {
    let mut source = ValidatedSong {
        root: root.to_path_buf(),
        performance: PerformanceDefinition {
            id: "project".into(),
            title: "Untitled project".into(),
            bpm: 120.0,
            sample_rate: 48_000,
            midi_devices: vec![],
            tracks: vec![],
            lanes: vec![],
            pages: vec![],
            controls: vec![],
            modifiers: vec![],
            modifier_controls: vec![],
            actions: vec![],
            ui: PerformanceUiDefinition::default(),
        },
        synths: BTreeMap::new(),
        samplers: BTreeMap::new(),
        sample_banks: BTreeMap::new(),
        note_patterns: BTreeMap::new(),
        parameter_patterns: BTreeMap::new(),
        dsps: BTreeMap::new(),
        pattern_voices: BTreeMap::new(),
        fingerprint: 0,
    };
    source.refresh_authoring_fingerprint();
    source
}

pub fn load_project_directory(path: impl AsRef<Path>) -> Result<ProjectDirectory, SongLoadError> {
    let requested = path.as_ref();
    let root = requested.canonicalize().map_err(|error| {
        SongLoadError::one(requested, format!("cannot open project directory: {error}"))
    })?;
    if !root.is_dir() {
        return Err(SongLoadError::one(&root, "project path is not a directory"));
    }
    let entry = root.join("main.mlperformance");
    let composition = match fs::symlink_metadata(&entry) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            return Err(SongLoadError::one(
                &entry,
                format!("cannot inspect composition entry: {error}"),
            ));
        }
        Ok(_) => {
            let resolved = entry.canonicalize().map_err(|error| {
                SongLoadError::one(&entry, format!("cannot resolve composition entry: {error}"))
            })?;
            if !resolved.starts_with(&root) {
                return Err(SongLoadError::one(
                    &entry,
                    "composition entry escapes project root",
                ));
            }
            Some(load_song_directory(&root)?)
        }
    };
    Ok(ProjectDirectory { root, composition })
}
