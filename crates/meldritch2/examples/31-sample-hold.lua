local graph = meldritch.graph()
local output = graph:module("output", "output")
local hold = graph:module("hold", "sample_hold", { threshold = 0.5 })
local trigger = graph:module("trigger", "sine", { frequency = 12000.0 })
local signal = graph:module("signal", "sine", { frequency = 6000.0 })
graph:connect(hold.out, output.input)
graph:connect(trigger.out, hold.trigger)
graph:connect(signal.out, hold.signal)
return graph
