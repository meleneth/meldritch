//! Fixed-block adapter to the pinned upstream crate. Runs on the render worker.
//! Only this external module uses f32; all surrounding DSP and source use f64.
use meldritch_dsl::{PlaitsModel, PlaitsSettings};
use mi_plaits_dsp::engine::{
    Engine, EngineParameters, additive_engine::AdditiveEngine, fm_engine::FmEngine,
};

pub(crate) const BLOCK_SIZE: usize = 24;
// Fixed inline state keeps construction and processing allocation-free.
#[allow(clippy::large_enum_variant)]
enum EngineState {
    Fm(FmEngine),
    Harmonic(AdditiveEngine),
}
pub(crate) struct PlaitsSource {
    engine: EngineState,
    settings: PlaitsSettings,
    main: [f32; BLOCK_SIZE],
    aux: [f32; BLOCK_SIZE],
    cursor: usize,
    use_aux: bool,
}

impl PlaitsSource {
    pub(crate) fn new(settings: PlaitsSettings, use_aux: bool) -> Self {
        let mut engine = match settings.model {
            PlaitsModel::Fm => EngineState::Fm(FmEngine::new()),
            PlaitsModel::Harmonic => EngineState::Harmonic(AdditiveEngine::new()),
        };
        match &mut engine {
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
        }
    }
    pub(crate) fn next(&mut self, frequency: f64) -> f64 {
        if self.cursor == BLOCK_SIZE {
            let parameters = EngineParameters {
                note: (69.0 + 12.0 * (frequency / 440.0).log2()) as f32,
                harmonics: self.settings.harmonics as f32,
                timbre: self.settings.timbre as f32,
                morph: self.settings.morph as f32,
                a0_normalized: 55.0 / 48_000.0,
                ..EngineParameters::default()
            };
            let mut already_enveloped = false;
            match &mut self.engine {
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn adapter_equals_independent_upstream_blocks_for_main_and_aux() {
        for model in [PlaitsModel::Fm, PlaitsModel::Harmonic] {
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
                };
                reference.init(48_000.0);
                reference.reset();
                let p = EngineParameters {
                    note: 69.0,
                    harmonics: 0.4,
                    timbre: 0.35,
                    morph: 0.5,
                    a0_normalized: 55.0 / 48_000.0,
                    ..EngineParameters::default()
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
