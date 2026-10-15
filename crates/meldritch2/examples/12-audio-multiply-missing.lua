local graph = meldritch.graph()
local amplitude = graph:module("amplitude", "constant", { value = 0.25 })
local product = graph:module("product", "multiply")
local output = graph:module("output", "output")
graph:connect(amplitude.out, product.left)
graph:connect(product.out, output.input)
return graph
