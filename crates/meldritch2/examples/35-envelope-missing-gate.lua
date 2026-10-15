local graph = meldritch.graph()
local output = graph:module("output", "output")
local envelope = graph:module("envelope", "envelope")
graph:connect(envelope.out, output.input)
return graph
