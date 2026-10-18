use std::collections::BTreeMap;

use mlua::{Lua, Table, Value};

use crate::contract;
use crate::model::{
    Connection, Control, Endpoint, ExternalInput, ExternalParameter, ExternalPort, Graph, MidiCc,
    MidiNotes, MidiPoly, MidiPolyVoice, Module, ParameterId, PolyOverflow, Screen, Subpatch,
    SubpatchInstance, Widget,
};

const API: &str = r#"
meldritch = { _subpatches = {} }
function meldritch.graph()
  local graph = { modules = {}, instances = {}, connections = {}, normalizations = {},
    controls = {}, midi_ccs = {}, note_bindings = {}, poly_bindings = {}, external_inputs = {}, external_outputs = {},
    external_parameters = {}, screens = {} }
  function graph:module(id, kind, values)
    assert(type(id) == "string" and type(kind) == "string", "module ID and type must be strings")
    local spec = meldritch._contract(kind)
    assert(spec, "unknown module type: " .. kind)
    local handle = { id = id }
    for _, name in ipairs(spec.inputs) do
      handle[name] = { module = id, port = name, direction = "input" }
    end
    for _, name in ipairs(spec.outputs) do
      handle[name] = { module = id, port = name, direction = "output" }
    end
    table.insert(self.modules, { id = id, kind = kind, values = values or {} })
    return handle
  end
  function graph:connect(source, destination)
    assert(type(source) == "table" and source.direction == "output", "source must be an output port")
    assert(type(destination) == "table" and destination.direction == "input", "destination must be an input port")
    table.insert(self.connections, { from = source, to = destination })
  end
  function graph:normalize(source, destination)
    assert(type(source) == "table" and source.direction == "output", "source must be an output port")
    assert(type(destination) == "table" and destination.direction == "input", "destination must be an input port")
    table.insert(self.normalizations, { from = source, to = destination })
  end
  function graph:input(name, destination, ...)
    assert(self.is_subpatch, "external ports belong to subpatch definitions")
    assert(type(name) == "string", "external input name must be a string")
    assert(type(destination) == "table" and destination.direction == "input", "external input must target an input port")
    local destinations = { destination, ... }
    for i = 1, 1 + select('#', ...) do
      local target = destinations[i]
      assert(type(target) == "table" and target.direction == "input", "external input must target input ports")
    end
    table.insert(self.external_inputs, { name = name, destinations = destinations })
  end
  function graph:output(name, source)
    assert(self.is_subpatch, "external ports belong to subpatch definitions")
    assert(type(name) == "string", "external output name must be a string")
    assert(type(source) == "table" and source.direction == "output", "external output must target an output port")
    table.insert(self.external_outputs, { name = name, endpoint = source })
  end
  function graph:parameter(name, module, parameter)
    assert(self.is_subpatch, "external parameters belong to subpatch definitions")
    assert(type(name) == "string", "external parameter name must be a string")
    assert(type(module) == "table" and type(module.id) == "string", "external parameter target must be a module or instance")
    assert(type(parameter) == "string", "external parameter target name must be a string")
    table.insert(self.external_parameters, { name = name, module = module.id, parameter = parameter })
  end
  function graph:instance(id, definition, values)
    assert(type(id) == "string", "instance ID must be a string")
    assert(type(definition) == "table" and definition.is_subpatch and
      meldritch._subpatches[definition.name] == definition, "instance requires a registered subpatch")
    local handle = { id = id }
    for _, port in ipairs(definition.external_inputs) do
      handle[port.name] = { module = id, port = port.name, direction = "input" }
    end
    for _, port in ipairs(definition.external_outputs) do
      handle[port.name] = { module = id, port = port.name, direction = "output" }
    end
    assert(values == nil or type(values) == "table", "instance values must be a table")
    table.insert(self.instances, { id = id, definition = definition.name, values = values or {} })
    return handle
  end
  function graph:control(id, module, parameter, range)
    assert(type(id) == "string", "control ID must be a string")
    assert(type(module) == "table" and type(module.id) == "string", "control target must be a module")
    assert(type(parameter) == "string", "control parameter must be a string")
    assert(type(range) == "table", "control range must be a table")
    table.insert(self.controls, { id = id, module = module.id, parameter = parameter,
      min = range.min, max = range.max })
  end
  function graph:midi_cc(control_id, channel, number)
    assert(type(control_id) == "string", "MIDI target must be a control ID")
    table.insert(self.midi_ccs, { control_id = control_id, channel = channel, number = number })
  end
  function graph:midi_notes(channel, pitch_control_id, gate_control_id, velocity_control_id)
    assert(type(pitch_control_id) == "string" and type(gate_control_id) == "string",
      "MIDI note targets must be control IDs")
    assert(velocity_control_id == nil or type(velocity_control_id) == "string",
      "MIDI velocity target must be a control ID")
    table.insert(self.note_bindings, { channel = channel,
      pitch_control_id = pitch_control_id, gate_control_id = gate_control_id,
      velocity_control_id = velocity_control_id })
  end
  function graph:midi_poly(channel, voices, overflow)
    assert(type(voices) == "table", "MIDI poly voices must be a table")
    table.insert(self.poly_bindings, { channel = channel, voices = voices, overflow = overflow })
  end
  function graph:screen(id, title)
    assert(type(id) == "string" and type(title) == "string", "screen ID and title must be strings")
    local screen = { id = id, title = title, widgets = {} }
    function screen:label(text)
      assert(type(text) == "string", "label text must be a string")
      table.insert(self.widgets, { kind = "label", text = text })
    end
    function screen:control(label, control_id)
      assert(type(label) == "string" and type(control_id) == "string", "control widget needs a label and control ID")
      table.insert(self.widgets, { kind = "control", label = label, control_id = control_id })
    end
    function screen:link(label, screen_id)
      assert(type(label) == "string" and type(screen_id) == "string", "link widget needs a label and screen ID")
      table.insert(self.widgets, { kind = "link", label = label, screen_id = screen_id })
    end
    table.insert(self.screens, screen)
    return screen
  end
  return graph
end
function meldritch.subpatch(name)
  assert(type(name) == "string" and name ~= "", "subpatch name must be a nonempty string")
  assert(not meldritch._subpatches[name], "duplicate subpatch definition: " .. name)
  local graph = meldritch.graph()
  graph.name = name
  graph.is_subpatch = true
  meldritch._subpatches[name] = graph
  return graph
end
"#;

pub fn read(script: &str) -> Result<Graph, String> {
    let lua = Lua::new();
    let discover = lua
        .create_function(|lua, kind: String| {
            let Some(c) = contract::find(&kind) else {
                return Ok(Value::Nil);
            };
            let spec = lua.create_table()?;
            let inputs = lua.create_table()?;
            for (i, p) in c.inputs.iter().enumerate() {
                inputs.set(i + 1, p.name)?;
            }
            let outputs = lua.create_table()?;
            for (i, p) in c.outputs.iter().enumerate() {
                outputs.set(i + 1, p.name)?;
            }
            let params = lua.create_table()?;
            for p in c.params {
                params.set(p.name, p.default)?;
            }
            spec.set("inputs", inputs)?;
            spec.set("outputs", outputs)?;
            spec.set("params", params)?;
            Ok(Value::Table(spec))
        })
        .map_err(|e| e.to_string())?;
    lua.load(API).exec().map_err(|e| e.to_string())?;
    let meldritch: Table = lua.globals().get("meldritch").map_err(|e| e.to_string())?;
    meldritch
        .set("_contract", discover)
        .map_err(|e| e.to_string())?;
    let table: Table = lua.load(script).eval().map_err(|e| e.to_string())?;
    let mut graph = graph_from_table(table)?;
    let definitions: Table = meldritch
        .get("_subpatches")
        .map_err(|e| format!("subpatch definitions: {e}"))?;
    for pair in definitions.pairs::<String, Table>() {
        let (name, definition) = pair.map_err(|e| e.to_string())?;
        let inputs = external_inputs(&definition)?;
        let outputs = external_ports(&definition, "external_outputs")?;
        let parameters = external_parameters(&definition)?;
        graph.subpatches.insert(
            name,
            Subpatch {
                graph: graph_from_table(definition)?,
                inputs,
                outputs,
                parameters,
            },
        );
    }
    Ok(graph)
}

fn external_inputs(table: &Table) -> Result<Vec<ExternalInput>, String> {
    let inputs: Table = table.get("external_inputs").map_err(|e| e.to_string())?;
    inputs
        .sequence_values::<Table>()
        .map(|item| {
            let item = item.map_err(|e| e.to_string())?;
            let destinations: Table = item.get("destinations").map_err(|e| e.to_string())?;
            Ok(ExternalInput {
                name: item.get("name").map_err(|e| e.to_string())?,
                destinations: destinations
                    .sequence_values::<Table>()
                    .map(|target| endpoint(target.map_err(|e| e.to_string())?))
                    .collect::<Result<Vec<_>, _>>()?,
            })
        })
        .collect()
}

fn external_parameters(table: &Table) -> Result<Vec<ExternalParameter>, String> {
    let parameters: Table = table
        .get("external_parameters")
        .map_err(|e| e.to_string())?;
    parameters
        .sequence_values::<Table>()
        .map(|item| {
            let item = item.map_err(|e| e.to_string())?;
            Ok(ExternalParameter {
                name: item.get("name").map_err(|e| e.to_string())?,
                target: ParameterId::new(
                    item.get::<String>("module").map_err(|e| e.to_string())?,
                    item.get::<String>("parameter").map_err(|e| e.to_string())?,
                ),
            })
        })
        .collect()
}

fn external_ports(table: &Table, field: &str) -> Result<Vec<ExternalPort>, String> {
    let ports: Table = table.get(field).map_err(|e| format!("{field}: {e}"))?;
    ports
        .sequence_values::<Table>()
        .map(|item| {
            let item = item.map_err(|e| e.to_string())?;
            Ok(ExternalPort {
                name: item.get("name").map_err(|e| e.to_string())?,
                endpoint: endpoint(item.get("endpoint").map_err(|e| e.to_string())?)?,
            })
        })
        .collect()
}

fn graph_from_table(table: Table) -> Result<Graph, String> {
    let mut graph = Graph::default();
    let screens: Table = table.get("screens").map_err(|e| e.to_string())?;
    for screen in screens.sequence_values::<Table>() {
        let screen = screen.map_err(|e| e.to_string())?;
        let widgets: Table = screen.get("widgets").map_err(|e| e.to_string())?;
        let mut parsed = Vec::new();
        for widget in widgets.sequence_values::<Table>() {
            let widget = widget.map_err(|e| e.to_string())?;
            let kind: String = widget.get("kind").map_err(|e| e.to_string())?;
            parsed.push(match kind.as_str() {
                "label" => Widget::Label(widget.get("text").map_err(|e| e.to_string())?),
                "control" => Widget::Control {
                    label: widget.get("label").map_err(|e| e.to_string())?,
                    control_id: widget.get("control_id").map_err(|e| e.to_string())?,
                },
                "link" => Widget::Link {
                    label: widget.get("label").map_err(|e| e.to_string())?,
                    screen_id: widget.get("screen_id").map_err(|e| e.to_string())?,
                },
                other => return Err(format!("unknown screen widget '{other}'")),
            });
        }
        graph.screens.push(Screen {
            id: screen.get("id").map_err(|e| e.to_string())?,
            title: screen.get("title").map_err(|e| e.to_string())?,
            widgets: parsed,
        });
    }
    let modules: Table = table
        .get("modules")
        .map_err(|e| format!("graph.modules: {e}"))?;
    for item in modules.sequence_values::<Table>() {
        let item = item.map_err(|e| e.to_string())?;
        let values: Table = item.get("values").map_err(|e| e.to_string())?;
        let mut params = BTreeMap::new();
        for pair in values.pairs::<String, f64>() {
            let (name, value) = pair.map_err(|e| e.to_string())?;
            params.insert(name, value);
        }
        graph.modules.push(Module {
            id: item.get("id").map_err(|e| e.to_string())?,
            kind: item.get("kind").map_err(|e| e.to_string())?,
            values: params,
        });
    }
    let instances: Table = table
        .get("instances")
        .map_err(|e| format!("graph.instances: {e}"))?;
    for item in instances.sequence_values::<Table>() {
        let item = item.map_err(|e| e.to_string())?;
        let values: Table = item.get("values").map_err(|e| e.to_string())?;
        let mut params = BTreeMap::new();
        for pair in values.pairs::<String, f64>() {
            let (name, value) = pair.map_err(|e| e.to_string())?;
            params.insert(name, value);
        }
        graph.instances.push(SubpatchInstance {
            id: item.get("id").map_err(|e| e.to_string())?,
            definition: item.get("definition").map_err(|e| e.to_string())?,
            values: params,
        });
    }
    let connections: Table = table
        .get("connections")
        .map_err(|e| format!("graph.connections: {e}"))?;
    for item in connections.sequence_values::<Table>() {
        let item = item.map_err(|e| e.to_string())?;
        graph.connections.push(Connection {
            from: endpoint(item.get("from").map_err(|e| e.to_string())?)?,
            to: endpoint(item.get("to").map_err(|e| e.to_string())?)?,
        });
    }
    let normalizations: Table = table
        .get("normalizations")
        .map_err(|e| format!("graph.normalizations: {e}"))?;
    for item in normalizations.sequence_values::<Table>() {
        let item = item.map_err(|e| e.to_string())?;
        graph.normalizations.push(Connection {
            from: endpoint(item.get("from").map_err(|e| e.to_string())?)?,
            to: endpoint(item.get("to").map_err(|e| e.to_string())?)?,
        });
    }
    let controls: Table = table
        .get("controls")
        .map_err(|e| format!("graph.controls: {e}"))?;
    for item in controls.sequence_values::<Table>() {
        let item = item.map_err(|e| e.to_string())?;
        graph.controls.push(Control {
            id: item.get("id").map_err(|e| e.to_string())?,
            target: ParameterId::new(
                item.get::<String>("module").map_err(|e| e.to_string())?,
                item.get::<String>("parameter").map_err(|e| e.to_string())?,
            ),
            min: item.get("min").map_err(|e| e.to_string())?,
            max: item.get("max").map_err(|e| e.to_string())?,
        });
    }
    let midi_ccs: Table = table
        .get("midi_ccs")
        .map_err(|e| format!("graph.midi_ccs: {e}"))?;
    for item in midi_ccs.sequence_values::<Table>() {
        let item = item.map_err(|e| e.to_string())?;
        graph.midi_ccs.push(MidiCc {
            control_id: item.get("control_id").map_err(|e| e.to_string())?,
            channel: item.get("channel").map_err(|e| e.to_string())?,
            number: item.get("number").map_err(|e| e.to_string())?,
        });
    }
    let midi_notes: Table = table
        .get("note_bindings")
        .map_err(|e| format!("graph.note_bindings: {e}"))?;
    for item in midi_notes.sequence_values::<Table>() {
        let item = item.map_err(|e| e.to_string())?;
        graph.midi_notes.push(MidiNotes {
            channel: item.get("channel").map_err(|e| e.to_string())?,
            pitch_control_id: item.get("pitch_control_id").map_err(|e| e.to_string())?,
            gate_control_id: item.get("gate_control_id").map_err(|e| e.to_string())?,
            velocity_control_id: item.get("velocity_control_id").map_err(|e| e.to_string())?,
        });
    }
    let midi_poly: Table = table.get("poly_bindings").map_err(|e| e.to_string())?;
    for item in midi_poly.sequence_values::<Table>() {
        let item = item.map_err(|e| e.to_string())?;
        let voices: Table = item.get("voices").map_err(|e| e.to_string())?;
        let mut parsed = Vec::new();
        for voice in voices.sequence_values::<Table>() {
            let voice = voice.map_err(|e| e.to_string())?;
            parsed.push(MidiPolyVoice {
                pitch_control_id: voice.get("pitch").map_err(|e| e.to_string())?,
                gate_control_id: voice.get("gate").map_err(|e| e.to_string())?,
                velocity_control_id: voice.get("velocity").map_err(|e| e.to_string())?,
            });
        }
        graph.midi_poly.push(MidiPoly {
            channel: item.get("channel").map_err(|e| e.to_string())?,
            voices: parsed,
            overflow: match item
                .get::<Option<String>>("overflow")
                .map_err(|e| e.to_string())?
                .as_deref()
            {
                None => PolyOverflow::Reject,
                Some("steal_oldest") => PolyOverflow::StealOldest,
                Some(other) => return Err(format!("unknown polyphonic overflow policy '{other}'")),
            },
        });
    }
    Ok(graph)
}

fn endpoint(table: Table) -> Result<Endpoint, String> {
    Ok(Endpoint {
        module: table.get("module").map_err(|e| e.to_string())?,
        port: table.get("port").map_err(|e| e.to_string())?,
    })
}
