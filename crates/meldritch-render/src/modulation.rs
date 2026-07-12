//! Deterministic free-running and tempo-synchronised modulation sources.

use meldritch_core::{Frame, Tempo};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LfoShape {
    Sine,
    Triangle,
    Saw,
    Square,
    SampleAndHold,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LfoRate {
    Hertz(f64),
    /// Length of one LFO cycle in quarter-note beats.
    Beats(f64),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lfo {
    pub shape: LfoShape,
    pub rate: LfoRate,
    pub phase: f64,
    pub seed: u64,
}

impl Lfo {
    #[must_use]
    pub fn value_at(self, frame: Frame, tempo: Tempo) -> f64 {
        let cycles = match self.rate {
            LfoRate::Hertz(hz) => {
                frame as f64 * hz.clamp(0.001, 100.0) / f64::from(tempo.sample_rate())
            }
            LfoRate::Beats(beats) => {
                frame as f64 / (tempo.frames_per_beat() * beats.clamp(1.0 / 64.0, 256.0))
            }
        };
        let cycle = cycles.floor() as u64;
        let phase = (cycles + self.phase.clamp(0.0, 1.0)).fract();
        match self.shape {
            LfoShape::Sine => (std::f64::consts::TAU * phase).sin(),
            LfoShape::Triangle => 1.0 - 4.0 * (phase - 0.5).abs(),
            LfoShape::Saw => phase.mul_add(2.0, -1.0),
            LfoShape::Square => {
                if phase < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
            LfoShape::SampleAndHold => deterministic_bipolar(self.seed, cycle),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ModulationDestination {
    FilterOctaves,
    Resonance,
    DriveOctaves,
    Level,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModulationPolarity {
    Bipolar,
    Unipolar,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModulationRoute {
    pub source: Lfo,
    pub destination: ModulationDestination,
    pub depth: f64,
    pub polarity: ModulationPolarity,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModulationMatrix {
    routes: Vec<ModulationRoute>,
}

impl ModulationMatrix {
    #[must_use]
    pub fn new(routes: Vec<ModulationRoute>) -> Self {
        Self { routes }
    }

    #[must_use]
    pub fn routes(&self) -> &[ModulationRoute] {
        &self.routes
    }

    #[must_use]
    pub fn value_at(&self, destination: ModulationDestination, frame: Frame, tempo: Tempo) -> f64 {
        self.routes
            .iter()
            .filter(|route| route.destination == destination)
            .map(|route| {
                let value = route.source.value_at(frame, tempo);
                let value = match route.polarity {
                    ModulationPolarity::Bipolar => value,
                    ModulationPolarity::Unipolar => value.mul_add(0.5, 0.5),
                };
                value * route.depth.clamp(-8.0, 8.0)
            })
            .sum::<f64>()
            .clamp(-8.0, 8.0)
    }
}

fn deterministic_bipolar(seed: u64, cycle: u64) -> f64 {
    let mut value = seed ^ cycle.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^= value >> 31;
    (value as f64 / u64::MAX as f64).mul_add(2.0, -1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tempo_lfo_tracks_beats_and_free_lfo_tracks_seconds() {
        let tempo = Tempo::new(120.0, 48_000).unwrap();
        let synced = Lfo {
            shape: LfoShape::Sine,
            rate: LfoRate::Beats(2.0),
            phase: 0.0,
            seed: 0,
        };
        let free = Lfo {
            rate: LfoRate::Hertz(1.0),
            ..synced
        };
        assert!(synced.value_at(0, tempo).abs() < 1e-12);
        assert!((synced.value_at(12_000, tempo) - 1.0).abs() < 1e-12);
        assert!(free.value_at(24_000, tempo).abs() < 1e-12);
    }

    #[test]
    fn sample_and_hold_is_seeded_stable_and_changes_per_cycle() {
        let tempo = Tempo::new(120.0, 48_000).unwrap();
        let lfo = Lfo {
            shape: LfoShape::SampleAndHold,
            rate: LfoRate::Beats(1.0),
            phase: 0.0,
            seed: 808,
        };
        assert_eq!(lfo.value_at(0, tempo), lfo.value_at(23_999, tempo));
        assert_ne!(lfo.value_at(0, tempo), lfo.value_at(24_000, tempo));
        assert_eq!(lfo.value_at(24_000, tempo), lfo.value_at(24_000, tempo));
    }

    #[test]
    fn matrix_sums_routes_with_polarity_and_bounded_depth() {
        let tempo = Tempo::new(120.0, 48_000).unwrap();
        let source = Lfo {
            shape: LfoShape::Square,
            rate: LfoRate::Beats(1.0),
            phase: 0.0,
            seed: 0,
        };
        let matrix = ModulationMatrix::new(vec![
            ModulationRoute {
                source,
                destination: ModulationDestination::FilterOctaves,
                depth: 2.0,
                polarity: ModulationPolarity::Bipolar,
            },
            ModulationRoute {
                source,
                destination: ModulationDestination::FilterOctaves,
                depth: 1.0,
                polarity: ModulationPolarity::Unipolar,
            },
        ]);
        assert_eq!(
            matrix.value_at(ModulationDestination::FilterOctaves, 0, tempo),
            3.0
        );
        assert_eq!(
            matrix.value_at(ModulationDestination::Resonance, 0, tempo),
            0.0
        );
    }
}
