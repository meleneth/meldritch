//! Authored instrument definitions, separate from Lua widgets and session journals.
use crate::{Result, constrained_vm, error};
use meldritch_app::InstrumentEditorView;
use meldritch_dsl::{InstrumentEdit, InstrumentValue, ValidatedSong, load_song_directory};
use mlua::LuaSerdeExt;
use serde::Deserialize;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Definition {
    kind: String,
    version: u32,
    base_fingerprint: String,
    instrument: String,
    parameters: Vec<InstrumentEdit>,
}

pub(crate) fn quote(value: &str) -> String {
    let mut out = String::from("\"");
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch.is_control() => {
                for byte in ch.to_string().bytes() {
                    out.push_str(&format!("\\{byte:03}"));
                }
            }
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}

/// Stable ordering, round-trip f64 formatting, explicit absolute values. No toggles.
pub fn encode_instrument(view: &InstrumentEditorView) -> Result<String> {
    let parameters = view.source.instrument_parameters(&view.synth)?;
    let mut text = format!(
        "-- MELDRITCH authored instrument; base song remains unchanged.\nreturn {{\n  kind = \"instrument\",\n  version = 1,\n  base_fingerprint = \"{:016x}\",\n  instrument = {},\n  parameters = {{\n",
        view.base_fingerprint,
        quote(&view.synth)
    );
    for parameter in parameters {
        let name = parameter.id.parameter.name();
        let value = match parameter.value {
            InstrumentValue::Number(number) if number.is_finite() => format!("{number:?}"),
            InstrumentValue::Waveform(waveform) => quote(waveform.name()),
            InstrumentValue::Model(model) => quote(model.name()),
            _ => return Err("cannot export nonfinite instrument value".into()),
        };
        text.push_str(&format!(
            "    {{ target = {{ synth = {}, module = {}, parameter = {} }}, value = {} }},\n",
            quote(&parameter.id.synth),
            quote(&parameter.id.module),
            quote(name),
            value
        ));
    }
    text.push_str("  },\n}\n");
    if text.len() > 256 * 1024 {
        return Err("instrument export exceeds Lua source size limit".into());
    }
    Ok(text)
}

/// Independently reload into the ordinary semantic model and existing compiler.
pub fn load_instrument(root: impl AsRef<Path>, source: &str) -> Result<ValidatedSong> {
    let (lua, _budget) = constrained_vm(source)?;
    let value = lua
        .load(source)
        .set_name("authored instrument")
        .set_mode(mlua::chunk::ChunkMode::Text)
        .eval()
        .map_err(error)?;
    let definition: Definition = lua.from_value(value).map_err(error)?;
    if definition.kind != "instrument" || definition.version != 1 {
        return Err("unsupported authored instrument kind/version".into());
    }
    let mut song = load_song_directory(root).map_err(error)?;
    if definition.base_fingerprint != format!("{:016x}", song.fingerprint()) {
        return Err("authored instrument base fingerprint changed".into());
    }
    let descriptors = song.instrument_parameters(&definition.instrument)?;
    if definition.parameters.len() != descriptors.len() {
        return Err(
            "instrument definition must contain every supported parameter exactly once".into(),
        );
    }
    let mut seen = std::collections::BTreeSet::new();
    for edit in definition.parameters {
        if edit.target.synth != definition.instrument || !seen.insert(edit.target.clone()) {
            return Err(
                "duplicate parameter or unrelated instrument in authored definition".into(),
            );
        }
        song = song.edit_instrument(&edit)?;
    }
    meldritch_render::song_host::CompiledSongPatch::compile_initial(&song)?;
    Ok(song)
}

pub fn load_instrument_file(root: impl AsRef<Path>, relative: &Path) -> Result<ValidatedSong> {
    let root = root.as_ref().canonicalize().map_err(error)?;
    let file = root.join(relative).canonicalize().map_err(error)?;
    if !file.starts_with(&root) {
        return Err("authored instrument reference escapes song root".into());
    }
    let source = fs::read_to_string(file).map_err(error)?;
    load_instrument(root, &source)
}

/// Atomic, create-new exports. Never overwrite a script, TOML or performance journal.
pub fn save_instrument(view: &InstrumentEditorView) -> Result<PathBuf> {
    let source = encode_instrument(view)?;
    save_definition(
        view.source.root(),
        view.base_fingerprint,
        "instrument",
        &source,
    )
}

pub(crate) fn save_definition(
    root: &Path,
    base_fingerprint: u64,
    kind: &str,
    source: &str,
) -> Result<PathBuf> {
    if source.len() > 256 * 1024 {
        return Err("authored export exceeds Lua source size limit".into());
    }
    let root = root.canonicalize().map_err(error)?;
    if load_song_directory(&root).map_err(error)?.fingerprint() != base_fingerprint {
        return Err(format!("cannot save {kind}: base song fingerprint changed"));
    }
    let directory = root.join("authored");
    fs::create_dir_all(&directory).map_err(error)?;
    if !directory.canonicalize().map_err(error)?.starts_with(&root) {
        return Err("authored directory escapes song root".into());
    }
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(error)?
        .as_nanos();
    let token = format!(
        "{stamp}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    let temporary = directory.join(format!(".{kind}-{token}.tmp"));
    let destination = directory.join(format!("{kind}-{token}.lua"));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(error)?;
        file.write_all(source.as_bytes()).map_err(error)?;
        file.sync_all().map_err(error)?;
        fs::hard_link(&temporary, &destination).map_err(error)?;
        fs::File::open(&directory)
            .and_then(|dir| dir.sync_all())
            .map_err(error)?;
        Ok(destination)
    })();
    let _ = fs::remove_file(temporary);
    result
}
