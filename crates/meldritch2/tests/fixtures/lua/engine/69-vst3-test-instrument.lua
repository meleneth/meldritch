local instrument = meldritch.instrument("Native VST3 test instrument")
local sequence = instrument:sequencer {
  bpm = 112,
  steps = 16,
  steps_per_beat = 4,
}
local notes = sequence:track("VST3 melody")
local pattern = sequence:pattern("fixture phrase")
pattern:notes("VST3 melody", {
  { step = 1, note = 57, velocity = 0.85, gate = 0.6 },
  { step = 5, note = 64, velocity = 0.70, gate = 0.5 },
  { step = 9, note = 69, velocity = 0.80, gate = 0.6 },
  { step = 13, note = 72, velocity = 0.65, gate = 0.4 },
})

local synth = instrument:vst3 {
  plugin = "Meldritch Test Instrument",
  paths = { "../../../target/vst3-fixtures" },
}
synth:set_parameter(0, 0.7)

local left = instrument:drive { amount = 1.7, output_gain = 0.8 }
local right = instrument:drive { amount = 1.7, output_gain = 0.8 }
instrument:connect(notes.pitch, synth.pitch)
instrument:connect(notes.gate, synth.gate)
instrument:connect(notes.velocity, synth.velocity)
instrument:connect(sequence.tempo, synth.tempo)
instrument:connect(synth.left, left.input)
instrument:connect(synth.right, right.input)
instrument:connect(left.out, instrument.output.left)
instrument:connect(right.out, instrument.output.right)

return instrument
