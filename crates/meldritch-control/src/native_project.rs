//! Self-contained authored project v2; Lua only describes Rust-validated source.
use crate::{Result, error};
use meldritch_dsl::{NativeProjectDefinition, ValidatedSong};
use mlua::LuaSerdeExt;
use std::path::Path;

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
