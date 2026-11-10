local graph = meldritch.graph()
local carrier = graph:module("carrier", "sine", { frequency = 6000.0 })
local output = graph:module("output", "output")
graph:connect(carrier.out, carrier.fm_hz)
graph:connect(carrier.out, output.input)
return graph
