use std::collections::{BTreeMap, BTreeSet};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use crate::contract::{self, FanIn};
use crate::model::{Graph, ParameterId};

#[derive(Clone, Debug)]
struct Op {
    process: fn(&mut [f64], &[f64], f64, f64) -> f64,
    state: Vec<f64>,
    inputs: Vec<usize>,
    output: usize,
    param: Option<usize>,
}

const NO_OVERRIDE: u64 = f64::NAN.to_bits();

/// A control-side handle for one parameter in one compiled graph revision.
#[derive(Clone, Debug)]
pub struct ParameterHandle {
    id: ParameterId,
    cell: Arc<AtomicU64>,
}

impl ParameterHandle {
    pub fn id(&self) -> &ParameterId {
        &self.id
    }

    pub fn publish(&self, value: Option<f64>) -> Result<(), String> {
        let bits = match value {
            Some(value) if value.is_finite() => value.to_bits(),
            Some(_) => {
                return Err(format!(
                    "{}.{} must be finite",
                    self.id.module, self.id.name
                ));
            }
            None => NO_OVERRIDE,
        };
        self.cell.store(bits, Ordering::Release);
        Ok(())
    }
}

/// An accepted, executable graph. Its processing buffers are allocated at compile time.
#[derive(Debug)]
pub struct Machine {
    ops: Vec<Op>,
    slots: Vec<f64>,
    input_scratch: Vec<f64>,
    params: Vec<f64>,
    authored: Vec<f64>,
    published: Vec<Arc<AtomicU64>>,
    param_ids: BTreeMap<ParameterId, usize>,
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
                for (slot, source) in op.inputs.iter().enumerate() {
                    self.input_scratch[slot] = self.slots[*source];
                }
                let param = op.param.map(|i| self.params[i]).unwrap_or(0.0);
                self.slots[op.output] = (op.process)(
                    &mut op.state,
                    &self.input_scratch[..op.inputs.len()],
                    param,
                    self.sample_rate,
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
        })
    }
    pub fn set_override(&self, id: &ParameterId, value: Option<f64>) -> Result<(), String> {
        let &index = self
            .param_ids
            .get(id)
            .ok_or_else(|| format!("unknown parameter {}.{}", id.module, id.name))?;
        let bits = match value {
            Some(value) if value.is_finite() => value.to_bits(),
            Some(_) => return Err(format!("{}.{} must be finite", id.module, id.name)),
            None => NO_OVERRIDE,
        };
        self.published[index].store(bits, Ordering::Release);
        Ok(())
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
        if contract.params.len() > 1 {
            return Err(format!(
                "module type '{}' exceeds the current one-parameter executor",
                module.kind
            ));
        }
        for (name, value) in &module.values {
            if !contract.params.iter().any(|p| p.name == name) {
                return Err(format!("{}.{} is not a parameter", module.id, name));
            }
            if !value.is_finite() {
                return Err(format!("{}.{} must be finite", module.id, name));
            }
        }
    }
    let outputs = graph.modules.iter().filter(|m| m.kind == "output").count();
    if outputs != 1 {
        return Err(format!(
            "expected exactly one output module, found {outputs}"
        ));
    }

    let mut input_edges = BTreeMap::new();
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
            .find(|p| p.name == edge.from.port)
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
        if sp.domain != dp.domain {
            return Err(format!(
                "incompatible ports '{}.{}' and '{}.{}'",
                edge.from.module, edge.from.port, edge.to.module, edge.to.port
            ));
        }
        let key = (dst, edge.to.port.as_str());
        if input_edges.insert(key, src).is_some() {
            match dp.fan_in {
                FanIn::One => {
                    return Err(format!(
                        "input '{}.{}' permits one connection",
                        edge.to.module, edge.to.port
                    ));
                }
            }
        }
        dependents[src].push(dst);
        indegree[dst] += 1;
    }
    for (i, module) in graph.modules.iter().enumerate() {
        let c = contract::find(&module.kind).expect("module checked");
        for port in c.inputs {
            if !input_edges.contains_key(&(i, port.name)) {
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
    for &i in &schedule {
        let module = &graph.modules[i];
        let c = contract::find(&module.kind).expect("module checked");
        for p in c.params {
            let index = params.len();
            params.push(*module.values.get(p.name).unwrap_or(&p.default));
            param_ids.insert(ParameterId::new(&module.id, p.name), index);
        }
    }
    let mut ops = Vec::with_capacity(schedule.len());
    for &i in &schedule {
        let module = &graph.modules[i];
        let c = contract::find(&module.kind).expect("module checked");
        let inputs = c
            .inputs
            .iter()
            .map(|port| input_edges[&(i, port.name)])
            .collect();
        let param = c
            .params
            .first()
            .map(|p| param_ids[&ParameterId::new(&module.id, p.name)]);
        ops.push(Op {
            process: c.process,
            state: vec![0.0; c.state_len],
            inputs,
            output: i,
            param,
        });
    }
    let output_slot = graph
        .modules
        .iter()
        .position(|m| m.kind == "output")
        .expect("one output checked");
    Ok(Machine {
        ops,
        slots: vec![0.0; graph.modules.len()],
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
        authored: params.clone(),
        published: (0..params.len())
            .map(|_| Arc::new(AtomicU64::new(NO_OVERRIDE)))
            .collect(),
        params,
        param_ids,
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
