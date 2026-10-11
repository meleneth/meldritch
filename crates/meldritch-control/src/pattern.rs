//! Version-1 Lua pattern definitions; host-owned persistence, no session replay.
use crate::{Result, constrained_vm, error};
use meldritch_app::PatternEditorView;
use meldritch_dsl::{AuthoredPattern, ValidatedSong, load_song_directory};
use mlua::LuaSerdeExt;
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Definition {
    kind: String,
    version: u32,
    base_fingerprint: String,
    pattern: AuthoredPattern,
}

pub fn encode_pattern(view: &PatternEditorView) -> Result<String> {
    use crate::instrument::quote;
    let p = &view.pattern;
    let mut text = format!(
        "-- MELDRITCH authored pattern; musical times use 960 PPQ.\nreturn {{\n  kind = \"pattern\",\n  version = 1,\n  base_fingerprint = \"{:016x}\",\n  pattern = {{\n    id = {}, track = {},\n    length_ticks = {}, looped = {}, next_note_id = {},\n    notes = {{\n",
        view.base_fingerprint,
        quote(&p.id),
        quote(&p.track),
        p.length_ticks,
        p.looped,
        p.next_note_id
    );
    for n in &p.notes {
        text.push_str(&format!("      {{ id = {}, note = {}, start_tick = {}, duration_ticks = {}, velocity = {:?} }},\n", quote(&n.id), quote(&n.note), n.start_tick, n.duration_ticks, n.velocity));
    }
    text.push_str("    },\n  },\n}\n");
    let (restored, _) = load_pattern(view.source.root(), &text)?;
    if restored.fingerprint() != view.source.fingerprint() {
        return Err("pattern export cannot include unrelated source edits".into());
    }
    Ok(text)
}
pub fn load_pattern(root: impl AsRef<Path>, source: &str) -> Result<(ValidatedSong, String)> {
    let (lua, _budget) = constrained_vm(source)?;
    let value = lua
        .load(source)
        .set_name("authored pattern")
        .set_mode(mlua::chunk::ChunkMode::Text)
        .eval()
        .map_err(error)?;
    let definition: Definition = lua.from_value(value).map_err(error)?;
    if definition.kind != "pattern" || definition.version != 1 {
        return Err("unsupported authored pattern kind/version".into());
    }
    let base = load_song_directory(root).map_err(error)?;
    if definition.base_fingerprint != format!("{:016x}", base.fingerprint()) {
        return Err("authored pattern base fingerprint changed".into());
    }
    let song = base.restore_authored_pattern(&definition.pattern)?;
    meldritch_render::song_host::CompiledSongPatch::compile_initial(&song)?;
    Ok((song, definition.pattern.id))
}
pub fn load_pattern_file(
    root: impl AsRef<Path>,
    relative: &Path,
) -> Result<(ValidatedSong, String)> {
    let root = root.as_ref().canonicalize().map_err(error)?;
    let path = root.join(relative).canonicalize().map_err(error)?;
    if !path.starts_with(&root) {
        return Err("authored pattern reference escapes song root".into());
    }
    load_pattern(root, &std::fs::read_to_string(path).map_err(error)?)
}
pub fn save_pattern(view: &PatternEditorView) -> Result<PathBuf> {
    let source = encode_pattern(view)?;
    crate::instrument::save_definition(
        view.source.root(),
        view.base_fingerprint,
        "pattern",
        &source,
    )
}
