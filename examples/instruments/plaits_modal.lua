-- Audition: cargo run -p meldritch2 -- perform examples/instruments/plaits_modal.lua
local instrument = meldritch.instrument("Plaits Modal")
local voice = instrument:plaits { model = "modal", harmonics = 0.35, timbre = 0.7, morph = 0.55 }
instrument:connect(instrument.note.pitch, voice.pitch); instrument:connect(instrument.note.gate, voice.gate)
instrument:connect(voice.main, instrument.output.left); instrument:connect(voice.aux, instrument.output.right)
instrument:control { id = "harmonics", label = "Harmonics", target = voice.harmonics, range = { 0, 1 } }
instrument:control { id = "timbre", label = "Timbre", target = voice.timbre, range = { 0, 1 } }
instrument:control { id = "morph", label = "Morph", target = voice.morph, range = { 0, 1 } }
return instrument
