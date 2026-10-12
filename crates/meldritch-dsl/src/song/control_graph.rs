//! Dimensionless control modules over the ordinary typed synth graph.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlUnit {
    Bipolar,
    UnitInterval,
}
impl ModuleDefinition {
    pub fn control_unit(&self, port: &str, direction: PortDirection) -> Option<ControlUnit> {
        match (self.kind, direction, port) {
            (ModuleKind::Lfo, PortDirection::Output, "control")
            | (ModuleKind::ControlScale, PortDirection::Input, "control") => {
                Some(ControlUnit::Bipolar)
            }
            (
                ModuleKind::Adsr | ModuleKind::ControlScale | ModuleKind::ControlMultiply,
                PortDirection::Output,
                "control",
            )
            | (ModuleKind::Vca, PortDirection::Input, "level")
            | (ModuleKind::ControlMultiply, PortDirection::Input, "left" | "right") => {
                Some(ControlUnit::UnitInterval)
            }
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum GraphLfoRate {
    Hertz(f64),
    Beats(f64),
}
#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphLfoShape {
    Sine,
    Triangle,
    Saw,
    Square,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlEvaluation {
    Audio,
}
#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlScope {
    PerInstance,
}
#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LfoReset {
    None,
    GateRise,
}
#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GraphLfoSettings {
    pub shape: GraphLfoShape,
    pub rate: GraphLfoRate,
    pub phase: f64,
    pub evaluation: ControlEvaluation,
    pub scope: ControlScope,
    pub reset: LfoReset,
}
impl Default for GraphLfoSettings {
    fn default() -> Self {
        Self {
            shape: GraphLfoShape::Sine,
            rate: GraphLfoRate::Beats(1.0),
            phase: 0.0,
            evaluation: ControlEvaluation::Audio,
            scope: ControlScope::PerInstance,
            reset: LfoReset::None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ControlScaleSettings {
    pub gain: f64,
    pub offset: f64,
    pub clamp_min: f64,
    pub clamp_max: f64,
}
impl Default for ControlScaleSettings {
    fn default() -> Self {
        Self {
            gain: 0.5,
            offset: 0.5,
            clamp_min: 0.0,
            clamp_max: 1.0,
        }
    }
}
pub(super) fn validate_control_settings(
    kind: ModuleKind,
    lfo: Option<GraphLfoSettings>,
    scale: Option<ControlScaleSettings>,
) -> Result<(), String> {
    if (kind == ModuleKind::Lfo) != lfo.is_some()
        || (kind == ModuleKind::ControlScale) != scale.is_some()
    {
        return Err("control module settings must match module kind".into());
    }
    if let Some(settings) = lfo {
        let valid = match settings.rate {
            GraphLfoRate::Hertz(value) => value.is_finite() && (0.001..=100.0).contains(&value),
            GraphLfoRate::Beats(value) => {
                value.is_finite() && (1.0 / 64.0..=256.0).contains(&value)
            }
        };
        if !valid || !settings.phase.is_finite() || !(0.0..1.0).contains(&settings.phase) {
            return Err("LFO requires a finite supported period/rate and phase in 0..1".into());
        }
    }
    if let Some(settings) = scale {
        if ![
            settings.gain,
            settings.offset,
            settings.clamp_min,
            settings.clamp_max,
        ]
        .iter()
        .all(|v| v.is_finite())
            || !(-8.0..=8.0).contains(&settings.gain)
            || !(-8.0..=8.0).contains(&settings.offset)
            || settings.clamp_min < 0.0
            || settings.clamp_max > 1.0
            || settings.clamp_min > settings.clamp_max
        {
            return Err(
                "scale requires finite gain/offset in -8..8 and ordered unit-interval clamps"
                    .into(),
            );
        }
    }
    Ok(())
}
impl SynthDefinition {
    /// Validate the complete bounded control DAG and return dependency order.
    /// Every new control node must reach the VCA; no hidden/ignored branches.
    pub fn control_order(&self) -> Result<Vec<String>, String> {
        let controls: BTreeSet<_> = self
            .modules
            .iter()
            .filter(|m| {
                matches!(
                    m.kind,
                    ModuleKind::Lfo | ModuleKind::ControlScale | ModuleKind::ControlMultiply
                )
            })
            .map(|m| m.id.as_str())
            .collect();
        if controls.is_empty() {
            return Ok(vec![]);
        }
        if self.modules.len() > 32 || self.cables.len() > 128 {
            return Err("control graph exceeds bounded patch limits".into());
        }
        if self.polyphony != 1 {
            return Err("graph controls currently require mono instance lowering".into());
        }
        let amp = self
            .modules
            .iter()
            .find(|m| m.kind == ModuleKind::Vca)
            .ok_or("control graph requires VCA")?;
        fn visit<'a>(
            graph: &'a SynthDefinition,
            endpoint: &str,
            bipolar: bool,
            active: &mut BTreeSet<&'a str>,
            order: &mut Vec<String>,
        ) -> Result<(), String> {
            let cable = graph
                .cables
                .iter()
                .find(|c| c.to == endpoint)
                .ok_or_else(|| format!("missing control cable to '{endpoint}'"))?;
            if graph.cables.iter().filter(|c| c.to == endpoint).count() != 1 {
                return Err("control input requires exactly one cable".into());
            }
            let (id, port) = cable
                .from
                .split_once('.')
                .ok_or("invalid control endpoint")?;
            let module = graph
                .modules
                .iter()
                .find(|m| m.id == id)
                .ok_or("unknown control source")?;
            if port != "control" || cable.signal != SignalType::Control {
                return Err("control source requires Control output".into());
            }
            if module.control_unit(port, PortDirection::Output)
                != Some(if bipolar {
                    ControlUnit::Bipolar
                } else {
                    ControlUnit::UnitInterval
                })
            {
                return Err("control unit mismatch: bipolar requires scale; VCA/multiply require unit interval".into());
            }
            if module.kind == ModuleKind::Adsr {
                return Ok(());
            }
            if !matches!(
                module.kind,
                ModuleKind::Lfo | ModuleKind::ControlScale | ModuleKind::ControlMultiply
            ) {
                return Err("unsupported control source".into());
            }
            if order.iter().any(|done| done == id) {
                return Ok(());
            }
            if !active.insert(module.id.as_str()) {
                return Err("unsupported control cycle".into());
            }
            match module.kind {
                ModuleKind::Lfo => {
                    let reset = graph
                        .cables
                        .iter()
                        .filter(|c| c.to == format!("{id}.reset"))
                        .collect::<Vec<_>>();
                    match (module.lfo.unwrap().reset, reset.as_slice()) {
                        (LfoReset::None, []) => {},
                        (LfoReset::GateRise, [c]) if c.from == "input.gate" && c.signal == SignalType::Gate => {},
                        _ => return Err("LFO reset policy requires exactly its explicit input.gate cable or none".into()),
                    }
                }
                ModuleKind::ControlScale => {
                    visit(graph, &format!("{id}.control"), true, active, order)?
                }
                ModuleKind::ControlMultiply => {
                    visit(graph, &format!("{id}.left"), false, active, order)?;
                    visit(graph, &format!("{id}.right"), false, active, order)?;
                }
                _ => unreachable!(),
            }
            active.remove(module.id.as_str());
            order.push(id.into());
            Ok(())
        }
        let mut order = vec![];
        visit(
            self,
            &format!("{}.level", amp.id),
            false,
            &mut BTreeSet::new(),
            &mut order,
        )?;
        if controls.len() != order.len() {
            return Err("unused control module outside executable VCA graph".into());
        }
        Ok(order)
    }
}
