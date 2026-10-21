//! Prepared adapter for the pinned upstream Plaits engines. Upstream renders
//! fixed 24-sample f32 blocks; Meldritch converts only at this primitive edge.
use mi_plaits_dsp::engine::{
    Engine, EngineParameters, TriggerState, additive_engine::AdditiveEngine,
    bass_drum_engine::BassDrumEngine, chord_engine::ChordEngine, fm_engine::FmEngine,
    grain_engine::GrainEngine, hihat_engine::HihatEngine, modal_engine::ModalEngine,
    noise_engine::NoiseEngine, particle_engine::ParticleEngine, snare_drum_engine::SnareDrumEngine,
    speech_engine::SpeechEngine, string_engine::StringEngine, swarm_engine::SwarmEngine,
    virtual_analog_engine::VirtualAnalogEngine, waveshaping_engine::WaveshapingEngine,
    wavetable_engine::WavetableEngine,
};
use mi_plaits_dsp::engine2::{
    chiptune_engine::ChiptuneEngine, phase_distortion_engine::PhaseDistortionEngine,
    six_op_engine::SixOpEngine, string_machine_engine::StringMachineEngine,
    virtual_analog_vcf_engine::VirtualAnalogVcfEngine, wave_terrain_engine::WaveTerrainEngine,
};
use mi_plaits_dsp::resources::sysex::{SYX_BANK_0, SYX_BANK_1, SYX_BANK_2};

const BLOCK: usize = 24;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
        Ok(match name {
            "fm" => Self::Fm,
            "harmonic" => Self::Harmonic,
            "virtual_analog" => Self::VirtualAnalog,
            "waveshaping" => Self::Waveshaping,
            "grain" => Self::Grain,
            "wavetable" => Self::Wavetable,
            "chord" => Self::Chord,
            "speech" => Self::Speech,
            "swarm" => Self::Swarm,
            "noise" => Self::Noise,
            "particle" => Self::Particle,
            "string" => Self::String,
            "modal" => Self::Modal,
            "bass_drum" => Self::BassDrum,
            "snare_drum" => Self::SnareDrum,
            "hihat" => Self::Hihat,
            "virtual_analog_vcf" => Self::VirtualAnalogVcf,
            "phase_distortion" => Self::PhaseDistortion,
            "six_op_a" => Self::SixOpA,
            "six_op_b" => Self::SixOpB,
            "six_op_c" => Self::SixOpC,
            "wave_terrain" => Self::WaveTerrain,
            "string_machine" => Self::StringMachine,
            "chiptune" => Self::Chiptune,
            _ => return Err(format!("unknown Plaits model '{name}'")),
        })
    }
}

#[allow(clippy::large_enum_variant)]
enum EngineState {
    Fm(FmEngine),
    Harmonic(AdditiveEngine),
    Other(Box<dyn Engine>),
}

pub(crate) struct PlaitsRuntime {
    engine: EngineState,
    main: [f32; BLOCK],
    aux: [f32; BLOCK],
    cursor: usize,
    previous_gate: bool,
    sample_rate: f64,
}

impl std::fmt::Debug for PlaitsRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlaitsRuntime").finish_non_exhaustive()
    }
}

impl PlaitsRuntime {
    pub(crate) fn new(model: PlaitsModel, sample_rate: f64) -> Self {
        let mut engine = match model {
            PlaitsModel::Fm => EngineState::Fm(FmEngine::new()),
            PlaitsModel::Harmonic => EngineState::Harmonic(AdditiveEngine::new()),
            other => EngineState::Other(create_engine(other)),
        };
        let init = |engine: &mut dyn Engine| {
            engine.init(sample_rate as f32);
            engine.reset();
        };
        match &mut engine {
            EngineState::Fm(e) => init(e),
            EngineState::Harmonic(e) => init(e),
            EngineState::Other(e) => init(e.as_mut()),
        }
        Self {
            engine,
            main: [0.0; BLOCK],
            aux: [0.0; BLOCK],
            cursor: BLOCK,
            previous_gate: false,
            sample_rate,
        }
    }

    pub(crate) fn process(&mut self, input: &[f64], params: &[f64], out: &mut [f64]) {
        if self.cursor == BLOCK {
            let gate = input[1].is_finite() && input[1] >= 0.5;
            let frequency = if input[0].is_finite() && input[0] > 0.0 {
                input[0]
            } else {
                params[0]
            };
            let modulation = |value: f64| if value.is_finite() { value } else { 0.0 };
            let p = EngineParameters {
                note: (69.0 + 12.0 * (frequency.clamp(1.0, self.sample_rate * 0.49) / 440.0).log2())
                    as f32,
                harmonics: (params[1] + modulation(input[3])).clamp(0.0, 1.0) as f32,
                timbre: (params[2] + modulation(input[4])).clamp(0.0, 1.0) as f32,
                morph: (params[3] + modulation(input[5])).clamp(0.0, 1.0) as f32,
                a0_normalized: (55.0 / self.sample_rate) as f32,
                accent: modulation(input[2]).clamp(0.0, 1.0) as f32,
                trigger: if gate && !self.previous_gate {
                    TriggerState::RisingEdge
                } else if gate {
                    TriggerState::High
                } else {
                    TriggerState::Low
                },
            };
            self.previous_gate = gate;
            let mut enveloped = false;
            match &mut self.engine {
                EngineState::Fm(e) => e.render(&p, &mut self.main, &mut self.aux, &mut enveloped),
                EngineState::Harmonic(e) => {
                    e.render(&p, &mut self.main, &mut self.aux, &mut enveloped)
                }
                EngineState::Other(e) => {
                    e.render(&p, &mut self.main, &mut self.aux, &mut enveloped)
                }
            }
            self.cursor = 0;
        }
        out[0] = f64::from(self.main[self.cursor]);
        out[1] = f64::from(self.aux[self.cursor]);
        self.cursor += 1;
    }
}

fn create_engine(model: PlaitsModel) -> Box<dyn Engine> {
    match model {
        PlaitsModel::VirtualAnalog => Box::new(VirtualAnalogEngine::new(BLOCK)),
        PlaitsModel::Waveshaping => Box::new(WaveshapingEngine::new()),
        PlaitsModel::Grain => Box::new(GrainEngine::new()),
        PlaitsModel::Wavetable => Box::new(WavetableEngine::new()),
        PlaitsModel::Chord => Box::new(ChordEngine::new()),
        PlaitsModel::Speech => Box::new(SpeechEngine::new(BLOCK)),
        PlaitsModel::Swarm => Box::new(SwarmEngine::new()),
        PlaitsModel::Noise => Box::new(NoiseEngine::new(BLOCK)),
        PlaitsModel::Particle => Box::new(ParticleEngine::new(BLOCK)),
        PlaitsModel::String => Box::new(StringEngine::new(BLOCK)),
        PlaitsModel::Modal => Box::new(ModalEngine::new(BLOCK)),
        PlaitsModel::BassDrum => Box::new(BassDrumEngine::new()),
        PlaitsModel::SnareDrum => Box::new(SnareDrumEngine::new()),
        PlaitsModel::Hihat => Box::new(HihatEngine::new(BLOCK)),
        PlaitsModel::VirtualAnalogVcf => Box::new(VirtualAnalogVcfEngine::new()),
        PlaitsModel::PhaseDistortion => Box::new(PhaseDistortionEngine::new(BLOCK)),
        PlaitsModel::WaveTerrain => Box::new(WaveTerrainEngine::new(BLOCK)),
        PlaitsModel::StringMachine => Box::new(StringMachineEngine::new()),
        PlaitsModel::Chiptune => Box::new(ChiptuneEngine::new()),
        PlaitsModel::SixOpA | PlaitsModel::SixOpB | PlaitsModel::SixOpC => {
            let mut e = SixOpEngine::new(BLOCK);
            e.load_syx_bank(match model {
                PlaitsModel::SixOpA => &SYX_BANK_0,
                PlaitsModel::SixOpB => &SYX_BANK_1,
                _ => &SYX_BANK_2,
            });
            Box::new(e)
        }
        PlaitsModel::Fm => Box::new(FmEngine::new()),
        PlaitsModel::Harmonic => Box::new(AdditiveEngine::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::{PlaitsModel, PlaitsRuntime};
    #[test]
    fn every_exposed_upstream_model_initializes_resets_and_renders_finite_main_and_aux() {
        for model in PlaitsModel::ALL {
            let mut engine = PlaitsRuntime::new(model, 48_000.0);
            let mut out = [0.0; 2];
            for _ in 0..48 {
                engine.process(
                    &[440.0, 1.0, 0.8, 0.0, 0.0, 0.0],
                    &[440.0, 0.5, 0.5, 0.5],
                    &mut out,
                );
                assert!(out.iter().all(|v| v.is_finite()), "{model:?}");
            }
        }
    }
    #[test]
    fn recreation_is_deterministic_and_sample_rates_are_supported() {
        for rate in [44_100.0, 48_000.0, 96_000.0] {
            let mut a = PlaitsRuntime::new(PlaitsModel::Harmonic, rate);
            let mut b = PlaitsRuntime::new(PlaitsModel::Harmonic, rate);
            let mut left = [0.0; 2];
            let mut right = [0.0; 2];
            for _ in 0..96 {
                a.process(
                    &[220.0, 1.0, 1.0, 0.0, 0.0, 0.0],
                    &[220.0, 0.7, 0.55, 0.2],
                    &mut left,
                );
                b.process(
                    &[220.0, 1.0, 1.0, 0.0, 0.0, 0.0],
                    &[220.0, 0.7, 0.55, 0.2],
                    &mut right,
                );
                assert_eq!(left, right);
            }
        }
    }
}
