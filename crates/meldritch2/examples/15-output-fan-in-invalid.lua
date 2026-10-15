local graph = meldritch.graph()
local left = graph:module("left", "constant", { value = 0.25 })
local right = graph:module("right", "constant", { value = 0.125 })
local output = graph:module("output", "output")
graph:connect(left.out, output.input)
graph:connect(right.out, output.input)
return graph
