-- Self-contained project v2: no seed song, controller script or capture required.
-- A patch is palette material; the composition instance binds pitch and gate separately.
return {
  kind = "project", version = 2,
  project = { id = "project", title = "Untitled project", bpm = 120, sample_rate = 48000 },
  patches = {
    {
      id = "voice-1", polyphony = 1, output_level_control = "level",
      inputs = { { id = "pitch", signal = "pitch" }, { id = "gate", signal = "gate" } },
      modules = {
        { id = "osc", kind = "oscillator", waveform = "sine" },
        { id = "env", kind = "adsr", attack = 0, decay = 0, sustain = 1, release = 0 },
        { id = "amp", kind = "vca" },
        { id = "out", kind = "audio_output", channels = 1 },
      },
      cables = {
        { from = "input.pitch", to = "osc.pitch", signal = "pitch" },
        { from = "input.gate", to = "env.gate", signal = "gate" },
        { from = "osc.audio", to = "amp.audio", signal = "audio" },
        { from = "env.control", to = "amp.level", signal = "control" },
        { from = "amp.audio", to = "out.audio", signal = "audio" },
      },
    },
  },
  patterns = {
    { id = "melody", length_ticks = 3840, looped = true, next_note_id = 3,
      notes = { { id = "note-2", note = "C3", start_tick = 0, duration_ticks = 480, velocity = 0.8 } } },
  },
  voices = {
    { id = "voice-1-instance", synth = "voice-1", length_ticks = 3840, looped = true,
      initial_pitch = 48, level = 0.8, pitch_pattern = "melody", gate_pattern = "melody" },
  },
}
