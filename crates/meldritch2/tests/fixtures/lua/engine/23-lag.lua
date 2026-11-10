local graph = meldritch.graph()
local output = graph:module("output", "output")
local lag = graph:module("lag", "lag", { response = 0.5 })
local source = graph:module("source", "constant", { value = 1.0 })
graph:connect(source.out, lag.input)
graph:connect(lag.out, output.input)
graph:control("response", lag, "response", { min = 0.0, max = 1.0 })
return graph
