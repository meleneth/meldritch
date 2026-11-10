local graph = meldritch.graph()
local output = graph:module("output", "output")
local gain = graph:module("gain", "gain", { gain = 0.5 })
local source = graph:module("source", "constant", { value = 0.25 })
graph:connect(source.out, gain.input)
graph:connect(gain.out, output.input)
return graph
