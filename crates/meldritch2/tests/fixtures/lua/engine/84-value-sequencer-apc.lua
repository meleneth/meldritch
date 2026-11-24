-- 84: ValueSequencer / APC mini mk2 / bespoke TUI
--
-- Hardware: APC mini mk2 faders 1-8 set step values (soft takeover).
-- Track buttons (Volume, Pan, Send, Device, Up, Down, Left, Right) toggle steps 1-8.
-- TUI: 1-8 toggle, left/right select, up/down adjust, Home/End min/max.
local graph = meldritch.graph()
graph:devices { apc_mini = "APC mini mk2 Control", midi_input = "MicroLab mk3" }

local notes = graph:value_sequencer {
  id = "notes",
  -- Values select the centers of seven equal scale-degree regions.
  values = {0.5/7, 2.5/7, 4.5/7, 6.5/7, 4.5/7, 2.5/7, 1.5/7, 3.5/7},
}
notes:controls("notes")
graph:apc_value_sequencer(notes)

local tui = graph:screen("value_sequence", "VALUE SEQUENCER / BOARD")
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
  }}}
}
tui:control("Diatonic shift (0=I, 3=IV, 4=V)", "degree_shift")
tui:control("Root capture armed", "root_arm")
local oscillator = graph:module("oscillator", "oscillator_sine")
graph:connect(pitch, oscillator.pitch)

local clock = graph:module("clock", "square", {frequency = 4.0})
graph:connect(clock.out, notes.advance)
local triggered = graph:module("enabled_clock", "multiply")
graph:connect(notes.enabled, triggered.left)
graph:connect(clock.out, triggered.right)
local env = graph:module("env", "envelope", {attack = 0.01, release = 0.008})
graph:connect(triggered.out, env.gate)
local amp = graph:module("amp", "vca")
graph:connect(oscillator.out, amp.input)
graph:connect(env.out, amp.cv)
local gain = graph:module("gain", "gain", {gain = 0.15})
graph:connect(amp.out, gain.input)
local output = graph:module("output", "output")
graph:connect(gain.out, output.input)
return graph
