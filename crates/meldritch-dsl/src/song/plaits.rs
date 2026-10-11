//! Typed pinned sourcebox model choices; authoring values stay f64.
use super::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaitsModel {
    Fm,
    Harmonic,
    VirtualAnalog,
    Waveshaping,
    Grain,
    Wavetable,
    Chord,
    Speech,
    Swarm,
    Noise,
    Particle,
    String,
    Modal,
    BassDrum,
    SnareDrum,
    Hihat,
    VirtualAnalogVcf,
    PhaseDistortion,
    SixOpA,
    SixOpB,
    SixOpC,
    WaveTerrain,
    StringMachine,
    Chiptune,
}
impl PlaitsModel {
    /// Stable authoring/navigation order; existing FM and harmonic names are unchanged.
    pub const ALL: [Self; 24] = [
        Self::Fm,
        Self::Harmonic,
        Self::VirtualAnalog,
        Self::Waveshaping,
        Self::Grain,
        Self::Wavetable,
        Self::Chord,
        Self::Speech,
        Self::Swarm,
        Self::Noise,
        Self::Particle,
        Self::String,
        Self::Modal,
        Self::BassDrum,
        Self::SnareDrum,
        Self::Hihat,
        Self::VirtualAnalogVcf,
        Self::PhaseDistortion,
        Self::SixOpA,
        Self::SixOpB,
        Self::SixOpC,
        Self::WaveTerrain,
        Self::StringMachine,
        Self::Chiptune,
    ];
    pub fn parse(name: &str) -> Result<Self, String> {
        Self::ALL
            .into_iter()
            .find(|m| m.name() == name)
            .ok_or_else(|| {
                format!(
                    "unknown Plaits model '{name}'; supported: {}",
                    Self::ALL.map(Self::name).join(", ")
                )
            })
    }
    pub const fn name(self) -> &'static str {
        match self {
            Self::Fm => "fm",
            Self::Harmonic => "harmonic",
            Self::VirtualAnalog => "virtual_analog",
            Self::Waveshaping => "waveshaping",
            Self::Grain => "grain",
            Self::Wavetable => "wavetable",
            Self::Chord => "chord",
            Self::Speech => "speech",
            Self::Swarm => "swarm",
            Self::Noise => "noise",
            Self::Particle => "particle",
            Self::String => "string",
            Self::Modal => "modal",
            Self::BassDrum => "bass_drum",
            Self::SnareDrum => "snare_drum",
            Self::Hihat => "hihat",
            Self::VirtualAnalogVcf => "virtual_analog_vcf",
            Self::PhaseDistortion => "phase_distortion",
            Self::SixOpA => "six_op_a",
            Self::SixOpB => "six_op_b",
            Self::SixOpC => "six_op_c",
            Self::WaveTerrain => "wave_terrain",
            Self::StringMachine => "string_machine",
            Self::Chiptune => "chiptune",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlaitsSettings {
    pub model: PlaitsModel,
    pub harmonics: f64,
    pub timbre: f64,
    pub morph: f64,
}
impl Default for PlaitsSettings {
    fn default() -> Self {
        Self {
            model: PlaitsModel::Fm,
            harmonics: 0.5,
            timbre: 0.5,
            morph: 0.5,
        }
    }
}
pub(super) fn validate_plaits(
    path: &Path,
    kind: ModuleKind,
    raw: &RawModule,
) -> Result<Option<PlaitsSettings>, SongLoadError> {
    if kind != ModuleKind::Plaits {
        if raw.model.is_some()
            || raw.harmonics.is_some()
            || raw.timbre.is_some()
            || raw.morph.is_some()
        {
            return Err(SongLoadError::one(
                path,
                format!("module '{}' has Plaits settings but is not plaits", raw.id),
            ));
        }
        return Ok(None);
    }
    if raw.waveform.is_some()
        || raw.frequency_hz.is_some()
        || raw.channels.is_some()
        || raw.cutoff_hz.is_some()
        || raw.resonance.is_some()
        || raw.time.is_some()
        || raw.feedback.is_some()
        || raw.mix.is_some()
        || raw.attack.is_some()
        || raw.decay.is_some()
        || raw.sustain.is_some()
        || raw.release.is_some()
    {
        return Err(SongLoadError::one(
            path,
            format!(
                "Plaits module '{}' contains unsupported configuration fields",
                raw.id
            ),
        ));
    }
    let settings = PlaitsSettings {
        model: raw.model.unwrap_or(PlaitsModel::Fm),
        harmonics: raw.harmonics.unwrap_or(0.5),
        timbre: raw.timbre.unwrap_or(0.5),
        morph: raw.morph.unwrap_or(0.5),
    };
    for (name, value) in [
        ("harmonics", settings.harmonics),
        ("timbre", settings.timbre),
        ("morph", settings.morph),
    ] {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(SongLoadError::one(
                path,
                format!(
                    "Plaits module '{}'.{name} must be finite within 0..1",
                    raw.id
                ),
            ));
        }
    }
    Ok(Some(settings))
}
