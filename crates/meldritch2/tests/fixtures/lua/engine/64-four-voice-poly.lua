local voice = meldritch.subpatch("voice")
local pitch = voice:module("pitch", "constant", { value = 0.0 })
local gate = voice:module("gate", "constant", { value = 0.0 })
local velocity = voice:module("velocity", "constant", { value = 1.0 })
local oscillator = voice:module("oscillator", "sine", { frequency = 0.0 })
local envelope = voice:module("envelope", "envelope", { attack = 0.01, release = 0.005 })
local amplitude = voice:module("amplitude", "multiply")
local level = voice:module("level", "gain", { gain = 0.25 })
voice:connect(pitch.out, oscillator.fm_hz)
voice:connect(gate.out, envelope.gate)
voice:connect(envelope.out, amplitude.left)
voice:connect(velocity.out, amplitude.right)
voice:connect(oscillator.out, level.input)
voice:connect(amplitude.out, level.mod)
voice:output("audio", level.out)
voice:parameter("pitch", pitch, "value")
voice:parameter("gate", gate, "value")
voice:parameter("velocity", velocity, "value")
voice:parameter("volume", level, "gain")

local graph = meldritch.graph()
local names = { "first", "second", "third", "fourth" }
local voices = {}
local bindings = {}
for i, name in ipairs(names) do
  voices[i] = graph:instance(name, voice, { volume = 0.125 })
  graph:control(name .. "-pitch", voices[i], "pitch", { min = 8.0, max = 13000.0 })
  graph:control(name .. "-gate", voices[i], "gate", { min = 0.0, max = 1.0 })
  graph:control(name .. "-velocity", voices[i], "velocity", { min = 0.0, max = 1.0 })
  bindings[i] = {
    pitch = name .. "-pitch", gate = name .. "-gate", velocity = name .. "-velocity"
  }
end
local first_pair = graph:module("first-pair", "mix")
local second_pair = graph:module("second-pair", "mix")
local mix = graph:module("mix", "mix")
local output = graph:module("output", "output")
graph:connect(voices[1].audio, first_pair.left)
graph:connect(voices[2].audio, first_pair.right)
graph:connect(voices[3].audio, second_pair.left)
graph:connect(voices[4].audio, second_pair.right)
graph:connect(first_pair.out, mix.left)
graph:connect(second_pair.out, mix.right)
graph:connect(mix.out, output.input)
graph:midi_poly(1, bindings)
return graph
