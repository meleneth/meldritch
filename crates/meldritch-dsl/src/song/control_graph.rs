//! Explicit control modules and pitch adapters over the ordinary typed synth graph.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlUnit {
    Bipolar,
    UnitInterval,
    Hertz,
}
impl ModuleDefinition {
    pub fn control_unit(&self, port: &str, direction: PortDirection) -> Option<ControlUnit> {
        match (self.kind, direction, port) {
            (ModuleKind::Lfo, PortDirection::Input, "rate") => Some(ControlUnit::Hertz),
            (ModuleKind::ControlAttenuate, _, "control") => Some(ControlUnit::Bipolar),
            (ModuleKind::ControlAttenuate, PortDirection::Input, "amount") => {
                Some(ControlUnit::UnitInterval)
            }
            (ModuleKind::ControlHertz, PortDirection::Input, "control") => {
                Some(ControlUnit::UnitInterval)
            }
            (ModuleKind::ControlHertz, PortDirection::Output, "control")
            | (ModuleKind::LowPass, PortDirection::Input, "cutoff") => Some(ControlUnit::Hertz),
            (ModuleKind::Lfo, PortDirection::Output, "control")
            | (ModuleKind::PitchModulate, PortDirection::Input, "control")
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
#[serde(deny_unknown_fields)]
pub struct ControlHertzSettings {
    pub gain_hz: f64,
    pub offset_hz: f64,
    pub clamp_min_hz: f64,
    pub clamp_max_hz: f64,
}
impl Default for ControlHertzSettings {
    fn default() -> Self {
        Self {
            gain_hz: 1800.0,
            offset_hz: 200.0,
            clamp_min_hz: 20.0,
            clamp_max_hz: 20000.0,
        }
    }
}
pub(super) fn validate_hertz_settings(
    kind: ModuleKind,
    settings: Option<ControlHertzSettings>,
) -> Result<(), String> {
    if (kind == ModuleKind::ControlHertz) != settings.is_some() {
        return Err("Hertz settings must match module kind".into());
    }
    if let Some(s) = settings {
        if ![s.gain_hz, s.offset_hz, s.clamp_min_hz, s.clamp_max_hz]
            .iter()
            .all(|v| v.is_finite())
            || !(-20000.0..=20000.0).contains(&s.gain_hz)
            || !(-20000.0..=20000.0).contains(&s.offset_hz)
            || s.clamp_min_hz < 0.001
            || s.clamp_max_hz > 20000.0
            || s.clamp_min_hz > s.clamp_max_hz
        {
            return Err("Hertz adapter requires finite gain/offset in -20000..20000 and ordered clamps in 0.001..20000 Hz".into());
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PitchModulationSettings {
    pub depth_semitones: f64,
}
impl Default for PitchModulationSettings {
    fn default() -> Self {
        Self {
            depth_semitones: 0.5,
        }
    }
}
pub(super) fn validate_pitch_settings(
    kind: ModuleKind,
    settings: Option<PitchModulationSettings>,
) -> Result<(), String> {
    if (kind == ModuleKind::PitchModulate) != settings.is_some() {
        return Err("pitch modulation settings must match module kind".into());
    }
    if let Some(settings) = settings {
        if !settings.depth_semitones.is_finite()
            || !(-24.0..=24.0).contains(&settings.depth_semitones)
        {
            return Err("pitch modulation requires finite depth in -24..24 semitones".into());
        }
    }
    Ok(())
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
    /// Every new control node must reach VCA level, oscillator pitch or filter cutoff;
    /// no hidden/ignored branches.
    pub fn control_order(&self) -> Result<Vec<String>, String> {
        let controls: BTreeSet<_> = self
            .modules
            .iter()
            .filter(|m| {
                matches!(
                    m.kind,
                    ModuleKind::Lfo
                        | ModuleKind::ControlScale
                        | ModuleKind::ControlMultiply
                        | ModuleKind::PitchModulate
                        | ModuleKind::ControlHertz
                        | ModuleKind::ControlAttenuate
                )
            })
            .map(|m| m.id.as_str())
            .collect();
        if controls.is_empty() {
            if self.cables.iter().any(|c| c.to.ends_with(".cutoff")) {
                return Err("filter cutoff requires an explicit Hertz adapter".into());
            }
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
            expected: ControlUnit,
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
            let actual = module.control_unit(port, PortDirection::Output);
            if actual != Some(expected) {
                return Err(format!(
                    "control unit mismatch at '{endpoint}': expected {expected:?}, found {actual:?}; use an explicit adapter"
                ));
            }
            if module.kind == ModuleKind::Adsr {
                return Ok(());
            }
            if !matches!(
                module.kind,
                ModuleKind::Lfo
                    | ModuleKind::ControlScale
                    | ModuleKind::ControlMultiply
                    | ModuleKind::ControlAttenuate
                    | ModuleKind::ControlHertz
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
                    let endpoint = format!("{id}.rate");
                    if let Some(cable) = graph.cables.iter().find(|c| c.to == endpoint) {
                        visit(graph, &endpoint, ControlUnit::Hertz, active, order)?;
                        let (source, _) =
                            cable.from.split_once('.').ok_or("invalid rate source")?;
                        let settings = graph
                            .modules
                            .iter()
                            .find(|m| m.id == source)
                            .and_then(|m| m.hertz)
                            .ok_or("LFO rate requires an explicit Hertz adapter")?;
                        if settings.clamp_min_hz < 0.001 || settings.clamp_max_hz > 100.0 {
                            return Err(
                                "LFO rate adapter must declare clamps within 0.001..100 Hz".into(),
                            );
                        }
                    }
                }
                ModuleKind::ControlScale => visit(
                    graph,
                    &format!("{id}.control"),
                    ControlUnit::Bipolar,
                    active,
                    order,
                )?,
                ModuleKind::ControlMultiply => {
                    visit(
                        graph,
                        &format!("{id}.left"),
                        ControlUnit::UnitInterval,
                        active,
                        order,
                    )?;
                    visit(
                        graph,
                        &format!("{id}.right"),
                        ControlUnit::UnitInterval,
                        active,
                        order,
                    )?;
                }
                ModuleKind::ControlAttenuate => {
                    visit(
                        graph,
                        &format!("{id}.control"),
                        ControlUnit::Bipolar,
                        active,
                        order,
                    )?;
                    visit(
                        graph,
                        &format!("{id}.amount"),
                        ControlUnit::UnitInterval,
                        active,
                        order,
                    )?;
                }
                ModuleKind::ControlHertz => visit(
                    graph,
                    &format!("{id}.control"),
                    ControlUnit::UnitInterval,
                    active,
                    order,
                )?,
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
            ControlUnit::UnitInterval,
            &mut BTreeSet::new(),
            &mut order,
        )?;
        let adapters = self
            .modules
            .iter()
            .filter(|m| m.kind == ModuleKind::PitchModulate)
            .collect::<Vec<_>>();
        if !adapters.is_empty() {
            if adapters.len() != 1 {
                return Err("pitch lowering requires exactly one pitch adapter".into());
            }
            let adapter = adapters[0];
            let oscillator = self.modules.iter().find(|m| m.kind == ModuleKind::Oscillator)
                .ok_or("pitch adapter currently requires an oscillator; Plaits lowering is unsupported")?;
            for (from, to) in [
                ("input.pitch".to_owned(), format!("{}.pitch", adapter.id)),
                (
                    format!("{}.pitch", adapter.id),
                    format!("{}.pitch", oscillator.id),
                ),
            ] {
                let cables = self
                    .cables
                    .iter()
                    .filter(|c| c.to == to)
                    .collect::<Vec<_>>();
                if !matches!(cables.as_slice(), [c] if c.from == from && c.signal == SignalType::Pitch)
                {
                    return Err("pitch adapter requires one explicit input.pitch → adapter.pitch → oscillator.pitch route".into());
                }
            }
            visit(
                self,
                &format!("{}.control", adapter.id),
                ControlUnit::Bipolar,
                &mut BTreeSet::new(),
                &mut order,
            )?;
            order.push(adapter.id.clone());
        }
        let cutoff_cables = self
            .cables
            .iter()
            .filter(|c| c.to.ends_with(".cutoff"))
            .collect::<Vec<_>>();
        match cutoff_cables.as_slice() {
            [] => {}
            [cable] => {
                let filter = self
                    .modules
                    .iter()
                    .find(|m| m.kind == ModuleKind::LowPass)
                    .ok_or("Hertz adapter requires a low-pass filter")?;
                if cable.to != format!("{}.cutoff", filter.id)
                    || cable.signal != SignalType::Control
                {
                    return Err("filter cutoff requires explicit Hertz adapter output".into());
                }
                visit(
                    self,
                    &format!("{}.cutoff", filter.id),
                    ControlUnit::Hertz,
                    &mut BTreeSet::new(),
                    &mut order,
                )?;
            }
            _ => {
                return Err("filter lowering requires at most one explicit cutoff cable".into());
            }
        }
        if controls.len() != order.len() {
            return Err("unused control module outside executable VCA/pitch/filter graph".into());
        }
        Ok(order)
    }
}
