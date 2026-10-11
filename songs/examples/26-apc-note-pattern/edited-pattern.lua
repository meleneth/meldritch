-- MELDRITCH authored pattern; musical times use 960 PPQ.
return {
  kind = "pattern",
  version = 1,
  base_fingerprint = "5295a12b3ad372d1",
  pattern = {
    id = "roll", track = "bass",
    length_ticks = 7680, looped = true, next_note_id = 4,
    notes = {
      { id = "note-1", note = "C3", start_tick = 0, duration_ticks = 4800, velocity = 0.8 },
      { id = "note-2", note = "C#3", start_tick = 480, duration_ticks = 480, velocity = 0.9 },
    },
  },
}
