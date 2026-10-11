use super::*;
pub use meldritch_dsl::{
    InstrumentEdit, InstrumentParameter, InstrumentParameterDefinition, InstrumentParameterId,
    InstrumentRange, InstrumentValue, InstrumentWaveform, PatchCreation, PatchEdit, PatchOperation,
    PlaitsModel, ValidatedSong,
};

/// Selection, presentation and staged patch operations only.
pub(crate) struct InstrumentEditor {
    pub synth: String,
    pub parameter: usize,
    pub visible: bool,
    pub save_status: Option<String>,
    pub patch_visible: bool,
    pub patch_base_fingerprint: u64,
    pub patch_scroll: u16,
    pub patch_input: String,
    pub patch_status: String,
    pub patch_operations: Vec<meldritch_dsl::PatchOperation>,
}

#[derive(Clone, Debug)]
pub struct InstrumentEditorView {
    pub base: Arc<ValidatedSong>,
    pub source: Arc<ValidatedSong>,
    pub base_fingerprint: u64,
    pub synth: String,
    pub selected_parameter: usize,
    pub visible: bool,
    pub parameters: Vec<InstrumentParameterDefinition>,
    pub defaults: Vec<InstrumentParameterDefinition>,
    pub save_status: Option<String>,
    pub patch_visible: bool,
    pub patch_base_fingerprint: u64,
    pub patch_scroll: u16,
    pub patch_input: String,
    pub patch_status: String,
    pub patch_operations: Vec<meldritch_dsl::PatchOperation>,
}

/// Revalidate synth graph changes over immutable provenance without permitting
/// changes to unrelated song definitions through the instrument reload boundary.
fn source_from_parameters(
    base: &ValidatedSong,
    source: &ValidatedSong,
) -> Result<ValidatedSong, String> {
    let mut reconstructed = base.clone();
    for id in source.synths().keys() {
        let imported = base
            .synths()
            .get(id)
            .ok_or_else(|| format!("instrument reload contains unknown synth '{id}'"))?;
        if &source.synths()[id] != imported {
            reconstructed = reconstructed.replace_patch(source.synths()[id].clone())?;
        }
    }
    Ok(reconstructed)
}

impl AppController {
    pub fn enable_instrument_editor(&mut self, song: ValidatedSong) -> Result<(), String> {
        self.enable_instrument_editor_from(song.clone(), song)
    }

    /// Initialize independently reloaded authored state, without recording load as edits.
    pub fn enable_instrument_editor_from(
        &mut self,
        base: ValidatedSong,
        song: ValidatedSong,
    ) -> Result<(), String> {
        meldritch_render::song_host::CompiledSongPatch::compile_initial(&song)?;
        let reconstructed = source_from_parameters(&base, &song)?;
        if reconstructed.fingerprint() != song.fingerprint() {
            return Err("instrument reload changed unsupported song semantics".into());
        }
        let synth = song
            .synths()
            .keys()
            .next()
            .ok_or("song has no editable synth")?
            .clone();
        song.instrument_parameters(&synth)?;
        let source = Arc::new(song);
        self.project_source = Some(project::ProjectSource {
            base: Arc::new(base),
            source,
        });
        self.instrument_editor = Some(InstrumentEditor {
            synth,
            parameter: 0,
            visible: false,
            save_status: None,
            patch_visible: false,
            patch_base_fingerprint: 0,
            patch_scroll: 0,
            patch_input: String::new(),
            patch_status: "Committed graph; no staged edits".into(),
            patch_operations: vec![],
        });
        Ok(())
    }

    pub fn observe_instrument_save(&mut self, status: String) {
        if let Some(editor) = &mut self.instrument_editor {
            editor.save_status = Some(status);
        }
    }

    pub(crate) fn instrument_view(&self) -> Option<InstrumentEditorView> {
        let owner = self.project_source.as_ref()?;
        self.instrument_editor
            .as_ref()
            .map(|editor| InstrumentEditorView {
                base: owner.base.clone(),
                source: owner.source.clone(),
                base_fingerprint: owner.base.fingerprint(),
                synth: editor.synth.clone(),
                selected_parameter: editor.parameter,
                visible: editor.visible,
                parameters: owner
                    .source
                    .instrument_parameters(&editor.synth)
                    .unwrap_or_default(),
                defaults: owner
                    .base
                    .instrument_parameters(&editor.synth)
                    .unwrap_or_default(),
                save_status: editor.save_status.clone(),
                patch_visible: editor.patch_visible,
                patch_base_fingerprint: editor.patch_base_fingerprint,
                patch_scroll: editor.patch_scroll,
                patch_input: editor.patch_input.clone(),
                patch_status: editor.patch_status.clone(),
                patch_operations: editor.patch_operations.clone(),
            })
    }

    pub(crate) fn edit_instrument(
        &mut self,
        edit: &InstrumentEdit,
    ) -> Result<AppCommandResult, AppCommandError> {
        let owner = self
            .project_source
            .as_mut()
            .ok_or_else(|| AppCommandError::Instrument("no authored song source".into()))?;
        let parameters = owner
            .source
            .instrument_parameters(&edit.target.synth)
            .map_err(AppCommandError::Instrument)?;
        let previous = parameters
            .iter()
            .find(|p| p.id == edit.target)
            .map(|p| p.value.clone());
        let next = owner
            .source
            .edit_instrument(edit)
            .map_err(AppCommandError::Instrument)?;
        let next = next
            .canonicalize_instrument_defaults(&owner.base)
            .map_err(AppCommandError::Instrument)?;
        // Reuse executable lowering as well as DSL parameter validation before acceptance.
        meldritch_render::song_host::CompiledSongPatch::compile_initial(&next)
            .map_err(AppCommandError::Instrument)?;
        let changed = previous
            .as_ref()
            .is_none_or(|previous| !previous.same_as(&edit.value));
        if changed {
            owner.source = Arc::new(next);
            if let Some(editor) = self.instrument_editor.as_mut() {
                editor.save_status = None;
            }
            let target = format!(
                "synth:{}/{}.{}",
                edit.target.synth,
                edit.target.module,
                edit.target.parameter.name()
            );
            if let InstrumentValue::Number(value) = edit.value {
                for control in &mut self.curated_controls {
                    if control.target == target {
                        control.value = Some(value);
                    }
                }
            }
        }
        if changed {
            self.rearm_assignment_target(&crate::EditableTarget::Synth {
                id: edit.target.synth.clone(),
                module: edit.target.module.clone(),
                parameter: edit.target.parameter,
            });
        }
        Ok(AppCommandResult::InstrumentEdited {
            edit: edit.clone(),
            previous: previous.unwrap(),
            changed,
        })
    }

    pub(crate) fn instrument_navigation(
        &mut self,
        synth_delta: i32,
        parameter_delta: i32,
    ) -> Result<AppCommandResult, AppCommandError> {
        let editor = self
            .instrument_editor
            .as_mut()
            .ok_or_else(|| AppCommandError::Instrument("no loaded instrument editor".into()))?;
        let owner = self.project_source.as_ref().unwrap();
        let ids: Vec<_> = owner.source.synths().keys().cloned().collect();
        if synth_delta != 0 {
            let index = ids.iter().position(|id| id == &editor.synth).unwrap();
            editor.synth = ids
                [(index as i64 + i64::from(synth_delta)).rem_euclid(ids.len() as i64) as usize]
                .clone();
            editor.parameter = 0;
            editor.patch_operations.clear();
            editor.patch_input.clear();
            editor.patch_status = "Instrument changed; draft cleared".into();
        }
        let count = owner
            .source
            .instrument_parameters(&editor.synth)
            .map_err(AppCommandError::Instrument)?
            .len();
        if count == 0 {
            return Err(AppCommandError::Instrument(
                "instrument has no supported parameters".into(),
            ));
        }
        editor.parameter = (editor.parameter as i64 + i64::from(parameter_delta))
            .rem_euclid(count as i64) as usize;
        Ok(AppCommandResult::InstrumentEditorChanged)
    }

    pub(crate) fn instrument_adjustment(
        &self,
        delta: i32,
    ) -> Result<InstrumentEdit, AppCommandError> {
        let view = self
            .instrument_view()
            .ok_or_else(|| AppCommandError::Instrument("no loaded instrument editor".into()))?;
        let parameter = &view.parameters[view.selected_parameter];
        let value = match parameter.value {
            InstrumentValue::Number(value) => {
                let step = match parameter.id.parameter {
                    InstrumentParameter::CutoffHz => 100.0,
                    _ => 0.01,
                };
                InstrumentValue::Number(value + step * f64::from(delta))
            }
            InstrumentValue::Model(value) => {
                let choices = meldritch_dsl::PlaitsModel::ALL;
                let index = choices.iter().position(|v| *v == value).unwrap();
                InstrumentValue::Model(
                    choices[(index as i64 + i64::from(delta)).rem_euclid(choices.len() as i64)
                        as usize],
                )
            }
            InstrumentValue::Waveform(value) => {
                let choices = [
                    InstrumentWaveform::Sine,
                    InstrumentWaveform::Triangle,
                    InstrumentWaveform::Saw,
                    InstrumentWaveform::Pulse,
                    InstrumentWaveform::Square,
                ];
                let index = choices.iter().position(|v| v == &value).unwrap();
                InstrumentValue::Waveform(
                    choices[(index as i64 + i64::from(delta)).rem_euclid(choices.len() as i64)
                        as usize],
                )
            }
        };
        Ok(InstrumentEdit {
            target: parameter.id.clone(),
            value,
        })
    }
}
