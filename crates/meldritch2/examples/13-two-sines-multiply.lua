local graph = meldritch.graph()
local carrier = graph:module("carrier", "sine", { frequency = 12000.0 })
local modulator = graph:module("modulator", "sine", { frequency = 6000.0 })
local product = graph:module("product", "multiply")
local output = graph:module("output", "output")
graph:connect(carrier.out, product.left)
graph:connect(modulator.out, product.right)
graph:connect(product.out, output.input)
return graph
