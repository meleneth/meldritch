-- Launch with:
--   cargo run -p meldritch2 -- perform examples/instruments/subtractive.lua
-- The instrument authoring layer expands note input into four independent
-- instances and routes MIDI channel 1 through the generic voice allocator.
local synth = meldritch.instrument("Subtractive")

synth:devices {
  midi_input = "MicroLab mk3:MicroLab mk3",
}

local osc = synth:oscillator {
  waveform = "saw",
  frequency = 440,
}

local filter = synth:lowpass {
  cutoff = 1200,
  resonance = 0.3,
}

local envelope = synth:adsr {
  attack = 0.01,
  decay = 0.2,
  sustain = 0.65,
  release = 0.4,
}

local amp = synth:vca {}

synth:connect(synth.note.pitch, osc.pitch)
synth:connect(synth.note.gate, envelope.gate)
synth:connect(osc.out, filter.input)
synth:connect(filter.out, amp.input)
synth:connect(envelope.out, amp.cv)
synth:connect(amp.out, synth.output.left)
synth:connect(amp.out, synth.output.right)

synth:control("Cutoff", filter.cutoff, {
  min = 80,
  max = 12000,
  scale = "log",
})

synth:control("Resonance", filter.resonance, {
  min = 0,
  max = 0.95,
})

synth:control("Attack", envelope.attack, {
  min = 0.001,
  max = 2,
})

synth:control("Release", envelope.release, {
  min = 0.01,
  max = 5,
})

return synth
