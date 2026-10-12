-- Invalid: a runtime overlay cannot silently become an authored default.
return {
  kind = "project", version = 2,
  project = { id = "project", title = "Untitled project", bpm = 120, sample_rate = 48000 },
  patches = {}, patterns = {}, voices = {},
  output_levels = { ["voice-1-instance/level"] = 0 },
}
