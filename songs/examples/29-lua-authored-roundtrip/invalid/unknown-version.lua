-- MELDRITCH composed authored music; journals and TOML stay unchanged.
return {
  kind = "project", version = 99,
  base_fingerprint = "54745c74a42665cf",
  instrument = {
    kind = "instrument",
    version = 2,
    base_fingerprint = "54745c74a42665cf",
    instrument = "bass",
    graph = {
        cables = {
          {
            from = "input.gate",
            signal = "gate",
            to = "env.gate",
          },
          {
            from = "env.control",
            signal = "control",
            to = "amp.level",
          },
          {
            from = "amp.audio",
            signal = "audio",
            to = "filter.audio",
          },
          {
            from = "filter.audio",
            signal = "audio",
            to = "out.audio",
          },
          {
            from = "input.pitch",
            signal = "pitch",
            to = "source.pitch",
          },
          {
            from = "source.audio",
            signal = "audio",
            to = "amp.audio",
          },
        },
        id = "bass",
        inputs = {
          {
            id = "pitch",
            signal = "pitch",
          },
          {
            id = "gate",
            signal = "gate",
          },
        },
        modules = {
          {
            attack = 0.004,
            decay = 0.16,
            id = "env",
            kind = "adsr",
            release = 0.09999999999999999,
            sustain = 0.55,
          },
          {
            id = "amp",
            kind = "vca",
          },
          {
            cutoff_hz = 6000.0,
            id = "filter",
            kind = "low_pass",
            resonance = 0.25,
          },
          {
            channels = 1,
            id = "out",
            kind = "audio_output",
          },
          {
            id = "source",
            kind = "oscillator",
            waveform = "triangle",
          },
        },
        polyphony = 8,
      },
  },
  pattern = {
    kind = "pattern",
    version = 1,
    base_fingerprint = "54745c74a42665cf",
    pattern = {
      id = "roll", track = "bass",
      length_ticks = 7680, looped = true, next_note_id = 5,
      notes = {
        { id = "note-1", note = "C3", start_tick = 0, duration_ticks = 1920, velocity = 0.8 },
        { id = "note-2", note = "E3", start_tick = 1920, duration_ticks = 480, velocity = 0.8 },
        { id = "note-3", note = "G3", start_tick = 3840, duration_ticks = 480, velocity = 0.8 },
        { id = "note-4", note = "C3", start_tick = 4800, duration_ticks = 1440, velocity = 0.8 },
      },
    },
  },
  controls = {
    kind = "controls", version = 1,
    base_fingerprint = "54745c74a42665cf",
    assignments = {
      pages = { "main" },
      targets = {
        { id = "cutoff", label = "Filter cutoff", target = { kind = "synth", id = "bass", module = "filter", parameter = "cutoff_hz" }, minimum = 80.0, maximum = 6000.0, step = 10.0 },
        { id = "volume", label = "Track volume", target = { kind = "track", id = "bass", parameter = "volume" }, minimum = 0.0, maximum = 2.0, step = 0.01 },
        { id = "pan", label = "Track pan", target = { kind = "track", id = "bass", parameter = "pan" }, minimum = -1.0, maximum = 1.0, step = 0.01 },
      },
      faders = {
        { id = "fader-1", device = "apc-mini", channel = 1, cc = 48, page = "main", target = "cutoff" },
        { id = "fader-2", device = "apc-mini", channel = 1, cc = 49, page = "main", target = "volume" },
        { id = "fader-3", device = "apc-mini", channel = 1, cc = 50, page = "main", target = "pan" },
      },
    },
    values = {
      { target = { kind = "synth", id = "bass", module = "filter", parameter = "cutoff_hz" }, value = 6000.0 },
      { target = { kind = "track", id = "bass", parameter = "volume" }, value = 0.5 },
      { target = { kind = "track", id = "bass", parameter = "pan" }, value = 1.0 },
    },
  },
}
