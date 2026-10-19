use std::collections::BTreeMap;

use mlua::{Function, Lua, Table, Value};

use crate::contract;
use crate::model::{
    Connection, Control, ControlScale, Endpoint, ExternalInput, ExternalParameter, ExternalPort,
    Graph, MidiCc, MidiNotes, MidiPoly, MidiPolyVoice, Module, ParameterId, PolyOverflow, Screen,
    Subpatch, SubpatchInstance, Widget,
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
      min = range.min, max = range.max, scale = range.scale })
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
    function screen:button(label, control_id, value)
      assert(type(label) == "string" and type(control_id) == "string" and type(value) == "number",
        "button widget needs a label, control ID, and numeric value")
      table.insert(self.widgets, { kind = "button", label = label, control_id = control_id, value = value })
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

local instrument_serial = 0

local function parameter(module, name)
  return { module = module.id, parameter = name, direction = "parameter" }
end

local function copy_values(values, names)
  local copied = {}
  values = values or {}
  for _, name in ipairs(names) do
    if values[name] ~= nil then copied[name] = values[name] end
  end
  return copied
end

local function decorate_scope(facade, scope, prefix)
  facade._scope = scope
  facade._counts = {}
  function facade:_id(kind)
    self._counts[kind] = (self._counts[kind] or 0) + 1
    return kind .. "_" .. self._counts[kind]
  end
  function facade:oscillator(values)
    values = values or {}
    local waveform = values.waveform or "sine"
    assert(waveform == "sine" or waveform == "saw", "oscillator waveform must be 'sine' or 'saw'")
    local kind = waveform == "saw" and "saw" or "oscillator_sine"
    local handle = scope:module(self:_id("oscillator"), kind, copy_values(values, { "frequency" }))
    handle.frequency = parameter(handle, "frequency")
    return handle
  end
  function facade:lfo(values)
    values = values or {}
    assert((values.waveform or "sine") == "sine", "LFO currently supports the sine waveform")
    local handle = scope:module(self:_id("lfo"), "oscillator_sine", copy_values(values, { "frequency" }))
    handle.frequency = parameter(handle, "frequency")
    return handle
  end
  function facade:lowpass(values)
    local handle = scope:module(self:_id("lowpass"), "lowpass", copy_values(values, { "cutoff", "resonance" }))
    handle.cutoff = parameter(handle, "cutoff")
    handle.resonance = parameter(handle, "resonance")
    return handle
  end
  function facade:adsr(values)
    local handle = scope:module(self:_id("adsr"), "adsr", copy_values(values,
      { "attack", "decay", "sustain", "release" }))
    handle.attack = parameter(handle, "attack")
    handle.decay = parameter(handle, "decay")
    handle.sustain = parameter(handle, "sustain")
    handle.release = parameter(handle, "release")
    return handle
  end
  function facade:vca(values)
    assert(values == nil or next(values) == nil, "VCA has no authored values")
    return scope:module(self:_id("vca"), "vca")
  end
  function facade:constant(values)
    local handle = scope:module(self:_id("constant"), "constant", copy_values(values, { "value" }))
    handle.value = parameter(handle, "value")
    return handle
  end
  function facade:multiply(values)
    assert(values == nil or next(values) == nil, "multiply has no authored values")
    local handle = scope:module(self:_id("multiply"), "multiply")
    handle.a = handle.left
    handle.b = handle.right
    return handle
  end
  function facade:mixer(values)
    values = values or {}
    assert(values.inputs == 2, "mixer currently requires exactly two inputs")
    local handle = scope:module(self:_id("mixer"), "mixer2")
    handle.input = { handle.input_1, handle.input_2 }
    return handle
  end
  function facade:connect(source, destination)
    scope:connect(source, destination)
  end
  function facade:subpatch(label)
    assert(type(label) == "string" and label ~= "", "subpatch label must be nonempty")
    local id = self:_id("subpatch")
    local definition_name = prefix .. "_" .. id
    local definition = meldritch.subpatch(definition_name)
    table.insert(scope.instances, { id = id, definition = definition_name, values = {} })
    local composite = { id = id }
    decorate_scope(composite, definition, definition_name)
    function composite:expose(name, target)
      assert(type(name) == "string" and name ~= "", "exposed name must be nonempty")
      assert(type(target) == "table", "exposed target must be a port or parameter")
      if target.direction == "output" then
        definition:output(name, target)
        self[name] = { module = id, port = name, direction = "output" }
      elseif target.direction == "parameter" then
        definition:parameter(name, { id = target.module }, target.parameter)
        self[name] = { module = id, parameter = name, direction = "parameter" }
      else
        error("expose supports output ports and writable parameters")
      end
    end
    return composite
  end
end

function meldritch.instrument(name)
  assert(type(name) == "string" and name ~= "", "instrument name must be nonempty")
  instrument_serial = instrument_serial + 1
  local prefix = "instrument_" .. instrument_serial
  local definition = meldritch.subpatch(prefix)
  local instrument = {
    name = name,
    _definition = definition,
    _controls = {},
    _left = nil,
    _right = nil,
    _uses_notes = false,
    note = {
      pitch = { direction = "note", signal = "pitch" },
      gate = { direction = "note", signal = "gate" },
    },
    output = {
      left = { direction = "terminal", channel = "left" },
      right = { direction = "terminal", channel = "right" },
    },
  }
  decorate_scope(instrument, definition, prefix)

  local ordinary_connect = instrument.connect
  function instrument:connect(source, destination)
    if destination.direction == "terminal" then
      assert(source.direction == "output", "instrument output requires a signal output")
      if destination.channel == "left" then self._left = source else self._right = source end
      return
    end
    if source.direction == "note" then
      self._uses_notes = true
      local field = "_note_" .. source.signal
      if not self[field] then
        local module = definition:module("note_" .. source.signal, "constant", { value = 0.0 })
        self[field] = module
      end
      definition:connect(self[field].out, destination)
      return
    end
    ordinary_connect(self, source, destination)
  end

  function instrument:control(label, target, range)
    assert(type(label) == "string" and label ~= "", "control label must be nonempty")
    assert(type(target) == "table" and target.direction == "parameter",
      "instrument control must target a writable parameter")
    assert(type(range) == "table", "instrument control requires a range")
    table.insert(self._controls, { label = label, target = target, range = range })
  end

  function instrument:_finalize()
    assert(self._left and self._right, "instrument must connect both left and right outputs")
    definition:output("left", self._left)
    definition:output("right", self._right)

    local exports = {}
    for index, control in ipairs(self._controls) do
      local exported = "control_" .. index
      definition:parameter(exported, { id = control.target.module }, control.target.parameter)
      exports[index] = exported
    end

    local root = meldritch.graph()
    local terminal = root:module("output", "stereo_output")
    if self._uses_notes then
      assert(self._note_pitch and self._note_gate, "note instruments must connect pitch and gate")
      definition:parameter("note_pitch", self._note_pitch, "value")
      definition:parameter("note_gate", self._note_gate, "value")
      local velocity = definition:module("note_velocity", "constant", { value = 1.0 })
      definition:parameter("note_velocity", velocity, "value")

      local voices = {}
      local bindings = {}
      for index = 1, 4 do
        local id = "voice_" .. index
        voices[index] = root:instance(id, definition)
        local pitch = "__" .. id .. "_pitch"
        local gate = "__" .. id .. "_gate"
        local velocity_id = "__" .. id .. "_velocity"
        root:control(pitch, voices[index], "note_pitch", { min = 8.0, max = 13000.0 })
        root.controls[#root.controls].hidden = true
        root:control(gate, voices[index], "note_gate", { min = 0.0, max = 1.0 })
        root.controls[#root.controls].hidden = true
        root:control(velocity_id, voices[index], "note_velocity", { min = 0.0, max = 1.0 })
        root.controls[#root.controls].hidden = true
        bindings[index] = { pitch = pitch, gate = gate, velocity = velocity_id }
      end
      local left = root:module("left_sum", "gain", { gain = 0.25 })
      local right = root:module("right_sum", "gain", { gain = 0.25 })
      for index = 1, 4 do
        root:connect(voices[index].left, left.input)
        root:connect(voices[index].right, right.input)
      end
      root:connect(left.out, terminal.left)
      root:connect(right.out, terminal.right)
      root:midi_poly(1, bindings)
      for index, control in ipairs(self._controls) do
        local targets = {}
        for voice = 2, 4 do
          targets[#targets + 1] = { module = voices[voice].id, parameter = exports[index] }
        end
        table.insert(root.controls, {
          id = control.label, module = voices[1].id, parameter = exports[index], targets = targets,
          min = control.range.min, max = control.range.max, scale = control.range.scale,
        })
      end
    else
      local instance = root:instance("instrument", definition)
      root:connect(instance.left, terminal.left)
      root:connect(instance.right, terminal.right)
      for index, control in ipairs(self._controls) do
        root:control(control.label, instance, exports[index], control.range)
      end
    end
    return root
  end
  return instrument
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
    let mut table: Table = lua.load(script).eval().map_err(|e| e.to_string())?;
    if let Some(finalize) = table
        .get::<Option<Function>>("_finalize")
        .map_err(|e| e.to_string())?
    {
        table = finalize.call(table.clone()).map_err(|e| e.to_string())?;
    }
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
                "button" => Widget::Button {
                    label: widget.get("label").map_err(|e| e.to_string())?,
                    control_id: widget.get("control_id").map_err(|e| e.to_string())?,
                    value: widget.get("value").map_err(|e| e.to_string())?,
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
            additional_targets: item
                .get::<Option<Table>>("targets")
                .map_err(|e| e.to_string())?
                .map(|targets| {
                    targets
                        .sequence_values::<Table>()
                        .map(|target| {
                            let target = target.map_err(|e| e.to_string())?;
                            Ok(ParameterId::new(
                                target.get::<String>("module").map_err(|e| e.to_string())?,
                                target
                                    .get::<String>("parameter")
                                    .map_err(|e| e.to_string())?,
                            ))
                        })
                        .collect::<Result<Vec<_>, String>>()
                })
                .transpose()?
                .unwrap_or_default(),
            min: item.get("min").map_err(|e| e.to_string())?,
            max: item.get("max").map_err(|e| e.to_string())?,
            scale: match item
                .get::<Option<String>>("scale")
                .map_err(|e| e.to_string())?
                .as_deref()
            {
                None | Some("linear") => ControlScale::Linear,
                Some("log") => ControlScale::Logarithmic,
                Some(other) => return Err(format!("unknown control scale '{other}'")),
            },
            hidden: item
                .get::<Option<bool>>("hidden")
                .map_err(|e| e.to_string())?
                .unwrap_or(false),
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
