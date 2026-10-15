local graph = meldritch.graph()
local output = graph:module("output", "output")
local oscillator = graph:module("oscillator", "sine", { frequency = 440.0 })
graph:connect(oscillator.out, output.input)
return graph
