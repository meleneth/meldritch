//! Bounded composition of existing authoring definitions, never another song model.
use crate::{Result, error};
use meldritch_app::{AppViewModel, AssignmentDefinition};
use meldritch_dsl::{ValidatedSong, load_song_directory};
use mlua::LuaSerdeExt;
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Definition {
    kind: String,
    version: u32,
    base_fingerprint: String,
    instrument: serde_json::Value,
    pattern: serde_json::Value,
    controls: serde_json::Value,
}
#[derive(Debug)]
pub struct AuthoredProject {
    pub song: ValidatedSong,
    pub instrument: String,
    pub pattern: String,
    pub assignments: AssignmentDefinition,
}
pub fn encode_project(view: &AppViewModel) -> Result<String> {
    let instrument = view
        .instrument_editor
        .as_ref()
        .ok_or("project requires an instrument")?;
    let pattern = view
        .pattern_editor
        .as_ref()
        .ok_or("project requires a selected pattern")?;
    let controls = view
        .assignments
        .as_ref()
        .ok_or("project requires declared assignments")?;
    if instrument.source.fingerprint() != pattern.source.fingerprint()
        || instrument.source.fingerprint() != controls.source.fingerprint()
    {
        return Err("authoring projections disagree about source revision".into());
    }
    let i = crate::instrument::encode_instrument_scoped(instrument, false)?;
    let p = crate::pattern::encode_pattern_scoped(pattern, false)?;
    let c = crate::assignments::encode_controls_scoped(controls, false)?;
    fn table(source: &str) -> Result<String> {
        let table = source
            .split_once("return ")
            .map(|(_, table)| table.trim())
            .ok_or("invalid component encoding")?;
        Ok(table
            .lines()
            .enumerate()
            .map(|(i, line)| {
                if i == 0 {
                    line.to_owned()
                } else {
                    format!("  {line}")
                }
            })
            .collect::<Vec<_>>()
            .join("\n"))
    }
    let source = format!(
        "-- MELDRITCH composed authored music; journals and TOML stay unchanged.\nreturn {{\n  kind = \"project\", version = 1,\n  base_fingerprint = \"{:016x}\",\n  instrument = {},\n  pattern = {},\n  controls = {},\n}}\n",
        instrument.base_fingerprint,
        table(&i)?,
        table(&p)?,
        table(&c)?
    );
    if source.len() > 256 * 1024 {
        return Err("project export exceeds source size limit".into());
    }
    let restored = load_project(instrument.source.root(), &source)?;
    if restored.song.fingerprint() != instrument.source.fingerprint()
        || restored.assignments != controls.definition
    {
        return Err("project export refuses unrelated or uncaptured source edits".into());
    }
    Ok(source)
}
pub fn load_project(root: impl AsRef<Path>, source: &str) -> Result<AuthoredProject> {
    let (lua, _) = crate::constrained_vm(source)?;
    let value = lua
        .load(source)
        .set_name("authored project")
        .set_mode(mlua::chunk::ChunkMode::Text)
        .eval()
        .map_err(error)?;
    let definition: Definition = lua.from_value(value).map_err(error)?;
    if definition.kind != "project" || definition.version != 1 {
        return Err("unsupported project authoring kind/version".into());
    }
    let base = load_song_directory(root).map_err(error)?;
    if definition.base_fingerprint != format!("{:016x}", base.fingerprint()) {
        return Err("authored project base fingerprint changed".into());
    }
    let component = |v: &serde_json::Value| -> Result<String> {
        Ok(format!(
            "return {}",
            crate::instrument::encode_graph_value(v, 0)?
        ))
    };
    let song = crate::instrument::apply_instrument(
        &base,
        base.clone(),
        &component(&definition.instrument)?,
    )?;
    let instrument = definition
        .instrument
        .get("instrument")
        .and_then(|v| v.as_str())
        .ok_or("missing instrument identity")?
        .to_owned();
    let (song, pattern) =
        crate::pattern::apply_pattern(&base, song, &component(&definition.pattern)?)?;
    // Two components must not silently disagree about a shared synth parameter.
    if let Some(values) = definition.controls.get("values").and_then(|v| v.as_array()) {
        for value in values {
            let edit: meldritch_dsl::ParameterEdit =
                serde_json::from_value(value.clone()).map_err(error)?;
            if matches!(&edit.target, meldritch_dsl::EditableTarget::Synth {id,..} if id==&instrument)
                && song.editable_parameter(&edit.target)?.value != edit.value
            {
                return Err(format!(
                    "project instrument and controls disagree about {}",
                    edit.target.identity()
                ));
            }
        }
    }
    let (song, assignments) =
        crate::assignments::apply_controls(&base, song, &component(&definition.controls)?)?;
    meldritch_render::song_host::CompiledSongPatch::compile_initial(&song)?;
    Ok(AuthoredProject {
        song,
        instrument,
        pattern,
        assignments,
    })
}
pub fn load_project_file(root: impl AsRef<Path>, relative: &Path) -> Result<AuthoredProject> {
    let root = root.as_ref().canonicalize().map_err(error)?;
    let path = root.join(relative).canonicalize().map_err(error)?;
    if !path.starts_with(&root) {
        return Err("authored project reference escapes song root".into());
    }
    load_project(root, &std::fs::read_to_string(path).map_err(error)?)
}
pub fn save_project(view: &AppViewModel) -> Result<PathBuf> {
    let instrument = view.instrument_editor.as_ref().ok_or("no authored song")?;
    crate::instrument::save_definition(
        instrument.source.root(),
        instrument.base_fingerprint,
        "project",
        &encode_project(view)?,
    )
}
