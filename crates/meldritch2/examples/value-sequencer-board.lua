-- A ValueSequencer is a Lua-authored Board of ordinary components.
-- No special Rust sequencer implementation or dedicated fader machinery.
local graph = meldritch.graph()
local sequence = graph:value_sequencer {
  id = "sequence",
  values = {0.0, 0.25, 0.5, 0.75, 1.0, 0.75, 0.5, 0.25},
}
sequence:controls("sequence")

local clock = graph:module("clock", "square", {frequency = 4.0})
local output = graph:module("output", "output")
graph:connect(clock.out, sequence.advance)
graph:connect(sequence.value, output.input)
return graph
