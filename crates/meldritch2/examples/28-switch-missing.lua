local graph = meldritch.graph()
local output = graph:module("output", "output")
local switch = graph:module("switch", "switch", { threshold = 0.5 })
local right = graph:module("right", "constant", { value = -0.25 })
local left = graph:module("left", "constant", { value = 0.25 })
graph:connect(left.out, switch.left)
graph:connect(right.out, switch.right)
graph:connect(switch.out, output.input)
return graph
