local graph = meldritch.graph()
local output = graph:module("output", "output")
local level = graph:module("level", "gain", { gain = 1.0 })
local carrier = graph:module("carrier", "sine", { frequency = 6000.0 })
local amplitude = graph:module("amplitude", "affine", {
  scale = 0.5,
  offset = 0.5
})
local depth = graph:module("depth", "gain", { gain = 6000.0 })
local modulator = graph:module("modulator", "sine", { frequency = 12000.0 })
graph:connect(modulator.out, depth.input)
graph:connect(modulator.out, amplitude.input)
graph:connect(depth.out, carrier.fm_hz)
graph:connect(amplitude.out, level.mod)
graph:connect(carrier.out, level.input)
graph:connect(level.out, output.input)
return graph
