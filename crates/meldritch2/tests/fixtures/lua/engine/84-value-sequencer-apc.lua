-- 84: ValueSequencer / APC mini mk2 / bespoke TUI
--
-- Hardware: APC mini mk2 faders 1-8 set step values (soft takeover).
-- Every APC pad column toggles the corresponding enable component.
-- TUI: 1-8 toggle, left/right select, up/down adjust, Home/End min/max.
local graph = meldritch.graph()
graph:devices { apc_mini = "APC mini" }

local notes = graph:value_sequencer {
  id = "notes",
  values = {60/128, 64/128, 67/128, 72/128, 67/128, 64/128, 62/128, 55/128},
}
notes:controls("notes")
graph:apc_value_sequencer(notes)

local tui = graph:screen("value_sequence", "VALUE SEQUENCER / BOARD")
tui:label("Eight independent values and eight independent enable switches")
tui:value_sequencer(notes)
tui:control("Sequence length", "notes_length")
tui:control("Loop", "notes_looped")

-- Musical meaning is established outside the generic ValueSequencer.
local note128 = graph:as_note128("note128", notes.value)
local pitch = graph:note128_to_hz("pitch", note128)
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
