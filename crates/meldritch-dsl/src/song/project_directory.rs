//! Project-root discovery around the existing semantic importer.
//!
//! A root without an entry has no selected composition. This adapter does not
//! manufacture definitions, create files or initialize version control.
use super::{SongLoadError, ValidatedSong, load_song_directory};
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
