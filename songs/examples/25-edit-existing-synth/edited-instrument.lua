-- MELDRITCH authored instrument; base song remains unchanged.
return {
  kind = "instrument",
  version = 1,
  base_fingerprint = "1614dae1e311d6f3",
  instrument = "edited-bass",
  parameters = {
    { target = { synth = "edited-bass", module = "env", parameter = "attack" }, value = 0.004 },
    { target = { synth = "edited-bass", module = "env", parameter = "decay" }, value = 0.16 },
    { target = { synth = "edited-bass", module = "env", parameter = "sustain" }, value = 0.55 },
    { target = { synth = "edited-bass", module = "env", parameter = "release" }, value = 0.25 },
    { target = { synth = "edited-bass", module = "filter", parameter = "cutoff_hz" }, value = 900.0 },
    { target = { synth = "edited-bass", module = "filter", parameter = "resonance" }, value = 0.25 },
    { target = { synth = "edited-bass", module = "osc", parameter = "waveform" }, value = "saw" },
  },
}
