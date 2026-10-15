local graph = meldritch.graph()
local modulator = graph:module("modulator", "sine", { frequency = 12000.0 })
local gain = graph:module("gain", "gain", { gain = 0.5 })
local output = graph:module("output", "output")
graph:connect(modulator.out, gain.mod)
graph:connect(gain.out, output.input)
return graph
