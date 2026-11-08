-- Self-Reproducing Performance Playground
--
-- This file is both an executable Meldritch composition and its own editor
-- template. SAVE rewrites the state block into a complete successor program;
-- no loader, project wrapper, or separate editor implementation is involved.

-- BEGIN SELF STATE
local STATE = {
  generation = 0,
  editor = {variant = "compact", transforms = {"transpose", "rotate", "invert", "quantize", "interpolate"}},
  key = {root = 0, mode = "minor", degree = 1, entry_step = 3},
  arp = {octaves = 2, division = 1, gate = 0.72, seed = 1949},
  plugins = {serum_patch = ""},
  pulse = {kick_freq=.25,kick_noise=.04,snare_tone=.54,hat_filter=.62,perc_wave=.48},
  controls = {bpm = 118, master = 0.42, drums = 0.72, melody = 0.62, arp = 0.42,
    live = 0.28, duck_depth = 0.64, duck_release = 0.14, sidechain_route = 0, morph = 0.5},
  patterns = {
    {name = "Up", length = 16, arp_mode = "up",
      notes = {{id=1,step=1,pitch=48,velocity=112,duration=3},{id=2,step=5,pitch=51,velocity=96,duration=2},{id=3,step=9,pitch=55,velocity=108,duration=3},{id=4,step=13,pitch=58,velocity=98,duration=2}},
      chords = {{step=1,notes={48,51,55,58}},{step=9,notes={53,56,60}}}},
    {name = "Down", length = 24, arp_mode = "down",
      notes = {{id=1,step=1,pitch=53,velocity=106,duration=4},{id=2,step=7,pitch=56,velocity=94,duration=3},{id=3,step=13,pitch=60,velocity=112,duration=4},{id=4,step=19,pitch=63,velocity=101,duration=3}},
      chords = {{step=1,notes={53,56,60,63}},{step=13,notes={48,51,55,58}}}},
    {name = "Up Down", length = 16, arp_mode = "up_down",
      notes = {{id=1,step=1,pitch=48,velocity=108,duration=6},{id=2,step=9,pitch=55,velocity=103,duration=6}},
      chords = {{step=1,notes={48,51,55,58}}}},
    {name = "Played", length = 32, arp_mode = "played",
      notes = {{id=1,step=1,pitch=48,velocity=102,duration=5},{id=2,step=9,pitch=58,velocity=97,duration=5},{id=3,step=17,pitch=55,velocity=110,duration=5},{id=4,step=25,pitch=51,velocity=92,duration=5}},
      chords = {{step=1,notes={48,58,51,55}},{step=17,notes={53,63,56,60}}}},
    {name = "Random", length = 20, arp_mode = "random",
      notes = {{id=1,step=1,pitch=48,velocity=105,duration=3},{id=2,step=6,pitch=51,velocity=92,duration=2},{id=3,step=11,pitch=55,velocity=114,duration=3},{id=4,step=16,pitch=58,velocity=98,duration=2}},
      chords = {{step=1,notes={48,51,55,58}},{step=11,notes={46,50,53,56}}}},
  },
  drums = {
    kick = {{id=101,step=1,velocity=127},{id=102,step=5,velocity=112},{id=103,step=9,velocity=124},{id=104,step=13,velocity=116}},
    snare = {{id=201,step=5,velocity=118},{id=202,step=13,velocity=124}},
    hat = {{id=301,step=1,velocity=74},{id=302,step=3,velocity=53},{id=303,step=5,velocity=82},{id=304,step=7,velocity=57},{id=305,step=9,velocity=77},{id=306,step=11,velocity=55},{id=307,step=13,velocity=88},{id=308,step=15,velocity=61}},
    percussion = {{id=401,step=4,velocity=67},{id=402,step=12,velocity=78}},
  },
}
-- END SELF STATE

local ACCEPTANCE = os.getenv("MELDRITCH_PLAYGROUND_ACCEPTANCE") == "1"
if ACCEPTANCE and STATE.generation==0 then
  STATE.patterns[1].notes[1].pitch=50
  STATE.controls.bpm=127;STATE.editor.variant="expanded"
  STATE.drums.kick[#STATE.drums.kick+1]={id=105,step=15,velocity=109}
end

-- The sandboxed composition VM intentionally omits Lua's debug library. Each
-- successor therefore carries its own source path as ordinary source-owned
-- state. Generation zero has a repository-relative conventional entry point;
-- relocatable launchers may override it explicitly.
local CURRENT_SOURCE = os.getenv("MELDRITCH_PLAYGROUND_SOURCE") or STATE.source
  or "crates/meldritch2/examples/81-self-reproducing-playground.lua"

local function sorted_keys(value)
  local keys = {}
  for key in pairs(value) do keys[#keys + 1] = key end
  table.sort(keys, function(a, b)
    if type(a) == type(b) then return a < b end
    return type(a) < type(b)
  end)
  return keys
end

local function is_array(value)
  local count, maximum = 0, 0
  for key in pairs(value) do
    if type(key) ~= "number" or key < 1 or key % 1 ~= 0 then return false end
    count, maximum = count + 1, math.max(maximum, key)
  end
  return count == maximum
end

local function serialize(value, depth)
  depth = depth or 0
  local kind = type(value)
  if kind == "string" then return string.format("%q", value) end
  if kind == "number" then return string.format("%.17g", value) end
  if kind == "boolean" or kind == "nil" then return tostring(value) end
  assert(kind == "table", "self state contains unsupported " .. kind)
  local indent, child = string.rep("  ", depth), string.rep("  ", depth + 1)
  local fields = {}
  if is_array(value) then
    for index = 1, #value do fields[#fields + 1] = child .. serialize(value[index], depth + 1) end
  else
    for _, key in ipairs(sorted_keys(value)) do
      local rendered = type(key) == "string" and key:match("^[%a_][%w_]*$") and key
        or "[" .. serialize(key, depth + 1) .. "]"
      fields[#fields + 1] = child .. rendered .. " = " .. serialize(value[key], depth + 1)
    end
  end
  if #fields == 0 then return "{}" end
  return "{\n" .. table.concat(fields, ",\n") .. "\n" .. indent .. "}"
end

-- Source-bearing objects can opt into this protocol without turning the
-- reproducer into one giant construction function. Plain configuration
-- objects use the deterministic table representation above.
local function lua_script_representation(object)
  if type(object)=="table" and type(object.lua_script_representation)=="function" then
    return object:lua_script_representation()
  end
  return serialize(object)
end

local function read_all(path)
  local file = assert(io.open(path, "rb"))
  local value = assert(file:read("*a")); assert(file:close())
  return value
end

local function write_atomic(path, value)
  local temporary = path .. ".tmp"
  local file = assert(io.open(temporary, "wb"))
  assert(file:write(value)); assert(file:flush()); assert(file:close())
  assert(os.rename(temporary, path))
end

local LAST_SUCCESSOR
local performance
local function reproduce()
  local path = CURRENT_SOURCE
  local source = read_all(path)
  local begin_marker, end_marker = "-- BEGIN SELF STATE\n", "-- END SELF STATE"
  local first, after_first = assert(source:find(begin_marker, 1, true))
  local last = assert(source:find(end_marker, after_first + 1, true))
  STATE.generation = STATE.generation + 1
  local directory = os.getenv("MELDRITCH_PLAYGROUND_OUTPUT_DIR")
  local basename = path:match("([^/]+)$"):gsub("%.g%d+%.lua$", ".lua")
  local stem = assert(basename:match("^(.*)%.lua$"))
  if not directory then directory = path:match("^(.*)/[^/]+$") or "." end
  local output = directory .. "/" .. stem .. ".g" .. STATE.generation .. ".lua"
  STATE.source = output
  local successor = source:sub(1, after_first)
    .. "local STATE = " .. lua_script_representation(STATE) .. "\n"
    .. source:sub(last)
  write_atomic(output, successor)
  LAST_SUCCESSOR = output
  return output
end

local ROOT_NAMES = {"C","C#","D","Eb","E","F","F#","G","Ab","A","Bb","B"}
local MODE_NAMES = {"major","minor","dorian","mixolydian","pentatonic"}
local INTERVALS = {
  major={0,2,4,5,7,9,11}, minor={0,2,3,5,7,8,10}, dorian={0,2,3,5,7,9,10},
  mixolydian={0,2,4,5,7,9,10}, pentatonic={0,2,4,7,9},
}
local APC_SCALES = {major={0,2,4,7,9},minor={0,3,5,7,10},dorian={0,2,3,7,9},
  mixolydian={0,2,4,7,10},pentatonic={0,2,4,7,9}}
local serum_bank

local function sync_controls()
  if not performance then return end
  local names = {"bpm","master","drums","melody","arp","live","duck_depth","duck_release","sidechain_route","morph",
    "key_root","scale_mode","root_degree"}
  for _, name in ipairs(names) do
    local ok, value = pcall(function() return performance:value(name) end)
    if ok and type(value) == "number" then STATE.controls[name] = value end
  end
  for name in pairs(STATE.pulse) do
    local ok,value=pcall(function() return performance:value(name) end)
    if ok and type(value)=="number" then STATE.pulse[name]=value end
  end
  if serum_bank then
    local ok,value=pcall(function() return performance:value("serum_preset") end)
    if ok and type(value)=="number" then
      STATE.plugins.serum_patch=serum_bank.patch_ids[math.floor(value)+1] or STATE.plugins.serum_patch
    end
  end
  STATE.key.root = math.floor(STATE.controls.key_root or STATE.key.root)
  STATE.key.degree = math.floor(STATE.controls.root_degree or (STATE.key.degree-1)) + 1
  STATE.key.mode = MODE_NAMES[math.floor(STATE.controls.scale_mode or 1) + 1] or STATE.key.mode
end

local function scale_root()
  sync_controls()
  local intervals = INTERVALS[STATE.key.mode]
  local degree = intervals[(STATE.key.degree - 1) % #intervals + 1]
  return (STATE.key.root + degree) % 12, intervals
end

local function snap_pitch(pitch)
  local root, intervals = scale_root()
  local best, distance = pitch, 128
  for candidate = 0, 127 do
    local relative = (candidate - root) % 12
    for _, interval in ipairs(intervals) do
      if relative == interval and math.abs(candidate - pitch) < distance then
        best, distance = candidate, math.abs(candidate - pitch)
      end
    end
  end
  return best
end

local instrument = meldritch.instrument("Self-Reproducing Lua Playground · generation " .. STATE.generation)
instrument:voice_mode("mono")
instrument:devices {
  midi_input=os.getenv("MICROLAB_MIDI") or "MicroLab mk3:MicroLab mk3",
  midi_controls={os.getenv("MICROLAB_MIDI") or "MicroLab mk3:MicroLab mk3"},
  apc_mini=os.getenv("APC_MINI_MIDI") or "APC mini mk2:APC mini mk2 Control",
}

local sequence = instrument:sequencer {bpm=STATE.controls.bpm,steps=32,steps_per_beat=4}
local melody_track = sequence:note_track("Melody", {voices=4})
local arp_track = sequence:note_track("Arpeggiator", {voices=2})
local kit = sequence:drum_kit {name="PULSE Playground",voices={
  {id="kick",label="Kick",note=36,channel=10},{id="snare",label="Snare",note=38,channel=10},
  {id="hat",label="Hat",note=42,channel=10},{id="percussion",label="Percussion",note=46,channel=10},
}}

local patterns, patterns_by_name = {}, {}
local function arp_notes(definition)
  local arp = meldritch.arp.new {mode=definition.arp_mode,octaves=STATE.arp.octaves,
    clock_division=STATE.arp.division,gate=STATE.arp.gate,seed=STATE.arp.seed}
  local events = arp:events(definition.length,
    meldritch.arp.from_pattern(definition.chords, definition.length), {generation=STATE.generation})
  local result = {}
  for index, event in ipairs(events) do result[index] = {id=index,step=event.step,pitch=event.note,
    velocity=math.max(1,math.floor(event.velocity*127+.5)),duration=math.max(1,math.floor(STATE.arp.gate*STATE.arp.division+.5))} end
  return result
end

local function write_pattern(index)
  local definition, pattern = STATE.patterns[index], patterns[index]
  pattern:notes(melody_track, definition.notes, definition.length)
  pattern:notes(arp_track, arp_notes(definition), definition.length)
  pattern:drums(kit, STATE.drums, 16)
end

for index, definition in ipairs(STATE.patterns) do
  patterns[index] = sequence:pattern(definition.name)
  patterns_by_name[definition.name] = patterns[index]
  write_pattern(index)
end
sequence:switch_pattern(STATE.patterns[1].name)

local function active_definition()
  local index = sequence:state().viewed_pattern
  return STATE.patterns[index], index
end

local function transform_notes(operation)
  sync_controls()
  local definition, index = active_definition()
  if operation == "transpose" then
    for _, note in ipairs(definition.notes) do note.pitch = snap_pitch(note.pitch + 1) end
  elseif operation == "rotate" then
    for _, note in ipairs(definition.notes) do note.step = note.step % definition.length + 1 end
  elseif operation == "invert" then
    local pivot = 60
    for _, note in ipairs(definition.notes) do note.pitch = snap_pitch(pivot * 2 - note.pitch) end
  elseif operation == "quantize" then
    for _, note in ipairs(definition.notes) do
      note.step = math.max(1,math.min(definition.length,math.floor((note.step-1)/2+.5)*2+1))
      note.pitch = snap_pitch(note.pitch)
    end
  end
  write_pattern(index)
end

local function interpolate()
  sync_controls()
  local left, right, target = STATE.patterns[1], STATE.patterns[2], STATE.patterns[3]
  local amount = STATE.controls.morph
  target.notes = {}
  local count = math.max(#left.notes,#right.notes)
  for index=1,count do
    local a, b = left.notes[(index-1)%#left.notes+1], right.notes[(index-1)%#right.notes+1]
    target.notes[index] = {id=index,step=math.floor(a.step+(b.step-a.step)*amount+.5),
      pitch=snap_pitch(math.floor(a.pitch+(b.pitch-a.pitch)*amount+.5)),
      velocity=math.floor(a.velocity+(b.velocity-a.velocity)*amount+.5),
      duration=math.max(1,math.floor(a.duration+(b.duration-a.duration)*amount+.5))}
  end
  write_pattern(3); sequence:switch_pattern(target.name,"beat")
end

local function add_scale_note(degree)
  sync_controls()
  local definition,index=active_definition()
  local root,intervals=scale_root()
  local pitch=48+root+intervals[(degree-1)%#intervals+1]
  local maximum=0;for _,note in ipairs(definition.notes) do maximum=math.max(maximum,note.id) end
  definition.notes[#definition.notes+1]={id=maximum+1,step=STATE.key.entry_step,pitch=pitch,velocity=104,duration=2}
  STATE.key.entry_step=STATE.key.entry_step%definition.length+1;write_pattern(index)
end

-- Try user-local plugins, but retain a fully executable native playground for
-- hosts without them. No plugin state or proprietary preset is stored here.
local native_only = os.getenv("MELDRITCH_PLAYGROUND_NATIVE_ONLY") == "1"
local home = os.getenv("HOME") or ""
local plugin_paths = {os.getenv("PULSE_VST3_PATH") or (home .. "/.vst3"),"/usr/lib/vst3"}
local PULSE, SERUM = "ABCDEF019182FAEB506C7365506C6473", "56534558667350736572756D20320000"
local pulse, pulse_parameters
if not native_only then
  local ok, value = pcall(function()
    local plugin=instrument:vst3 {id="pulse",plugin=PULSE,paths=plugin_paths,midi_channels={10,10,10,10},state_file=os.getenv("PULSE_STATE")}
    plugin:connect_drum_kit(kit);return plugin
  end)
  if ok then
    pulse=value;pulse_parameters={};for _,info in ipairs(pulse:parameter_info()) do pulse_parameters[info.name]=info end
  end
end

local drum_output
local pulse_controls = {}
if pulse then
  drum_output=pulse.out
  for _,spec in ipairs({{"kick_freq","Kick frequency","KICK Synth Freq"},{"kick_noise","Kick noise","KICK Noise Level"},
    {"snare_tone","Snare tone","SNARE Noise Tone"},{"hat_filter","Hat filter","HAT Noise Filter"},
    {"perc_wave","Perc wave","PERC Synth Wave"}}) do
    local info=pulse_parameters[spec[3]]
    if info and info.automatable and not info.read_only then
      instrument:control {id=spec[1],label=spec[2],target=pulse:parameter(info.id),range={0,1}}
      pulse:set_parameter(info.id,STATE.pulse[spec[1]])
      pulse_controls[#pulse_controls+1]=spec
    end
  end
else
  local bus=instrument:mixer {inputs=8,headroom=.32}
  local models={"bass_drum","snare_drum","hihat","modal"}
  for index,voice in ipairs(kit.voices) do
    local synth=instrument:plaits {model=models[index],harmonics=.35,timbre=.48,morph=.36}
    local env=instrument:adsr {attack=.001,decay=.10+index*.025,sustain=0,release=.04}
    local amp=instrument:vca {};instrument:connect(voice.track.gate,synth.gate);instrument:connect(voice.track.gate,env.gate)
    instrument:connect(synth.main,amp.input);instrument:connect(env.out,amp.cv)
    instrument:connect(amp.out,bus.input[index].left);instrument:connect(amp.out,bus.input[index].right)
  end
  drum_output={left=bus.left,right=bus.right,direction="stereo_output"}
end

local serum_bank_path
if not native_only then
  serum_bank_path=os.getenv("MELDRITCH_SERUM_BANK")
  if serum_bank_path then
    local ok,value=pcall(function()
      local bank=instrument:vst3 {id="serum_library",plugin=SERUM,paths=plugin_paths,patch_bank=serum_bank_path,
        initial_patch=os.getenv("MELDRITCH_SERUM_PATCH") or (STATE.plugins.serum_patch~="" and STATE.plugins.serum_patch or nil)}
      bank:connect_track(melody_track,sequence);return bank
    end)
    if ok then serum_bank=value end
  end
end
if serum_bank then
  instrument:control {id="serum_preset",label="Serum preset",target=serum_bank.preset,range={0,#serum_bank.preset_names-1}}
end

local melody_output
if serum_bank then melody_output=serum_bank.out
else
  local synth=instrument:plaits {model="virtual_analog_vcf",harmonics=.43,timbre=.52,morph=.28}
  local env=instrument:adsr {attack=.008,decay=.22,sustain=.52,release=.48};local amp=instrument:vca {}
  instrument:connect(melody_track.voices[1].pitch,synth.pitch);instrument:connect(melody_track.voices[1].gate,synth.gate)
  instrument:connect(melody_track.voices[1].velocity,synth.velocity);instrument:connect(melody_track.voices[1].gate,env.gate)
  instrument:connect(synth.main,amp.input);instrument:connect(env.out,amp.cv)
  melody_output={left=amp.out,right=amp.out,direction="stereo_output"}
end

local arp_synth=instrument:plaits {model="harmonic",harmonics=.63,timbre=.58,morph=.31}
local arp_env=instrument:adsr {attack=.002,decay=.09,sustain=.34,release=.12};local arp_amp=instrument:vca {}
instrument:connect(arp_track.voices[1].pitch,arp_synth.pitch);instrument:connect(arp_track.voices[1].gate,arp_synth.gate)
instrument:connect(arp_track.voices[1].velocity,arp_synth.velocity);instrument:connect(arp_track.voices[1].gate,arp_env.gate)
instrument:connect(arp_synth.main,arp_amp.input);instrument:connect(arp_env.out,arp_amp.cv)

-- MicroLab uses the existing live-note path for audition. Retained note-entry
-- capture is intentionally not claimed because the host exposes no Lua chord
-- or note-entry snapshot.
local live=instrument:plaits {model="fm",harmonics=.52,timbre=.44,morph=.37}
local live_env=instrument:adsr {attack=.006,decay=.18,sustain=.55,release=.58};local live_amp=instrument:vca {}
instrument:connect(instrument.note.pitch,live.pitch);instrument:connect(instrument.note.gate,live.gate)
instrument:connect(instrument.note.velocity,live.velocity);instrument:connect(instrument.note.gate,live_env.gate)
instrument:connect(live.main,live_amp.input);instrument:connect(live_env.out,live_amp.cv)

local route=instrument:switch {threshold=.5};local route_control=instrument:constant {value=STATE.controls.sidechain_route}
instrument:connect(route_control.out,route.select);instrument:connect(kit.voices[1].track.gate,route.left)
instrument:connect(kit.voices[4].track.gate,route.right)
local duck_l=instrument:ducker {threshold=.02,depth=STATE.controls.duck_depth,attack=.0015,release=STATE.controls.duck_release}
local duck_r=instrument:ducker {threshold=.02,depth=STATE.controls.duck_depth,attack=.0015,release=STATE.controls.duck_release}
instrument:connect(melody_output.left,duck_l.input);instrument:connect(melody_output.right,duck_r.input)
instrument:connect(route.out,duck_l.sidechain);instrument:connect(route.out,duck_r.sidechain)

local mix=instrument:mixer {inputs=8,headroom=STATE.controls.master}
local function stereo_level(id,label,source,value,slot)
  local left,right=instrument:gain {gain=value},instrument:gain {gain=value}
  instrument:connect(source.left,left.input);instrument:connect(source.right,right.input)
  instrument:connect(left.out,mix.input[slot].left);instrument:connect(right.out,mix.input[slot].right)
  instrument:control {id=id,label=label,target=left.gain,targets={right.gain},range={0,1.25}}
end
stereo_level("drums","Drum level",drum_output,STATE.controls.drums,1)
stereo_level("melody","Melody level",{left=duck_l.out,right=duck_r.out},STATE.controls.melody,2)
stereo_level("arp","Arpeggiator level",{left=arp_amp.out,right=arp_amp.out},STATE.controls.arp,3)
stereo_level("live","MicroLab audition",{left=live_amp.out,right=live_amp.out},STATE.controls.live,4)
instrument:control {id="master",label="Master",target=mix.headroom,range={.04,.8}}
instrument:control {id="bpm",label="Tempo",target=sequence.bpm,range={60,180}}
instrument:control {id="duck_depth",label="Sidechain depth",target=duck_l.depth,targets={duck_r.depth},range={0,1}}
instrument:control {id="duck_release",label="Sidechain release",target=duck_l.release,targets={duck_r.release},range={.01,1.5},curve="log"}
instrument:control {id="sidechain_route",label="Sidechain source",target=route_control.value,range={0,1}}
local morph=instrument:constant {value=STATE.controls.morph};instrument:control {id="morph",label="Pattern morph",target=morph.value,range={0,1}}
local key_root=instrument:constant {value=STATE.key.root};instrument:control {id="key_root_target",label="Key",target=key_root.value,range={0,11}}
local scale_mode=instrument:constant {value=1};instrument:control {id="scale_mode_target",label="Mode",target=scale_mode.value,range={0,#MODE_NAMES-1}}
local root_degree=instrument:constant {value=STATE.key.degree-1};instrument:control {id="root_degree_target",label="Root degree",target=root_degree.value,range={0,6}}
instrument:connect(mix.left,instrument.output.left);instrument:connect(mix.right,instrument.output.right)

local server=meldritch.server {host="127.0.0.1",port=tonumber(os.getenv("MELDRITCH_PLAYGROUND_PORT") or "8081"),anonymous_role="leader"}
server:websocket("/performance",{})
performance=server:performance {name="Self-Reproducing Lua Playground · generation "..STATE.generation}
performance:role("leader",{controls="*"});performance:role("observer",{controls={}})
local editor=performance:plane {id="editor",label="Lua-Owned Musical Editor",roles={"leader"},read_only_roles={"observer"}}
editor:piano_roll {id="melody_roll",label="Melody (16-step browser viewport)",track=melody_track,steps=16,pitch_min=36,pitch_max=84}
editor:pattern_selector {id="patterns",label="Variable-length patterns",sequencer=sequence,boundary="bar"}
editor:preset_selector {id="key_root",label="Key",target="key_root_target",presets=ROOT_NAMES}
editor:preset_selector {id="scale_mode",label="Scale / mode",target="scale_mode_target",presets=MODE_NAMES}
editor:preset_selector {id="root_degree",label="Root degree",target="root_degree_target",presets={"1","2","3","4","5","6","7"}}
editor:knob {id="morph",label="A/B morph",target="morph",min=0,max=1}
editor:trigger {id="add_root",label="ADD SCALE ROOT",triggered=function() add_scale_note(1) end}
editor:trigger {id="add_third",label="ADD SCALE THIRD",triggered=function() add_scale_note(3) end}
editor:trigger {id="transpose",label="SCALE-LOCK +1",triggered=function() transform_notes("transpose") end}
editor:trigger {id="rotate",label="ROTATE +1",triggered=function() transform_notes("rotate") end}
editor:trigger {id="invert",label="INVERT",triggered=function() transform_notes("invert") end}
editor:trigger {id="quantize",label="QUANTIZE",triggered=function() transform_notes("quantize") end}
editor:trigger {id="interpolate",label="INTERPOLATE A/B",triggered=interpolate}
editor:trigger {id="editor_variant",label="TOGGLE SUCCESSOR LAYOUT",triggered=function()
  STATE.editor.variant=STATE.editor.variant=="compact" and "expanded" or "compact" end}
editor:trigger {id="save_successor",label="SAVE SUCCESSOR LUA",triggered=function() sync_controls();reproduce() end}

local rhythm=performance:plane {id="rhythm",label="PULSE Pattern and Sound",roles={"leader"},read_only_roles={"observer"}}
rhythm:drum_grid {id="pulse_grid",label="PULSE drum pattern",kit=kit,steps=16}
for _,spec in ipairs(pulse_controls) do rhythm:knob {id=spec[1],label=spec[2],target=spec[1],min=0,max=1} end

local sound=performance:plane {id="sound",label=serum_bank and "Extracted Serum Library" or "Native fallback",roles={"leader"},read_only_roles={"observer"}}
if serum_bank then sound:preset_selector {id="serum_preset",label="Serum preset",target="serum_preset",presets=serum_bank.preset_names}
else sound:trigger {id="serum_gap",label="SET MELDRITCH_SERUM_BANK FOR SERUM",triggered=function() end} end
for _,spec in ipairs({{"bpm","Tempo",60,180},{"master","Master",.04,.8},{"drums","Drums",0,1.25},
  {"melody","Melody",0,1.25},{"arp","Arp",0,1.25},{"live","MicroLab",0,1.25},
  {"duck_depth","Duck depth",0,1},{"duck_release","Duck release",.01,1.5}}) do
  sound:fader {id=spec[1],label=spec[2],target=spec[1],min=spec[3],max=spec[4],scale=spec[1]=="duck_release" and "log" or "linear"}
end
sound:preset_selector {id="sidechain_route",label="Sidechain source",target="sidechain_route",presets={"Kick","Percussion"}}
if STATE.editor.variant=="expanded" then
  sound:trigger {id="expanded_marker",label="EXPANDED EDITOR GENERATION",triggered=function() end}
end

server:get("/api/playground",function() return {generation=STATE.generation,successor=LAST_SUCCESSOR,
  editor=STATE.editor,key=STATE.key,patterns=STATE.patterns,limitations={live_midi_chord_arp="unsupported: no retained Lua MIDI chord snapshot",
  browser_pattern_tail="unsupported: piano-roll projection is fixed at 16 steps"}} end)
server:post("/api/playground/save",function() sync_controls();return {successor=reproduce()},201 end)

instrument:performance {layout="step_grid",sequencer=sequence,
  keys={Space="transport",s="start",x="stop",r="restart",Up="track:up",Down="track:down",Left="step:previous",Right="step:next",
    ["["]="bank:previous",["]"]="bank:next",n="pattern:next",N="pattern:previous",[","]="rotate:left",["."]="rotate:right",
    ["1"]="toggle:1",["2"]="toggle:2",["3"]="toggle:3",["4"]="toggle:4",["5"]="toggle:5",["6"]="toggle:6",["7"]="toggle:7",["8"]="toggle:8"},
  focus={controls={"bpm","master","drums","melody","arp","duck_depth","duck_release","morph"},
    midi={{channel=1,cc=74},{channel=1,cc=71},{channel=1,cc=76},{channel=1,cc=77}}},
  apc={pages={"steps","performance"},performance_rows={"pattern","mute","solo","track","restart","","",""},
    piano_roll={drums={"PULSE Playground kick","PULSE Playground snare","PULSE Playground hat"},track="Melody Voice 1",root=48,scale=APC_SCALES[STATE.key.mode]},
    track_buttons={"pattern:previous","pattern:next","bank:previous","bank:next","key:down","key:up","length:decrease","length:increase"},
    faders={"bpm","drums","melody","arp","duck_depth","duck_release","morph","live"},
    scene_buttons={"transport","restart","pattern:0","pattern:1","pattern:2","pattern:3","pattern:4",""},master="master"}}

instrument:control_surface {id="arp_deck",device="stream_deck_xl",initial_page="arp",pages={{id="arp",buttons={
  {id="up",index=0,label="ARP UP",press={command="pattern:0"}},{id="down",index=1,label="ARP DOWN",press={command="pattern:1"}},
  {id="up_down",index=2,label="UP / DOWN",press={command="pattern:2"}},{id="played",index=3,label="PLAYED",press={command="pattern:3"}},
  {id="random",index=4,label="RANDOM",press={command="pattern:4"}},{id="start",index=8,label="START",press={command="start"}},
  {id="stop",index=9,label="STOP",press={command="stop"}},{id="restart",index=10,label="RESTART",press={command="restart"}},
  {id="arp_up",index=16,label="ARP +",press={control="arp",value=.72}},{id="arp_down",index=17,label="ARP -",press={control="arp",value=.25}},
  {id="duck_on",index=24,label="DUCK ON",press={control="duck_depth",value=.68}},{id="duck_off",index=25,label="DUCK OFF",press={control="duck_depth",value=0}},
}}}}

local screen=instrument:screen("playground","Self-Reproducing Lua Playground")
screen:performance_status();screen:pattern_grid();screen:performance_controls()
screen:label("Browser: http://127.0.0.1:"..tostring(os.getenv("MELDRITCH_PLAYGROUND_PORT") or 8081).." · SAVE writes generation "..(STATE.generation+1))
screen:label("Live MIDI chord arpeggiation is unavailable; all five modes use authored chord patterns.")
sequence:start()

-- Deterministic non-interactive acceptance driver used by the focused shell
-- regression. It exercises the same state and reproduction functions as the
-- browser triggers, while graph inspection avoids opening hardware.
if ACCEPTANCE then reproduce() end

return instrument
