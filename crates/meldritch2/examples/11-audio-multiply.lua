local graph = meldritch.graph()
local output = graph:module("output", "output")
local product = graph:module("product", "multiply")
local oscillator = graph:module("oscillator", "sine", { frequency = 12000.0 })
local amplitude = graph:module("amplitude", "constant", { value = 0.25 })
graph:connect(amplitude.out, product.left)
graph:connect(oscillator.out, product.right)
graph:connect(product.out, output.input)
return graph
