-- Launch with:
--   cargo run -p meldritch2 -- perform examples/instruments/drone.lua
-- carrier.fm is an additive frequency offset in Hz. A two-input mixer averages
-- its inputs, giving the default three-voice topology bounded headroom.
local drone = meldritch.instrument("Drone")

-- A reusable composite module constructed from primitives.
local function voice(parent, frequency, detune)
  local v = parent:subpatch("Drone Voice")

  local carrier = v:oscillator {
    waveform = "saw",
    frequency = frequency,
  }

  local modulator = v:lfo {
    waveform = "sine",
    frequency = 0.18,
  }

  local depth = v:constant {
    value = detune,
  }

  local modulation = v:multiply {}

  v:connect(modulator.out, modulation.a)
  v:connect(depth.out, modulation.b)
  v:connect(modulation.out, carrier.fm)

  v:expose("audio", carrier.out)
  v:expose("rate", modulator.frequency)
  v:expose("depth", depth.value)

  return v
end

local left_voice = voice(drone, 110, 0.6)
local right_voice = voice(drone, 110.7, 0.9)
local center_voice = voice(drone, 55, 0.2)

local left_mix = drone:mixer { inputs = 2 }
local right_mix = drone:mixer { inputs = 2 }

drone:connect(left_voice.audio, left_mix.input[1])
drone:connect(center_voice.audio, left_mix.input[2])

drone:connect(right_voice.audio, right_mix.input[1])
drone:connect(center_voice.audio, right_mix.input[2])

drone:connect(left_mix.out, drone.output.left)
drone:connect(right_mix.out, drone.output.right)

drone:control("Left modulation rate", left_voice.rate, {
  min = 0.01,
  max = 8,
  scale = "log",
})

drone:control("Right modulation rate", right_voice.rate, {
  min = 0.01,
  max = 8,
  scale = "log",
})

drone:control("Left modulation depth", left_voice.depth, {
  min = 0,
  max = 5,
})

drone:control("Right modulation depth", right_voice.depth, {
  min = 0,
  max = 5,
})

return drone
