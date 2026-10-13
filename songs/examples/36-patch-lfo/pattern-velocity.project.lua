-- A velocity pattern changes an explicit Control while a separate gate remains high.
return {
  kind = "project", version = 2,
  project = { id = "pattern-velocity", title = "Independent pattern dynamics", bpm = 120, sample_rate = 48000 },
  patches = {
    { id = "bass", polyphony = 1,
      inputs = { { id = "pitch", signal = "pitch" }, { id = "gate", signal = "gate" }, { id = "velocity", signal = "control" } },
      modules = {
        { id = "osc", kind = "oscillator", waveform = "sine" },
        { id = "env", kind = "adsr", attack = 0, decay = 0, sustain = 1, release = 0.005 },
        { id = "amp", kind = "vca" },
        { id = "out", kind = "audio_output", channels = 1 },
        { id = "shape", kind = "control_multiply" },
      },
      cables = {
        { from = "input.pitch", to = "osc.pitch", signal = "pitch" },
        { from = "input.gate", to = "env.gate", signal = "gate" },
        { from = "osc.audio", to = "amp.audio", signal = "audio" },
        { from = "env.control", to = "shape.left", signal = "control" },
        { from = "input.velocity", to = "shape.right", signal = "control" },
        { from = "shape.control", to = "amp.level", signal = "control" },
        { from = "amp.audio", to = "out.audio", signal = "audio" },
      },
    },
  },
  patterns = {
    { id = "pitch", length_ticks = 1920, looped = true, next_note_id = 3,
      notes = {
        { id = "note-1", note = "C3", start_tick = 0, duration_ticks = 960, velocity = 0.1 },
        { id = "note-2", note = "G3", start_tick = 960, duration_ticks = 960, velocity = 0.9 },
      } },
    { id = "gate", length_ticks = 1920, looped = true, next_note_id = 3,
      notes = {
        { id = "note-1", note = "C6", start_tick = 0, duration_ticks = 1440, velocity = 0.1 },
        { id = "note-2", note = "C6", start_tick = 1440, duration_ticks = 480, velocity = 0.9 },
      } },
    { id = "dynamics", length_ticks = 1920, looped = true, next_note_id = 4,
      notes = {
        { id = "note-1", note = "C4", start_tick = 0, duration_ticks = 480, velocity = 0.25 },
        { id = "note-2", note = "C4", start_tick = 480, duration_ticks = 480, velocity = 0.75 },
        { id = "note-3", note = "C4", start_tick = 960, duration_ticks = 480, velocity = 0.5 },
      } },
  },
  voices = {
    { id = "bass-instance", synth = "bass", length_ticks = 1920, looped = true,
      initial_pitch = 48, level = 0.8, pitch_pattern = "pitch", gate_pattern = "gate",
      velocity_pattern = "dynamics" },
  },
}
