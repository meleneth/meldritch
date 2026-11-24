-- 85: Serum bass and lead + PULSE drums / composed harmonic performance
--
-- Hardware: APC mini mk2 faders 1-8 set step values (soft takeover).
-- Track buttons (Volume, Pan, Send, Device, Up, Down, Left, Right) toggle steps 1-8.
-- TUI: 1-8 toggle, left/right select, up/down adjust, Home/End min/max.
local graph = meldritch.graph()
graph:devices { apc_mini = os.getenv("APC_MINI_MIDI") or "APC mini mk2 Control", midi_input = os.getenv("MICROLAB_MIDI") or "MicroLab mk3" }

local notes = graph:value_sequencer {
  id = "notes",
  -- Values select the centers of seven equal scale-degree regions.
  values = {0.5/7, 2.5/7, 4.5/7, 6.5/7, 4.5/7, 2.5/7, 1.5/7, 3.5/7},
}
notes:controls("notes")
graph:apc_value_sequencer(notes)

local tui = graph:screen("value_sequence", "SERUM + PULSE / HARMONIC PERFORMANCE")
tui:label("Seven scale degrees per fader; track buttons enable steps")
tui:label("Adapters: keyboard Hz -> capture_root (Note128)")
tui:label("Value -> scale_degree_to_note128 (major, I/IV/V) -> note128_to_hz")
tui:value_sequencer(notes)
-- The dedicated TUI widget owns the rendering of the normal length/loop controls.

-- Musical meaning is established outside the generic ValueSequencer.
local keyboard_pitch = graph:module("keyboard_pitch", "constant", {value = 261.625565})
local keyboard_gate = graph:module("keyboard_gate", "constant", {value = 0})
local arm = graph:module("root_arm", "constant", {value = 0})
local shift = graph:module("degree_shift", "constant", {value = 0})
graph:control("keyboard_pitch", keyboard_pitch, "value", {min = 1, max = 20000})
graph:control("keyboard_gate", keyboard_gate, "value", {min = 0, max = 1})
graph:control("root_arm", arm, "value", {min = 0, max = 1})
graph:control("degree_shift", shift, "value", {min = 0, max = 4})
graph:midi_notes(1, "keyboard_pitch", "keyboard_gate")

local root = graph:module("root", "capture_root", {initial_midi = 60})
graph:connect(graph:as_frequency_hz("keyboard_hz", keyboard_pitch.out), root.pitch)
graph:connect(keyboard_gate.out, root.gate)
graph:connect(arm.out, root.arm)

local degree = graph:module("scale", "scale_degree_to_note128")
graph:connect(notes.value, degree.input)
graph:connect(root.out, degree.root)
graph:connect(shift.out, degree.transpose)
local pitch = graph:note128_to_hz("pitch", degree.out)

graph:control_surface {
  id = "harmony", device = "stream_deck_xl", initial_page = "harmony",
  pages = {{id = "harmony", buttons = {
    {id = "capture", index = 0, label = "HOLD ROOT",
      press = {control = "root_arm", value = 1},
      release = {control = "root_arm", value = 0}},
    {id = "tonic", index = 1, label = "I / ROOT",
      press = {control = "degree_shift", value = 0}},
    {id = "fourth", index = 2, label = "IV / 4TH",
      press = {control = "degree_shift", value = 3}},
    {id = "fifth", index = 3, label = "V / 5TH",
      press = {control = "degree_shift", value = 4}},
    {id = "lead_mute", index = 8, label = "MUTE LEAD", mode = "toggle", press = {control = "lead_on", value = 0}, release = {control = "lead_on", value = 1}},
    {id = "bass_mute", index = 9, label = "MUTE BASS", mode = "toggle", press = {control = "bass_on", value = 0}, release = {control = "bass_on", value = 1}},
    {id = "drum_mute", index = 10, label = "MUTE DRUM", mode = "toggle", press = {control = "drums_on", value = 0}, release = {control = "drums_on", value = 1}},
    {id = "kick_mute", index = 16, label = "MUTE KICK", mode = "toggle", press = {control = "kick_on", value = 0}, release = {control = "kick_on", value = 1}},
    {id = "snare_mute", index = 17, label = "MUTE SNARE", mode = "toggle", press = {control = "snare_on", value = 0}, release = {control = "snare_on", value = 1}},
    {id = "hat_mute", index = 18, label = "MUTE HAT", mode = "toggle", press = {control = "hat_on", value = 0}, release = {control = "hat_on", value = 1}},
    {id = "perc_mute", index = 19, label = "MUTE PERC", mode = "toggle", press = {control = "perc_on", value = 0}, release = {control = "perc_on", value = 1}},
  }}}
}
tui:control("Diatonic shift (0=I, 3=IV, 4=V)", "degree_shift")
tui:control("Root capture armed", "root_arm")

-- Restore actual captured Serum states, never silently substitute Init.
local serum_id = "56534558667350736572756D20320000"
local catalog = os.getenv("MELDRITCH_SERUM_CATALOG") or
  (assert(os.getenv("HOME")) .. "/.local/share/meldritch2/patches/serum2/factory")
local patches = meldritch._vst3_patch_bank {plugin = serum_id, patch_bank = catalog}
local function serum(id, desired)
  for _, patch in ipairs(patches) do
    if patch.id == desired or patch.name == desired then
      local runtime = meldritch._vst3 {plugin = serum_id, state_file = patch.file}
      tui:label(id .. ": " .. patch.name)
      return graph:module(id, "vst3", {}, runtime)
    end
  end
  error("Missing captured Serum patch '" .. desired .. "' in " .. catalog)
end
local lead = serum("lead", os.getenv("MELDRITCH_SERUM_LEAD") or "LD - Classic Techno Lead")
local bass = serum("bass", os.getenv("MELDRITCH_SERUM_BASS") or "BA - Gnarly Reese")
local pulse = graph:module("drums", "vst3", {}, meldritch._vst3 {
  plugin = "ABCDEF019182FAEB506C7365506C6473",
  midi_channels = {10,10,10,10},
  state_file = os.getenv("PULSE_STATE"),
})

local function constant(id, value)
  return graph:module(id, "constant", {value = value}).out
end
local function product(id, a, b)
  local m = graph:module(id, "multiply")
  graph:connect(a, m.left)
  graph:connect(b, m.right)
  return m.out
end
local function control(id, value, min, max)
  local m = graph:module(id, "constant", {value = value})
  graph:control(id, m, "value", {min = min, max = max})
  tui:control(id, id)
  return m.out
end

-- A single clock drives every independent Board. At 120 BPM it emits sixteenths.
local clock = graph:module("clock", "square", {frequency = 8})
graph:connect(clock.out, notes.advance)
local tempo = constant("tempo", 120)
local lead_on = control("lead_on", 1, 0, 1)
local bass_on = control("bass_on", 1, 0, 1)
local drums_on = control("drums_on", 1, 0, 1)
graph:connect(pitch, lead.pitch)
graph:connect(product("lead_gate", product("lead_step", clock.out, notes.enabled), lead_on), lead.gate)
graph:connect(constant("lead_velocity", 0.72), lead.velocity)
graph:connect(tempo, lead.tempo)

-- Independent bass pattern, sharing the harmonic root and diatonic shift.
local bass_steps = graph:value_sequencer {id = "bass_steps",
  values = {0.5/7,0,0,4.5/7,0.5/7,0,0,2.5/7,0.5/7,0,0,4.5/7,0.5/7,0,0,3.5/7}}
graph:connect(clock.out, bass_steps.advance)
local bass_degree = graph:module("bass_scale", "scale_degree_to_note128")
graph:connect(bass_steps.value, bass_degree.input)
graph:connect(root.out, bass_degree.root)
graph:connect(shift.out, bass_degree.transpose)
local bass_octave = graph:module("bass_octave", "affine", {scale = 1, offset = -24/128})
graph:connect(bass_degree.out, bass_octave.input)
graph:connect(graph:note128_to_hz("bass_hz", graph:as_note128("bass_note", bass_octave.out)), bass.pitch)
local bass_rhythm = graph:value_sequencer {id = "bass_rhythm",
  values = {1,0,0,1,1,0,0,1,1,0,0,1,1,0,0,1}}
graph:connect(clock.out, bass_rhythm.advance)
graph:connect(product("bass_gate", product("bass_step_gate", clock.out, bass_rhythm.value), bass_on), bass.gate)
graph:connect(constant("bass_velocity", 0.8), bass.velocity)
graph:connect(tempo, bass.tempo)
graph:connect(tempo, pulse.tempo)

-- Four independent drum-value Boards; values are velocity, zero is a rest.
local lanes = {
  {id="kick", note=36, steps={1,0,0,0,0.9,0,0,0,1,0,0,0,0.9,0,0,0}},
  {id="snare", note=38, steps={0,0,0,0,0.85,0,0,0,0,0,0,0,0.9,0,0,0}},
  {id="hat", note=42, steps={0,0.5,0.85,0.4,0,0.5,0.8,0.4,0,0.5,0.85,0.4,0,0.5,0.8,0.4}},
  {id="perc", note=46, steps={0,0,0,0.6,0,0,0,0,0,0,0,0.5,0,0,0.7,0}},
}
for index, lane in ipairs(lanes) do
  local seq = graph:value_sequencer {id = lane.id .. "_steps", values = lane.steps}
  graph:connect(clock.out, seq.advance)
  local lane_on = control(lane.id .. "_on", 1, 0, 1)
  local velocity = product(lane.id .. "_velocity", seq.value, lane_on)
  local gate = product(lane.id .. "_gate", clock.out, velocity)
  local suffix = index == 1 and "" or "_" .. index
  local note = graph:as_note128(lane.id .. "_note", constant(lane.id .. "_pitch", lane.note / 128))
  graph:connect(graph:note128_to_hz(lane.id .. "_hz", note), pulse["pitch" .. suffix])
  graph:connect(gate, pulse["gate" .. suffix])
  graph:connect(velocity, pulse["velocity" .. suffix])
end

-- Stereo mixer: separate post-instrument levels and master headroom.
local mix = graph:module("mix", "stereo_mixer8", {headroom = 0.35})
for index, entry in ipairs({
  {id = "lead", synth = lead, level = 0.65, enabled = lead_on},
  {id = "bass", synth = bass, level = 0.65, enabled = bass_on},
  {id = "drums", synth = pulse, level = 0.85, enabled = drums_on},
}) do
  local level = control(entry.id .. "_level", entry.level, 0, 1)
  local amplitude = product(entry.id .. "_amplitude", level, entry.enabled)
  for _, side in ipairs({"left", "right"}) do
    local signal = product(entry.id .. "_" .. side, entry.synth[side], amplitude)
    graph:connect(signal, mix[side .. "_" .. index])
  end
end
graph:control("master", mix, "headroom", {min = 0.01, max = 1})
tui:control("Master", "master")
local output = graph:module("output", "stereo_output")
graph:connect(mix.left, output.left)
graph:connect(mix.right, output.right)
return graph
