//! Fixed-block adapter to the pinned upstream crate. Runs on the render worker.
//! Only this external module uses f32; all surrounding DSP and source use f64.
use meldritch_dsl::{PlaitsModel, PlaitsSettings};
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

pub(crate) const BLOCK_SIZE: usize = 24;
// Keep the historical two engines inline. Other fixed buffers are allocated on the worker.
#[allow(clippy::large_enum_variant)]
enum EngineState {
    Fm(FmEngine),
    Harmonic(AdditiveEngine),
    Other(Box<dyn Engine>),
}
pub(crate) struct PlaitsSource {
    engine: EngineState,
    settings: PlaitsSettings,
    main: [f32; BLOCK_SIZE],
    aux: [f32; BLOCK_SIZE],
    cursor: usize,
    use_aux: bool,
    patched_gate: bool,
    gate_high: bool,
    pending_trigger: bool,
    accent: f32,
}

impl PlaitsSource {
    pub(crate) fn new(settings: PlaitsSettings, use_aux: bool) -> Self {
        Self::with_gate(settings, use_aux, false)
    }
    pub(crate) fn with_gate(settings: PlaitsSettings, use_aux: bool, patched_gate: bool) -> Self {
        let mut engine = match settings.model {
            PlaitsModel::Fm => EngineState::Fm(FmEngine::new()),
            PlaitsModel::Harmonic => EngineState::Harmonic(AdditiveEngine::new()),
            model => EngineState::Other(create_engine(model)),
        };
        match &mut engine {
            EngineState::Other(e) => {
                e.init(48_000.0);
                e.reset();
            }
            EngineState::Fm(e) => {
                e.init(48_000.0);
                e.reset();
            }
            EngineState::Harmonic(e) => {
                e.init(48_000.0);
                e.reset();
            }
        }
        Self {
            engine,
            settings,
            main: [0.0; BLOCK_SIZE],
            aux: [0.0; BLOCK_SIZE],
            cursor: BLOCK_SIZE,
            use_aux,
            patched_gate,
            gate_high: false,
            pending_trigger: false,
            accent: 1.0,
        }
    }
    pub(crate) fn note_on(&mut self, velocity: f64) {
        self.gate_high = true;
        self.pending_trigger = true;
        self.accent = velocity as f32;
    }
    pub(crate) fn note_off(&mut self) {
        self.gate_high = false;
    }
    pub(crate) fn next(&mut self, frequency: f64) -> f64 {
        if self.cursor == BLOCK_SIZE {
            let parameters = EngineParameters {
                note: (69.0 + 12.0 * (frequency / 440.0).log2()) as f32,
                harmonics: self.settings.harmonics as f32,
                timbre: self.settings.timbre as f32,
                morph: self.settings.morph as f32,
                a0_normalized: 55.0 / 48_000.0,
                accent: self.accent,
                trigger: if !self.patched_gate {
                    TriggerState::Unpatched
                } else if self.pending_trigger {
                    TriggerState::RisingEdge
                } else if self.gate_high {
                    TriggerState::High
                } else {
                    TriggerState::Low
                },
            };
            self.pending_trigger = false;
            let mut already_enveloped = false;
            match &mut self.engine {
                EngineState::Other(e) => e.render(
                    &parameters,
                    &mut self.main,
                    &mut self.aux,
                    &mut already_enveloped,
                ),
                EngineState::Fm(e) => e.render(
                    &parameters,
                    &mut self.main,
                    &mut self.aux,
                    &mut already_enveloped,
                ),
                EngineState::Harmonic(e) => e.render(
                    &parameters,
                    &mut self.main,
                    &mut self.aux,
                    &mut already_enveloped,
                ),
            }
            self.cursor = 0;
        }
        let sample = if self.use_aux {
            self.aux[self.cursor]
        } else {
            self.main[self.cursor]
        };
        self.cursor += 1;
        f64::from(sample)
    }
}

// Only constructors and immutable bundled resources are used; no Voice/LPG wrapper.
fn create_engine(model: PlaitsModel) -> Box<dyn Engine> {
    match model {
        PlaitsModel::VirtualAnalog => Box::new(VirtualAnalogEngine::new(BLOCK_SIZE)),
        PlaitsModel::Waveshaping => Box::new(WaveshapingEngine::new()),
        PlaitsModel::Grain => Box::new(GrainEngine::new()),
        PlaitsModel::Wavetable => Box::new(WavetableEngine::new()),
        PlaitsModel::Chord => Box::new(ChordEngine::new()),
        PlaitsModel::Speech => Box::new(SpeechEngine::new(BLOCK_SIZE)),
        PlaitsModel::Swarm => Box::new(SwarmEngine::new()),
        PlaitsModel::Noise => Box::new(NoiseEngine::new(BLOCK_SIZE)),
        PlaitsModel::Particle => Box::new(ParticleEngine::new(BLOCK_SIZE)),
        PlaitsModel::String => Box::new(StringEngine::new(BLOCK_SIZE)),
        PlaitsModel::Modal => Box::new(ModalEngine::new(BLOCK_SIZE)),
        PlaitsModel::BassDrum => Box::new(BassDrumEngine::new()),
        PlaitsModel::SnareDrum => Box::new(SnareDrumEngine::new()),
        PlaitsModel::Hihat => Box::new(HihatEngine::new(BLOCK_SIZE)),
        PlaitsModel::VirtualAnalogVcf => Box::new(VirtualAnalogVcfEngine::new()),
        PlaitsModel::PhaseDistortion => Box::new(PhaseDistortionEngine::new(BLOCK_SIZE)),
        PlaitsModel::WaveTerrain => Box::new(WaveTerrainEngine::new(BLOCK_SIZE)),
        PlaitsModel::StringMachine => Box::new(StringMachineEngine::new()),
        PlaitsModel::Chiptune => Box::new(ChiptuneEngine::new()),
        PlaitsModel::SixOpA | PlaitsModel::SixOpB | PlaitsModel::SixOpC => {
            let mut e = SixOpEngine::new(BLOCK_SIZE);
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
    use super::*;
    #[test]
    fn gate_edges_latch_until_next_block_and_releases_reach_upstream() {
        for use_aux in [false, true] {
            let settings = PlaitsSettings {
                model: PlaitsModel::SixOpA,
                harmonics: 0.4,
                timbre: 0.35,
                morph: 0.5,
            };
            let mut adapter = PlaitsSource::with_gate(settings, use_aux, true);
            let mut reference = SixOpEngine::new(BLOCK_SIZE);
            reference.load_syx_bank(&SYX_BANK_0);
            reference.init(48000.0);
            reference.reset();
            let mut main = [0.0; BLOCK_SIZE];
            let mut aux = [0.0; BLOCK_SIZE];
            for block in 0..8 {
                let trigger = match block {
                    0 | 3 => TriggerState::Low,
                    1 | 4 => TriggerState::RisingEdge,
                    2 => TriggerState::High,
                    _ => TriggerState::Low,
                };
                let parameters = EngineParameters {
                    note: 69.0,
                    harmonics: 0.4,
                    timbre: 0.35,
                    morph: 0.5,
                    a0_normalized: 55.0 / 48000.0,
                    accent: if block == 0 { 1.0 } else { 0.8 },
                    trigger,
                };
                reference.render(&parameters, &mut main, &mut aux, &mut false);
                let expected = if use_aux { aux } else { main };
                for (offset, sample) in expected.into_iter().enumerate() {
                    let frame = block * BLOCK_SIZE + offset;
                    if frame == 7 || frame == 79 {
                        adapter.note_on(0.8);
                    }
                    if frame == 55 || frame == 80 {
                        adapter.note_off();
                    }
                    assert_eq!(adapter.next(440.0), f64::from(sample), "frame {frame}");
                }
            }
        }
    }
    #[test]
    fn adapter_equals_independent_upstream_blocks_for_main_and_aux() {
        for model in [
            PlaitsModel::Fm,
            PlaitsModel::Harmonic,
            PlaitsModel::VirtualAnalog,
            PlaitsModel::Waveshaping,
            PlaitsModel::Grain,
            PlaitsModel::Wavetable,
            PlaitsModel::Chord,
            PlaitsModel::VirtualAnalogVcf,
            PlaitsModel::PhaseDistortion,
            PlaitsModel::WaveTerrain,
        ] {
            for use_aux in [false, true] {
                let settings = PlaitsSettings {
                    model,
                    harmonics: 0.4,
                    timbre: 0.35,
                    morph: 0.5,
                };
                let mut adapter = PlaitsSource::new(settings, use_aux);
                let mut reference: Box<dyn Engine> = match model {
                    PlaitsModel::Fm => Box::new(FmEngine::new()),
                    PlaitsModel::Harmonic => Box::new(AdditiveEngine::new()),
                    model => create_engine(model),
                };
                reference.init(48_000.0);
                reference.reset();
                let p = EngineParameters {
                    note: 69.0,
                    harmonics: 0.4,
                    timbre: 0.35,
                    morph: 0.5,
                    a0_normalized: 55.0 / 48_000.0,
                    trigger: TriggerState::Unpatched,
                    accent: 1.0,
                };
                for _ in 0..64 {
                    let mut main = [0.0; BLOCK_SIZE];
                    let mut aux = [0.0; BLOCK_SIZE];
                    reference.render(&p, &mut main, &mut aux, &mut false);
                    let expected = if use_aux { aux } else { main };
                    for sample in expected {
                        assert_eq!(adapter.next(440.0), f64::from(sample));
                    }
                }
            }
        }
    }
}
