//! Supported authoring identities resolved against the existing source graph.
use super::*;
use serde::{Deserialize, Serialize};
pub(super) const fn unity_volume() -> f64 {
    1.0
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum EditableTarget {
    Synth {
        id: String,
        module: String,
        parameter: InstrumentParameter,
    },
    Dsp {
        id: String,
        module: String,
        parameter: DelayParameter,
    },
    Track {
        id: String,
        parameter: MixerParameter,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DelayParameter {
    Feedback,
    Mix,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MixerParameter {
    Volume,
    Pan,
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ParameterEdit {
    pub target: EditableTarget,
    pub value: f64,
}
#[derive(Clone, Debug, PartialEq)]
pub struct EditableParameter {
    pub value: f64,
    pub minimum: f64,
    pub maximum: f64,
    pub step: f64,
    pub unit: &'static str,
}
impl EditableTarget {
    pub fn identity(&self) -> String {
        match self {
            Self::Synth {
                id,
                module,
                parameter,
            } => format!("synth:{id}/{module}.{}", parameter.name()),
            Self::Dsp {
                id,
                module,
                parameter,
            } => format!(
                "dsp:{id}/{module}.{}",
                if *parameter == DelayParameter::Mix {
                    "mix"
                } else {
                    "feedback"
                }
            ),
            Self::Track { id, parameter } => format!(
                "track:{id}/{}",
                if *parameter == MixerParameter::Pan {
                    "pan"
                } else {
                    "volume"
                }
            ),
        }
    }
}
impl ValidatedSong {
    pub fn editable_parameter(&self, target: &EditableTarget) -> Result<EditableParameter, String> {
        let (value, minimum, maximum, step, unit) = match target {
            EditableTarget::Synth {
                id,
                module,
                parameter,
            } => {
                if !matches!(
                    parameter,
                    InstrumentParameter::CutoffHz | InstrumentParameter::Resonance
                ) {
                    return Err("faders support numeric synth cutoff/resonance targets".into());
                }
                let descriptor = self
                    .instrument_parameters(id)?
                    .into_iter()
                    .find(|p| p.id.module == *module && p.id.parameter == *parameter)
                    .ok_or_else(|| format!("missing parameter {}", target.identity()))?;
                if descriptor.owner != InstrumentParameterOwner::Source {
                    return Err(
                        "parameter is owned by automation; choose a source-owned target".into(),
                    );
                }
                let InstrumentValue::Number(value) = descriptor.value else {
                    return Err("fader target must be numeric".into());
                };
                let InstrumentRange::Number {
                    minimum,
                    maximum,
                    minimum_inclusive,
                } = descriptor.range
                else {
                    return Err("fader target needs a numeric range".into());
                };
                let maximum = maximum.unwrap_or(f64::MAX);
                let minimum = if minimum_inclusive {
                    minimum
                } else {
                    f64::from_bits(1)
                };
                (
                    value,
                    minimum,
                    maximum,
                    if *parameter == InstrumentParameter::CutoffHz {
                        1.0
                    } else {
                        0.001
                    },
                    if *parameter == InstrumentParameter::CutoffHz {
                        "Hz"
                    } else {
                        "ratio"
                    },
                )
            }
            EditableTarget::Dsp {
                id,
                module,
                parameter,
            } => {
                let dsp = self
                    .dsps
                    .get(id)
                    .ok_or_else(|| format!("missing DSP '{id}'"))?;
                if self.performance.tracks.len() != 1
                    || !self.performance.tracks[0].dsp_ids.contains(id)
                {
                    return Err(
                        "editable DSP targets require the supported single-track delay render path"
                            .into(),
                    );
                }
                let module = dsp
                    .modules
                    .iter()
                    .find(|m| m.id == *module && m.kind == ModuleKind::TempoDelay)
                    .ok_or_else(|| {
                        format!("missing tempo-delay parameter {}", target.identity())
                    })?;
                let name = if *parameter == DelayParameter::Mix {
                    "mix"
                } else {
                    "feedback"
                };
                if self
                    .parameter_patterns
                    .values()
                    .flat_map(|p| &p.lanes)
                    .any(|p| {
                        p.target.owner == ParameterOwner::Dsp
                            && p.target.definition_id == *id
                            && p.target.module_id == module.id
                            && p.target.parameter == name
                    })
                {
                    return Err("DSP parameter is owned by automation".into());
                }
                if *parameter == DelayParameter::Mix {
                    (module.mix.unwrap_or(0.0), 0.0, 1.0, 0.001, "ratio")
                } else {
                    (module.feedback.unwrap_or(0.0), 0.0, 0.99, 0.001, "ratio")
                }
            }
            EditableTarget::Track { id, parameter } => {
                let track = self
                    .performance
                    .tracks
                    .iter()
                    .find(|t| t.id == *id)
                    .ok_or_else(|| format!("missing mixer track '{id}'"))?;
                if *parameter == MixerParameter::Pan {
                    (track.pan.unwrap_or(0.0), -1.0, 1.0, 0.001, "balance")
                } else {
                    (track.volume, 0.0, 2.0, 0.001, "gain")
                }
            }
        };
        Ok(EditableParameter {
            value,
            minimum,
            maximum,
            step,
            unit,
        })
    }
    pub fn edit_parameter(&self, edit: &ParameterEdit) -> Result<Self, String> {
        let spec = self.editable_parameter(&edit.target)?;
        if !edit.value.is_finite() || !(spec.minimum..=spec.maximum).contains(&edit.value) {
            return Err(format!(
                "{} must be finite in {}..{} {}",
                edit.target.identity(),
                spec.minimum,
                spec.maximum,
                spec.unit
            ));
        }
        if edit.value == spec.value
            && !matches!(&edit.target, EditableTarget::Track {id, parameter: MixerParameter::Pan} if self.performance.tracks.iter().any(|t| t.id == *id && t.pan.is_none()))
        {
            return Ok(self.clone());
        }
        let mut next = match &edit.target {
            EditableTarget::Synth {
                id,
                module,
                parameter,
            } => self.edit_instrument(&InstrumentEdit {
                target: InstrumentParameterId {
                    synth: id.clone(),
                    module: module.clone(),
                    parameter: *parameter,
                },
                value: InstrumentValue::Number(edit.value),
            })?,
            _ => self.clone(),
        };
        match &edit.target {
            EditableTarget::Track { id, parameter } => {
                let track = next
                    .performance
                    .tracks
                    .iter_mut()
                    .find(|t| t.id == *id)
                    .unwrap();
                if *parameter == MixerParameter::Pan {
                    track.pan = Some(edit.value)
                } else {
                    track.volume = edit.value
                }
            }
            EditableTarget::Dsp {
                id,
                module,
                parameter,
            } => {
                let module = next
                    .dsps
                    .get_mut(id)
                    .unwrap()
                    .modules
                    .iter_mut()
                    .find(|m| m.id == *module)
                    .unwrap();
                if *parameter == DelayParameter::Mix {
                    module.mix = Some(edit.value)
                } else {
                    module.feedback = Some(edit.value)
                }
            }
            _ => {}
        }
        next.refresh_authoring_fingerprint();
        Ok(next)
    }
}
