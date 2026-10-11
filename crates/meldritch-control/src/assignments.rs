//! Authored fader bindings reuse the existing MIDI mapper and physical pickup.
use crate::{
    CommandRequest, Result,
    devices::{MidiObservation, MidiPickup, decode_midi},
};
use meldritch_app::*;
use mlua::LuaSerdeExt;
use serde::Deserialize;
use std::path::{Path, PathBuf};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Definition {
    kind: String,
    version: u32,
    base_fingerprint: String,
    assignments: AssignmentDefinition,
    values: Vec<ParameterEdit>,
}
fn target_lua(target: &EditableTarget) -> String {
    use crate::instrument::quote as q;
    match target {
        EditableTarget::Synth {
            id,
            module,
            parameter,
        } => format!(
            "{{ kind = \"synth\", id = {}, module = {}, parameter = {} }}",
            q(id),
            q(module),
            q(parameter.name())
        ),
        EditableTarget::Dsp {
            id,
            module,
            parameter,
        } => format!(
            "{{ kind = \"dsp\", id = {}, module = {}, parameter = {} }}",
            q(id),
            q(module),
            q(if *parameter == meldritch_dsl::DelayParameter::Mix {
                "mix"
            } else {
                "feedback"
            })
        ),
        EditableTarget::Track { id, parameter } => format!(
            "{{ kind = \"track\", id = {}, parameter = {} }}",
            q(id),
            q(if *parameter == meldritch_dsl::MixerParameter::Pan {
                "pan"
            } else {
                "volume"
            })
        ),
    }
}
pub fn encode_controls(view: &AssignmentEditorView) -> Result<String> {
    encode_controls_scoped(view, true)
}
pub(crate) fn encode_controls_scoped(
    view: &AssignmentEditorView,
    standalone: bool,
) -> Result<String> {
    use crate::instrument::quote as q;
    view.definition.validate(&view.source)?;
    let mut text = format!(
        "-- MELDRITCH authored control assignments and parameter values.\nreturn {{\n  kind = \"controls\", version = 1,\n  base_fingerprint = \"{:016x}\",\n  assignments = {{\n    pages = {{ {} }},\n    targets = {{\n",
        view.base.fingerprint(),
        view.definition
            .pages
            .iter()
            .map(|p| q(p))
            .collect::<Vec<_>>()
            .join(", ")
    );
    for t in &view.definition.targets {
        text.push_str(&format!("      {{ id = {}, label = {}, target = {}, minimum = {:?}, maximum = {:?}, step = {:?} }},\n",q(&t.id),q(&t.label),target_lua(&t.target),t.minimum,t.maximum,t.step));
    }
    text.push_str("    },\n    faders = {\n");
    for f in &view.definition.faders {
        text.push_str(&format!(
            "      {{ id = {}, device = {}, channel = {}, cc = {}, page = {}, target = {} }},\n",
            q(&f.id),
            q(&f.device),
            f.channel,
            f.cc,
            q(&f.page),
            f.target.as_ref().map_or("nil".into(), |s| q(s))
        ));
    }
    text.push_str("    },\n  },\n  values = {\n");
    for t in &view.definition.targets {
        let source = view.source.editable_parameter(&t.target)?;
        let base = view.base.editable_parameter(&t.target)?;
        let pan_changed = matches!(&t.target, EditableTarget::Track {id,parameter:meldritch_dsl::MixerParameter::Pan} if view.source.performance().tracks().iter().find(|t|t.id()==id).unwrap().pan()!=view.base.performance().tracks().iter().find(|t|t.id()==id).unwrap().pan());
        if source.value != base.value || pan_changed {
            text.push_str(&format!(
                "    {{ target = {}, value = {:?} }},\n",
                target_lua(&t.target),
                source.value
            ));
        }
    }
    text.push_str("  },\n}\n");
    if standalone
        && load_controls(view.source.root(), &text)?.0.fingerprint() != view.source.fingerprint()
    {
        return Err("controls export refuses unrelated or uncatalogued source edits".into());
    }
    Ok(text)
}
pub fn load_controls(
    root: impl AsRef<Path>,
    source: &str,
) -> Result<(ValidatedSong, AssignmentDefinition)> {
    let base = meldritch_dsl::load_song_directory(root).map_err(crate::error)?;
    apply_controls(&base, base.clone(), source)
}
pub(crate) fn apply_controls(
    base: &ValidatedSong,
    mut song: ValidatedSong,
    source: &str,
) -> Result<(ValidatedSong, AssignmentDefinition)> {
    let (lua, _) = crate::constrained_vm(source)?;
    let value = lua
        .load(source)
        .set_name("authored controls")
        .set_mode(mlua::chunk::ChunkMode::Text)
        .eval()
        .map_err(crate::error)?;
    let d: Definition = lua.from_value(value).map_err(crate::error)?;
    if d.kind != "controls" || d.version != 1 {
        return Err("unsupported controls authoring kind/version".into());
    }
    if d.base_fingerprint != format!("{:016x}", base.fingerprint()) {
        return Err("authored controls base fingerprint changed".into());
    }
    d.assignments.validate(&song)?;
    let defaults_base = song.clone();
    let mut seen = std::collections::BTreeSet::new();
    for edit in d.values {
        if !seen.insert(edit.target.identity())
            || !d
                .assignments
                .targets
                .iter()
                .any(|t| t.target == edit.target)
        {
            return Err("duplicate or undeclared saved parameter target".into());
        }
        song = song.edit_parameter(&edit)?;
    }
    song = song.canonicalize_instrument_defaults(&defaults_base)?;
    meldritch_render::song_host::CompiledSongPatch::compile_initial(&song)?;
    Ok((song, d.assignments))
}
pub fn load_controls_file(
    root: impl AsRef<Path>,
    relative: &Path,
) -> Result<(ValidatedSong, AssignmentDefinition)> {
    let root = root.as_ref().canonicalize().map_err(crate::error)?;
    let path = root.join(relative).canonicalize().map_err(crate::error)?;
    if !path.starts_with(&root) {
        return Err("authored controls reference escapes song root".into());
    }
    load_controls(root, &std::fs::read_to_string(path).map_err(crate::error)?)
}
pub fn save_controls(view: &AssignmentEditorView) -> Result<PathBuf> {
    crate::instrument::save_definition(
        view.source.root(),
        view.base.fingerprint(),
        "controls",
        &encode_controls(view)?,
    )
}
impl crate::performance::LuaPerformance {
    pub fn fader(
        &mut self,
        source: &str,
        bytes: &[u8],
        frame: u64,
        app: &mut AppController,
        pickup: &mut MidiPickup,
        audio: &mut crate::song_host::SongAudioHost,
        capture: &mut crate::session::PerformanceSessionCapture,
    ) -> Result<Option<bool>> {
        let Some(MidiObservation::Control { channel, cc, value }) = decode_midi(bytes) else {
            return Ok(None);
        };
        let Some(view) = app.assignment_view() else {
            return Ok(None);
        };
        let Some(fader) = view.definition.faders.iter().find(|f| {
            f.device == source && f.channel == channel + 1 && f.cc == cc && f.page == view.page
        }) else {
            return Ok(view
                .definition
                .faders
                .iter()
                .any(|f| f.device == source && f.channel == channel + 1 && f.cc == cc)
                .then_some(false));
        };
        let id = fader.id.clone();
        if fader.target.is_none() {
            app.observe_assignment(&id, value, false);
            self.plane.refresh(app)?;
            return Ok(Some(false));
        }
        u32::try_from(frame)
            .map_err(|_| "session frame exceeds u32; observation rejected before mutation")?;
        let physical = MidiControlInput {
            device: source.into(),
            channel: channel + 1,
            cc,
            value,
        };
        let binding = MidiControlBinding {
            control_id: format!("assignment:{id}"),
            device: source.into(),
            page_id: None,
            channel: Some(channel + 1),
            cc,
            minimum: 0.0,
            maximum: 1.0,
            action: MidiControlAction::Absolute,
        };
        let input = map_midi_control_input(&[binding], physical.clone())
            .ok_or("invalid mapped observation")?;
        let acquired = pickup.observe(&physical, &input, &app.view_model())?;
        app.observe_assignment(&id, value, acquired);
        if !acquired {
            self.plane.refresh(app)?;
            return Ok(Some(false));
        }
        let result = app
            .handle_input(input)
            .map_err(|e| format!("fader '{id}': {e:?}"))?;
        pickup.acknowledge(&physical, &app.view_model());
        app.observe_assignment(&id, value, true);
        let target = view
            .definition
            .targets
            .iter()
            .find(|t| Some(&t.id) == fader.target.as_ref())
            .unwrap()
            .target
            .clone();
        let value = app
            .assignment_view()
            .unwrap()
            .source
            .editable_parameter(&target)?
            .value;
        let request = CommandRequest::EditParameter {
            edit: ParameterEdit { target, value },
        };
        let recorded = capture.record_command(app, &request, &result, source, frame, self.tempo());
        let scheduled = audio.accepted(&app.view_model(), &result);
        recorded?;
        scheduled?;
        capture.checkpoint()?;
        audio.refresh_view(app);
        self.plane.refresh(app)?;
        Ok(Some(true))
    }
}
