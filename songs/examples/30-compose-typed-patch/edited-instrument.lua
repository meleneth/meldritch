-- A complete typed graph snapshot; the track and pattern references stay in the base.
return {
  kind = "instrument", version = 2,
  base_fingerprint = "45511b08684bc43b",
  instrument = "edited-bass",
  graph = {
    id = "edited-bass", polyphony = 1,
    inputs = {
      {id = "pitch", signal = "pitch"},
      {id = "gate", signal = "gate"},
    },
    modules = {
      {id = "source", kind = "oscillator", waveform = "saw"},
      {id = "contour", kind = "adsr", attack = 0.01, decay = 0.0, sustain = 1.0, release = 0.2},
      {id = "gain", kind = "vca"},
      {id = "tone", kind = "low_pass", cutoff_hz = 800.0, resonance = 0.2},
      {id = "out", kind = "audio_output", channels = 1},
    },
    cables = {
      {from = "input.pitch", to = "source.pitch", signal = "pitch"},
      {from = "input.gate", to = "contour.gate", signal = "gate"},
      {from = "source.audio", to = "gain.audio", signal = "audio"},
      {from = "contour.control", to = "gain.level", signal = "control"},
      {from = "gain.audio", to = "tone.audio", signal = "audio"},
      {from = "tone.audio", to = "out.audio", signal = "audio"},
    },
  },
}
