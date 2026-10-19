use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use mlua::{AnyUserData, Function, Lua, Table, UserData, UserDataMethods, Value};

use crate::contract;
use crate::model::{
    Action, Connection, Control, ControlScale, Endpoint, ExternalInput, ExternalParameter,
    ExternalPort, Graph, MidiCc, MidiNotes, MidiPoly, MidiPolyVoice, Module, ModuleRuntime,
    ParameterId, Performance, PolyOverflow, Screen, Subpatch, SubpatchInstance, Widget,
};
use crate::sample::{PreparedSample, SampleBank};
use crate::sequencer::SequencerControl;

#[derive(Clone, Debug)]
struct LuaSampleBank(SampleBank);
#[derive(Clone, Debug)]
struct LuaSample(Arc<PreparedSample>);
#[derive(Clone, Debug)]
struct LuaSequencer(Arc<SequencerControl>);
#[derive(Clone, Debug)]
struct LuaSequencerTrack {
    sequencer: Arc<SequencerControl>,
    track: usize,
}
#[derive(Clone, Debug)]
struct LuaSequencerTempo(Arc<SequencerControl>);
impl UserData for LuaSample {}
impl UserData for LuaSequencerTrack {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("borrow_track", |_, this, ()| Ok(this.track));
    }
}
impl UserData for LuaSequencerTempo {}
impl UserData for LuaSampleBank {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("load", |lua, this, (name, variant): (String, usize)| {
            lua.create_userdata(LuaSample(
                this.0.load(&name, variant).map_err(mlua::Error::external)?,
            ))
        });
    }
}
impl UserData for LuaSequencer {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("track", |lua, this, name: String| {
            let track = this.0.add_track(name).map_err(mlua::Error::external)?;
            lua.create_userdata(LuaSequencerTrack {
                sequencer: Arc::clone(&this.0),
                track,
            })
        });
        methods.add_method("set_steps", |_, this, (track, values): (usize, Table)| {
            let values = values
                .sequence_values::<f32>()
                .collect::<mlua::Result<Vec<_>>>()?;
            this.0
                .set_steps(track, &values)
                .map_err(mlua::Error::external)
        });
        methods.add_method("start", |_, this, ()| {
            this.0.start();
            Ok(())
        });
        methods.add_method("stop", |_, this, ()| {
            this.0.stop();
            Ok(())
        });
        methods.add_method("restart", |_, this, ()| {
            this.0.restart();
            Ok(())
        });
    }
}

const API: &str = r#"
meldritch = { _subpatches = {} }
function meldritch.graph()
  local graph = { modules = {}, instances = {}, connections = {}, normalizations = {},
    controls = {}, midi_ccs = {}, note_bindings = {}, poly_bindings = {}, external_inputs = {}, external_outputs = {},
    external_parameters = {}, screens = {}, actions = {}, performance = nil, _devices = {} }
  function graph:devices(devices)
    assert(type(devices) == "table", "devices must be a table")
    assert(devices.audio_output == nil or
      (type(devices.audio_output) == "string" and devices.audio_output ~= ""),
      "audio_output device name must be a nonempty string")
    assert(devices.midi_input == nil or
      (type(devices.midi_input) == "string" and devices.midi_input ~= ""),
      "midi_input device name must be a nonempty string")
    assert(devices.apc_mini == nil or
      (type(devices.apc_mini) == "string" and devices.apc_mini ~= ""),
      "apc_mini device name must be a nonempty string")
    for name, _ in pairs(devices) do
      assert(name == "audio_output" or name == "midi_input" or name == "apc_mini", "unknown device role: " .. tostring(name))
    end
    self._devices = {
      audio_output = devices.audio_output,
      midi_input = devices.midi_input,
      apc_mini = devices.apc_mini,
    }
  end
  function graph:module(id, kind, values, runtime)
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
    table.insert(self.modules, { id = id, kind = kind, values = values or {}, runtime = runtime })
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
  function facade:devices(devices)
    scope:devices(devices)
  end
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
    if values.inputs == 8 then
      local handle = scope:module(self:_id("mixer"), "stereo_mixer8", { headroom = values.headroom or 0.25 })
      handle.input = {}
      for i = 1, 8 do handle.input[i] = { left = handle["left_" .. i], right = handle["right_" .. i], direction = "stereo_input" } end
      return handle
    end
    assert(values.inputs == 2, "mixer requires two mono or eight stereo inputs")
    local handle = scope:module(self:_id("mixer"), "mixer2")
    handle.input = { handle.input_1, handle.input_2 }
    return handle
  end
  function facade:sample_player(values)
    assert(type(values) == "table" and values.sample ~= nil, "sample_player requires a prepared sample")
    local handle = scope:module(self:_id("sample_player"), "sample_player", { gain = values.gain or 1.0 }, values.sample)
    handle.out = { left = handle.left, right = handle.right, direction = "stereo_output" }
    return handle
  end
  function facade:sequencer(values)
    assert(type(values) == "table", "sequencer requires settings")
    local native = meldritch._sequencer(self.name or prefix, values.bpm, values.steps, values.steps_per_beat)
    local tempo = scope:module(self:_id("sequencer_tempo"), "sequencer_tempo", { bpm = values.bpm }, meldritch._sequencer_tempo(native))
    local sequence = { _native = native, _tracks = {}, bpm = parameter(tempo, "bpm") }
    function sequence:track(name)
      assert(type(name) == "string" and name ~= "", "track name must be nonempty")
      local runtime = self._native:track(name)
      local track = runtime:borrow_track()
      self._tracks[name] = track
      return scope:module(facade:_id("sequence_track"), "sequencer_track", {}, runtime)
    end
    function sequence:steps(name, values)
      local track = assert(self._tracks[name], "unknown sequencer track: " .. tostring(name))
      self._native:set_steps(track, values)
    end
    local function command(self, kind)
      if facade._capturing_action then
        table.insert(facade._actions, { label = facade._capturing_action, native = self._native, action = kind })
      else self._native[kind](self._native) end
    end
    function sequence:start() command(self, "start") end
    function sequence:stop() command(self, "stop") end
    function sequence:restart() command(self, "restart") end
    return sequence
  end
  function facade:connect(source, destination)
    if source.direction == "stereo_output" and destination.direction == "stereo_input" then
      scope:connect(source.left, destination.left); scope:connect(source.right, destination.right); return
    end
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
    _actions = {},
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

  function instrument:action(label, callback)
    assert(type(label) == "string" and type(callback) == "function", "action requires a label and callback")
    self._capturing_action = label; callback(); self._capturing_action = nil
  end
  function instrument:performance(spec)
    assert(type(spec) == "table" and spec.layout == "step_grid" and spec.sequencer, "performance requires a step_grid sequencer")
    self._performance = { layout = spec.layout, native = spec.sequencer._native }
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
    root._devices = definition._devices
    root.actions = self._actions
    root.performance = self._performance
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
    let root = std::env::current_dir().map_err(|error| error.to_string())?;
    read_with_root(script, &root)
}

pub fn read_with_root(script: &str, project_root: &Path) -> Result<Graph, String> {
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
    let root = project_root.to_path_buf();
    let samples = lua
        .create_function(move |lua, spec: Table| {
            let source: String = spec.get("source")?;
            let bank: String = spec.get("bank")?;
            let cache = spec.get::<Option<bool>>("cache")?.unwrap_or(true);
            let bank = match source.as_str() {
                "strudel" => SampleBank::strudel(bank, cache, root.clone()),
                "local" => {
                    let manifest: String = spec.get("manifest")?;
                    SampleBank::local(bank, PathBuf::from(manifest), cache, root.clone())
                }
                _ => {
                    return Err(mlua::Error::external(format!(
                        "unsupported sample source '{source}'"
                    )));
                }
            }
            .map_err(mlua::Error::external)?;
            lua.create_userdata(LuaSampleBank(bank))
        })
        .map_err(|e| e.to_string())?;
    meldritch
        .set("samples", samples)
        .map_err(|e| e.to_string())?;
    let sequencer = lua
        .create_function(
            |lua, (name, bpm, steps, resolution): (String, f64, usize, u32)| {
                lua.create_userdata(LuaSequencer(
                    SequencerControl::new(name, bpm, steps, resolution)
                        .map_err(mlua::Error::external)?,
                ))
            },
        )
        .map_err(|e| e.to_string())?;
    meldritch
        .set("_sequencer", sequencer)
        .map_err(|e| e.to_string())?;
    let tempo = lua
        .create_function(|lua, native: AnyUserData| {
            let native = native.borrow::<LuaSequencer>()?;
            lua.create_userdata(LuaSequencerTempo(Arc::clone(&native.0)))
        })
        .map_err(|e| e.to_string())?;
    meldritch
        .set("_sequencer_tempo", tempo)
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
    let devices: Table = table
        .get("_devices")
        .map_err(|e| format!("graph devices: {e}"))?;
    graph.devices.audio_output = devices
        .get::<Option<String>>("audio_output")
        .map_err(|e| format!("audio_output device: {e}"))?;
    graph.devices.midi_input = devices
        .get::<Option<String>>("midi_input")
        .map_err(|e| format!("midi_input device: {e}"))?;
    graph.devices.apc_mini = devices
        .get::<Option<String>>("apc_mini")
        .map_err(|e| format!("apc_mini device: {e}"))?;
    if let Some(performance) = table
        .get::<Option<Table>>("performance")
        .map_err(|e| e.to_string())?
    {
        let native: AnyUserData = performance.get("native").map_err(|e| e.to_string())?;
        let native = native.borrow::<LuaSequencer>().map_err(|e| e.to_string())?;
        graph.performance = Some(Performance {
            layout: performance.get("layout").map_err(|e| e.to_string())?,
            sequencer: Arc::clone(&native.0),
        });
    }
    let actions: Table = table.get("actions").map_err(|e| e.to_string())?;
    for action in actions.sequence_values::<Table>() {
        let action = action.map_err(|e| e.to_string())?;
        let native: AnyUserData = action.get("native").map_err(|e| e.to_string())?;
        let native = native.borrow::<LuaSequencer>().map_err(|e| e.to_string())?;
        let kind: String = action.get("action").map_err(|e| e.to_string())?;
        graph.actions.push(Action {
            label: action.get("label").map_err(|e| e.to_string())?,
            sequencer: Arc::clone(&native.0),
            action: match kind.as_str() {
                "start" => crate::model::TransportAction::Start,
                "stop" => crate::model::TransportAction::Stop,
                "restart" => crate::model::TransportAction::Restart,
                _ => return Err(format!("unknown transport action '{kind}'")),
            },
        });
    }
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
        let runtime = match item
            .get::<Option<AnyUserData>>("runtime")
            .map_err(|e| e.to_string())?
        {
            None => ModuleRuntime::Contract,
            Some(value) if value.is::<LuaSample>() => ModuleRuntime::Sample(Arc::clone(
                &value.borrow::<LuaSample>().map_err(|e| e.to_string())?.0,
            )),
            Some(value) if value.is::<LuaSequencerTrack>() => {
                let value = value
                    .borrow::<LuaSequencerTrack>()
                    .map_err(|e| e.to_string())?;
                ModuleRuntime::SequencerTrack {
                    sequencer: Arc::clone(&value.sequencer),
                    track: value.track,
                }
            }
            Some(value) if value.is::<LuaSequencerTempo>() => {
                ModuleRuntime::SequencerTempo(Arc::clone(
                    &value
                        .borrow::<LuaSequencerTempo>()
                        .map_err(|e| e.to_string())?
                        .0,
                ))
            }
            Some(_) => return Err("unknown module runtime payload".into()),
        };
        graph.modules.push(Module {
            id: item.get("id").map_err(|e| e.to_string())?,
            kind: item.get("kind").map_err(|e| e.to_string())?,
            values: params,
            runtime,
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

#[cfg(test)]
mod tests {
    use super::{read, read_with_root};
    use crate::compiler::compile_at_rate;
    use std::{fs, io::Cursor};

    #[test]
    fn reads_expected_device_names_from_graph() {
        let graph = read(
            r#"
            local graph = meldritch.graph()
            graph:devices {
              audio_output = "Output Alpha",
              midi_input = "Input Beta",
            }
            return graph
            "#,
        )
        .unwrap();
        assert_eq!(graph.devices.audio_output.as_deref(), Some("Output Alpha"));
        assert_eq!(graph.devices.midi_input.as_deref(), Some("Input Beta"));
    }

    #[test]
    fn rejects_unknown_or_empty_device_preferences() {
        let unknown = read(
            r#"
            local graph = meldritch.graph()
            graph:devices { controller = "MicroLab" }
            return graph
            "#,
        )
        .unwrap_err();
        assert!(unknown.contains("unknown device role"));

        let empty = read(
            r#"
            local graph = meldritch.graph()
            graph:devices { midi_input = "" }
            return graph
            "#,
        )
        .unwrap_err();
        assert!(empty.contains("must be a nonempty string"));
    }

    #[test]
    fn instrument_finalization_keeps_device_preferences() {
        let source = include_str!("../../../examples/instruments/subtractive.lua");
        let graph = read(source).unwrap();
        assert_eq!(graph.devices.audio_output, None);
        assert_eq!(
            graph.devices.midi_input.as_deref(),
            Some("MicroLab mk3:MicroLab mk3")
        );
    }

    #[test]
    fn production_tr909_example_composes_and_renders_with_local_fixture() {
        let root = std::env::temp_dir().join(format!("meldritch-tr909-lua-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let mut wav = Cursor::new(Vec::new());
        {
            let mut writer = hound::WavWriter::new(
                &mut wav,
                hound::WavSpec {
                    channels: 2,
                    sample_rate: 800,
                    bits_per_sample: 16,
                    sample_format: hound::SampleFormat::Int,
                },
            )
            .unwrap();
            for frame in 0..64 {
                writer
                    .write_sample::<i16>(if frame == 0 { 24_000 } else { 4_000 })
                    .unwrap();
                writer
                    .write_sample::<i16>(if frame == 0 { -24_000 } else { -4_000 })
                    .unwrap();
            }
            writer.finalize().unwrap();
        }
        fs::write(root.join("fixture.wav"), wav.into_inner()).unwrap();
        fs::write(
            root.join("strudel.json"),
            r#"{
          "RolandTR909_bd":"fixture.wav", "RolandTR909_sd":"fixture.wav",
          "RolandTR909_cp":"fixture.wav", "RolandTR909_hh":"fixture.wav",
          "RolandTR909_oh":"fixture.wav", "RolandTR909_lt":"fixture.wav",
          "RolandTR909_ht":"fixture.wav", "RolandTR909_rd":"fixture.wav"
        }"#,
        )
        .unwrap();
        let source = include_str!("../../../examples/instruments/tr909.lua").replace(
            "source = \"strudel\",",
            "source = \"local\", manifest = \"strudel.json\",",
        );
        let graph = read_with_root(&source, &root).unwrap();
        assert_eq!(graph.devices.audio_output, None);
        assert!(graph.performance.is_some());
        assert_eq!(graph.actions.len(), 3);
        let mut machine = compile_at_rate(&graph, 800.0).unwrap();
        assert_eq!(
            machine
                .order()
                .iter()
                .filter(|id| id.contains("sample_player"))
                .count(),
            8
        );
        assert_eq!(machine.sequencer_controls()[0].track_names().len(), 8);
        let mut left = vec![0.0; 800];
        let mut right = vec![0.0; 800];
        machine.process_stereo(&mut left, &mut right);
        assert!(left.iter().chain(&right).all(|sample| sample.is_finite()));
        assert!(left.iter().any(|sample| sample.abs() > 0.001));
        assert!(right.iter().any(|sample| sample.abs() > 0.001));
    }
}
