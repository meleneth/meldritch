//! Self-contained authored project v2; Lua only describes Rust-validated source.
use crate::{Result, error};
use meldritch_dsl::{NativeProjectDefinition, ValidatedSong};
use mlua::LuaSerdeExt;
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub const NATIVE_PROJECT_ENTRY: &str = "main.project.lua";

/// Host-owned disk boundary; accepted musical state remains in AppController.
pub struct NativeProjectStore {
    root: PathBuf,
    expected: Option<Vec<u8>>,
    source: Option<ValidatedSong>,
    saved_fingerprint: Option<u64>,
}

fn entry_bytes(root: &Path) -> Result<Option<Vec<u8>>> {
    let path = root.join(NATIVE_PROJECT_ENTRY);
    let metadata = match fs::symlink_metadata(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        result => result.map_err(error)?,
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(
            "native project entry must be an ordinary file, not a symlink or directory".into(),
        );
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(error)?
        .take(256 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(error)?;
    if bytes.len() > 256 * 1024 {
        return Err("native project entry exceeds source size limit".into());
    }
    Ok(Some(bytes))
}

impl NativeProjectStore {
    /// Read-only discovery. Lua stays in the non-realtime control crate.
    pub fn open(project: &meldritch_dsl::ProjectDirectory) -> Result<Self> {
        let root = project.root().to_owned();
        let expected = entry_bytes(&root)?;
        if expected.is_some() && project.composition().is_some() {
            return Err(
                "ambiguous project: main.project.lua and main.mlperformance both exist".into(),
            );
        }
        let source = expected
            .as_deref()
            .map(|bytes| load_native_project(&root, std::str::from_utf8(bytes).map_err(error)?))
            .transpose()?;
        let saved_fingerprint = source.as_ref().map(ValidatedSong::fingerprint);
        Ok(Self {
            root,
            expected,
            source,
            saved_fingerprint,
        })
    }

    pub fn source(&self) -> Option<&ValidatedSong> {
        self.source.as_ref()
    }
    pub fn saved_fingerprint(&self) -> Option<u64> {
        self.saved_fingerprint
    }

    fn check_expected(&self) -> Result<()> {
        match fs::symlink_metadata(self.root.join("main.mlperformance")) {
            Ok(_) => return Err("native save refuses an existing legacy composition entry".into()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("cannot inspect legacy entry: {e}")),
        }
        if entry_bytes(&self.root)? != self.expected {
            return Err("native project entry changed externally; reopen before saving".into());
        }
        Ok(())
    }

    pub fn save(&mut self, source: &ValidatedSong) -> Result<PathBuf> {
        if source.root() != self.root {
            return Err("native save source belongs to a different project root".into());
        }
        let encoded = encode_native_project(source)?;
        self.check_expected()?;
        let temporary = self.root.join(".main.project.lua.tmp");
        // create_new never follows or overwrites a foreign/stale temporary file.
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(error)?;
        let destination = self.root.join(NATIVE_PROJECT_ENTRY);
        let result = (|| {
            file.write_all(encoded.as_bytes()).map_err(error)?;
            file.sync_all().map_err(error)?;
            drop(file);
            self.check_expected()?;
            fs::rename(&temporary, &destination).map_err(error)?;
            Ok(destination)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        if result.is_ok() {
            self.expected = Some(encoded.into_bytes());
            self.saved_fingerprint = Some(source.fingerprint());
            self.source = Some(source.clone());
        }
        result
    }
}

fn validate_executable(source: &ValidatedSong) -> Result<()> {
    for id in source.synths().keys() {
        meldritch_render::song_render::validate_palette_synth(source, id).map_err(error)?;
    }
    meldritch_render::song_host::CompiledSongPatch::compile_initial(source)?;
    Ok(())
}

pub fn load_native_project(root: impl AsRef<Path>, lua_source: &str) -> Result<ValidatedSong> {
    let (lua, _) = crate::constrained_vm(lua_source)?;
    let value = lua
        .load(lua_source)
        .set_name("self-contained authored project")
        .set_mode(mlua::chunk::ChunkMode::Text)
        .eval()
        .map_err(error)?;
    let definition: NativeProjectDefinition = lua.from_value(value).map_err(error)?;
    let source = ValidatedSong::restore_native_project(root, &definition)?;
    validate_executable(&source)?;
    Ok(source)
}

pub fn encode_native_project(source: &ValidatedSong) -> Result<String> {
    validate_executable(source)?;
    let definition = source.native_project_definition()?;
    let value = serde_json::to_value(definition).map_err(error)?;
    let encoded = format!(
        "-- MELDRITCH self-contained authored project; runtime and captures are separate.\nreturn {}\n",
        crate::instrument::encode_graph_value(&value, 0)?
    );
    if encoded.len() > 256 * 1024 {
        return Err("self-contained project exceeds Lua source size limit".into());
    }
    let restored = load_native_project(source.root(), &encoded)?;
    if restored != *source {
        return Err("self-contained project refuses a lossy Lua round trip".into());
    }
    Ok(encoded)
}
