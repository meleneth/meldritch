-- Must reject unknown membership; never open this as an empty project.
return {
  kind = "project", version = 3,
  project = { id = "invalid", title = "Missing member", bpm = 120, sample_rate = 48000 },
  patches = {}, patterns = {}, voices = {},
  compositions = { { id = "held", voices = { "missing" } } },
  selected_composition = "held",
}
