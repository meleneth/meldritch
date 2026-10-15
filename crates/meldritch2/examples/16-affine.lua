local graph = meldritch.graph()
local output = graph:module("output", "output")
local transform = graph:module("transform", "affine", {
  scale = 0.5,
  offset = 0.125
})
local source = graph:module("source", "constant", { value = 0.25 })
graph:connect(source.out, transform.input)
graph:connect(transform.out, output.input)
return graph
