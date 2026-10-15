local graph = meldritch.graph()
local output = graph:module("output", "output")
local gain = graph:module("gain", "gain", { gain = 0.5 })
local modulator = graph:module("modulator", "sine", { frequency = 12000.0 })
local source = graph:module("source", "constant", { value = 0.25 })
graph:connect(source.out, gain.input)
graph:connect(modulator.out, gain.mod)
graph:connect(gain.out, output.input)
return graph
