//! Bounded worker-side lowering of explicit controls and pitch adapters.
use crate::modulation::{Lfo, LfoRate, LfoShape};
use meldritch_core::Tempo;
use meldritch_dsl::{
    ControlScaleSettings, GraphLfoRate, GraphLfoSettings, GraphLfoShape, LfoReset, ModuleKind,
    SynthDefinition,
};

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ControlNode {
    Lfo(GraphLfoSettings),
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
}
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ControlPlan {
    pub nodes: Vec<ControlNode>,
    pub output: usize,
    pub pitch_offset: Option<usize>,
}
impl ControlPlan {
    pub fn compile(graph: &SynthDefinition) -> Result<Option<Self>, String> {
        let order = graph.control_order()?;
        if order.is_empty() {
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
                ModuleKind::Lfo => ControlNode::Lfo(module.lfo().unwrap()),
                ModuleKind::ControlScale => ControlNode::Scale {
                    input: resolve(&format!("{id}.control"))?,
                    settings: module.scale().unwrap(),
                },
                ModuleKind::ControlMultiply => ControlNode::Multiply {
                    left: resolve(&format!("{id}.left"))?,
                    right: resolve(&format!("{id}.right"))?,
                },
                ModuleKind::PitchModulate => ControlNode::PitchModulate {
                    input: resolve(&format!("{id}.control"))?,
                    depth_semitones: module.pitch_modulation().unwrap().depth_semitones,
                },
                _ => return Err("unsupported lowered control node".into()),
            });
        }
        let amp = graph
            .modules()
            .iter()
            .find(|m| m.kind() == ModuleKind::Vca)
            .ok_or("missing VCA")?;
        Ok(Some(Self {
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
        values: &mut [f64; 33],
        origins: &mut [u64; 32],
    ) -> f64 {
        values[0] = envelope;
        for (index, node) in self.nodes.iter().enumerate() {
            values[index + 1] = match node {
                ControlNode::Lfo(settings) => {
                    if gate_rise && settings.reset == LfoReset::GateRise {
                        origins[index] = frame;
                    }
                    Lfo {
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
                    }
                    .value_at(frame - origins[index], tempo)
                }
                ControlNode::Scale { input, settings } => values[*input]
                    .mul_add(settings.gain, settings.offset)
                    .clamp(settings.clamp_min, settings.clamp_max),
                ControlNode::Multiply { left, right } => values[*left] * values[*right],
                ControlNode::PitchModulate {
                    input,
                    depth_semitones,
                } => values[*input] * depth_semitones,
            };
        }
        values[self.output]
    }
}
