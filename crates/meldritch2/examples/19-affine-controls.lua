local graph = meldritch.graph()
local source = graph:module("source", "constant", { value = 0.25 })
local transform = graph:module("transform", "affine", {
  scale = 0.5,
  offset = 0.125
})
local output = graph:module("output", "output")
graph:connect(source.out, transform.input)
graph:connect(transform.out, output.input)
graph:control("scale", transform, "scale", { min = 0.0, max = 1.0 })
graph:control("offset", transform, "offset", { min = -0.5, max = 0.5 })
graph:midi_cc("scale", 1, 7)
graph:midi_cc("offset", 1, 8)
return graph
