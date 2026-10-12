-- First audible graph-LFO checkpoint; all modulation cables are explicit.
return {
  kind = "project", version = 2,
  project = { id = "patch-lfo", title = "Explicit tremolo", bpm = 120, sample_rate = 48000 },
  patches = {
    { id = "bass", polyphony = 1, output_level_control = "level",
      inputs = { { id = "pitch", signal = "pitch" }, { id = "gate", signal = "gate" } },
      modules = {
        { id = "osc", kind = "oscillator", waveform = "sine" },
        { id = "env", kind = "adsr", attack = 0, decay = 0, sustain = 1, release = 0 },
        { id = "amp", kind = "vca" },
        { id = "out", kind = "audio_output", channels = 1 },
        { id = "slow", kind = "lfo", lfo = { shape = "sine", rate = { kind = "beats", value = 1 }, phase = 0,
          evaluation = "audio", scope = "per_instance", reset = "none" } },
        { id = "unipolar", kind = "control_scale", scale = { gain = 0.5, offset = 0.5, clamp_min = 0, clamp_max = 1 } },
        { id = "shape", kind = "control_multiply" },
      },
      cables = {
        { from = "input.pitch", to = "osc.pitch", signal = "pitch" },
        { from = "input.gate", to = "env.gate", signal = "gate" },
        { from = "osc.audio", to = "amp.audio", signal = "audio" },
        { from = "amp.audio", to = "out.audio", signal = "audio" },
        { from = "slow.control", to = "unipolar.control", signal = "control" },
        { from = "env.control", to = "shape.left", signal = "control" },
        { from = "unipolar.control", to = "shape.right", signal = "control" },
        { from = "shape.control", to = "amp.level", signal = "control" },
      },
    },
  },
  patterns = {
    { id = "melody", length_ticks = 3840, looped = true, next_note_id = 2,
      notes = { { id = "note-1", note = "C3", start_tick = 0, duration_ticks = 3840, velocity = 0.8 } } },
    { id = "rhythm", length_ticks = 3840, looped = true, next_note_id = 3,
      notes = {
        { id = "note-1", note = "C6", start_tick = 0, duration_ticks = 1440, velocity = 0.1 },
        { id = "note-2", note = "C6", start_tick = 1440, duration_ticks = 1920, velocity = 0.1 },
      } },
  },
  voices = {
    { id = "bass-instance", synth = "bass", length_ticks = 3840, looped = true,
      initial_pitch = 48, level = 0.8, pitch_pattern = "melody", gate_pattern = "rhythm" },
  },
}
