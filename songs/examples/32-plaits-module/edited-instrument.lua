-- MELDRITCH authored instrument; base song remains unchanged.
return {
  kind = "instrument",
  version = 1,
  base_fingerprint = "59f6c8d405d3e43f",
  instrument = "edited-bass",
  parameters = {
    { target = { synth = "edited-bass", module = "env", parameter = "attack" }, value = 0.004 },
    { target = { synth = "edited-bass", module = "env", parameter = "decay" }, value = 0.16 },
    { target = { synth = "edited-bass", module = "env", parameter = "sustain" }, value = 0.55 },
    { target = { synth = "edited-bass", module = "env", parameter = "release" }, value = 0.09 },
    { target = { synth = "edited-bass", module = "filter", parameter = "cutoff_hz" }, value = 1200.0 },
    { target = { synth = "edited-bass", module = "filter", parameter = "resonance" }, value = 0.25 },
    { target = { synth = "edited-bass", module = "osc", parameter = "model" }, value = "harmonic" },
    { target = { synth = "edited-bass", module = "osc", parameter = "harmonics" }, value = 0.4 },
    { target = { synth = "edited-bass", module = "osc", parameter = "timbre" }, value = 0.35 },
    { target = { synth = "edited-bass", module = "osc", parameter = "morph" }, value = 0.5 },
  },
}
