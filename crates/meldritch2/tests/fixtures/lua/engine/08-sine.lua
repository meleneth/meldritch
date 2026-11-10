local graph = meldritch.graph()
local oscillator = graph:module("oscillator", "sine", { frequency = 12000.0 })
local gain = graph:module("gain", "gain", { gain = 0.25 })
local output = graph:module("output", "output")
graph:connect(oscillator.out, gain.input)
graph:connect(gain.out, output.input)
return graph
