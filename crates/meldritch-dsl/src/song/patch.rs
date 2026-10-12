//! Typed edits of the existing source graph, not an editor-owned graph format.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DependentCables {
    Reject,
    Disconnect,
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum PatchOperation {
    Instantiate {
        id: String,
        kind: ModuleKind,
    },
    Remove {
        id: String,
        cables: DependentCables,
    },
    Connect {
        from: String,
        to: String,
    },
    Disconnect {
        from: String,
        to: String,
    },
    SetParameter {
        module: String,
        parameter: InstrumentParameter,
        value: InstrumentValue,
    },
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PatchEdit {
    pub synth: String,
    pub operations: Vec<PatchOperation>,
}

/// Create one mono palette definition with explicit external inputs and the
/// existing typed operation batch. It does not create a track or timed material.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PatchCreation {
    #[serde(default)]
    pub inputs: Vec<PatchInput>,
    pub edit: PatchEdit,
}

impl PatchCreation {
    /// Convenience wiring expands entirely into ordinary inspectable operations.
    #[must_use]
    pub fn mono_voice(id: impl Into<String>) -> Self {
        let mut operations = vec![];
        for (id, kind) in [
            ("osc", ModuleKind::Oscillator),
            ("env", ModuleKind::Adsr),
            ("amp", ModuleKind::Vca),
            ("out", ModuleKind::AudioOutput),
        ] {
            operations.push(PatchOperation::Instantiate {
                id: id.into(),
                kind,
            });
        }
        for (from, to) in [
            ("input.pitch", "osc.pitch"),
            ("input.gate", "env.gate"),
            ("osc.audio", "amp.audio"),
            ("env.control", "amp.level"),
            ("amp.audio", "out.audio"),
        ] {
            operations.push(PatchOperation::Connect {
                from: from.into(),
                to: to.into(),
            });
        }
        Self {
            inputs: vec![
                PatchInput {
                    id: "pitch".into(),
                    signal: SignalType::Pitch,
                },
                PatchInput {
                    id: "gate".into(),
                    signal: SignalType::Gate,
                },
            ],
            edit: PatchEdit {
                synth: id.into(),
                operations,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ModulePort {
    pub endpoint: String,
    pub direction: PortDirection,
    pub signal: SignalType,
}
impl ModuleDefinition {
    /// Enumerate through the same port resolver used by the importer/editor.
    pub fn ports(&self) -> Vec<ModulePort> {
        let modules = BTreeMap::from([(self.id.as_str(), self.kind)]);
        let inputs = BTreeMap::new();
        let mut ports = vec![];
        for direction in [PortDirection::Input, PortDirection::Output] {
            for name in ["pitch", "gate", "control", "audio", "level", "aux"] {
                let endpoint = format!("{}.{}", self.id, name);
                if let Ok(signal) = resolve_port(
                    Path::new("module ports"),
                    &modules,
                    &inputs,
                    &endpoint,
                    direction,
                ) {
                    ports.push(ModulePort {
                        endpoint,
                        direction,
                        signal,
                    });
                }
            }
        }
        ports
    }
}

impl ModuleKind {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Oscillator => "oscillator",
            Self::Adsr => "adsr",
            Self::Vca => "vca",
            Self::LowPass => "low_pass",
            Self::TempoDelay => "tempo_delay",
            Self::AudioOutput => "audio_output",
            Self::Plaits => "plaits",
        }
    }
}
fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id != "input"
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
}
fn instantiate(id: &str, kind: ModuleKind) -> Result<ModuleDefinition, String> {
    if !valid_id(id) {
        return Err(format!(
            "module id '{id}' must be 1..64 ASCII letters/digits/-/_ and cannot be input"
        ));
    }
    if kind == ModuleKind::TempoDelay {
        return Err("tempo_delay belongs to DSP graphs; unsupported in this synth editor".into());
    }
    Ok(ModuleDefinition {
        id: id.into(),
        kind,
        plaits: (kind == ModuleKind::Plaits).then(PlaitsSettings::default),
        waveform: (kind == ModuleKind::Oscillator).then(|| "sine".into()),
        frequency_hz: None,
        channels: (kind == ModuleKind::AudioOutput).then_some(1),
        cutoff_hz: (kind == ModuleKind::LowPass).then_some(20_000.0),
        resonance: (kind == ModuleKind::LowPass).then_some(0.0),
        time: None,
        feedback: None,
        mix: None,
        attack: (kind == ModuleKind::Adsr).then_some(0.0),
        decay: (kind == ModuleKind::Adsr).then_some(0.0),
        sustain: (kind == ModuleKind::Adsr).then_some(1.0),
        release: (kind == ModuleKind::Adsr).then_some(0.0),
    })
}
impl ValidatedSong {
    /// Import a complete native mono palette graph through the existing importer.
    /// Executable lowering still belongs to the caller's acceptance boundary.
    pub fn restore_palette_patch(&self, graph: SynthDefinition) -> Result<Self, String> {
        if !valid_id(&graph.id)
            || graph.polyphony != 1
            || graph.inputs.len() > 32
            || graph.inputs.iter().any(|i| !valid_id(&i.id))
            || graph.modules.iter().any(|m| !valid_id(&m.id))
            || graph
                .modules
                .iter()
                .any(|m| m.kind == ModuleKind::AudioOutput && m.channels.unwrap_or(1) != 1)
        {
            return Err("native palette requires valid identities and mono graph layout".into());
        }
        if self.synths.contains_key(&graph.id) || self.samplers.contains_key(&graph.id) {
            return Err(format!("duplicate instrument id '{}'", graph.id));
        }
        if self.synths.len() + self.samplers.len() >= 32 {
            return Err("construction exceeds 32 instruments".into());
        }
        let mut candidate = self.clone();
        candidate.synths.insert(graph.id.clone(), graph.clone());
        let candidate = candidate.replace_patch(graph.clone())?;
        candidate.validate_executable_patch(&graph.id)?;
        Ok(candidate)
    }
    /// Build on a clone and reuse operation/importer validation; intermediate
    /// drafts remain private. Executable compilation belongs to the app boundary.
    pub fn create_patch(&self, creation: &PatchCreation) -> Result<Self, String> {
        let id = &creation.edit.synth;
        if !valid_id(id) {
            return Err(
                "synth id must be 1..64 ASCII letters/digits/-/_ and cannot be input".into(),
            );
        }
        if self.synths.contains_key(id) || self.samplers.contains_key(id) {
            return Err(format!("duplicate instrument id '{id}'"));
        }
        if self.synths.len() + self.samplers.len() >= 32 || creation.inputs.len() > 32 {
            return Err("construction exceeds 32 instruments or inputs".into());
        }
        let mut next = self.clone();
        next.synths.insert(
            id.clone(),
            SynthDefinition {
                output_level_control: None,
                id: id.clone(),
                polyphony: 1,
                inputs: creation.inputs.clone(),
                modules: vec![],
                cables: vec![],
            },
        );
        let candidate = next.preview_patch(&creation.edit)?;
        let next = candidate.replace_patch(candidate.synths[id].clone())?;
        next.validate_executable_patch(id)?;
        Ok(next)
    }
    /// Structural subset used in addition to ordinary executable lowering.
    pub fn validate_executable_patch(&self, synth: &str) -> Result<(), String> {
        let graph = self
            .synths
            .get(synth)
            .ok_or_else(|| format!("unknown synth '{synth}'"))?;
        let with_filter = graph.modules.iter().any(|m| m.kind == ModuleKind::LowPass);
        let expected_modules = if with_filter { 5 } else { 4 };
        let patched_plaits_gate = graph.modules.iter().any(|m| {
            m.kind == ModuleKind::Plaits
                && graph
                    .cables
                    .iter()
                    .any(|c| c.from == "input.gate" && c.to == format!("{}.gate", m.id))
        });
        let expected_cables = if with_filter { 6 } else { 5 } + usize::from(patched_plaits_gate);
        if graph.modules.len() != expected_modules || graph.cables.len() != expected_cables {
            return Err("unsupported note topology: one source, ADSR, VCA, output and optional low-pass with exactly the executable cables required".into());
        }
        Ok(())
    }
    /// Apply a bounded batch to a clone. Intermediate graphs may be incomplete.
    /// Ports are validated as connected; complete compilation remains the app boundary.
    pub fn preview_patch(&self, edit: &PatchEdit) -> Result<Self, String> {
        if edit.operations.len() > 128 {
            return Err("patch batch exceeds 128 operations".into());
        }
        if !self.synths.contains_key(&edit.synth) {
            return Err(format!("unknown synth '{}'", edit.synth));
        }
        let mut next = self.clone();
        for operation in &edit.operations {
            if let PatchOperation::SetParameter {
                module,
                parameter,
                value,
            } = operation
            {
                next = next.edit_instrument(&InstrumentEdit {
                    target: InstrumentParameterId {
                        synth: edit.synth.clone(),
                        module: module.clone(),
                        parameter: *parameter,
                    },
                    value: value.clone(),
                })?;
                continue;
            }
            let graph = next.synths.get_mut(&edit.synth).unwrap();
            match operation {
                PatchOperation::Instantiate { id, kind } => {
                    if graph.modules.len() >= 32 {
                        return Err("patch exceeds 32 modules".into());
                    }
                    if graph.modules.iter().any(|m| &m.id == id) {
                        return Err(format!("duplicate module id '{id}'"));
                    }
                    graph.modules.push(instantiate(id, *kind)?);
                }
                PatchOperation::Remove { id, cables } => {
                    let index = graph
                        .modules
                        .iter()
                        .position(|m| &m.id == id)
                        .ok_or_else(|| format!("unknown module '{id}'"))?;
                    let dependent = |c: &CableDefinition| {
                        c.from.split_once('.').is_some_and(|(m, _)| m == id)
                            || c.to.split_once('.').is_some_and(|(m, _)| m == id)
                    };
                    if graph.cables.iter().any(dependent) && *cables == DependentCables::Reject {
                        return Err(format!(
                            "module '{id}' has dependent cables; disconnect explicitly or use cables=disconnect"
                        ));
                    }
                    if *cables == DependentCables::Disconnect {
                        graph.cables.retain(|c| !dependent(c));
                    }
                    graph.modules.remove(index);
                }
                PatchOperation::Connect { from, to } => {
                    if graph.cables.len() >= 128 {
                        return Err("patch exceeds 128 cables".into());
                    }
                    if graph.cables.iter().any(|c| &c.to == to) {
                        return Err(format!(
                            "input '{to}' already has a cable; disconnect it first"
                        ));
                    }
                    let modules = graph
                        .modules
                        .iter()
                        .map(|m| (m.id.as_str(), m.kind))
                        .collect();
                    let inputs = graph
                        .inputs
                        .iter()
                        .map(|i| (i.id.as_str(), i.signal))
                        .collect();
                    let a = resolve_port(
                        Path::new("authored patch"),
                        &modules,
                        &inputs,
                        from,
                        PortDirection::Output,
                    )
                    .map_err(|e| e.to_string())?;
                    let b = resolve_port(
                        Path::new("authored patch"),
                        &modules,
                        &inputs,
                        to,
                        PortDirection::Input,
                    )
                    .map_err(|e| e.to_string())?;
                    if a != b {
                        return Err(format!(
                            "incompatible cable '{from} -> {to}': {a:?} to {b:?}"
                        ));
                    }
                    graph.cables.push(CableDefinition {
                        from: from.clone(),
                        to: to.clone(),
                        signal: a,
                    });
                }
                PatchOperation::Disconnect { from, to } => {
                    let index = graph
                        .cables
                        .iter()
                        .position(|c| &c.from == from && &c.to == to)
                        .ok_or_else(|| format!("missing cable '{from} -> {to}'"))?;
                    graph.cables.remove(index);
                }
                PatchOperation::SetParameter { .. } => unreachable!(),
            }
        }
        next.refresh_authoring_fingerprint();
        Ok(next)
    }
    /// Re-run the ordinary synth importer and typed-port boundary on a graph.
    pub fn replace_patch(&self, graph: SynthDefinition) -> Result<Self, String> {
        if graph.output_level_control.as_ref().is_some_and(|id| {
            id.is_empty()
                || id.len() > 64
                || !id
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
        }) {
            return Err("invalid output-level control identity".into());
        }
        let previous = self
            .synths
            .get(&graph.id)
            .ok_or_else(|| format!("unknown synth '{}'", graph.id))?;
        if graph.inputs != previous.inputs || graph.polyphony != previous.polyphony {
            return Err(
                "patch subset preserves synth identity, polyphony and declared external inputs"
                    .into(),
            );
        }
        if graph.modules.len() > 32 || graph.cables.len() > 128 {
            return Err("patch size exceeds authoring limits".into());
        }
        let channels = |g: &SynthDefinition| {
            g.modules
                .iter()
                .find(|m| m.kind == ModuleKind::AudioOutput)
                .and_then(|m| m.channels)
                .unwrap_or(1)
        };
        if channels(&graph) != channels(previous) {
            return Err("patch authoring preserves output channel layout".into());
        }
        let raw = RawSynthFile {
            meldritch: RawHeader {
                kind: "synth".into(),
                version: 1,
            },
            synth: RawSynth {
                id: graph.id.clone(),
                polyphony: graph.polyphony,
            },
            inputs: graph
                .inputs
                .iter()
                .map(|i| RawInput {
                    id: i.id.clone(),
                    signal: format!("{:?}", i.signal).to_ascii_lowercase(),
                })
                .collect(),
            modules: graph
                .modules
                .iter()
                .map(|m| RawModule {
                    id: m.id.clone(),
                    kind: m.kind.name().into(),
                    model: m.plaits.map(|p| p.model),
                    harmonics: m.plaits.map(|p| p.harmonics),
                    timbre: m.plaits.map(|p| p.timbre),
                    morph: m.plaits.map(|p| p.morph),
                    waveform: m.waveform.clone(),
                    frequency_hz: m.frequency_hz,
                    channels: m.channels,
                    cutoff_hz: m.cutoff_hz,
                    resonance: m.resonance,
                    time: m.time.clone(),
                    feedback: m.feedback,
                    mix: m.mix,
                    attack: m.attack,
                    decay: m.decay,
                    sustain: m.sustain,
                    release: m.release,
                })
                .collect(),
            cables: graph
                .cables
                .iter()
                .map(|c| RawCable {
                    from: c.from.clone(),
                    to: c.to.clone(),
                })
                .collect(),
        };
        let mut validated =
            parse_synth(Path::new("authored patch"), raw).map_err(|e| e.to_string())?;
        // This declaration belongs to the authored graph snapshot, not the
        // historical .mlsynth TOML schema reconstructed above.
        validated.output_level_control = graph.output_level_control.clone();
        if graph != validated {
            return Err("graph includes inconsistent typed cable/module state".into());
        }
        let mut destinations = BTreeSet::new();
        let mut edges = BTreeMap::<&str, Vec<&str>>::new();
        for m in &graph.modules {
            if !valid_id(&m.id) {
                return Err(format!("unsupported module identity '{}'", m.id));
            }
            // Reject irrelevant settings rather than preserve a misleading configuration.
            let mut allowed = instantiate(&m.id, m.kind)?;
            match m.kind {
                ModuleKind::Oscillator => {
                    allowed.waveform = m.waveform.clone();
                    allowed.frequency_hz = m.frequency_hz;
                }
                ModuleKind::Adsr => {
                    allowed.attack = m.attack;
                    allowed.decay = m.decay;
                    allowed.sustain = m.sustain;
                    allowed.release = m.release;
                }
                ModuleKind::LowPass => {
                    allowed.cutoff_hz = m.cutoff_hz;
                    allowed.resonance = m.resonance;
                }
                ModuleKind::AudioOutput => {
                    allowed.channels = m.channels;
                }
                ModuleKind::Plaits => {
                    allowed.plaits = m.plaits;
                }
                _ => {}
            }
            // Omitted imported defaults are allowed; unrelated fields are not.
            if allowed.kind != m.kind
                || (m.waveform.is_some() && allowed.waveform != m.waveform)
                || (m.frequency_hz.is_some() && allowed.frequency_hz != m.frequency_hz)
                || (m.channels.is_some() && allowed.channels != m.channels)
                || (m.cutoff_hz.is_some() && allowed.cutoff_hz != m.cutoff_hz)
                || (m.resonance.is_some() && allowed.resonance != m.resonance)
                || m.time.is_some()
                || m.feedback.is_some()
                || m.mix.is_some()
                || (m.attack.is_some() && allowed.attack != m.attack)
                || (m.decay.is_some() && allowed.decay != m.decay)
                || (m.sustain.is_some() && allowed.sustain != m.sustain)
                || (m.release.is_some() && allowed.release != m.release)
            {
                return Err(format!(
                    "module '{}' contains unsupported configuration",
                    m.id
                ));
            }
            edges.insert(&m.id, vec![]);
        }
        for c in &graph.cables {
            if !destinations.insert(&c.to) {
                return Err(format!("ambiguous input '{}': multiple cables", c.to));
            }
            let (a, _) = c.from.split_once('.').unwrap();
            let (b, _) = c.to.split_once('.').unwrap();
            if a != "input" {
                edges.get_mut(a).unwrap().push(b);
            }
        }
        fn visit<'a>(
            id: &'a str,
            edges: &BTreeMap<&'a str, Vec<&'a str>>,
            active: &mut BTreeSet<&'a str>,
            done: &mut BTreeSet<&'a str>,
        ) -> Result<(), String> {
            if done.contains(id) {
                return Ok(());
            }
            if !active.insert(id) {
                return Err(format!("unsupported cycle involving module '{id}'"));
            }
            for next in &edges[id] {
                visit(next, edges, active, done)?;
            }
            active.remove(id);
            done.insert(id);
            Ok(())
        }
        let mut done = BTreeSet::new();
        for id in edges.keys() {
            visit(id, &edges, &mut BTreeSet::new(), &mut done)?;
        }
        // Automation, curated controls, and parameter patterns may refer to modules.
        for target in self
            .parameter_patterns
            .values()
            .flat_map(|p| &p.lanes)
            .map(|l| &l.target)
            .chain(self.performance.controls.iter().map(|c| &c.target))
        {
            if target.owner == ParameterOwner::Synth
                && target.definition_id == graph.id
                && !graph.modules.iter().any(|m| m.id == target.module_id)
            {
                return Err(format!(
                    "module '{}' is referenced by parameter target; detach it first",
                    target.module_id
                ));
            }
        }
        let mut next = self.clone();
        next.synths.insert(graph.id.clone(), graph);
        for before in self.instrument_parameters(previous.id())? {
            if before.owner != InstrumentParameterOwner::Source {
                let after = next
                    .instrument_parameters(previous.id())?
                    .into_iter()
                    .find(|p| p.id == before.id);
                if after.is_none_or(|p| !p.value.same_as(&before.value)) {
                    return Err(format!(
                        "parameter {:?} is owned by authored automation; patch cannot replace its authored default",
                        before.id
                    ));
                }
            }
        }
        for target in next
            .parameter_patterns
            .values()
            .flat_map(|p| &p.lanes)
            .map(|l| &l.target)
            .chain(next.performance.controls.iter().map(|c| &c.target))
        {
            if target.owner == ParameterOwner::Synth {
                let value = format!(
                    "synth:{}/{}.{}",
                    target.definition_id, target.module_id, target.parameter
                );
                parse_global_parameter_target(
                    Path::new("authored patch"),
                    &value,
                    &next.synths,
                    &next.dsps,
                )
                .map_err(|e| e.to_string())?;
            }
        }
        next.refresh_authoring_fingerprint();
        Ok(next)
    }
    pub fn edit_patch(&self, edit: &PatchEdit) -> Result<Self, String> {
        let draft = self.preview_patch(edit)?;
        self.replace_patch(draft.synths[&edit.synth].clone())
    }
}
