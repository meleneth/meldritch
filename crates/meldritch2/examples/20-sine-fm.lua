local graph = meldritch.graph()
local output = graph:module("output", "output")
local carrier = graph:module("carrier", "sine", { frequency = 6000.0 })
local depth = graph:module("depth", "gain", { gain = 6000.0 })
local modulator = graph:module("modulator", "sine", { frequency = 12000.0 })
graph:connect(modulator.out, depth.input)
graph:connect(depth.out, carrier.fm_hz)
graph:connect(carrier.out, output.input)
return graph
