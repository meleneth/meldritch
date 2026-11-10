local graph = meldritch.graph()
local output = graph:module("output", "output")
local hold = graph:module("hold", "sample_hold", { threshold = 0.5 })
local signal = graph:module("signal", "sine", { frequency = 6000.0 })
graph:connect(signal.out, hold.signal)
graph:connect(hold.out, output.input)
return graph
