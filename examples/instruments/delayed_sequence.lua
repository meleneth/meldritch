-- Audition: cargo run -p meldritch2 -- play examples/instruments/delayed_sequence.lua 20
local instrument = meldritch.instrument("Delayed Pulse Sequence")
local sequence = instrument:sequencer { bpm = 112, steps = 16, steps_per_beat = 4 }
local gate = sequence:track("Pulse")
sequence:steps("Pulse", { 1,0,0,0, 0,0,1,0, 1,0,0,1, 0,0,1,0 })
sequence:start()
local oscillator = instrument:oscillator { waveform = "pulse", frequency = 110, pulse_width = 0.3 }
local envelope = instrument:adsr { attack = 0.001, decay = 0.08, sustain = 0, release = 0.04 }
local amp = instrument:vca {}
local delay = instrument:delay { time = 0.321, feedback = 0.52, mix = 0.38 }
instrument:connect(gate.out, envelope.gate); instrument:connect(oscillator.out, amp.input)
instrument:connect(envelope.out, amp.cv); instrument:connect(amp.out, delay.input)
instrument:connect(amp.out, instrument.output.left); instrument:connect(delay.out, instrument.output.right)
instrument:control { id = "delay_mix", label = "Delay wet/dry", target = delay.mix, range = { 0, 1 } }
return instrument
