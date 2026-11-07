local instrument = meldritch.instrument("Note Performance")

local sequence = instrument:sequencer { bpm = 112, steps = 16, steps_per_beat = 4 }
local kick_track = sequence:track("Kick")
local bass_track = sequence:note_track("Bass", { voices = 3 })
local lead_track = sequence:note_track("Lead", { voices = 4 })

local groove = sequence:pattern("Foundation")
groove:steps("Kick", { 1,0,0,0, 1,0,0,0, 1,0,0,0, 1,0,0,0 })
groove:notes(bass_track, {
  { id=1, step=1, pitch="C2", velocity=116, duration=3 },
  { id=2, step=5, pitch="C2", velocity=102, duration=2 },
  { id=3, step=9, pitch="E2", velocity=108, duration=3 },
  { id=4, step=13, pitch="G2", velocity=112, duration=2 },
}, 16)
groove:notes(lead_track, {
  { id=1, step=1, pitch="C4", velocity=92, duration=6 },
  { id=2, step=1, pitch="E4", velocity=86, duration=6 },
  { id=3, step=1, pitch="G4", velocity=82, duration=6 },
  { id=4, step=9, pitch="D4", velocity=90, duration=6 },
  { id=5, step=9, pitch="F4", velocity=84, duration=6 },
  { id=6, step=9, pitch="A4", velocity=80, duration=6 },
}, 16)

local lift = sequence:pattern("Lift")
lift:steps("Kick", { 1,0,1,0, 1,0,1,1, 1,0,1,0, 1,1,1,1 })
lift:notes(bass_track, {
  { id=1, step=1, pitch="F2", velocity=120, duration=2 },
  { id=2, step=4, pitch="G2", velocity=100, duration=2 },
  { id=3, step=7, pitch="A2", velocity=112, duration=3 },
  { id=4, step=11, pitch="C3", velocity=118, duration=2 },
  { id=5, step=14, pitch="G2", velocity=106, duration=2 },
}, 16)
lift:notes(lead_track, {
  { id=1, step=1, pitch="F4", velocity=94, duration=4 },
  { id=2, step=1, pitch="A4", velocity=88, duration=4 },
  { id=3, step=1, pitch="C5", velocity=84, duration=4 },
  { id=4, step=7, pitch="G4", velocity=96, duration=4 },
  { id=5, step=7, pitch="B4", velocity=88, duration=4 },
  { id=6, step=7, pitch="D5", velocity=82, duration=4 },
  { id=7, step=13, pitch="A4", velocity=102, duration=3 },
}, 16)
sequence:switch_pattern("Foundation")

local kick = instrument:oscillator { waveform = "sine", frequency = 52.0 }
local kick_env = instrument:adsr { attack = 0.001, decay = 0.12, sustain = 0.0, release = 0.04 }
local kick_amp = instrument:vca {}
instrument:connect(kick_track.gate, kick_env.gate)
instrument:connect(kick.out, kick_amp.input)
instrument:connect(kick_env.out, kick_amp.cv)

local amp_envelopes, filter_envelopes, filters, sounds = {}, {}, {}, {}
local function voice(track, model, index)
  local synth = instrument:plaits { model = model, harmonics = 0.35, timbre = 0.48, morph = 0.32 }
  local filter = instrument:svf { cutoff = 1050.0, resonance = 0.28 }
  local amp_env = instrument:adsr { attack = 0.015, decay = 0.18, sustain = 0.68, release = 0.32 }
  local filter_env = instrument:adsr { attack = 0.01, decay = 0.24, sustain = 0.18, release = 0.3 }
  local amp = instrument:vca {}
  instrument:connect(track.voices[index].pitch, synth.pitch)
  instrument:connect(track.voices[index].gate, synth.gate)
  instrument:connect(track.voices[index].velocity, synth.velocity)
  instrument:connect(track.voices[index].gate, amp_env.gate)
  instrument:connect(track.voices[index].gate, filter_env.gate)
  instrument:connect(synth.out, filter.input)
  filter:modulate("cutoff", filter_env.out, { depth = 2800.0 })
  instrument:connect(filter.lowpass, amp.input)
  instrument:connect(amp_env.out, amp.cv)
  table.insert(amp_envelopes, amp_env)
  table.insert(filter_envelopes, filter_env)
  table.insert(filters, filter)
  table.insert(sounds, amp.out)
end
for i=1,3 do voice(bass_track, "fm", i) end
for i=1,4 do voice(lead_track, "wavetable", i) end
table.insert(sounds, kick_amp.out)

local function pair(left, right)
  local mix = instrument:mixer { inputs = 2 }
  instrument:connect(left, mix.input[1]); instrument:connect(right, mix.input[2])
  return mix.out
end
while #sounds > 1 do
  local next = {}
  for i=1,#sounds,2 do table.insert(next, sounds[i+1] and pair(sounds[i], sounds[i+1]) or sounds[i]) end
  sounds = next
end
local master = instrument:gain { gain = 0.12 }
instrument:connect(sounds[1], master.input)
instrument:connect(master.out, instrument.output.left)
instrument:connect(master.out, instrument.output.right)

local function shared_control(id, label, field, modules, range, curve)
  local targets = {}; for i=2,#modules do table.insert(targets, modules[i][field]) end
  instrument:control { id=id, label=label, target=modules[1][field], targets=targets, range=range, curve=curve }
end
shared_control("amp_attack", "Amp Attack", "attack", amp_envelopes, {0.001, 5.0}, "log")
shared_control("amp_decay", "Amp Decay", "decay", amp_envelopes, {0.001, 5.0}, "log")
shared_control("amp_sustain", "Amp Sustain", "sustain", amp_envelopes, {0.0, 1.0})
shared_control("amp_release", "Amp Release", "release", amp_envelopes, {0.001, 10.0}, "log")
shared_control("filter_attack", "Filter Attack", "attack", filter_envelopes, {0.001, 5.0}, "log")
shared_control("filter_decay", "Filter Decay", "decay", filter_envelopes, {0.001, 5.0}, "log")
shared_control("filter_sustain", "Filter Sustain", "sustain", filter_envelopes, {0.0, 1.0})
shared_control("filter_release", "Filter Release", "release", filter_envelopes, {0.001, 10.0}, "log")
shared_control("filter_cutoff", "Cutoff", "cutoff", filters, {80.0, 8000.0}, "log")
shared_control("filter_resonance", "Resonance", "resonance", filters, {0.0, 0.92})

local server = meldritch.server { host="127.0.0.1", port=8080 }
server:websocket("/performance", {})
local performance = server:performance { name="Note and Envelope Performance" }
performance:role("drummer", { controls={} })
performance:role("synth", { controls={"filter_cutoff","filter_resonance"} })
performance:role("leader", { controls="*" })
performance:role("observer", { controls={} })
server:credential("drummer-demo-token", "drummer")
server:credential("synth-demo-token", "synth")
server:credential("leader-demo-token", "leader")
server:credential("observer-demo-token", "observer")

local rhythm = performance:plane { id="rhythm", label="Rhythm and Bass", roles={"drummer","leader"}, read_only_roles={"observer"} }
rhythm:step_sequencer { id="kick", label="Kick", track=kick_track, pattern=groove, steps=16 }
rhythm:piano_roll { id="bass_notes", label="Bass Notes", track=bass_track, steps=16, pitch_min=36, pitch_max=60 }
rhythm:pattern_selector { id="bass_patterns", label="Patterns", sequencer=sequence, boundary="bar" }
rhythm:trigger { id="fill", label="FILL", triggered=function() sequence:switch_pattern("Lift", "beat") end }

local synthesis = performance:plane { id="synthesis", label="Synthesis", roles={"synth","leader"}, read_only_roles={"observer"} }
synthesis:piano_roll { id="lead_notes", label="Chord Notes", track=lead_track, steps=16, pitch_min=48, pitch_max=74 }
synthesis:adsr { id="amp_envelope", label="Amplitude ADSR", attack="amp_attack", decay="amp_decay", sustain="amp_sustain", release="amp_release" }
synthesis:adsr { id="filter_envelope", label="Filter ADSR", attack="filter_attack", decay="filter_decay", sustain="filter_sustain", release="filter_release" }
synthesis:fader { id="filter_cutoff", label="Cutoff", target="filter_cutoff", min=80.0, max=8000.0, scale="log", unit="Hz" }
synthesis:knob { id="filter_resonance", label="Resonance", target="filter_resonance", min=0.0, max=0.92 }
synthesis:pattern_selector { id="lead_patterns", label="Patterns", sequencer=sequence, boundary="bar" }

return instrument
