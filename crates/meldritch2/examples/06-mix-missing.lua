local graph = meldritch.graph()
local source = graph:module("source", "constant", { value = 0.25 })
local mix = graph:module("mix", "mix")
local output = graph:module("output", "output")
graph:connect(source.out, mix.left)
graph:connect(mix.out, output.input)
return graph
