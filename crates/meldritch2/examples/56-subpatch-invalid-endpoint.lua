local voice = meldritch.subpatch("broken_voice")
local oscillator = voice:module("oscillator", "sine", { frequency = 440.0 })
voice:output("audio", { module = oscillator.id, port = "missing", direction = "output" })

local graph = meldritch.graph()
local instance = graph:instance("voice", voice)
local output = graph:module("output", "output")
graph:connect(instance.audio, output.input)
return graph
