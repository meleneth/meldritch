-- MELDRITCH authored pattern; musical times use 960 PPQ.
return {
  kind = "pattern",
  version = 1,
  base_fingerprint = "816e0db35ff5b1a7",
  pattern = {
    id = "roll", track = "drums",
    length_ticks = 3840, looped = true, next_note_id = 11,
    notes = {
      { id = "note-1", note = "C2", start_tick = 0, duration_ticks = 240, velocity = 0.8, slot = "kick" },
      { id = "note-2", note = "D2", start_tick = 0, duration_ticks = 240, velocity = 0.9, slot = "snare" },
      { id = "note-4", note = "F#2", start_tick = 0, duration_ticks = 240, velocity = 0.8, slot = "closed-hat" },
      { id = "note-5", note = "A#2", start_tick = 0, duration_ticks = 240, velocity = 0.8, slot = "open-hat" },
      { id = "note-6", note = "A2", start_tick = 0, duration_ticks = 240, velocity = 0.8, slot = "low-tom" },
      { id = "note-7", note = "D3", start_tick = 0, duration_ticks = 240, velocity = 0.8, slot = "high-tom" },
      { id = "note-8", note = "D#3", start_tick = 0, duration_ticks = 240, velocity = 0.8, slot = "ride" },
      { id = "note-9", note = "D#2", start_tick = 0, duration_ticks = 240, velocity = 0.8, slot = "clap" },
      { id = "note-10", note = "C2", start_tick = 1920, duration_ticks = 240, velocity = 0.8, slot = "kick" },
    },
  },
}
