use std::collections::{BTreeMap, BTreeSet};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use crate::contract::{self, FanIn, Role};
use crate::midi::NoteState;
use crate::model::{Connection, Graph, MidiCc, MidiNotes, ParameterId};

#[derive(Clone, Debug)]
struct Op {
    process: fn(&mut [f64], &[f64], &[f64], f64, &mut [f64]),
    state: Vec<f64>,
    inputs: Vec<InputPlan>,
    outputs: std::ops::Range<usize>,
    params: Vec<usize>,
}

#[derive(Clone, Debug)]
struct InputPlan {
    sources: Vec<usize>,
    unconnected: f64,
}

#[derive(Debug)]
struct NotePair {
    channel: u8,
    pitch_index: usize,
    gate_index: usize,
    velocity_index: Option<usize>,
    state: Arc<NoteState>,
    last_pitch: f64,
    last_gate: f64,
    last_velocity: f64,
}

const NO_OVERRIDE: u64 = f64::NAN.to_bits();

pub(crate) fn validate_parameter_value(
    id: &ParameterId,
    range: Option<(f64, f64)>,
    value: f64,
) -> Result<(), String> {
    if !value.is_finite() {
        return Err(format!("{}.{} must be finite", id.module, id.name));
    }
    if let Some((min, max)) = range {
        if value < min || value > max {
            return Err(format!("{}.{} must be in {min}..{max}", id.module, id.name));
        }
    }
    Ok(())
}

/// A control-side handle for one parameter in one compiled graph revision.
#[derive(Clone, Debug)]
pub struct ParameterHandle {
    id: ParameterId,
    cell: Arc<AtomicU64>,
    range: Option<(f64, f64)>,
}

impl ParameterHandle {
    pub fn id(&self) -> &ParameterId {
        &self.id
    }

    pub fn publish(&self, value: Option<f64>) -> Result<(), String> {
        let bits = match value {
            Some(value) => {
                validate_parameter_value(&self.id, self.range, value)?;
                value.to_bits()
            }
            None => NO_OVERRIDE,
        };
        self.cell.store(bits, Ordering::Release);
        Ok(())
    }

    pub fn override_value(&self) -> Option<f64> {
        let bits = self.cell.load(Ordering::Acquire);
        (bits != NO_OVERRIDE).then(|| f64::from_bits(bits))
    }
}

#[derive(Clone, Debug)]
struct ControlBinding {
    target: ParameterId,
    min: f64,
    max: f64,
}

/// A named instrument control bound to a parameter in one graph revision.
#[derive(Clone, Debug)]
pub struct ControlHandle {
    id: String,
    parameter: ParameterHandle,
    min: f64,
    max: f64,
}

impl ControlHandle {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn target(&self) -> &ParameterId {
        self.parameter.id()
    }
    pub fn range(&self) -> (f64, f64) {
        (self.min, self.max)
    }
    pub fn override_value(&self) -> Option<f64> {
        self.parameter.override_value()
    }
    pub fn publish(&self, value: f64) -> Result<(), String> {
        if !value.is_finite() || value < self.min || value > self.max {
            return Err(format!(
                "control '{}' requires a finite value in {}..{}",
                self.id, self.min, self.max
            ));
        }
        self.parameter.publish(Some(value))
    }
    pub fn clear(&self) -> Result<(), String> {
        self.parameter.publish(None)
    }
}

/// An accepted, executable graph. Its processing buffers are allocated at compile time.
#[derive(Debug)]
pub struct Machine {
    revision: Arc<()>,
    ops: Vec<Op>,
    slots: Vec<f64>,
    input_scratch: Vec<f64>,
    param_scratch: Vec<f64>,
    params: Vec<f64>,
    authored: Vec<f64>,
    published: Vec<Arc<AtomicU64>>,
    param_ranges: Vec<Option<(f64, f64)>>,
    param_ids: BTreeMap<ParameterId, usize>,
    controls: BTreeMap<String, ControlBinding>,
    midi_ccs: Vec<MidiCc>,
    midi_notes: Vec<MidiNotes>,
    poly_notes: Vec<PolyMapping>,
    note_pairs: Vec<NotePair>,
    output_slot: usize,
    order: Vec<String>,
    authored_normalizations: Vec<Connection>,
    effective_connections: Vec<Connection>,
    sample_rate: f64,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct TimedNoteTarget {
    pub pitch_index: usize,
    pub gate_index: usize,
    pub velocity_index: Option<usize>,
    pub min_pitch: f64,
    pub max_pitch: f64,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct PolyVoiceTarget {
    pub pitch_index: usize,
    pub gate_index: usize,
    pub velocity_index: usize,
}

#[derive(Debug)]
struct PolyMapping {
    channel: u8,
    voices: [PolyVoiceTarget; 2],
}

impl Machine {
    pub fn process(&mut self, buffer: &mut [f64]) {
        self.prepare_buffer();
        self.process_prepared(buffer);
    }

    pub(crate) fn prepare_buffer(&mut self) {
        // Snapshot all published values once, before processing this buffer.
        for (index, cell) in self.published.iter().enumerate() {
            let bits = cell.load(Ordering::Acquire);
            self.params[index] = if bits == NO_OVERRIDE {
                self.authored[index]
            } else {
                f64::from_bits(bits)
            };
        }
        for pair in &mut self.note_pairs {
            let before = pair.state.epoch();
            if before & 1 == 0 {
                let read = |index: usize| {
                    let bits = self.published[index].load(Ordering::Acquire);
                    if bits == NO_OVERRIDE {
                        self.authored[index]
                    } else {
                        f64::from_bits(bits)
                    }
                };
                let pitch = read(pair.pitch_index);
                let gate = read(pair.gate_index);
                let velocity = pair.velocity_index.map(&read);
                if before == pair.state.epoch() {
                    pair.last_pitch = pitch;
                    pair.last_gate = gate;
                    if let Some(velocity) = velocity {
                        pair.last_velocity = velocity;
                    }
                }
            }
            self.params[pair.pitch_index] = pair.last_pitch;
            self.params[pair.gate_index] = pair.last_gate;
            if let Some(index) = pair.velocity_index {
                self.params[index] = pair.last_velocity;
            }
        }
    }

    pub(crate) fn process_prepared(&mut self, buffer: &mut [f64]) {
        for sample in buffer {
            for op in &mut self.ops {
                for (slot, input) in op.inputs.iter().enumerate() {
                    self.input_scratch[slot] = if input.sources.is_empty() {
                        input.unconnected
                    } else {
                        input
                            .sources
                            .iter()
                            .fold(0.0, |sum, source| sum + self.slots[*source])
                    };
                }
                for (slot, index) in op.params.iter().enumerate() {
                    self.param_scratch[slot] = self.params[*index];
                }
                (op.process)(
                    &mut op.state,
                    &self.input_scratch[..op.inputs.len()],
                    &self.param_scratch[..op.params.len()],
                    self.sample_rate,
                    &mut self.slots[op.outputs.clone()],
                );
            }
            *sample = self.slots[self.output_slot];
        }
    }

    pub fn order(&self) -> &[String] {
        &self.order
    }
    pub fn authored_normalizations(&self) -> &[Connection] {
        &self.authored_normalizations
    }
    pub fn effective_connections(&self) -> &[Connection] {
        &self.effective_connections
    }
    pub fn sample_rate(&self) -> f64 {
        self.sample_rate
    }
    pub(crate) fn revision_token(&self) -> Arc<()> {
        Arc::clone(&self.revision)
    }
    pub(crate) fn is_revision(&self, token: &Arc<()>) -> bool {
        Arc::ptr_eq(&self.revision, token)
    }
    pub(crate) fn resolve_timed_note(
        &self,
        channel: u8,
        pitch_hz: f64,
    ) -> Result<(usize, usize), String> {
        let target = self.timed_note_target(channel)?;
        if !pitch_hz.is_finite() || pitch_hz < target.min_pitch || pitch_hz > target.max_pitch {
            return Err(format!(
                "timed note pitch must be finite and within control range for channel {channel}: {}..{}",
                target.min_pitch, target.max_pitch
            ));
        }
        Ok((target.pitch_index, target.gate_index))
    }
    pub(crate) fn timed_note_target(&self, channel: u8) -> Result<TimedNoteTarget, String> {
        let binding = self
            .midi_notes
            .iter()
            .find(|binding| binding.channel == channel)
            .ok_or_else(|| format!("no MIDI note mapping for channel {channel}"))?;
        let pitch = &self.controls[&binding.pitch_control_id];
        let gate = &self.controls[&binding.gate_control_id];
        Ok(TimedNoteTarget {
            pitch_index: self.param_ids[&pitch.target],
            gate_index: self.param_ids[&gate.target],
            velocity_index: binding
                .velocity_control_id
                .as_ref()
                .map(|id| self.param_ids[&self.controls[id].target]),
            min_pitch: pitch.min,
            max_pitch: pitch.max,
        })
    }
    pub(crate) fn poly_note_targets(&self, channel: u8) -> Result<[PolyVoiceTarget; 2], String> {
        self.poly_notes
            .iter()
            .find(|mapping| mapping.channel == channel)
            .map(|mapping| mapping.voices)
            .ok_or_else(|| format!("no polyphonic MIDI mapping for channel {channel}"))
    }
    pub(crate) fn poly_note_channels(&self) -> impl Iterator<Item = u8> + '_ {
        self.poly_notes.iter().map(|mapping| mapping.channel)
    }
    pub fn has_poly_notes(&self) -> bool {
        !self.poly_notes.is_empty()
    }
    pub(crate) fn apply_timed_note(
        &mut self,
        pitch_index: usize,
        gate_index: usize,
        pitch_hz: f64,
        gate: bool,
    ) {
        self.params[pitch_index] = pitch_hz;
        self.params[gate_index] = if gate { 1.0 } else { 0.0 };
    }
    pub(crate) fn apply_timed_velocity(&mut self, index: Option<usize>, velocity: f64) {
        if let Some(index) = index {
            self.params[index] = velocity;
        }
    }
    pub fn parameter_ids(&self) -> impl Iterator<Item = &ParameterId> {
        self.param_ids.keys()
    }
    pub fn authored_value(&self, id: &ParameterId) -> Option<f64> {
        self.param_ids.get(id).map(|&i| self.authored[i])
    }
    pub fn effective_value(&self, id: &ParameterId) -> Option<f64> {
        self.param_ids.get(id).map(|&i| {
            let bits = self.published[i].load(Ordering::Acquire);
            if bits == NO_OVERRIDE {
                self.authored[i]
            } else {
                f64::from_bits(bits)
            }
        })
    }
    pub fn parameter_handle(&self, id: &ParameterId) -> Option<ParameterHandle> {
        self.param_ids.get(id).map(|&i| ParameterHandle {
            id: id.clone(),
            cell: Arc::clone(&self.published[i]),
            range: self.param_ranges[i],
        })
    }
    pub fn control_ids(&self) -> impl Iterator<Item = &str> {
        self.controls.keys().map(String::as_str)
    }
    pub fn midi_ccs(&self) -> &[MidiCc] {
        &self.midi_ccs
    }
    pub fn midi_notes(&self) -> &[MidiNotes] {
        &self.midi_notes
    }
    pub(crate) fn midi_note_state(&self, channel: u8) -> Option<Arc<NoteState>> {
        self.note_pairs
            .iter()
            .find(|pair| pair.channel == channel)
            .map(|pair| Arc::clone(&pair.state))
    }
    pub fn control_handle(&self, id: &str) -> Option<ControlHandle> {
        let binding = self.controls.get(id)?;
        Some(ControlHandle {
            id: id.to_owned(),
            parameter: self.parameter_handle(&binding.target)?,
            min: binding.min,
            max: binding.max,
        })
    }
    pub fn set_override(&self, id: &ParameterId, value: Option<f64>) -> Result<(), String> {
        self.parameter_handle(id)
            .ok_or_else(|| format!("unknown parameter {}.{}", id.module, id.name))?
            .publish(value)
    }
}

pub fn compile(graph: &Graph) -> Result<Machine, String> {
    compile_at_rate(graph, 48_000.0)
}

pub fn compile_at_rate(graph: &Graph, sample_rate: f64) -> Result<Machine, String> {
    if !sample_rate.is_finite() || sample_rate < 1.0 {
        return Err("sample rate must be finite and at least 1 Hz".into());
    }
    let expanded = crate::expand::expand(graph)?;
    let graph = &expanded;
    let mut ids = BTreeMap::new();
    for (index, module) in graph.modules.iter().enumerate() {
        if module.id.is_empty() {
            return Err("module ID must not be empty".into());
        }
        if ids.insert(module.id.as_str(), index).is_some() {
            return Err(format!("duplicate module ID '{}'", module.id));
        }
        let contract = contract::find(&module.kind)
            .ok_or_else(|| format!("unknown module type '{}'", module.kind))?;
        for (name, value) in &module.values {
            let param = contract
                .params
                .iter()
                .find(|p| p.name == name)
                .ok_or_else(|| format!("{}.{} is not a parameter", module.id, name))?;
            validate_parameter_value(&ParameterId::new(&module.id, name), param.range, *value)?;
        }
    }
    let output_slots: Vec<usize> = graph
        .modules
        .iter()
        .enumerate()
        .filter(|(_, module)| {
            contract::find(&module.kind).expect("module checked").role == Role::Output
        })
        .map(|(index, _)| index)
        .collect();
    if output_slots.len() != 1 {
        return Err(format!(
            "expected exactly one output module, found {}",
            output_slots.len()
        ));
    }

    // Slots belong to declared output ports. The Output role has no exposed
    // port, but owns one private slot for the final sample.
    let mut slot_bases = Vec::with_capacity(graph.modules.len());
    let mut slot_count = 0;
    for module in &graph.modules {
        slot_bases.push(slot_count);
        let c = contract::find(&module.kind).expect("module checked");
        slot_count += c.outputs.len().max(1);
    }

    // Validate both authored edge sets before selecting effective edges. An
    // overridden normalization is still an authored connection and must be valid.
    let resolve = |edge: &Connection| -> Result<(usize, usize, usize, FanIn), String> {
        let &src = ids
            .get(edge.from.module.as_str())
            .ok_or_else(|| format!("unknown source module '{}'", edge.from.module))?;
        let &dst = ids
            .get(edge.to.module.as_str())
            .ok_or_else(|| format!("unknown destination module '{}'", edge.to.module))?;
        let sc = contract::find(&graph.modules[src].kind).expect("module checked");
        let dc = contract::find(&graph.modules[dst].kind).expect("module checked");
        let sp = sc
            .outputs
            .iter()
            .enumerate()
            .find(|(_, p)| p.name == edge.from.port)
            .ok_or_else(|| {
                format!(
                    "invalid output port '{}.{}'",
                    edge.from.module, edge.from.port
                )
            })?;
        let dp = dc
            .inputs
            .iter()
            .find(|p| p.name == edge.to.port)
            .ok_or_else(|| format!("invalid input port '{}.{}'", edge.to.module, edge.to.port))?;
        if sp.1.domain != dp.domain {
            return Err(format!(
                "incompatible ports '{}.{}' and '{}.{}'",
                edge.from.module, edge.from.port, edge.to.module, edge.to.port
            ));
        }
        Ok((src, dst, slot_bases[src] + sp.0, dp.fan_in))
    };
    for edge in graph.connections.iter().chain(&graph.normalizations) {
        resolve(edge)?;
    }
    let mut normalized_destinations = BTreeSet::new();
    for edge in &graph.normalizations {
        if !normalized_destinations.insert(&edge.to) {
            return Err(format!(
                "ambiguous normalization for input '{}.{}'",
                edge.to.module, edge.to.port
            ));
        }
    }
    let explicit_destinations: BTreeSet<_> =
        graph.connections.iter().map(|edge| &edge.to).collect();
    let effective_connections: Vec<Connection> = graph
        .connections
        .iter()
        .chain(
            graph
                .normalizations
                .iter()
                .filter(|edge| !explicit_destinations.contains(&edge.to)),
        )
        .cloned()
        .collect();

    let mut input_edges: BTreeMap<(usize, &str), Vec<(usize, usize)>> = BTreeMap::new();
    let mut dependents = vec![Vec::<usize>::new(); graph.modules.len()];
    let mut indegree = vec![0usize; graph.modules.len()];
    for edge in &effective_connections {
        let (src, dst, source_slot, fan_in) = resolve(edge)?;
        let key = (dst, edge.to.port.as_str());
        let sources = input_edges.entry(key).or_default();
        if !sources.is_empty() && fan_in == FanIn::One {
            return Err(format!(
                "input '{}.{}' permits one connection",
                edge.to.module, edge.to.port
            ));
        }
        sources.push((src, source_slot));
        dependents[src].push(dst);
        indegree[dst] += 1;
    }
    for (i, module) in graph.modules.iter().enumerate() {
        let c = contract::find(&module.kind).expect("module checked");
        for port in c.inputs {
            if !input_edges.contains_key(&(i, port.name)) && port.unconnected.is_none() {
                return Err(format!(
                    "required input '{}.{}' is disconnected",
                    module.id, port.name
                ));
            }
        }
    }
    let mut ready: BTreeSet<(String, usize)> = graph
        .modules
        .iter()
        .enumerate()
        .filter(|(i, _)| indegree[*i] == 0)
        .map(|(i, m)| (m.id.clone(), i))
        .collect();
    let mut schedule = Vec::with_capacity(graph.modules.len());
    while let Some((_, i)) = ready.pop_first() {
        schedule.push(i);
        for &next in &dependents[i] {
            indegree[next] -= 1;
            if indegree[next] == 0 {
                ready.insert((graph.modules[next].id.clone(), next));
            }
        }
    }
    if schedule.len() != graph.modules.len() {
        return Err("graph contains an unsupported cycle".into());
    }

    let mut params = Vec::new();
    let mut param_ids = BTreeMap::new();
    let mut param_ranges = Vec::new();
    for &i in &schedule {
        let module = &graph.modules[i];
        let c = contract::find(&module.kind).expect("module checked");
        for p in c.params {
            let index = params.len();
            params.push(*module.values.get(p.name).unwrap_or(&p.default));
            param_ranges.push(p.range);
            param_ids.insert(ParameterId::new(&module.id, p.name), index);
        }
    }
    let mut controls = BTreeMap::new();
    for control in &graph.controls {
        if control.id.is_empty() {
            return Err("control ID must not be empty".into());
        }
        if !control.min.is_finite()
            || !control.max.is_finite()
            || control.min >= control.max
            || !(control.max - control.min).is_finite()
        {
            return Err(format!(
                "control '{}' requires finite min < max",
                control.id
            ));
        }
        let Some(&parameter_index) = param_ids.get(&control.target) else {
            return Err(format!(
                "control '{}' targets unknown parameter '{}.{}'",
                control.id, control.target.module, control.target.name
            ));
        };
        if let Some((min, max)) = param_ranges[parameter_index] {
            if control.min < min || control.max > max {
                return Err(format!(
                    "control '{}' range must stay within {}.{} {min}..{max}",
                    control.id, control.target.module, control.target.name
                ));
            }
        }
        if controls
            .insert(
                control.id.clone(),
                ControlBinding {
                    target: control.target.clone(),
                    min: control.min,
                    max: control.max,
                },
            )
            .is_some()
        {
            return Err(format!("duplicate control ID '{}'", control.id));
        }
    }
    let mut mapped_ccs = BTreeSet::new();
    for binding in &graph.midi_ccs {
        if !(1..=16).contains(&binding.channel) || binding.number > 127 {
            return Err(format!(
                "MIDI CC mapping for '{}' requires channel 1..16 and number 0..127",
                binding.control_id
            ));
        }
        if !controls.contains_key(&binding.control_id) {
            return Err(format!(
                "MIDI CC mapping targets unknown control '{}'",
                binding.control_id
            ));
        }
        if !mapped_ccs.insert((binding.channel, binding.number)) {
            return Err(format!(
                "duplicate MIDI CC mapping for channel {} CC {}",
                binding.channel, binding.number
            ));
        }
    }
    let mut mapped_note_channels = BTreeSet::new();
    let mut mapped_note_targets = BTreeSet::new();
    for binding in &graph.midi_notes {
        if !(1..=16).contains(&binding.channel) {
            return Err(format!(
                "MIDI note mapping requires channel 1..16, got {}",
                binding.channel
            ));
        }
        if !mapped_note_channels.insert(binding.channel) {
            return Err(format!(
                "duplicate MIDI note mapping for channel {}",
                binding.channel
            ));
        }
        let pitch = controls.get(&binding.pitch_control_id).ok_or_else(|| {
            format!(
                "MIDI note mapping targets unknown pitch control '{}'",
                binding.pitch_control_id
            )
        })?;
        let gate = controls.get(&binding.gate_control_id).ok_or_else(|| {
            format!(
                "MIDI note mapping targets unknown gate control '{}'",
                binding.gate_control_id
            )
        })?;
        if pitch.target == gate.target {
            return Err(
                "MIDI note pitch and gate controls must target different parameters".into(),
            );
        }
        if !mapped_note_targets.insert(&pitch.target) || !mapped_note_targets.insert(&gate.target) {
            return Err("MIDI note mappings must use distinct pitch and gate targets".into());
        }
        let lowest_hz = 440.0 * 2.0_f64.powf((0.0 - 69.0) / 12.0);
        let highest_hz = 440.0 * 2.0_f64.powf((127.0 - 69.0) / 12.0);
        if pitch.min > lowest_hz || pitch.max < highest_hz {
            return Err(format!(
                "MIDI note pitch control '{}' must cover all MIDI note frequencies",
                binding.pitch_control_id
            ));
        }
        if gate.min > 0.0 || gate.max < 1.0 {
            return Err(format!(
                "MIDI note gate control '{}' must cover 0..1",
                binding.gate_control_id
            ));
        }
        if let Some(id) = &binding.velocity_control_id {
            let velocity = controls.get(id).ok_or_else(|| {
                format!("MIDI note mapping targets unknown velocity control '{id}'")
            })?;
            if velocity.target == pitch.target
                || velocity.target == gate.target
                || !mapped_note_targets.insert(&velocity.target)
            {
                return Err("MIDI note velocity must target a distinct parameter".into());
            }
            if velocity.min > 0.0 || velocity.max < 1.0 {
                return Err(format!("MIDI note velocity control '{id}' must cover 0..1"));
            }
        }
    }
    let mut poly_notes = Vec::new();
    for binding in &graph.midi_poly {
        if !(1..=16).contains(&binding.channel) {
            return Err(format!(
                "polyphonic MIDI mapping requires channel 1..16, got {}",
                binding.channel
            ));
        }
        if !mapped_note_channels.insert(binding.channel) {
            return Err(format!(
                "duplicate MIDI note mapping for channel {}",
                binding.channel
            ));
        }
        if binding.voices.len() != 2 {
            return Err("polyphonic MIDI mapping requires exactly two voices".into());
        }
        let mut voices = Vec::with_capacity(2);
        for (number, voice) in binding.voices.iter().enumerate() {
            let resolve = |id: &str| {
                controls.get(id).ok_or_else(|| {
                    format!(
                        "polyphonic voice {} targets unknown control '{id}'",
                        number + 1
                    )
                })
            };
            let pitch = resolve(&voice.pitch_control_id)?;
            let gate = resolve(&voice.gate_control_id)?;
            let velocity = resolve(&voice.velocity_control_id)?;
            for target in [&pitch.target, &gate.target, &velocity.target] {
                if !mapped_note_targets.insert(target) {
                    return Err("polyphonic MIDI voices must use distinct parameter targets".into());
                }
            }
            let lowest_hz = 440.0 * 2.0_f64.powf((0.0 - 69.0) / 12.0);
            let highest_hz = 440.0 * 2.0_f64.powf((127.0 - 69.0) / 12.0);
            if pitch.min > lowest_hz || pitch.max < highest_hz {
                return Err(format!(
                    "polyphonic pitch control '{}' must cover all MIDI note frequencies",
                    voice.pitch_control_id
                ));
            }
            if gate.min > 0.0 || gate.max < 1.0 {
                return Err(format!(
                    "polyphonic gate control '{}' must cover 0..1",
                    voice.gate_control_id
                ));
            }
            if velocity.min > 0.0 || velocity.max < 1.0 {
                return Err(format!(
                    "polyphonic velocity control '{}' must cover 0..1",
                    voice.velocity_control_id
                ));
            }
            voices.push(PolyVoiceTarget {
                pitch_index: param_ids[&pitch.target],
                gate_index: param_ids[&gate.target],
                velocity_index: param_ids[&velocity.target],
            });
        }
        poly_notes.push(PolyMapping {
            channel: binding.channel,
            voices: [voices[0], voices[1]],
        });
    }
    let note_pairs = graph
        .midi_notes
        .iter()
        .map(|binding| {
            let pitch_index = param_ids[&controls[&binding.pitch_control_id].target];
            let gate_index = param_ids[&controls[&binding.gate_control_id].target];
            let velocity_index = binding
                .velocity_control_id
                .as_ref()
                .map(|id| param_ids[&controls[id].target]);
            NotePair {
                channel: binding.channel,
                pitch_index,
                gate_index,
                velocity_index,
                state: Arc::new(NoteState::new()),
                last_pitch: params[pitch_index],
                last_gate: params[gate_index],
                last_velocity: velocity_index.map_or(1.0, |index| params[index]),
            }
        })
        .collect();
    let mut ops = Vec::with_capacity(schedule.len());
    for &i in &schedule {
        let module = &graph.modules[i];
        let c = contract::find(&module.kind).expect("module checked");
        let inputs = c
            .inputs
            .iter()
            .map(|port| {
                let mut sources = input_edges
                    .get(&(i, port.name))
                    .cloned()
                    .unwrap_or_default();
                sources.sort_by(|a, b| {
                    graph.modules[a.0]
                        .id
                        .cmp(&graph.modules[b.0].id)
                        .then(a.1.cmp(&b.1))
                });
                InputPlan {
                    sources: sources.into_iter().map(|(_, slot)| slot).collect(),
                    unconnected: port.unconnected.unwrap_or(0.0),
                }
            })
            .collect();
        let params = c
            .params
            .iter()
            .map(|p| param_ids[&ParameterId::new(&module.id, p.name)])
            .collect();
        ops.push(Op {
            process: c.process,
            state: vec![0.0; c.state_len],
            inputs,
            outputs: slot_bases[i]..slot_bases[i] + c.outputs.len().max(1),
            params,
        });
    }
    let output_slot = slot_bases[output_slots[0]];
    Ok(Machine {
        revision: Arc::new(()),
        ops,
        slots: vec![0.0; slot_count],
        input_scratch: vec![
            0.0;
            graph
                .modules
                .iter()
                .map(|m| contract::find(&m.kind)
                    .expect("module checked")
                    .inputs
                    .len())
                .max()
                .unwrap_or(0)
        ],
        param_scratch: vec![
            0.0;
            graph
                .modules
                .iter()
                .map(|m| contract::find(&m.kind)
                    .expect("module checked")
                    .params
                    .len())
                .max()
                .unwrap_or(0)
        ],
        authored: params.clone(),
        published: (0..params.len())
            .map(|_| Arc::new(AtomicU64::new(NO_OVERRIDE)))
            .collect(),
        params,
        param_ranges,
        param_ids,
        controls,
        midi_ccs: graph.midi_ccs.clone(),
        midi_notes: graph.midi_notes.clone(),
        poly_notes,
        note_pairs,
        output_slot,
        order: schedule
            .iter()
            .map(|&i| graph.modules[i].id.clone())
            .collect(),
        authored_normalizations: graph.normalizations.clone(),
        effective_connections,
        sample_rate,
    })
}

#[derive(Default)]
pub struct Host {
    accepted: Option<Machine>,
}

impl Host {
    pub fn submit(&mut self, candidate: &Graph) -> Result<(), String> {
        let machine = compile(candidate)?;
        self.accepted = Some(machine);
        Ok(())
    }
    pub fn accepted(&mut self) -> Option<&mut Machine> {
        self.accepted.as_mut()
    }
}
