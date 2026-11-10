-- A reusable event-bridge example: Rust sequences and renders the voice while
-- Lua observes only the events explicitly requested by this screen.
local instrument = meldritch.instrument("Event Plaits")
local sequence = instrument:sequencer {
  bpm = 112,
  steps = 16,
  steps_per_beat = 4,
}
local notes = sequence:track("Plaits")
local pattern = sequence:pattern("Pulse")
pattern:notes("Plaits", {
  { step = 1, note = 48, velocity = 1.0, gate = 0.7 },
  { step = 5, note = 55, velocity = 0.8, gate = 0.5 },
  { step = 9, note = 60, velocity = 0.9, gate = 0.7 },
  { step = 13, note = 58, velocity = 0.7, gate = 0.5 },
})

local voice = instrument:plaits {
  model = "harmonic",
  harmonics = 0.62,
  timbre = 0.48,
  morph = 0.28,
}
local envelope = instrument:adsr {
  attack = 0.005,
  decay = 0.18,
  sustain = 0.55,
  release = 0.25,
}
local amp = instrument:vca {}
instrument:connect(notes.pitch, voice.pitch)
instrument:connect(notes.gate, voice.gate)
instrument:connect(notes.velocity, voice.velocity)
instrument:connect(notes.gate, envelope.gate)
instrument:connect(voice.main, amp.input)
instrument:connect(envelope.out, amp.cv)
instrument:connect(amp.out, instrument.output.left)
instrument:connect(voice.aux, instrument.output.right)

instrument:control {
  id = "timbre",
  label = "Timbre",
  target = voice.timbre,
  range = { 0, 1 },
}

local events = meldritch.event_ui()
local screen = instrument:screen("activity", "Event Plaits")
screen:event_activity("Sequenced voice", events)
screen:on(sequence, "step_advanced", function(event)
  events:step(event.step, event.pattern, event.position)
end)
screen:on(sequence, "note_on", function(event)
  events:trigger(event.track, event.pitch, event.velocity)
end)
screen:on_mount(function()
  local state = sequence:state()
  events:step(state.step, state.pattern, state.position)
end)

-- Persistent performance logic uses the same API. Two subscribers initially
-- share one Rust publication; removing one leaves the other subscription live.
local observer = sequence:on("note_on", function(_event) end)
local removable = sequence:on("note_on", function(_event) end)
removable:disconnect()

return instrument
