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
    #[serde(default)]
    parameters: Vec<InstrumentEdit>,
    graph: Option<meldritch_dsl::SynthDefinition>,
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

fn encode_graph_value(value: &serde_json::Value, depth: usize) -> Result<String> {
    use serde_json::Value;
    let indent = "  ".repeat(depth);
    match value {
        Value::Null => Ok("nil".into()),
        Value::Bool(v) => Ok(v.to_string()),
        Value::Number(v) => Ok(v.to_string()),
        Value::String(v) => Ok(quote(v)),
        Value::Array(values) => {
            let mut out = String::from("{\n");
            for v in values {
                out.push_str(&format!(
                    "{}  {},\n",
                    indent,
                    encode_graph_value(v, depth + 1)?
                ));
            }
            out.push_str(&format!("{indent}}}"));
            Ok(out)
        }
        Value::Object(fields) => {
            let mut out = String::from("{\n");
            for (name, v) in fields {
                if !v.is_null() {
                    out.push_str(&format!(
                        "{}  {} = {},\n",
                        indent,
                        name,
                        encode_graph_value(v, depth + 1)?
                    ));
                }
            }
            out.push_str(&format!("{indent}}}"));
            Ok(out)
        }
    }
}

fn validate_executable_patch(song: &ValidatedSong, synth: &str) -> Result<()> {
    meldritch_render::song_host::CompiledSongPatch::compile_initial(song)?;
    song.validate_executable_patch(synth)?;
    Ok(())
}

/// Stable ordering, round-trip f64 formatting, explicit absolute values. No toggles.
pub fn encode_instrument(view: &InstrumentEditorView) -> Result<String> {
    let graph = &view.source.synths()[&view.synth];
    // Use v1 only if ordinary parameter replay can represent the exact graph.
    // This catches structural defaults as well as module/cable identity changes.
    let mut replay = view.base.as_ref().clone();
    let mut representable = true;
    for parameter in view.source.instrument_parameters(&view.synth)? {
        match replay.edit_instrument(&InstrumentEdit {
            target: parameter.id,
            value: parameter.value,
        }) {
            Ok(next) => replay = next,
            Err(_) => {
                representable = false;
                break;
            }
        }
    }
    if !representable || replay.synths()[&view.synth] != *graph {
        let value = serde_json::to_value(graph).map_err(error)?;
        let text = format!(
            "-- MELDRITCH authored patch; immutable TOML base and journals are unchanged.\nreturn {{\n  kind = \"instrument\",\n  version = 2,\n  base_fingerprint = \"{:016x}\",\n  instrument = {},\n  graph = {},\n}}\n",
            view.base_fingerprint,
            quote(&view.synth),
            encode_graph_value(&value, 2)?
        );
        if text.len() > 256 * 1024 {
            return Err("patch export exceeds source size limit".into());
        }
        return Ok(text);
    }
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
    if definition.kind != "instrument" || ![1, 2].contains(&definition.version) {
        return Err("unsupported authored instrument kind/version".into());
    }
    let mut song = load_song_directory(root).map_err(error)?;
    if definition.base_fingerprint != format!("{:016x}", song.fingerprint()) {
        return Err("authored instrument base fingerprint changed".into());
    }
    if definition.version == 2 {
        if !definition.parameters.is_empty() {
            return Err("patch definition cannot also declare parameter overrides".into());
        }
        let graph = definition.graph.ok_or("version 2 requires a graph")?;
        if graph.id() != definition.instrument {
            return Err("patch graph identity differs from selected instrument".into());
        }
        song = song.replace_patch(graph)?;
        // Match the exact app acceptance boundary, including no silently ignored modules/cables.
        validate_executable_patch(&song, &definition.instrument)?;
        return Ok(song);
    }
    if definition.graph.is_some() {
        return Err("version 1 does not support graph fields".into());
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
