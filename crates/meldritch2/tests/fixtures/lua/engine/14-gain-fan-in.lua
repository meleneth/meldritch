local graph = meldritch.graph()
local output = graph:module("output", "output")
local gain = graph:module("gain", "gain", { gain = 0.5 })
local right = graph:module("right", "constant", { value = 0.125 })
local left = graph:module("left", "constant", { value = 0.25 })
graph:connect(right.out, gain.input)
graph:connect(left.out, gain.input)
graph:connect(gain.out, output.input)
return graph
