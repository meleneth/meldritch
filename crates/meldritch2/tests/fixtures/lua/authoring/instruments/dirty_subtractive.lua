-- Audition: cargo run -p meldritch2 -- perform examples/instruments/dirty_subtractive.lua
local instrument = meldritch.instrument("Dirty Subtractive")
local oscillator = instrument:oscillator { waveform = "pulse", frequency = 110, pulse_width = 0.42 }
local lfo = instrument:lfo { frequency = 0.35 }
local filter = instrument:svf { cutoff = 1400, resonance = 0.45 }
filter:modulate("cutoff", lfo, { depth = 850 })
local drive = instrument:drive { amount = 3.0, output_gain = 0.65 }
local envelope = instrument:adsr { attack = 0.01, decay = 0.18, sustain = 0.62, release = 0.5 }
local amp = instrument:vca {}
local delay = instrument:delay { time = 0.24, feedback = 0.3, mix = 0.18 }
instrument:connect(instrument.note.pitch, oscillator.pitch)
instrument:connect(instrument.note.gate, envelope.gate)
instrument:connect(oscillator.out, filter.input)
instrument:connect(filter.lowpass, drive.input)
instrument:connect(drive.out, amp.input); instrument:connect(envelope.out, amp.cv)
instrument:connect(amp.out, delay.input)
instrument:connect(amp.out, instrument.output.left); instrument:connect(delay.out, instrument.output.right)
instrument:control { id = "pitch", label = "Oscillator pitch", target = oscillator.frequency, range = { 20, 20000 }, curve = "log" }
instrument:control { id = "cutoff", label = "Filter cutoff", target = filter.cutoff, range = { 80, 12000 }, curve = "log" }
instrument:control { id = "resonance", label = "Resonance", target = filter.resonance, range = { 0, 0.95 } }
instrument:control { id = "drive", label = "Drive", target = drive.amount, range = { 0.1, 12 } }
instrument:control { id = "delay_mix", label = "Delay wet/dry", target = delay.mix, range = { 0, 1 } }
return instrument
