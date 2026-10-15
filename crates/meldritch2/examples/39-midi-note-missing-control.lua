local graph = meldritch.graph()
local output = graph:module("output", "output")
local oscillator = graph:module("oscillator", "sine")
graph:connect(oscillator.out, output.input)
graph:control("pitch", oscillator, "frequency", { min = 8.0, max = 13000.0 })
graph:midi_notes(1, "pitch", "missing")
return graph
