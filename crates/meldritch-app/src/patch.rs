use super::*;
use meldritch_dsl::{DependentCables, ModuleKind, PatchEdit, PatchOperation, PlaitsModel};
impl AppController {
    pub(crate) fn edit_patch(
        &mut self,
        edit: &PatchEdit,
    ) -> Result<AppCommandResult, AppCommandError> {
        let editor = self
            .instrument_editor
            .as_ref()
            .ok_or_else(|| AppCommandError::Instrument("no loaded synth".into()))?;
        let next = editor
            .source
            .edit_patch(edit)
            .map_err(AppCommandError::Instrument)?;
        // Reject executable topology unsupported by the ordinary compiler.
        meldritch_render::song_host::CompiledSongPatch::compile_initial(&next)
            .map_err(AppCommandError::Instrument)?;
        next.validate_executable_patch(&edit.synth)
            .map_err(AppCommandError::Instrument)?;
        // Existing source parameter references and assignments cannot become dangling.
        if let Some(assignments) = self.assignment_view() {
            for target in &assignments.definition.targets {
                next.editable_parameter(&target.target)
                    .map_err(AppCommandError::Instrument)?;
            }
        }
        let previous = editor
            .source
            .instrument_parameters(&edit.synth)
            .map_err(AppCommandError::Instrument)?;
        let parameters = next
            .instrument_parameters(&edit.synth)
            .map_err(AppCommandError::Instrument)?;
        let changed = next.fingerprint() != editor.source.fingerprint();
        if changed {
            let editor = self.instrument_editor.as_mut().unwrap();
            editor.source = Arc::new(next);
            editor.parameter = 0;
            editor.save_status = None;
            for parameter in parameters {
                if previous
                    .iter()
                    .any(|p| p.id == parameter.id && p.value.same_as(&parameter.value))
                {
                    continue;
                }
                if let InstrumentValue::Number(value) = parameter.value {
                    let target = format!(
                        "synth:{}/{}.{}",
                        parameter.id.synth,
                        parameter.id.module,
                        parameter.id.parameter.name()
                    );
                    for c in &mut self.curated_controls {
                        if c.target == target {
                            c.value = Some(value);
                        }
                    }
                }
                self.rearm_assignment_target(&EditableTarget::Synth {
                    id: parameter.id.synth,
                    module: parameter.id.module,
                    parameter: parameter.id.parameter,
                });
            }
        }
        Ok(AppCommandResult::PatchEdited {
            synth: edit.synth.clone(),
            operations: edit.operations.clone(),
            changed,
        })
    }
    pub(crate) fn patch_submit(&mut self) -> Result<AppCommandResult, AppCommandError> {
        let editor = self
            .instrument_editor
            .as_ref()
            .ok_or_else(|| AppCommandError::Instrument("no loaded synth".into()))?;
        let line = editor.patch_input.clone();
        let words: Vec<_> = line.split_whitespace().collect();
        if words.as_slice() == ["commit"] {
            if !editor.patch_operations.is_empty()
                && editor.patch_base_fingerprint != editor.source.fingerprint()
            {
                return Err(AppCommandError::Instrument(
                    "draft base changed; clear and restage against the current source".into(),
                ));
            }
            let edit = PatchEdit {
                synth: editor.synth.clone(),
                operations: editor.patch_operations.clone(),
            };
            let result = self.edit_patch(&edit)?;
            let e = self.instrument_editor.as_mut().unwrap();
            e.patch_operations.clear();
            e.patch_input.clear();
            e.patch_status = "Committed graph; no staged edits".into();
            return Ok(result);
        }
        if words.as_slice() == ["clear"] {
            let e = self.instrument_editor.as_mut().unwrap();
            e.patch_operations.clear();
            e.patch_input.clear();
            e.patch_status = "Committed graph; no staged edits".into();
            return Ok(AppCommandResult::InstrumentEditorChanged);
        }
        if words.as_slice() == ["save"] {
            self.instrument_editor.as_mut().unwrap().patch_input.clear();
            return Ok(AppCommandResult::InstrumentSaveRequested);
        }
        let operation = parse_operation(&words).map_err(AppCommandError::Instrument)?;
        let mut operations = editor.patch_operations.clone();
        operations.push(operation);
        let edit = PatchEdit {
            synth: editor.synth.clone(),
            operations: operations.clone(),
        };
        editor
            .source
            .preview_patch(&edit)
            .map_err(AppCommandError::Instrument)?;
        let validation = editor.source.edit_patch(&edit).and_then(|next| {
            next.validate_executable_patch(&edit.synth)?;
            meldritch_render::song_host::CompiledSongPatch::compile_initial(&next).map(|_| ())
        });
        let status = match validation {
            Ok(()) => "Draft validates/compiles; enter commit to publish".into(),
            Err(e) => format!("Draft incomplete/invalid: {e}"),
        };
        let e = self.instrument_editor.as_mut().unwrap();
        if e.patch_operations.is_empty() {
            e.patch_base_fingerprint = e.source.fingerprint();
        }
        e.patch_operations = operations;
        e.patch_input.clear();
        e.patch_status = status;
        Ok(AppCommandResult::InstrumentEditorChanged)
    }
}
fn parse_operation(words: &[&str]) -> Result<PatchOperation, String> {
    match words {
        ["add", id, kind] => Ok(PatchOperation::Instantiate { id: (*id).into(), kind: match *kind {
            "oscillator" => ModuleKind::Oscillator, "plaits" => ModuleKind::Plaits, "adsr" => ModuleKind::Adsr,
            "vca" => ModuleKind::Vca, "low_pass" => ModuleKind::LowPass, "audio_output" => ModuleKind::AudioOutput,
            _ => return Err(format!("unsupported synth module '{kind}'")),
        } }),
        ["remove", id, policy] => Ok(PatchOperation::Remove { id: (*id).into(), cables: match *policy {
            "reject" => DependentCables::Reject, "disconnect" => DependentCables::Disconnect,
            _ => return Err("remove policy must be reject or disconnect".into()),
        } }),
        ["connect", from, to] => Ok(PatchOperation::Connect { from: (*from).into(), to: (*to).into() }),
        ["disconnect", from, to] => Ok(PatchOperation::Disconnect { from: (*from).into(), to: (*to).into() }),
        ["set", module, parameter, value] => {
            let parameter = match *parameter {
                "waveform" => InstrumentParameter::Waveform, "model" => InstrumentParameter::Model,
                "harmonics" => InstrumentParameter::Harmonics, "timbre" => InstrumentParameter::Timbre,
                "morph" => InstrumentParameter::Morph, "attack" => InstrumentParameter::Attack,
                "decay" => InstrumentParameter::Decay, "sustain" => InstrumentParameter::Sustain,
                "release" => InstrumentParameter::Release, "cutoff_hz" => InstrumentParameter::CutoffHz,
                "resonance" => InstrumentParameter::Resonance,
                _ => return Err(format!("unsupported parameter '{parameter}'")),
            };
            let value = match parameter {
                InstrumentParameter::Model => InstrumentValue::Model(PlaitsModel::parse(value)?),
                InstrumentParameter::Waveform => InstrumentValue::Waveform(match *value {
                    "sine" => InstrumentWaveform::Sine, "triangle" => InstrumentWaveform::Triangle,
                    "saw" => InstrumentWaveform::Saw, "pulse" => InstrumentWaveform::Pulse, "square" => InstrumentWaveform::Square,
                    _ => return Err("waveform must be sine/triangle/saw/pulse/square".into()),
                }),
                _ => InstrumentValue::Number(value.parse().map_err(|_| "parameter requires a number")?),
            };
            Ok(PatchOperation::SetParameter { module: (*module).into(), parameter, value })
        }
        _ => Err("use add ID TYPE | remove ID reject/disconnect | connect FROM TO | disconnect FROM TO | set MODULE PARAM VALUE | commit | clear | save".into()),
    }
}
