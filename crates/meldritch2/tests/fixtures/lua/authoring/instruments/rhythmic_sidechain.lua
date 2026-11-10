-- Audition: cargo run -p meldritch2 -- play examples/instruments/rhythmic_sidechain.lua 20
-- The cached Strudel 909 bank is prepared before audio starts. Kick audio is
-- both audible and the real detector signal; no duplicate trigger envelope is used.
local instrument = meldritch.instrument("Rhythmic Sidechain")
local kit = meldritch.samples { source = "strudel", bank = "RolandTR909", cache = true }
local sequence = instrument:sequencer { bpm = 122, steps = 16, steps_per_beat = 4 }
local kick_trigger = sequence:track("Kick")
sequence:steps("Kick", { 1,0,0,0, 1,0,0,0, 1,0,0,0, 1,0,1,0 })
sequence:start()
local kick = instrument:sample_player { sample = kit:load("bd", 0), gain = 0.85 }
instrument:connect(kick_trigger.out, kick.trigger)

local follower = instrument:envelope_follower { attack = 0.002, release = 0.18 }
local duck = instrument:affine { scale = -1.25, offset = 1.0 }
instrument:connect(kick.left, follower.input); instrument:connect(follower.out, duck.input)
local oscillator = instrument:oscillator { waveform = "saw", frequency = 82.41 }
local filter = instrument:svf { cutoff = 850, resonance = 0.4 }
local amp = instrument:vca {}
local compressor = instrument:compressor {
  threshold = -24, ratio = 8, attack = 0.002, release = 0.16, makeup_gain = 0,
}
instrument:connect(oscillator.out, filter.input); instrument:connect(filter.lowpass, amp.input)
instrument:connect(duck.out, amp.cv); instrument:connect(amp.out, compressor.input)
instrument:connect(kick.left, compressor.sidechain)
local left = instrument:mixer { inputs = 2 }
instrument:connect(kick.left, left.input[1]); instrument:connect(compressor.out, left.input[2])
instrument:connect(left.out, instrument.output.left)
-- The right channel is a synth-only monitor, making the authored ducking
-- directly measurable while the left remains the actual drum+synth mix.
instrument:connect(compressor.out, instrument.output.right)
return instrument
