-- Geonkick 3.9 + Serum 2 multi-performer rhythm section.
-- Override GEONKICK_VST3 and GEONKICK_KIT for non-default installations.
local instrument = meldritch.instrument("Geonkick Rhythm Section")
local home = assert(os.getenv("HOME"), "HOME is required")
local geonkick_path = os.getenv("GEONKICK_VST3") or (home .. "/.local/lib/vst3")
local geonkick_kit = os.getenv("GEONKICK_KIT") or (home .. "/.local/share/geonkick/presets/TimeKit/TimeKit.gkit")

local sequence = instrument:sequencer { bpm=124, steps=16, steps_per_beat=4 }
local kit = sequence:drum_kit {
  name="TimeKit",
  voices={
    {id="kick",label="Kick",note=24},
    {id="snare",label="Snare",note=26},
    {id="clap",label="Clap",note=27},
    {id="closed_hat",label="Closed Hat",note=30},
    {id="hat",label="Hat",note=32},
    {id="open_hat",label="Open Hat",note=34},
    {id="shaker",label="Shaker",note=58},
  },
}
local serum_track = sequence:note_track("Serum Bass", {voices=4})

local function groove(pattern, fill)
  local kicks={{step=1,velocity=127},{step=5,velocity=112},{step=9,velocity=124},{step=13,velocity=116}}
  local snares={{step=5,velocity=116},{step=13,velocity=124}}
  if fill then
    table.insert(kicks,{step=16,velocity=92})
    table.insert(snares,{step=16,velocity=105})
  end
  pattern:drums(kit, {
    kick=kicks,
    snare=snares,
    clap={{step=5,velocity=74},{step=13,velocity=88}},
    closed_hat={{step=1,velocity=76},{step=3,velocity=58},{step=5,velocity=82},{step=7,velocity=62},{step=9,velocity=78},{step=11,velocity=57},{step=13,velocity=86},{step=15,velocity=65}},
    open_hat={{step=7,velocity=74},{step=15,velocity=82}},
    shaker={{step=4,velocity=48},{step=8,velocity=55},{step=12,velocity=50},{step=16,velocity=66}},
  },16)
end
local warehouse = sequence:pattern("Warehouse")
groove(warehouse,false)
warehouse:notes(serum_track, {
  {id=1,step=1,pitch="C2",velocity=112,duration=3},
  {id=2,step=5,pitch="C2",velocity=96,duration=2},
  {id=3,step=9,pitch="D#2",velocity=108,duration=3},
  {id=4,step=13,pitch="G2",velocity=104,duration=2},
},16)
local pressure = sequence:pattern("Pressure Fill")
groove(pressure,true)
pressure:notes(serum_track, {
  {id=1,step=1,pitch="F2",velocity=116,duration=3},
  {id=2,step=4,pitch="G2",velocity=102,duration=2},
  {id=3,step=7,pitch="G#2",velocity=110,duration=3},
  {id=4,step=11,pitch="C3",velocity=120,duration=2},
  {id=5,step=14,pitch="G2",velocity=106,duration=2},
},16)
sequence:switch_pattern("Warehouse")

local drums = instrument:vst3 {
  plugin="E2CC953290E863F06405E13924E34B24",
  paths={geonkick_path}, state_file=geonkick_kit, activate_all_outputs=true,
}
drums:connect_drum_kit(kit)

local serum = instrument:vst3 { plugin="56534558667350736572756D20320000" }
serum:connect_track(serum_track,sequence)
serum:set_parameter(0,0.46)
serum:set_parameter(2000003,0.48)
serum:set_parameter(2000004,0.27)
instrument:control {id="serum_cutoff",label="Serum Cutoff",target=serum:parameter(2000003),range={0,1}}
instrument:control {id="serum_resonance",label="Serum Resonance",target=serum:parameter(2000004),range={0,1}}

local mix = instrument:mixer {inputs=8,headroom=0.58}
instrument:connect(drums.out,mix.input[1])
instrument:connect(serum.out,mix.input[2])
instrument:connect(mix.left,instrument.output.left)
instrument:connect(mix.right,instrument.output.right)

local server = meldritch.server {host="127.0.0.1",port=8080}
server:websocket("/performance",{})
local performance = server:performance {name="Geonkick Rhythm Section"}
performance:role("drummer",{controls={}})
performance:role("serum",{controls={"serum_cutoff","serum_resonance"}})
performance:role("leader",{controls="*"})
performance:role("observer",{controls={}})
server:credential("drummer-demo-token","drummer")
server:credential("serum-demo-token","serum")
server:credential("leader-demo-token","leader")
server:credential("observer-demo-token","observer")

local rhythm = performance:plane {id="rhythm",label="Geonkick TimeKit",roles={"drummer","leader"},read_only_roles={"observer"}}
rhythm:drum_grid {id="geonkick_grid",label="Drum Pattern",kit=kit,steps=16}
rhythm:pattern_selector {id="drum_patterns",label="Patterns",sequencer=sequence,boundary="bar"}
rhythm:trigger {id="fill",label="FILL",triggered=function() sequence:switch_pattern("Pressure Fill","beat") end}

local synth = performance:plane {id="serum",label="Serum Bass",roles={"serum","leader"},read_only_roles={"observer"}}
synth:piano_roll {id="serum_notes",label="Bass Notes",track=serum_track,steps=16,pitch_min=24,pitch_max=60}
synth:knob {id="serum_cutoff",label="Cutoff",target="serum_cutoff",min=0,max=1}
synth:knob {id="serum_resonance",label="Resonance",target="serum_resonance",min=0,max=1}

local leader = performance:plane {id="leader",label="Arrangement",roles={"leader"},read_only_roles={"observer"}}
leader:pattern_selector {id="arrangement",label="Pattern",sequencer=sequence,boundary="bar"}
leader:trigger {id="restart",label="RESTART",triggered=function() sequence:restart() end}
leader:trigger {id="stop",label="STOP",triggered=function() sequence:stop() end}
return instrument
