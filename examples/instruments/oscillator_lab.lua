-- Audition: cargo run -p meldritch2 -- play examples/instruments/oscillator_lab.lua 15
-- Left layers saw + triangle; right layers square + pulse. A slow sine changes
-- pulse width, so the channels remain audibly distinct.
local instrument = meldritch.instrument("Oscillator Lab")
local saw = instrument:oscillator { waveform = "saw", frequency = 110 }
local triangle = instrument:oscillator { waveform = "triangle", frequency = 220 }
local square = instrument:oscillator { waveform = "square", frequency = 110 }
local pulse = instrument:oscillator { waveform = "pulse", frequency = 220, pulse_width = 0.35 }
local pwm = instrument:lfo { frequency = 0.2 }
pulse:modulate("pulse_width", pwm, { depth = 0.25 })
local left = instrument:mixer { inputs = 2 }
local right = instrument:mixer { inputs = 2 }
instrument:connect(saw.out, left.input[1]); instrument:connect(triangle.out, left.input[2])
instrument:connect(square.out, right.input[1]); instrument:connect(pulse.out, right.input[2])
instrument:connect(left.out, instrument.output.left); instrument:connect(right.out, instrument.output.right)
return instrument
