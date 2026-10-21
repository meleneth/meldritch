local instrument = meldritch.instrument("MIDI button hooks")

local sequence = instrument:sequencer {
  bpm = 120,
  steps = 8,
  steps_per_beat = 4,
}
local trigger = sequence:track("pulse")
local tone = instrument:oscillator { waveform = "sine", frequency = 220 }
local amp = instrument:vca {}
instrument:connect(tone.out, amp.input)
instrument:connect(trigger.out, amp.cv)
instrument:connect(amp.out, instrument.output.left)
instrument:connect(amp.out, instrument.output.right)
sequence:steps("pulse", { 1,0,0,0, 1,0,0,0 })

-- Controller profiles remain authored data. A button may arrive as a note, CC,
-- or Program Change; press and release edges can have different behavior.
instrument:on_midi_button({
  name = "Stop from note", message = "note", channel = 10, number = 36,
}, function()
  sequence:stop()
end)

instrument:on_midi_button({
  name = "Start from CC release", message = "cc", channel = 1, number = 64,
  edge = "release",
}, function()
  sequence:start()
end)

instrument:on_midi_button({
  name = "Restart from program", message = "program", channel = 1, number = 3,
}, function()
  sequence:restart()
end)

-- Replace these deliberately synthetic bytes with one complete packet copied
-- from `meldritch2 midi-sniff`; exact matching also covers SysEx messages.
instrument:on_midi_packet({
  name = "Vendor packet stop",
  bytes = { 0xf0, 0x7d, 0x4d, 0x32, 0x01, 0xf7 },
}, function()
  sequence:stop()
end)

return instrument
