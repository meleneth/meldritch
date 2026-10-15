use std::collections::{BTreeMap, BTreeSet};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use crate::contract::{self, FanIn, Role};
use crate::model::{Graph, MidiCc, ParameterId};

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

const NO_OVERRIDE: u64 = f64::NAN.to_bits();

fn validate_parameter_value(
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
    output_slot: usize,
    order: Vec<String>,
    sample_rate: f64,
}

impl Machine {
    pub fn process(&mut self, buffer: &mut [f64]) {
        // Snapshot all published values once, before processing this buffer.
        for (index, cell) in self.published.iter().enumerate() {
            let bits = cell.load(Ordering::Acquire);
            self.params[index] = if bits == NO_OVERRIDE {
                self.authored[index]
            } else {
                f64::from_bits(bits)
            };
        }
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
    pub fn sample_rate(&self) -> f64 {
        self.sample_rate
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

    let mut input_edges: BTreeMap<(usize, &str), Vec<(usize, usize)>> = BTreeMap::new();
    let mut dependents = vec![Vec::<usize>::new(); graph.modules.len()];
    let mut indegree = vec![0usize; graph.modules.len()];
    for edge in &graph.connections {
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
        let key = (dst, edge.to.port.as_str());
        let sources = input_edges.entry(key).or_default();
        if !sources.is_empty() && dp.fan_in == FanIn::One {
            return Err(format!(
                "input '{}.{}' permits one connection",
                edge.to.module, edge.to.port
            ));
        }
        sources.push((src, slot_bases[src] + sp.0));
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
        output_slot,
        order: schedule
            .iter()
            .map(|&i| graph.modules[i].id.clone())
            .collect(),
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
