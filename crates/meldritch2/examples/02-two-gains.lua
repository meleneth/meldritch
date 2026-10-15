local graph = meldritch.graph()
local source = graph:module("source", "constant", { value = 0.25 })
local gain_a = graph:module("gain_a", "gain", { gain = 0.5 })
local gain_b = graph:module("gain_b", "gain", { gain = 0.5 })
local output = graph:module("output", "output")
graph:connect(source.out, gain_a.input)
graph:connect(gain_a.out, gain_b.input)
graph:connect(gain_b.out, output.input)
return graph
