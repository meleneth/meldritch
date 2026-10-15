local graph = meldritch.graph()
local source = graph:module("source", "constant", { value = 0.25 })
local gain = graph:module("gain", "gain", { gain = 0.5 })
local output = graph:module("output", "output")
graph:connect(source.out, gain.input)
graph:connect(gain.out, output.input)
return graph
