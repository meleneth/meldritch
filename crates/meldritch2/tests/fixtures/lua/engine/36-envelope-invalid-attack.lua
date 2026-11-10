local graph = meldritch.graph()
local output = graph:module("output", "output")
local envelope = graph:module("envelope", "envelope", { attack = 1.5 })
local gate = graph:module("gate", "constant", { value = 1.0 })
graph:connect(gate.out, envelope.gate)
graph:connect(envelope.out, output.input)
return graph
