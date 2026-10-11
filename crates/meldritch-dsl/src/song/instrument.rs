//! The first instrument-authoring subset; edits preserve the existing graph.
use super::*;
use serde::{Deserialize, Serialize};

type ParameterSpec = (
    InstrumentParameter,
    InstrumentValue,
    &'static str,
    f64,
    Option<f64>,
    bool,
);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstrumentParameter {
    Waveform,
    Attack,
    Decay,
    Sustain,
    Release,
    CutoffHz,
    Resonance,
    Model,
    Harmonics,
    Timbre,
    Morph,
}

impl InstrumentParameter {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Waveform => "waveform",
            Self::Attack => "attack",
            Self::Decay => "decay",
            Self::Sustain => "sustain",
            Self::Release => "release",
            Self::CutoffHz => "cutoff_hz",
            Self::Resonance => "resonance",
            Self::Model => "model",
            Self::Harmonics => "harmonics",
            Self::Timbre => "timbre",
            Self::Morph => "morph",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstrumentWaveform {
    Sine,
    Triangle,
    Saw,
    Pulse,
    Square,
}

impl InstrumentWaveform {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Sine => "sine",
            Self::Triangle => "triangle",
            Self::Saw => "saw",
            Self::Pulse => "pulse",
            Self::Square => "square",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum InstrumentValue {
    Number(f64),
    Waveform(InstrumentWaveform),
    Model(PlaitsModel),
}

impl InstrumentValue {
    pub fn same_as(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Number(a), Self::Number(b)) => a.to_bits() == b.to_bits(),
            _ => self == other,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub enum InstrumentParameterOwner {
    Source,
    Automation(Vec<String>),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InstrumentParameterId {
    pub synth: String,
    pub module: String,
    pub parameter: InstrumentParameter,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InstrumentEdit {
    pub target: InstrumentParameterId,
    pub value: InstrumentValue,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub enum InstrumentRange {
    Number {
        minimum: f64,
        maximum: Option<f64>,
        minimum_inclusive: bool,
    },
    Waveform,
    Model,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct InstrumentParameterDefinition {
    pub id: InstrumentParameterId,
    pub module_kind: String,
    pub unit: String,
    pub range: InstrumentRange,
    pub value: InstrumentValue,
    pub owner: InstrumentParameterOwner,
}

impl ValidatedSong {
    /// Only parameters used by the existing note compiler are editable.
    /// Structural/automated values are not silently translated into live overrides.
    pub fn instrument_parameters(
        &self,
        synth: &str,
    ) -> Result<Vec<InstrumentParameterDefinition>, String> {
        let synth_def = self
            .synths
            .get(synth)
            .ok_or_else(|| format!("unknown instrument '{synth}'"))?;
        let mut values = Vec::new();
        for module in &synth_def.modules {
            let (kind, parameters): (&str, Vec<ParameterSpec>) = match module.kind {
                ModuleKind::Oscillator => {
                    let name = module.waveform.as_deref().unwrap_or("sine");
                    let waveform = match name {
                        "sine" => InstrumentWaveform::Sine,
                        "triangle" => InstrumentWaveform::Triangle,
                        "saw" => InstrumentWaveform::Saw,
                        "pulse" => InstrumentWaveform::Pulse,
                        "square" => InstrumentWaveform::Square,
                        _ => return Err(format!("unsupported instrument waveform '{name}'")),
                    };
                    (
                        "oscillator",
                        vec![(
                            InstrumentParameter::Waveform,
                            InstrumentValue::Waveform(waveform),
                            "choice",
                            0.0,
                            None,
                            true,
                        )],
                    )
                }
                ModuleKind::Plaits => {
                    let p = module.plaits.unwrap();
                    (
                        "plaits",
                        vec![
                            (
                                InstrumentParameter::Model,
                                InstrumentValue::Model(p.model),
                                "choice",
                                0.0,
                                None,
                                true,
                            ),
                            (
                                InstrumentParameter::Harmonics,
                                InstrumentValue::Number(p.harmonics),
                                "normalized",
                                0.0,
                                Some(1.0),
                                true,
                            ),
                            (
                                InstrumentParameter::Timbre,
                                InstrumentValue::Number(p.timbre),
                                "normalized",
                                0.0,
                                Some(1.0),
                                true,
                            ),
                            (
                                InstrumentParameter::Morph,
                                InstrumentValue::Number(p.morph),
                                "normalized",
                                0.0,
                                Some(1.0),
                                true,
                            ),
                        ],
                    )
                }
                ModuleKind::Adsr => (
                    "adsr",
                    vec![
                        (
                            InstrumentParameter::Attack,
                            InstrumentValue::Number(module.attack.unwrap_or(0.0)),
                            "seconds",
                            0.0,
                            None,
                            true,
                        ),
                        (
                            InstrumentParameter::Decay,
                            InstrumentValue::Number(module.decay.unwrap_or(0.0)),
                            "seconds",
                            0.0,
                            None,
                            true,
                        ),
                        (
                            InstrumentParameter::Sustain,
                            InstrumentValue::Number(module.sustain.unwrap_or(1.0)),
                            "level",
                            0.0,
                            Some(1.0),
                            true,
                        ),
                        (
                            InstrumentParameter::Release,
                            InstrumentValue::Number(module.release.unwrap_or(0.0)),
                            "seconds",
                            0.0,
                            None,
                            true,
                        ),
                    ],
                ),
                ModuleKind::LowPass => (
                    "low_pass",
                    vec![
                        (
                            InstrumentParameter::CutoffHz,
                            InstrumentValue::Number(module.cutoff_hz.unwrap_or(20_000.0)),
                            "Hz",
                            0.0,
                            None,
                            false,
                        ),
                        (
                            InstrumentParameter::Resonance,
                            InstrumentValue::Number(module.resonance.unwrap_or(0.0)),
                            "level",
                            0.0,
                            Some(1.0),
                            true,
                        ),
                    ],
                ),
                _ => continue,
            };
            for (parameter, value, unit, minimum, maximum, minimum_inclusive) in parameters {
                let range = if parameter == InstrumentParameter::Waveform {
                    InstrumentRange::Waveform
                } else if parameter == InstrumentParameter::Model {
                    InstrumentRange::Model
                } else {
                    InstrumentRange::Number {
                        minimum,
                        maximum,
                        minimum_inclusive,
                    }
                };
                let patterns = self
                    .parameter_patterns
                    .values()
                    .filter(|pattern| {
                        pattern.lanes.iter().any(|lane| {
                            lane.target.owner == ParameterOwner::Synth
                                && lane.target.definition_id == synth
                                && lane.target.module_id == module.id
                                && lane.target.parameter == parameter.name()
                        })
                    })
                    .map(|pattern| pattern.id.clone())
                    .collect::<Vec<_>>();
                values.push(InstrumentParameterDefinition {
                    id: InstrumentParameterId {
                        synth: synth.to_owned(),
                        module: module.id.clone(),
                        parameter,
                    },
                    module_kind: kind.to_owned(),
                    unit: unit.to_owned(),
                    range,
                    value,
                    owner: if patterns.is_empty() {
                        InstrumentParameterOwner::Source
                    } else {
                        InstrumentParameterOwner::Automation(patterns)
                    },
                });
            }
        }
        values.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(values)
    }

    /// Copy-on-edit validated revision. No files, Lua or render state are mutated.
    pub fn edit_instrument(&self, edit: &InstrumentEdit) -> Result<Self, String> {
        let descriptor = self
            .instrument_parameters(&edit.target.synth)?
            .into_iter()
            .find(|p| p.id == edit.target)
            .ok_or_else(|| {
                format!(
                    "unknown or unsupported instrument parameter {:?}",
                    edit.target
                )
            })?;
        match (&descriptor.range, &edit.value) {
            (InstrumentRange::Waveform, InstrumentValue::Waveform(_)) => {}
            (InstrumentRange::Model, InstrumentValue::Model(_)) => {}
            (
                InstrumentRange::Number {
                    minimum,
                    maximum,
                    minimum_inclusive,
                },
                InstrumentValue::Number(value),
            ) if value.is_finite()
                && (if *minimum_inclusive {
                    *value >= *minimum
                } else {
                    *value > *minimum
                })
                && maximum.is_none_or(|max| *value <= max) => {}
            _ => {
                return Err(format!(
                    "parameter {:?} requires {:?}; received {:?}",
                    edit.target, descriptor.range, edit.value
                ));
            }
        }
        if descriptor.value.same_as(&edit.value) {
            return Ok(self.clone());
        }
        if descriptor.owner != InstrumentParameterOwner::Source {
            return Err(format!(
                "parameter {:?} is owned by authored automation; detach it before editing",
                edit.target
            ));
        }
        let mut next = self.clone();
        let module = next
            .synths
            .get_mut(&edit.target.synth)
            .unwrap()
            .modules
            .iter_mut()
            .find(|m| m.id == edit.target.module)
            .unwrap();
        match (&edit.target.parameter, &edit.value) {
            (InstrumentParameter::Waveform, InstrumentValue::Waveform(value)) => {
                module.waveform = Some(value.name().to_owned())
            }
            (InstrumentParameter::Model, InstrumentValue::Model(value)) => {
                module.plaits.as_mut().unwrap().model = *value
            }
            (parameter, InstrumentValue::Number(value)) => match parameter {
                InstrumentParameter::Attack => module.attack = Some(*value),
                InstrumentParameter::Decay => module.decay = Some(*value),
                InstrumentParameter::Sustain => module.sustain = Some(*value),
                InstrumentParameter::Release => module.release = Some(*value),
                InstrumentParameter::CutoffHz => module.cutoff_hz = Some(*value),
                InstrumentParameter::Resonance => module.resonance = Some(*value),
                InstrumentParameter::Harmonics => {
                    module.plaits.as_mut().unwrap().harmonics = *value
                }
                InstrumentParameter::Timbre => module.plaits.as_mut().unwrap().timbre = *value,
                InstrumentParameter::Morph => module.plaits.as_mut().unwrap().morph = *value,
                _ => unreachable!(),
            },
            _ => unreachable!(),
        }
        next.fingerprint = fingerprint_song(
            &next.performance,
            &next.synths,
            &next.samplers,
            &next.note_patterns,
            &next.parameter_patterns,
            &next.dsps,
            &next.sample_banks,
        );
        Ok(next)
    }
}
