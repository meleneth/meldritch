-- Audition: cargo run -p meldritch2 -- perform examples/instruments/plaits_harmonic.lua
local instrument = meldritch.instrument("Plaits Harmonic")
local voice = instrument:plaits { model = "harmonic", harmonics = 0.7, timbre = 0.55, morph = 0.2 }
local mix = instrument:mixer { inputs = 2 }
local delay = instrument:delay { time = 0.28, feedback = 0.4, mix = 0.25 }
instrument:connect(instrument.note.pitch, voice.pitch); instrument:connect(instrument.note.gate, voice.gate)
instrument:connect(instrument.note.velocity, voice.velocity)
instrument:connect(voice.main, mix.input[1]); instrument:connect(voice.aux, mix.input[2])
instrument:connect(mix.out, delay.input)
instrument:connect(mix.out, instrument.output.left); instrument:connect(delay.out, instrument.output.right)
instrument:control { id = "harmonics", label = "Harmonics", target = voice.harmonics, range = { 0, 1 } }
instrument:control { id = "timbre", label = "Timbre", target = voice.timbre, range = { 0, 1 } }
instrument:control { id = "morph", label = "Morph", target = voice.morph, range = { 0, 1 } }
instrument:control { id = "delay_mix", label = "Delay wet/dry", target = delay.mix, range = { 0, 1 } }
return instrument
