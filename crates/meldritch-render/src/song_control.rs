//! Bounded worker-side lowering of explicit controls and pitch adapters.
use crate::modulation::{Lfo, LfoRate, LfoShape};
use meldritch_core::Tempo;
use meldritch_dsl::{
    ControlEvaluation, ControlHertzSettings, ControlScaleSettings, GraphLfoRate, GraphLfoSettings,
    GraphLfoShape, LfoReset, ModuleKind, SynthDefinition,
};

// Slot 0 is envelope, 1..=32 are bounded graph nodes; independent inputs
// do not participate in node ordering or consume module slots.
pub(crate) const VELOCITY_INPUT: usize = 33;
pub(crate) const BEND_INPUT: usize = 34;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ControlNode {
    Lfo {
        settings: GraphLfoSettings,
        rate: Option<usize>,
    },
    Scale {
        input: usize,
        settings: ControlScaleSettings,
    },
    Multiply {
        left: usize,
        right: usize,
    },
    PitchModulate {
        input: usize,
        depth_semitones: f64,
    },
    Hertz {
        input: usize,
        settings: ControlHertzSettings,
    },
}
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ControlPlan {
    pub nodes: Vec<ControlNode>,
    pub output: usize,
    pub pitch_offset: Option<usize>,
    pub cutoff: Option<usize>,
}
impl ControlPlan {
    pub fn compile(graph: &SynthDefinition) -> Result<Option<Self>, String> {
        let order = graph.control_order()?;
        if order.is_empty()
            && !graph
                .cables()
                .iter()
                .any(|c| matches!(c.from(), "input.velocity" | "input.bend"))
        {
            return Ok(None);
        }
        let envelope = graph
            .modules()
            .iter()
            .find(|m| m.kind() == ModuleKind::Adsr)
            .ok_or("missing envelope")?;
        let resolve = |endpoint: &str| -> Result<usize, String> {
            let source = graph
                .cables()
                .iter()
                .find(|c| c.to() == endpoint)
                .ok_or("missing control input")?
                .from();
            if source == "input.velocity" {
                return Ok(VELOCITY_INPUT);
            }
            if source == "input.bend" {
                return Ok(BEND_INPUT);
            }
            let (id, _) = source.split_once('.').ok_or("invalid control source")?;
            if id == envelope.id() {
                return Ok(0);
            }
            order
                .iter()
                .position(|node| node == id)
                .map(|index| index + 1)
                .ok_or_else(|| "missing lowered control source".into())
        };
        let mut nodes = vec![];
        for id in &order {
            let module = graph.modules().iter().find(|m| m.id() == id).unwrap();
            nodes.push(match module.kind() {
                ModuleKind::Lfo => ControlNode::Lfo {
                    settings: module.lfo().unwrap(),
                    rate: if graph
                        .cables()
                        .iter()
                        .any(|c| c.to() == format!("{id}.rate"))
                    {
                        Some(resolve(&format!("{id}.rate"))?)
                    } else {
                        None
                    },
                },
                ModuleKind::ControlScale => ControlNode::Scale {
                    input: resolve(&format!("{id}.control"))?,
                    settings: module.scale().unwrap(),
                },
                ModuleKind::ControlMultiply => ControlNode::Multiply {
                    left: resolve(&format!("{id}.left"))?,
                    right: resolve(&format!("{id}.right"))?,
                },
                ModuleKind::ControlAttenuate => ControlNode::Multiply {
                    left: resolve(&format!("{id}.control"))?,
                    right: resolve(&format!("{id}.amount"))?,
                },
                ModuleKind::PitchModulate => ControlNode::PitchModulate {
                    input: resolve(&format!("{id}.control"))?,
                    depth_semitones: module.pitch_modulation().unwrap().depth_semitones,
                },
                ModuleKind::ControlHertz => ControlNode::Hertz {
                    input: resolve(&format!("{id}.control"))?,
                    settings: module.hertz().unwrap(),
                },
                _ => return Err("unsupported lowered control node".into()),
            });
        }
        let amp = graph
            .modules()
            .iter()
            .find(|m| m.kind() == ModuleKind::Vca)
            .ok_or("missing VCA")?;
        let cutoff = graph
            .modules()
            .iter()
            .find(|m| m.kind() == ModuleKind::LowPass)
            .map(|m| format!("{}.cutoff", m.id()))
            .filter(|endpoint| graph.cables().iter().any(|c| c.to() == endpoint))
            .map(|endpoint| resolve(&endpoint))
            .transpose()?;
        Ok(Some(Self {
            cutoff,
            pitch_offset: order
                .iter()
                .position(|id| {
                    graph
                        .modules()
                        .iter()
                        .any(|m| m.id() == id && m.kind() == ModuleKind::PitchModulate)
                })
                .map(|index| index + 1),
            nodes,
            output: resolve(&format!("{}.level", amp.id()))?,
        }))
    }
    pub fn value_at(
        &self,
        frame: u64,
        gate_rise: bool,
        envelope: f64,
        tempo: Tempo,
        values: &mut [f64; 35],
        origins: &mut [u64; 32],
        phases: &mut [f64; 32],
        latches: &mut [f64; 32],
    ) -> f64 {
        values[0] = envelope;
        for (index, node) in self.nodes.iter().enumerate() {
            values[index + 1] = match node {
                ControlNode::Lfo { settings, rate } => {
                    if gate_rise && settings.reset == LfoReset::GateRise {
                        origins[index] = frame;
                    }
                    let mut source = Lfo {
                        shape: match settings.shape {
                            GraphLfoShape::Sine => LfoShape::Sine,
                            GraphLfoShape::Triangle => LfoShape::Triangle,
                            GraphLfoShape::Saw => LfoShape::Saw,
                            GraphLfoShape::Square => LfoShape::Square,
                        },
                        rate: match settings.rate {
                            GraphLfoRate::Hertz(value) => LfoRate::Hertz(value),
                            GraphLfoRate::Beats(value) => LfoRate::Beats(value),
                        },
                        phase: settings.phase,
                        seed: 0,
                    };
                    let instantaneous = if let Some(rate) = rate {
                        if frame == 0 || (gate_rise && settings.reset == LfoReset::GateRise) {
                            phases[index] = settings.phase;
                        }
                        source.phase = phases[index];
                        let value = source.value_at(0, tempo);
                        phases[index] = (phases[index]
                            + values[*rate] / f64::from(tempo.sample_rate()))
                        .fract();
                        value
                    } else {
                        source.value_at(frame - origins[index], tempo)
                    };
                    match settings.evaluation {
                        ControlEvaluation::Audio => instantaneous,
                        ControlEvaluation::Control { period_frames } => {
                            if frame % u64::from(period_frames) == 0
                                || (gate_rise && settings.reset == LfoReset::GateRise)
                            {
                                latches[index] = instantaneous;
                            }
                            latches[index]
                        }
                        ControlEvaluation::Event => {
                            if gate_rise {
                                latches[index] = instantaneous;
                            }
                            latches[index]
                        }
                    }
                }
                ControlNode::Scale { input, settings } => values[*input]
                    .mul_add(settings.gain, settings.offset)
                    .clamp(settings.clamp_min, settings.clamp_max),
                ControlNode::Multiply { left, right } => values[*left] * values[*right],
                ControlNode::PitchModulate {
                    input,
                    depth_semitones,
                } => values[*input] * depth_semitones,
                ControlNode::Hertz { input, settings } => values[*input]
                    .mul_add(settings.gain_hz, settings.offset_hz)
                    .clamp(settings.clamp_min_hz, settings.clamp_max_hz),
            };
        }
        values[self.output]
    }
}
