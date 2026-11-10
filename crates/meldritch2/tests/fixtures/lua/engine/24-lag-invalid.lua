local graph = meldritch.graph()
local source = graph:module("source", "constant", { value = 1.0 })
local lag = graph:module("lag", "lag", { response = 1.5 })
local output = graph:module("output", "output")
graph:connect(source.out, lag.input)
graph:connect(lag.out, output.input)
return graph
