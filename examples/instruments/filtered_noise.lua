-- Audition: cargo run -p meldritch2 -- play examples/instruments/filtered_noise.lua 15
local instrument = meldritch.instrument("Filtered Noise")
local noise = instrument:noise { amplitude = 0.4 }
local filter = instrument:svf { cutoff = 900, resonance = 0.65 }
local sweep = instrument:lfo { frequency = 0.08 }
filter:modulate("cutoff", sweep, { depth = 700 })
instrument:connect(noise.out, filter.input)
instrument:connect(filter.lowpass, instrument.output.left)
instrument:connect(filter.bandpass, instrument.output.right)
instrument:control { id = "brightness", label = "Brightness", target = filter.cutoff,
  range = { 80, 12000 }, curve = "log" }
instrument:control { id = "resonance", label = "Resonance", target = filter.resonance,
  range = { 0, 0.95 } }
return instrument
