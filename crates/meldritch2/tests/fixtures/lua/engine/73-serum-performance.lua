-- Native Linux Serum 2 multi-performer example. Serum's stable VST3 class ID
-- keeps discovery independent of its display name.
local instrument = meldritch.instrument("Serum 2 Performance")
local SERUM = "56534558667350736572756D20320000"

local sequence = instrument:sequencer { bpm=116, steps=16, steps_per_beat=4 }
local kick_track = sequence:track("Kick")
local serum_track = sequence:note_track("Serum Lead", { voices=4 })

local pulse = sequence:pattern("Pulse")
pulse:steps("Kick", {1,0,0,0, 1,0,0,0, 1,0,0,0, 1,0,0,0})
pulse:notes(serum_track, {
  {id=1,step=1,pitch="C3",velocity=112,duration=3},
  {id=2,step=5,pitch="G3",velocity=98,duration=2},
  {id=3,step=9,pitch="C4",velocity=108,duration=3},
  {id=4,step=13,pitch="D#4",velocity=102,duration=2},
}, 16)
local bloom = sequence:pattern("Bloom")
bloom:steps("Kick", {1,0,1,0, 1,0,0,1, 1,0,1,0, 1,1,0,1})
bloom:notes(serum_track, {
  {id=1,step=1,pitch="C4",velocity=104,duration=6},
  {id=2,step=1,pitch="D#4",velocity=94,duration=6},
  {id=3,step=1,pitch="G4",velocity=90,duration=6},
  {id=4,step=9,pitch="F4",velocity=106,duration=6},
  {id=5,step=9,pitch="G#4",velocity=94,duration=6},
  {id=6,step=9,pitch="C5",velocity=88,duration=6},
}, 16)
sequence:switch_pattern("Pulse")

-- These are deterministic, preloaded performance patches made from Serum's
-- enumerated normalized VST3 parameters. They do not read proprietary preset
-- files and each owns an independent Serum instance and state.
local bank = instrument:vst3_bank {
  id="serum_patch", plugin=SERUM,
  presets={
    {name="Clean Pulse", parameters={
      [0]=0.52, [1000001]=0.72, [1000039]=0.18,
      [2000000]=1.0, [2000001]=1.0, [2000003]=0.56, [2000004]=0.18, [2000005]=0.08,
    }},
    {name="Glass Motion", parameters={
      [0]=0.48, [1000001]=0.68, [1000039]=0.72,
      [2000000]=1.0, [2000001]=1.0, [2000003]=0.71, [2000004]=0.34, [2000005]=0.21,
    }},
    {name="Driven Bloom", parameters={
      [0]=0.44, [1000001]=0.78, [1000039]=0.43,
      [2000000]=1.0, [2000001]=1.0, [2000003]=0.43, [2000004]=0.48, [2000005]=0.52,
    }},
  },
}
bank:connect_track(serum_track, sequence)

local kick = instrument:oscillator { waveform="sine", frequency=52.0 }
local kick_env = instrument:adsr { attack=0.001, decay=0.13, sustain=0.0, release=0.04 }
local kick_amp = instrument:vca {}
instrument:connect(kick_track.gate, kick_env.gate)
instrument:connect(kick.out, kick_amp.input)
instrument:connect(kick_env.out, kick_amp.cv)

local mix = instrument:mixer { inputs=8, headroom=0.32 }
instrument:connect(bank.out, mix.input[1])
instrument:connect(kick_amp.out, mix.input[2].left)
instrument:connect(kick_amp.out, mix.input[2].right)
instrument:connect(mix.left, instrument.output.left)
instrument:connect(mix.right, instrument.output.right)

instrument:control { id="serum_preset", label="Serum Patch", target=bank.preset, range={0,2} }
local function serum_control(id, label, parameter_id)
  local first, targets = bank:parameter_targets(parameter_id)
  instrument:control { id=id, label=label, target=first, targets=targets, range={0,1} }
end
serum_control("serum_level", "Main Level", 0)
serum_control("serum_filter_cutoff", "Filter Cutoff", 2000003)
serum_control("serum_filter_resonance", "Filter Resonance", 2000004)
serum_control("serum_filter_drive", "Filter Drive", 2000005)
serum_control("serum_a_position", "Oscillator A Position", 1000039)
serum_control("serum_b_position", "Oscillator B Position", 1001039)
serum_control("serum_macro_1", "Macro 1", 7000000)
serum_control("serum_macro_2", "Macro 2", 7001000)

local server = meldritch.server { host="127.0.0.1", port=8080 }
server:websocket("/performance", {})
local performance = server:performance { name="Serum 2 Performance" }
performance:role("drummer", {controls={}})
performance:role("serum", {controls={"serum_preset","serum_level","serum_filter_cutoff","serum_filter_resonance","serum_filter_drive","serum_a_position","serum_b_position","serum_macro_1","serum_macro_2"}})
performance:role("leader", {controls="*"})
performance:role("observer", {controls={}})
server:credential("drummer-demo-token", "drummer")
server:credential("serum-demo-token", "serum")
server:credential("leader-demo-token", "leader")
server:credential("observer-demo-token", "observer")

local rhythm = performance:plane { id="rhythm", label="Drums", roles={"drummer","leader"}, read_only_roles={"observer"} }
rhythm:step_sequencer { id="kick", label="Kick", track=kick_track, pattern=pulse, steps=16 }
rhythm:trigger { id="fill", label="FILL", triggered=function() sequence:switch_pattern("Bloom", "beat") end }

local serum = performance:plane { id="serum", label="Serum 2", roles={"serum","leader"}, read_only_roles={"observer"} }
serum:piano_roll { id="serum_notes", label="Serum Piano Roll", track=serum_track, steps=16, pitch_min=36, pitch_max=84 }
serum:pattern_selector { id="serum_patterns", label="Patterns", sequencer=sequence, boundary="bar" }
serum:preset_selector { id="serum_preset", label="Performance Patch", target="serum_preset", presets=bank.preset_names }
serum:fader { id="serum_level", label="Main Level", target="serum_level", min=0, max=1 }
serum:fader { id="serum_filter_cutoff", label="Filter Cutoff", target="serum_filter_cutoff", min=0, max=1, unit="normalized" }
serum:knob { id="serum_filter_resonance", label="Filter Resonance", target="serum_filter_resonance", min=0, max=1 }
serum:knob { id="serum_filter_drive", label="Filter Drive", target="serum_filter_drive", min=0, max=1 }
serum:knob { id="serum_a_position", label="Osc A WT Position", target="serum_a_position", min=0, max=1 }
serum:knob { id="serum_b_position", label="Osc B WT Position", target="serum_b_position", min=0, max=1 }
serum:knob { id="serum_macro_1", label="Macro 1", target="serum_macro_1", min=0, max=1 }
serum:knob { id="serum_macro_2", label="Macro 2", target="serum_macro_2", min=0, max=1 }

local leader = performance:plane { id="leader", label="Transport", roles={"leader"}, read_only_roles={"observer"} }
leader:trigger { id="restart", label="RESTART", triggered=function() sequence:restart() end }
leader:trigger { id="stop", label="STOP", triggered=function() sequence:stop() end }

return instrument
