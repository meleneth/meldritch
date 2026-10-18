//! Compile-time lowering of authored subpatch instances to primitive graphs.

use std::collections::{BTreeMap, BTreeSet};

use crate::compiler::validate_parameter_value;
use crate::contract::{self, Role};
use crate::model::{
    Connection, Endpoint, ExternalInput, ExternalParameter, ExternalPort, Graph, ParameterId,
    Subpatch,
};

#[derive(Default)]
struct Ports {
    inputs: BTreeMap<String, Vec<Endpoint>>,
    outputs: BTreeMap<String, Endpoint>,
    parameters: BTreeMap<String, ParameterId>,
}

pub fn expand(graph: &Graph) -> Result<Graph, String> {
    if graph.subpatches.is_empty() && graph.instances.is_empty() {
        return Ok(graph.clone());
    }
    // Visit every definition, including those not instantiated by the root.
    // This catches recursive and invalid interface declarations before a
    // candidate can replace an accepted machine.
    for (name, definition) in &graph.subpatches {
        let mut scratch = Graph::default();
        let mut ids = BTreeSet::new();
        let mut stack = vec![name.clone()];
        expand_scope(
            &definition.graph,
            &format!("{name}/"),
            &graph.subpatches,
            &mut stack,
            &mut scratch,
            &mut ids,
            Some((
                &definition.inputs,
                &definition.outputs,
                &definition.parameters,
            )),
        )?;
    }

    let mut expanded = Graph {
        controls: graph.controls.clone(),
        midi_ccs: graph.midi_ccs.clone(),
        midi_notes: graph.midi_notes.clone(),
        midi_poly: graph.midi_poly.clone(),
        screens: graph.screens.clone(),
        ..Graph::default()
    };
    let mut ids = BTreeSet::new();
    expand_scope(
        graph,
        "",
        &graph.subpatches,
        &mut Vec::new(),
        &mut expanded,
        &mut ids,
        None,
    )?;
    Ok(expanded)
}

fn external_outputs(
    declarations: &[ExternalPort],
    local: &BTreeMap<String, Ports>,
) -> Result<BTreeMap<String, Endpoint>, String> {
    let mut ports = BTreeMap::new();
    for declaration in declarations {
        if declaration.name.is_empty() {
            return Err("external port name must not be empty".into());
        }
        let endpoint = resolve_output(local, &declaration.endpoint)?;
        if ports.insert(declaration.name.clone(), endpoint).is_some() {
            return Err(format!("duplicate external port '{}'", declaration.name));
        }
    }
    Ok(ports)
}

fn external_inputs(
    declarations: &[ExternalInput],
    local: &BTreeMap<String, Ports>,
) -> Result<BTreeMap<String, Vec<Endpoint>>, String> {
    let mut ports = BTreeMap::new();
    for declaration in declarations {
        if declaration.name.is_empty() || declaration.destinations.is_empty() {
            return Err("external input needs a name and destination".into());
        }
        let mut targets = Vec::new();
        let mut seen = BTreeSet::new();
        for destination in &declaration.destinations {
            for endpoint in resolve_input(local, destination)? {
                if !seen.insert(endpoint.clone()) {
                    return Err(format!(
                        "duplicate destination for external input '{}'",
                        declaration.name
                    ));
                }
                targets.push(endpoint);
            }
        }
        if ports.insert(declaration.name.clone(), targets).is_some() {
            return Err(format!("duplicate external port '{}'", declaration.name));
        }
    }
    Ok(ports)
}

fn external_parameters(
    declarations: &[ExternalParameter],
    local: &BTreeMap<String, Ports>,
) -> Result<BTreeMap<String, ParameterId>, String> {
    let mut parameters = BTreeMap::new();
    for declaration in declarations {
        if declaration.name.is_empty() {
            return Err("external parameter name must not be empty".into());
        }
        let target = resolve_parameter(local, &declaration.target)?;
        if parameters.values().any(|existing| existing == &target) {
            return Err(format!(
                "duplicate external parameter target '{}.{}'",
                target.module, target.name
            ));
        }
        if parameters
            .insert(declaration.name.clone(), target)
            .is_some()
        {
            return Err(format!(
                "duplicate external parameter '{}'",
                declaration.name
            ));
        }
    }
    Ok(parameters)
}

fn resolve_parameter(
    local: &BTreeMap<String, Ports>,
    target: &ParameterId,
) -> Result<ParameterId, String> {
    let ports = local
        .get(&target.module)
        .ok_or_else(|| format!("unknown module '{}' in subpatch parameter", target.module))?;
    ports.parameters.get(&target.name).cloned().ok_or_else(|| {
        format!(
            "invalid parameter '{}.{}' in subpatch",
            target.module, target.name
        )
    })
}

fn validate_authored_value(
    module: &crate::model::Module,
    name: &str,
    value: f64,
) -> Result<(), String> {
    let contract = contract::find(&module.kind)
        .ok_or_else(|| format!("unknown module type '{}'", module.kind))?;
    let parameter = contract
        .params
        .iter()
        .find(|parameter| parameter.name == name)
        .ok_or_else(|| format!("{}.{} is not a parameter", module.id, name))?;
    validate_parameter_value(&ParameterId::new(&module.id, name), parameter.range, value)
}

fn resolve_input(
    local: &BTreeMap<String, Ports>,
    endpoint: &Endpoint,
) -> Result<Vec<Endpoint>, String> {
    let ports = local.get(&endpoint.module).ok_or_else(|| {
        format!(
            "unknown module '{}' in subpatch connection",
            endpoint.module
        )
    })?;
    ports.inputs.get(&endpoint.port).cloned().ok_or_else(|| {
        format!(
            "invalid input port '{}.{}' in subpatch connection",
            endpoint.module, endpoint.port
        )
    })
}

fn resolve_output(
    local: &BTreeMap<String, Ports>,
    endpoint: &Endpoint,
) -> Result<Endpoint, String> {
    let ports = local.get(&endpoint.module).ok_or_else(|| {
        format!(
            "unknown module '{}' in subpatch connection",
            endpoint.module
        )
    })?;
    ports.outputs.get(&endpoint.port).cloned().ok_or_else(|| {
        format!(
            "invalid output port '{}.{}' in subpatch connection",
            endpoint.module, endpoint.port
        )
    })
}

#[allow(clippy::too_many_arguments)]
fn expand_scope(
    scope: &Graph,
    prefix: &str,
    definitions: &BTreeMap<String, Subpatch>,
    stack: &mut Vec<String>,
    expanded: &mut Graph,
    ids: &mut BTreeSet<String>,
    interface: Option<(&[ExternalInput], &[ExternalPort], &[ExternalParameter])>,
) -> Result<Ports, String> {
    if !prefix.is_empty()
        && (!scope.controls.is_empty()
            || !scope.midi_ccs.is_empty()
            || !scope.midi_notes.is_empty()
            || !scope.midi_poly.is_empty()
            || !scope.screens.is_empty())
    {
        return Err("controls, MIDI bindings, and screens belong to the parent graph".into());
    }
    if !prefix.is_empty() && !scope.subpatches.is_empty() {
        return Err("subpatch definitions must be registered on the root graph".into());
    }
    let mut local = BTreeMap::new();
    for module in &scope.modules {
        if module.id.is_empty() || local.contains_key(&module.id) {
            return Err(format!("duplicate or empty module ID '{}'", module.id));
        }
        let contract = contract::find(&module.kind)
            .ok_or_else(|| format!("unknown module type '{}'", module.kind))?;
        if !prefix.is_empty() && contract.role == Role::Output {
            return Err("subpatches expose output ports rather than Output modules".into());
        }
        let local_id = module.id.clone();
        let qualified = format!("{prefix}{local_id}");
        if !ids.insert(qualified.clone()) {
            return Err(format!("duplicate expanded module ID '{qualified}'"));
        }
        let ports = Ports {
            inputs: contract
                .inputs
                .iter()
                .map(|port| {
                    (
                        port.name.to_owned(),
                        vec![Endpoint {
                            module: qualified.clone(),
                            port: port.name.into(),
                        }],
                    )
                })
                .collect(),
            outputs: contract
                .outputs
                .iter()
                .map(|port| {
                    (
                        port.name.to_owned(),
                        Endpoint {
                            module: qualified.clone(),
                            port: port.name.into(),
                        },
                    )
                })
                .collect(),
            parameters: contract
                .params
                .iter()
                .map(|parameter| {
                    (
                        parameter.name.to_owned(),
                        ParameterId::new(&qualified, parameter.name),
                    )
                })
                .collect(),
        };
        let mut module = module.clone();
        module.id = qualified;
        for (name, value) in &module.values {
            validate_authored_value(&module, name, *value)?;
        }
        expanded.modules.push(module);
        local.insert(local_id, ports);
    }
    for instance in &scope.instances {
        if instance.id.is_empty() || local.contains_key(&instance.id) {
            return Err(format!("duplicate or empty instance ID '{}'", instance.id));
        }
        let definition = definitions
            .get(&instance.definition)
            .ok_or_else(|| format!("unknown subpatch definition '{}'", instance.definition))?;
        if stack.contains(&instance.definition) {
            return Err(format!(
                "recursive subpatch definition '{}'",
                instance.definition
            ));
        }
        stack.push(instance.definition.clone());
        let ports = expand_scope(
            &definition.graph,
            &format!("{prefix}{}/", instance.id),
            definitions,
            stack,
            expanded,
            ids,
            Some((
                &definition.inputs,
                &definition.outputs,
                &definition.parameters,
            )),
        )?;
        stack.pop();
        for (name, value) in &instance.values {
            let target = ports.parameters.get(name).ok_or_else(|| {
                format!(
                    "invalid authored parameter '{}.{}' for instance",
                    instance.id, name
                )
            })?;
            let module = expanded
                .modules
                .iter_mut()
                .find(|module| module.id == target.module)
                .ok_or_else(|| {
                    format!("unresolved exported parameter '{}.{}'", instance.id, name)
                })?;
            validate_authored_value(module, &target.name, *value)?;
            module.values.insert(target.name.clone(), *value);
        }
        local.insert(instance.id.clone(), ports);
    }
    for edge in &scope.connections {
        let from = resolve_output(&local, &edge.from)?;
        for to in resolve_input(&local, &edge.to)? {
            expanded.connections.push(Connection {
                from: from.clone(),
                to,
            });
        }
    }
    for edge in &scope.normalizations {
        let from = resolve_output(&local, &edge.from)?;
        for to in resolve_input(&local, &edge.to)? {
            expanded.normalizations.push(Connection {
                from: from.clone(),
                to,
            });
        }
    }
    if let Some((inputs, outputs, parameters)) = interface {
        let inputs = external_inputs(inputs, &local)?;
        let outputs = external_outputs(outputs, &local)?;
        let parameters = external_parameters(parameters, &local)?;
        if let Some(name) = inputs.keys().find(|name| outputs.contains_key(*name)) {
            return Err(format!("external input and output share name '{name}'"));
        }
        Ok(Ports {
            inputs,
            outputs,
            parameters,
        })
    } else {
        for control in &mut expanded.controls {
            if let Some(instance) = scope
                .instances
                .iter()
                .find(|instance| instance.id == control.target.module)
            {
                control.target = resolve_parameter(&local, &control.target).map_err(|_| {
                    format!(
                        "invalid exported parameter '{}.{}' for instance '{}'",
                        instance.id, control.target.name, instance.id
                    )
                })?;
            }
        }
        Ok(Ports::default())
    }
}
