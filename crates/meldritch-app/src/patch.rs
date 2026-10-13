use super::*;
use meldritch_dsl::{DependentCables, ModuleKind, PatchEdit, PatchOperation, PlaitsModel};
impl AppController {
    pub(crate) fn create_patch(
        &mut self,
        creation: &PatchCreation,
    ) -> Result<AppCommandResult, AppCommandError> {
        let owner = self
            .project_source
            .as_ref()
            .ok_or_else(|| AppCommandError::Instrument("no authored project source".into()))?;
        let next = owner
            .source
            .create_patch(creation)
            .map_err(AppCommandError::Instrument)?;
        meldritch_render::song_render::validate_palette_synth(&next, &creation.edit.synth)
            .map_err(|e| AppCommandError::Instrument(e.to_string()))?;
        meldritch_render::song_host::CompiledSongPatch::compile_initial(&next)
            .map_err(AppCommandError::Instrument)?;
        self.project_source.as_mut().unwrap().source = Arc::new(next);
        Ok(AppCommandResult::PatchEdited {
            synth: creation.edit.synth.clone(),
            operations: creation.edit.operations.clone(),
            changed: true,
        })
    }

    pub(crate) fn edit_patch(
        &mut self,
        edit: &PatchEdit,
    ) -> Result<AppCommandResult, AppCommandError> {
        let owner = self
            .project_source
            .as_ref()
            .ok_or_else(|| AppCommandError::Instrument("no loaded synth".into()))?;
        let next = owner
            .source
            .edit_patch(edit)
            .map_err(AppCommandError::Instrument)?;
        // Reject executable topology unsupported by the ordinary compiler.
        meldritch_render::song_host::CompiledSongPatch::compile_initial(&next)
            .map_err(AppCommandError::Instrument)?;
        next.validate_executable_patch(&edit.synth)
            .map_err(AppCommandError::Instrument)?;
        meldritch_render::song_render::validate_palette_synth(&next, &edit.synth)
            .map_err(|e| AppCommandError::Instrument(e.to_string()))?;
        // Existing source parameter references and assignments cannot become dangling.
        if let Some(assignments) = self.assignment_view() {
            for target in &assignments.definition.targets {
                next.editable_parameter(&target.target)
                    .map_err(AppCommandError::Instrument)?;
            }
        }
        let previous = owner
            .source
            .instrument_parameters(&edit.synth)
            .map_err(AppCommandError::Instrument)?;
        let parameters = next
            .instrument_parameters(&edit.synth)
            .map_err(AppCommandError::Instrument)?;
        let changed = next.fingerprint() != owner.source.fingerprint();
        if changed {
            self.project_source.as_mut().unwrap().source = Arc::new(next);
            if let Some(editor) = self.instrument_editor.as_mut() {
                editor.parameter = 0;
                editor.save_status = None;
            }
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
        let owner = self.project_source.as_ref().unwrap();
        let line = editor.patch_input.clone();
        let words: Vec<_> = line.split_whitespace().collect();
        if words.as_slice() == ["commit"] {
            if !editor.patch_operations.is_empty()
                && editor.patch_base_fingerprint != owner.source.fingerprint()
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
        owner
            .source
            .preview_patch(&edit)
            .map_err(AppCommandError::Instrument)?;
        let validation = owner.source.edit_patch(&edit).and_then(|next| {
            next.validate_executable_patch(&edit.synth)?;
            meldritch_render::song_render::validate_palette_synth(&next, &edit.synth)
                .map_err(|e| e.to_string())?;
            meldritch_render::song_host::CompiledSongPatch::compile_initial(&next).map(|_| ())
        });
        let status = match validation {
            Ok(()) => "Draft validates/compiles; enter commit to publish".into(),
            Err(e) => format!("Draft incomplete/invalid: {e}"),
        };
        let e = self.instrument_editor.as_mut().unwrap();
        if e.patch_operations.is_empty() {
            e.patch_base_fingerprint = owner.source.fingerprint();
        }
        e.patch_operations = operations;
        e.patch_input.clear();
        e.patch_status = status;
        Ok(AppCommandResult::InstrumentEditorChanged)
    }
}
fn parse_operation(words: &[&str]) -> Result<PatchOperation, String> {
    match words {
        ["input", "add", id, signal] => Ok(PatchOperation::AddInput {
            id: (*id).into(),
            signal: match *signal {
                "control" => meldritch_dsl::SignalType::Control,
                "gate" => meldritch_dsl::SignalType::Gate,
                _ => return Err("native input add requires control or gate".into()),
            },
        }),
        ["input", "remove", id, policy] => Ok(PatchOperation::RemoveInput {
            id: (*id).into(),
            cables: match *policy {
                "reject" => DependentCables::Reject,
                "disconnect" => DependentCables::Disconnect,
                _ => return Err("input remove policy must be reject or disconnect".into()),
            },
        }),
        ["add", id, kind] => Ok(PatchOperation::Instantiate { id: (*id).into(), kind: match *kind {
            "oscillator" => ModuleKind::Oscillator, "plaits" => ModuleKind::Plaits, "adsr" => ModuleKind::Adsr,
            "vca" => ModuleKind::Vca, "low_pass" => ModuleKind::LowPass, "audio_output" => ModuleKind::AudioOutput,
            "lfo" => ModuleKind::Lfo, "control_scale" => ModuleKind::ControlScale, "control_multiply" => ModuleKind::ControlMultiply,
            "pitch_modulate" => ModuleKind::PitchModulate,
            "control_hertz" => ModuleKind::ControlHertz,
            "control_attenuate" => ModuleKind::ControlAttenuate,
            "gate_hold" => ModuleKind::GateHold,
            _ => return Err(format!("unsupported synth module '{kind}'")),
        } }),
        ["remove", id, policy] => Ok(PatchOperation::Remove { id: (*id).into(), cables: match *policy {
            "reject" => DependentCables::Reject, "disconnect" => DependentCables::Disconnect,
            _ => return Err("remove policy must be reject or disconnect".into()),
        } }),
        ["connect", from, to] => Ok(PatchOperation::Connect { from: (*from).into(), to: (*to).into() }),
        ["disconnect", from, to] => Ok(PatchOperation::Disconnect { from: (*from).into(), to: (*to).into() }),
        ["hertz", module, gain, offset, min, max] => Ok(PatchOperation::ConfigureControlHertz { module: (*module).into(), settings: meldritch_dsl::ControlHertzSettings {
            gain_hz: gain.parse().map_err(|_| "Hertz gain requires a number")?, offset_hz: offset.parse().map_err(|_| "Hertz offset requires a number")?,
            clamp_min_hz: min.parse().map_err(|_| "Hertz minimum requires a number")?, clamp_max_hz: max.parse().map_err(|_| "Hertz maximum requires a number")?,
        } }),
        ["pitch-depth", module, depth] => Ok(PatchOperation::ConfigurePitchModulation { module: (*module).into(), settings: meldritch_dsl::PitchModulationSettings {
            depth_semitones: depth.parse().map_err(|_| "pitch depth requires semitones")?,
        } }),
        ["lfo", module, shape, rate_kind, value, "phase", phase, "reset", reset] => {
            use meldritch_dsl::{GraphLfoSettings, GraphLfoRate, GraphLfoShape, LfoReset};
            let value = value.parse().map_err(|_| "LFO rate requires a number")?;
            Ok(PatchOperation::ConfigureLfo { module: (*module).into(), settings: GraphLfoSettings {
                shape: match *shape { "sine" => GraphLfoShape::Sine, "triangle" => GraphLfoShape::Triangle, "saw" => GraphLfoShape::Saw, "square" => GraphLfoShape::Square, _ => return Err("LFO shape requires sine/triangle/saw/square".into()) },
                rate: match *rate_kind { "beats" => GraphLfoRate::Beats(value), "hertz" => GraphLfoRate::Hertz(value), _ => return Err("LFO rate requires beats or hertz".into()) },
                phase: phase.parse().map_err(|_| "LFO phase requires a number")?,
                reset: match *reset { "none" => LfoReset::None, "gate_rise" => LfoReset::GateRise, _ => return Err("LFO reset requires none or gate_rise".into()) },
                ..GraphLfoSettings::default()
            } })
        }
        ["scale", module, gain, offset, min, max] => Ok(PatchOperation::ConfigureControlScale { module: (*module).into(), settings: meldritch_dsl::ControlScaleSettings {
            gain: gain.parse().map_err(|_| "scale gain requires a number")?, offset: offset.parse().map_err(|_| "scale offset requires a number")?,
            clamp_min: min.parse().map_err(|_| "scale minimum requires a number")?, clamp_max: max.parse().map_err(|_| "scale maximum requires a number")?,
        } }),
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
