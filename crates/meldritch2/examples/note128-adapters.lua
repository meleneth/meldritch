-- The generic ValueSequencer contains no MIDI or frequency awareness.
-- The adapter performs MIDI note/128 -> MIDI note -> Hz mapping at its boundary.
-- A VST3 module's `pitch` input currently consumes Hz, not note numbers.
local graph = meldritch.graph()
local steps = graph:value_sequencer {
  id = "notes",
  values = {60/128, 64/128, 67/128, 72/128}
}
local clock = graph:module("clock", "square", {frequency = 2.0})
graph:connect(clock.out, steps.advance)

local typed_note = graph:as_note128("note128", steps.value)
local hz = graph:note128_to_hz("pitch_to_hz", typed_note)
-- When wiring an actual VST3 instance, use:
-- graph:connect(hz, synth.pitch)

-- This output demonstrates the adapter with no plugin required.
local output = graph:module("output", "output")
graph:connect(hz, output.input)
return graph
