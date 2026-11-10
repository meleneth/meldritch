local graph = meldritch.graph()
local source = graph:module("source", "constant", { value = 0.5 })
local normal = graph:module("normal", "constant", { value = 0.5 })
local override = graph:module("override", "constant", { value = 0.25 })
local gain = graph:module("gain", "gain", { gain = 1.0 })
local output = graph:module("output", "output")

graph:connect(source.out, gain.input)
graph:normalize(normal.out, gain.mod)
local use_override = false
if use_override then
  graph:connect(override.out, gain.mod)
end
graph:connect(gain.out, output.input)
return graph
