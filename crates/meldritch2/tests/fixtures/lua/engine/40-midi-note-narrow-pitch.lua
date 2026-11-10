local graph = meldritch.graph()
local output = graph:module("output", "output")
local oscillator = graph:module("oscillator", "sine")
local gate = graph:module("gate", "constant")
graph:connect(oscillator.out, output.input)
graph:control("pitch", oscillator, "frequency", { min = 20.0, max = 2000.0 })
graph:control("gate", gate, "value", { min = 0.0, max = 1.0 })
graph:midi_notes(1, "pitch", "gate")
return graph
