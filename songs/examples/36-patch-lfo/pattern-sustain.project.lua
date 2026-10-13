-- A rhythm releases its key gate while a separately authored hold pattern sustains it.
return {
  kind = "project", version = 2,
  project = { id = "pattern-sustain", title = "Independent pattern hold", bpm = 120, sample_rate = 48000 },
  patches = {
    { id = "bass", polyphony = 1,
      inputs = { { id = "pitch", signal = "pitch" }, { id = "gate", signal = "gate" }, { id = "sustain", signal = "gate" } },
      modules = {
        { id = "osc", kind = "oscillator", waveform = "sine" },
        { id = "env", kind = "adsr", attack = 0, decay = 0, sustain = 1, release = 0.005 },
        { id = "hold", kind = "gate_hold" },
        { id = "amp", kind = "vca" },
        { id = "out", kind = "audio_output", channels = 1 },
      },
      cables = {
        { from = "input.pitch", to = "osc.pitch", signal = "pitch" },
        { from = "input.gate", to = "hold.gate", signal = "gate" },
        { from = "input.sustain", to = "hold.hold", signal = "gate" },
        { from = "hold.gate", to = "env.gate", signal = "gate" },
        { from = "osc.audio", to = "amp.audio", signal = "audio" },
        { from = "env.control", to = "amp.level", signal = "control" },
        { from = "amp.audio", to = "out.audio", signal = "audio" },
      },
    },
  },
  patterns = {
    { id = "pitch", length_ticks = 1920, looped = true, next_note_id = 3,
      notes = {
        { id = "note-1", note = "C3", start_tick = 0, duration_ticks = 960, velocity = 0.2 },
        { id = "note-2", note = "G3", start_tick = 960, duration_ticks = 960, velocity = 0.9 },
      } },
    { id = "rhythm", length_ticks = 1920, looped = true, next_note_id = 2,
      notes = { { id = "note-1", note = "C6", start_tick = 0, duration_ticks = 480, velocity = 0.4 } } },
    { id = "hold-time", length_ticks = 1920, looped = true, next_note_id = 2,
      notes = { { id = "note-1", note = "C1", start_tick = 480, duration_ticks = 960, velocity = 0.1 } } },
  },
  voices = {
    { id = "bass-instance", synth = "bass", length_ticks = 1920, looped = true,
      initial_pitch = 48, level = 0.8, pitch_pattern = "pitch", gate_pattern = "rhythm",
      sustain_pattern = "hold-time" },
  },
}
