-- Pressure System
-- 208 bars / 120 BPM / 6:56. Sample-driven psychedelic big beat.
--
-- Acoustic drums: AVL Drumkits, Glen MacArthur, CC BY-SA 3.0.
-- https://github.com/studiorack/avl-drumkits/tree/b06cb2c27359cbc68a830a82bddced05bd73d1a0
-- Found sound: Oculus Audio Pack 1, Meta Creative Audio, CC BY 4.0.
-- https://github.com/facebookincubator/Meta-open-sound/tree/251c0be57d5d1b3242b91f53e0cde62ab1cb42d9/corpora/oculus-audio-pack
-- Required credit: "Oculus Audio Pack 01" by Oculus VR, LLC
-- (https://developers.meta.com/vr/downloads/package/oculus-audio-pack-1/),
-- licensed under CC BY 4.0. OGG previews were transcoded from original WAVs.

local instrument=meldritch.instrument("Pressure System")
instrument:voice_mode("mono")
instrument:devices {midi_input="MicroLab mk3:MicroLab mk3",
  midi_controls={"MicroLab mk3:MicroLab mk3"},
  apc_mini="APC mini mk2:APC mini mk2 Control"}

local sequence=instrument:sequencer {bpm=120,steps=128,steps_per_beat=4}
local names={"Break Kick","Break Snare","Ghost Snare","Closed Hat","Open Hat","Floor Tom",
  "Electro Kick","Clap","Hydraulic","Impact","Bass","Low Drone","Hook","Machine Room",
  "Industrial Drone","Breath Forward","Breath Reverse"}
local track={}
for _,name in ipairs(names) do track[name]=sequence:track(name) end

local function hits(length,events)
  local out={};for i=1,length do out[i]=0 end
  for _,event in ipairs(events) do out[event[1]]=event[2] end
  return out
end
local function repeat_hits(length,period,events)
  local out={};for i=1,length do out[i]=0 end
  for base=0,length-1,period do
    for _,event in ipairs(events) do if base+event[1]<=length then out[base+event[1]]=event[2] end end
  end
  return out
end
local function drum(track_name,name,length,events)
  return track[track_name]:clip(name,{length=length,steps=hits(length,events)})
end
local function repeated(track_name,name,length,period,events)
  return track[track_name]:clip(name,{length=length,steps=repeat_hits(length,period,events)})
end
local function notes(track_name,name,length,events)
  return track[track_name]:clip(name,{length=length,notes=events})
end

-- The break is assembled from individually recorded acoustic kit pieces. Its
-- 32/64/128-step cycles deliberately disagree with the electronic layers.
local kick_hint=drum("Break Kick","Kick: distant",64,{{1,.48},{19,.28},{33,.54},{55,.32}})
local kick_a=repeated("Break Kick","Kick: body",32,16,{{1,1},{7,.68},{11,.34},{15,.82}})
local kick_b=drum("Break Kick","Kick: shove",64,{{1,1},{7,.72},{11,.38},{15,.86},{17,.96},{23,.58},{27,.82},{31,.46},{33,1},{39,.74},{43,.42},{47,.88},{49,.98},{54,.48},{57,.76},{63,.68}})
local kick_half=repeated("Break Kick","Kick: half time",32,16,{{1,1},{11,.42}})
local kick_release=drum("Break Kick","Kick: release",128,{{1,1},{7,.7},{15,.84},{17,.98},{23,.62},{31,.78},{33,1},{39,.72},{47,.86},{49,.96},{55,.56},{63,.8},{65,1},{71,.7},{79,.84},{81,.9},{95,.74},{97,.84},{111,.66},{113,.72},{127,.52}})
local kick_bleed=drum("Break Kick","Kick: bleed off",128,{{1,1},{15,.82},{17,.94},{31,.76},{33,.88},{47,.7},{49,.8},{63,.62},{65,.72},{81,.58},{97,.45}})

local snare_hint=drum("Break Snare","Snare: shadows",64,{{13,.32},{29,.44},{45,.28},{61,.52}})
local snare_a=repeated("Break Snare","Snare: concrete",32,16,{{5,.88},{13,1},{16,.28}})
local snare_b=drum("Break Snare","Snare: displaced",64,{{5,.92},{13,1},{21,.82},{28,.34},{29,1},{37,.94},{45,1},{52,.28},{53,.86},{60,.42},{61,1},{64,.58}})
local snare_half=repeated("Break Snare","Snare: half time",32,16,{{13,1}})
local snare_fill=drum("Break Snare","Snare: rupture",64,{{5,.9},{13,1},{21,.84},{29,1},{37,.92},{45,1},{51,.28},{53,.72},{57,.46},{59,.58},{61,.72},{62,.8},{63,.9},{64,1}})
local snare_bleed=drum("Break Snare","Snare: bleed off",128,{{13,1},{29,.96},{45,.9},{61,.86},{77,.78},{93,.68},{109,.54}})

local ghost_a=drum("Ghost Snare","Ghosts: loose bolts",64,{{4,.18},{10,.24},{16,.16},{20,.3},{26,.2},{32,.34},{36,.2},{42,.28},{48,.18},{52,.34},{58,.24},{63,.42}})
local ghost_dense=repeated("Ghost Snare","Ghosts: ratchet",32,16,{{3,.2},{4,.34},{8,.18},{10,.3},{12,.22},{15,.42},{16,.28}})
local ghost_sparse=drum("Ghost Snare","Ghosts: vacuum",128,{{12,.2},{31,.32},{48,.18},{63,.38},{80,.22},{95,.44},{112,.28},{124,.36},{127,.5}})

local hats_hint=drum("Closed Hat","Hats: pressure ticks",64,{{7,.2},{15,.3},{23,.22},{31,.4},{39,.24},{47,.34},{55,.28},{63,.5}})
local hats_a=repeated("Closed Hat","Hats: piston",32,8,{{1,.5},{3,.34},{5,.62},{7,.4}})
local hats_b=drum("Closed Hat","Hats: skitter",64,{{1,.54},{3,.38},{5,.7},{7,.42},{9,.56},{11,.44},{13,.76},{15,.48},{17,.6},{19,.4},{21,.72},{23,.5},{25,.62},{27,.46},{29,.8},{30,.26},{31,.68},{32,.38},{33,.58},{35,.42},{37,.7},{39,.48},{41,.62},{43,.46},{45,.78},{47,.52},{49,.64},{51,.44},{53,.74},{55,.5},{57,.68},{58,.3},{59,.78},{61,.56},{63,.86},{64,.48}})
local hats_half=repeated("Closed Hat","Hats: half light",32,8,{{1,.42},{5,.58},{7,.28}})
local hats_bleed=drum("Closed Hat","Hats: bleed off",128,{{1,.56},{3,.38},{5,.64},{7,.42},{9,.54},{11,.36},{13,.66},{15,.44},{17,.52},{21,.6},{25,.48},{29,.56},{33,.46},{37,.52},{41,.42},{45,.48},{49,.4},{57,.42},{65,.36},{73,.3},{81,.26},{97,.2}})
local open_a=repeated("Open Hat","Open hats: lift",32,16,{{12,.38},{28,.48}})
local open_b=drum("Open Hat","Open hats: shear",64,{{8,.32},{16,.44},{24,.36},{31,.56},{40,.34},{48,.48},{56,.4},{63,.62}})

local tom_a=drum("Floor Tom","Tom: undercarriage",64,{{11,.42},{15,.64},{27,.48},{31,.72},{43,.54},{47,.7},{58,.46},{61,.62},{63,.82}})
local tom_b=drum("Floor Tom","Tom: broken axle",64,{{4,.42},{7,.62},{12,.48},{15,.78},{20,.4},{23,.68},{28,.5},{31,.84},{36,.48},{39,.72},{44,.54},{47,.88},{52,.5},{55,.76},{59,.62},{61,.74},{63,.9},{64,1}})
local tom_sparse=drum("Floor Tom","Tom: sonar",128,{{1,.56},{33,.42},{65,.62},{97,.48},{121,.7},{125,.82},{127,.94}})

local electro_hint=drum("Electro Kick","Sub kick: omen",64,{{1,.36},{33,.42}})
local electro_a=repeated("Electro Kick","Sub kick: lock",32,16,{{1,.72},{15,.4}})
local electro_heavy=repeated("Electro Kick","Sub kick: overload",32,16,{{1,.82},{7,.34},{15,.5}})
local clap_a=repeated("Clap","Clap: wall",32,16,{{5,.44},{13,.62}})
local clap_sparse=drum("Clap","Clap: distance",64,{{13,.3},{45,.36}})

local hydraulic_sparse=drum("Hydraulic","Hydraulic: warning",128,{{1,.7},{65,.54}})
local hydraulic_rhythm=drum("Hydraulic","Hydraulic: answer",128,{{1,.72},{25,.44},{49,.62},{73,.38},{97,.68},{113,.46}})
local impact_sparse=drum("Impact","Impact: bulkhead",128,{{1,.82},{63,.42},{97,.66},{127,.9}})
local impact_build=drum("Impact","Impact: alarm",128,{{1,.7},{33,.52},{65,.74},{81,.42},{97,.8},{113,.54},{121,.66},{125,.78},{127,1}})

-- A one-bar cell stays recognizable while accent, register, and breathing
-- space change. D is the pressure center; C and Eb are structural abrasions.
local bass_primary=notes("Bass","Bass: main valve",128,{
  {step=1,note=38,velocity=1,gate=2.8},{step=7,note=38,velocity=.62,gate=.7},{step=10,note=45,velocity=.84,gate=1.2},{step=15,note=36,velocity=.76,gate=.7},
  {step=17,note=38,velocity=.96,gate=2.2},{step=23,note=50,velocity=.7,gate=.8},{step=27,note=41,velocity=.82,gate=1.2},{step=31,note=45,velocity=.58,gate=.6},
  {step=33,note=38,velocity=1,gate=3.1},{step=40,note=45,velocity=.76,gate=.8},{step=43,note=36,velocity=.64,gate=.7},{step=47,note=41,velocity=.86,gate=1},
  {step=49,note=34,velocity=.94,gate=2.5},{step=55,note=41,velocity=.72,gate=.8},{step=58,note=45,velocity=.84,gate=1},{step=63,note=36,velocity=.7,gate=.7},
  {step=65,note=38,velocity=1,gate=2.5},{step=71,note=38,velocity=.58,gate=.6},{step=74,note=45,velocity=.86,gate=1.4},{step=79,note=36,velocity=.74,gate=.7},
  {step=81,note=38,velocity=.98,gate=2.2},{step=87,note=50,velocity=.72,gate=.7},{step=91,note=41,velocity=.82,gate=1.1},{step=95,note=45,velocity=.62,gate=.6},
  {step=97,note=41,velocity=.96,gate=2.8},{step=104,note=48,velocity=.7,gate=.8},{step=107,note=38,velocity=.82,gate=.8},{step=111,note=45,velocity=.9,gate=1},
  {step=113,note=36,velocity=1,gate=2.5},{step=119,note=43,velocity=.76,gate=.8},{step=123,note=41,velocity=.84,gate=1},{step=127,note=33,velocity=.72,gate=.7}})
local bass_half=notes("Bass","Bass: suspended mass",128,{{step=1,note=26,velocity=1,gate=7},{step=17,note=26,velocity=.72,gate=3},{step=33,note=29,velocity=.94,gate=7},{step=49,note=24,velocity=.78,gate=4},{step=65,note=26,velocity=1,gate=8},{step=97,note=24,velocity=.9,gate=8}})
local bass_release=notes("Bass","Bass: decompression",128,{
  {step=1,note=26,velocity=1,gate=3},{step=7,note=38,velocity=.72,gate=.7},{step=10,note=45,velocity=.9,gate=1},{step=15,note=36,velocity=.82,gate=.6},
  {step=17,note=26,velocity=1,gate=2},{step=22,note=38,velocity=.7,gate=.8},{step=25,note=50,velocity=.92,gate=.8},{step=29,note=41,velocity=.82,gate=1},{step=31,note=45,velocity=.72,gate=.6},
  {step=33,note=29,velocity=1,gate=3},{step=40,note=41,velocity=.76,gate=.8},{step=43,note=48,velocity=.88,gate=1},{step=47,note=38,velocity=.7,gate=.6},
  {step=49,note=24,velocity=1,gate=3},{step=55,note=36,velocity=.72,gate=.8},{step=58,note=43,velocity=.88,gate=1},{step=63,note=33,velocity=.78,gate=.7},
  {step=65,note=26,velocity=1,gate=2},{step=69,note=38,velocity=.66,gate=.6},{step=73,note=45,velocity=.9,gate=.8},{step=77,note=50,velocity=.78,gate=.7},{step=81,note=26,velocity=1,gate=3},
  {step=97,note=29,velocity=.96,gate=4},{step=113,note=24,velocity=.92,gate=5}})
local bass_bleed=notes("Bass","Bass: bleed off",128,{{step=1,note=38,velocity=.9,gate=3},{step=10,note=45,velocity=.72,gate=1},{step=17,note=38,velocity=.84,gate=3},{step=33,note=34,velocity=.78,gate=4},{step=49,note=38,velocity=.72,gate=5},{step=65,note=26,velocity=.62,gate=8},{step=97,note=26,velocity=.42,gate=16}})

local low_drone=notes("Low Drone","Drone: chamber",128,{{step=1,note=26,velocity=.68,gate=62},{step=65,note=24,velocity=.64,gate=62}})
local low_drone_shift=notes("Low Drone","Drone: shear",128,{{step=1,note=29,velocity=.72,gate=46},{step=49,note=25,velocity=.66,gate=30},{step=81,note=26,velocity=.7,gate=46}})
local low_drone_air=notes("Low Drone","Drone: negative space",128,{{step=1,note=26,velocity=.46,gate=30},{step=65,note=24,velocity=.42,gate=30}})

local hook_fragment=notes("Hook","Hook: beacon fragments",128,{{step=13,note=62,velocity=.42,gate=2},{step=29,note=65,velocity=.5,gate=2},{step=61,note=60,velocity=.46,gate=3},{step=93,note=69,velocity=.54,gate=4},{step=125,note=65,velocity=.58,gate=2}})
local hook_main=notes("Hook","Hook: pressure signal",128,{{step=1,note=62,velocity=.82,gate=3},{step=7,note=65,velocity=.64,gate=1},{step=11,note=69,velocity=.9,gate=2},{step=17,note=60,velocity=.76,gate=3},{step=23,note=67,velocity=.7,gate=1},{step=27,note=65,velocity=.84,gate=3},{step=33,note=62,velocity=.88,gate=4},{step=41,note=70,velocity=.72,gate=1},{step=45,note=69,velocity=.92,gate=3},{step=49,note=58,velocity=.78,gate=3},{step=55,note=65,velocity=.7,gate=1},{step=59,note=60,velocity=.84,gate=4},{step=65,note=62,velocity=.86,gate=3},{step=75,note=69,velocity=.9,gate=2},{step=81,note=60,velocity=.76,gate=3},{step=91,note=65,velocity=.86,gate=3},{step=97,note=65,velocity=.9,gate=4},{step=105,note=72,velocity=.72,gate=1},{step=109,note=70,velocity=.92,gate=3},{step=113,note=57,velocity=.8,gate=3},{step=123,note=60,velocity=.88,gate=5}})
local hook_return=notes("Hook","Hook: signal transformed",128,{{step=1,note=74,velocity=.92,gate=4},{step=9,note=69,velocity=.7,gate=1},{step=13,note=77,velocity=.86,gate=2},{step=17,note=72,velocity=.82,gate=3},{step=25,note=67,velocity=.72,gate=1},{step=29,note=74,velocity=.9,gate=4},{step=33,note=70,velocity=.94,gate=3},{step=41,note=65,velocity=.7,gate=1},{step=45,note=72,velocity=.88,gate=3},{step=49,note=69,velocity=.86,gate=4},{step=57,note=60,velocity=.68,gate=1},{step=61,note=65,velocity=.9,gate=5},{step=65,note=74,velocity=.96,gate=4},{step=77,note=69,velocity=.82,gate=2},{step=81,note=72,velocity=.9,gate=3},{step=93,note=65,velocity=.86,gate=4},{step=97,note=77,velocity=1,gate=5},{step=109,note=70,velocity=.84,gate=3},{step=113,note=69,velocity=.9,gate=4},{step=125,note=62,velocity=.82,gate=3}})
local hook_bleed=notes("Hook","Hook: last signal",128,{{step=1,note=62,velocity=.72,gate=4},{step=11,note=69,velocity=.7,gate=3},{step=33,note=60,velocity=.62,gate=5},{step=65,note=50,velocity=.5,gate=8},{step=97,note=50,velocity=.3,gate=16}})

local machine_once=drum("Machine Room","Machine room bed",128,{{1,1}})
local industrial_once=drum("Industrial Drone","Industrial weather",128,{{1,1}})
local breath_phrase=drum("Breath Forward","Breath: inhale code",128,{{1,.64},{37,.42},{65,.58},{105,.48}})
local breath_return=drum("Breath Reverse","Breath: time reversed",128,{{1,.5},{29,.4},{61,.58},{93,.46},{121,.68}})

local function scene(name,values)
  local explicit={};for _,track_name in ipairs(names) do explicit[track_name]=false end
  for key,value in pairs(values) do explicit[key]=value end
  return sequence:scene(name,explicit)
end

local pressure_front=scene("Pressure Front",{["Low Drone"]=low_drone_air,["Machine Room"]=machine_once,["Impact"]=impact_sparse})
local valve_noise=scene("Valve Noise",{["Break Kick"]=kick_hint,["Break Snare"]=snare_hint,["Closed Hat"]=hats_hint,["Low Drone"]=low_drone,["Machine Room"]=machine_once,["Hydraulic"]=hydraulic_sparse,["Hook"]=hook_fragment})
local first_weight=scene("First Weight",{["Break Kick"]=kick_a,["Break Snare"]=snare_a,["Ghost Snare"]=ghost_a,["Closed Hat"]=hats_a,["Open Hat"]=open_a,["Floor Tom"]=tom_a,["Electro Kick"]=electro_a,["Clap"]=clap_a,Bass=bass_primary,["Low Drone"]=low_drone,["Machine Room"]=machine_once,["Breath Forward"]=breath_phrase})
local full_bore=scene("Full Bore",{["Break Kick"]=kick_b,["Break Snare"]=snare_b,["Ghost Snare"]=ghost_a,["Closed Hat"]=hats_b,["Open Hat"]=open_b,["Floor Tom"]=tom_a,["Electro Kick"]=electro_a,["Clap"]=clap_a,Bass=bass_primary,["Low Drone"]=low_drone,Hook=hook_main,["Machine Room"]=machine_once,["Hydraulic"]=hydraulic_rhythm})
local ratchet=scene("Ratchet",{["Break Kick"]=kick_b,["Break Snare"]=snare_b,["Ghost Snare"]=ghost_dense,["Closed Hat"]=hats_b,["Open Hat"]=open_b,["Floor Tom"]=tom_b,["Electro Kick"]=electro_heavy,["Clap"]=clap_a,["Hydraulic"]=hydraulic_rhythm,Bass=bass_primary,["Low Drone"]=low_drone_shift,Hook=hook_main,["Industrial Drone"]=industrial_once})
local redline=scene("Redline",{["Break Kick"]=kick_b,["Break Snare"]=snare_fill,["Ghost Snare"]=ghost_dense,["Closed Hat"]=hats_b,["Open Hat"]=open_b,["Floor Tom"]=tom_b,["Electro Kick"]=electro_heavy,["Clap"]=clap_a,["Hydraulic"]=hydraulic_rhythm,Impact=impact_build,Bass=bass_release,["Low Drone"]=low_drone_shift,Hook=hook_main,["Industrial Drone"]=industrial_once})
local shear=scene("Shear",{["Break Kick"]=kick_half,["Break Snare"]=snare_half,["Ghost Snare"]=ghost_sparse,["Closed Hat"]=hats_half,["Floor Tom"]=tom_sparse,["Clap"]=clap_sparse,Bass=bass_half,["Low Drone"]=low_drone_shift,["Machine Room"]=machine_once,["Industrial Drone"]=industrial_once,["Breath Forward"]=breath_phrase})
local vacuum=scene("Vacuum Pocket",{["Ghost Snare"]=ghost_sparse,["Floor Tom"]=tom_sparse,["Hydraulic"]=hydraulic_sparse,Impact=impact_sparse,["Low Drone"]=low_drone_air,["Machine Room"]=machine_once,["Industrial Drone"]=industrial_once,["Breath Forward"]=breath_phrase})
local primer=scene("Primer",{["Break Kick"]=kick_hint,["Break Snare"]=snare_hint,["Closed Hat"]=hats_hint,["Floor Tom"]=tom_sparse,["Electro Kick"]=electro_hint,Bass=bass_half,["Low Drone"]=low_drone,["Machine Room"]=machine_once,["Breath Reverse"]=breath_return})
local reconnect=scene("Reconnect",{["Break Kick"]=kick_a,["Break Snare"]=snare_b,["Ghost Snare"]=ghost_a,["Closed Hat"]=hats_a,["Open Hat"]=open_a,["Floor Tom"]=tom_b,["Electro Kick"]=electro_a,["Clap"]=clap_a,Bass=bass_primary,["Low Drone"]=low_drone,Hook=hook_fragment,["Breath Reverse"]=breath_return})
local return_pressure=scene("Return Pressure",{["Break Kick"]=kick_b,["Break Snare"]=snare_fill,["Ghost Snare"]=ghost_dense,["Closed Hat"]=hats_b,["Open Hat"]=open_b,["Floor Tom"]=tom_b,["Electro Kick"]=electro_heavy,["Clap"]=clap_a,["Hydraulic"]=hydraulic_rhythm,Impact=impact_build,Bass=bass_release,["Low Drone"]=low_drone_shift,Hook=hook_return,["Industrial Drone"]=industrial_once,["Breath Reverse"]=breath_return})
local release=scene("Major Release",{["Break Kick"]=kick_release,["Break Snare"]=snare_b,["Ghost Snare"]=ghost_dense,["Closed Hat"]=hats_b,["Open Hat"]=open_b,["Floor Tom"]=tom_b,["Electro Kick"]=electro_heavy,["Clap"]=clap_a,["Hydraulic"]=hydraulic_rhythm,Impact=impact_build,Bass=bass_release,["Low Drone"]=low_drone,Hook=hook_return,["Machine Room"]=machine_once,["Industrial Drone"]=industrial_once,["Breath Reverse"]=breath_return})
local aftershock=scene("Aftershock",{["Break Kick"]=kick_release,["Break Snare"]=snare_b,["Ghost Snare"]=ghost_a,["Closed Hat"]=hats_a,["Open Hat"]=open_a,["Floor Tom"]=tom_a,["Electro Kick"]=electro_a,["Clap"]=clap_sparse,Bass=bass_primary,["Low Drone"]=low_drone,Hook=hook_main,["Machine Room"]=machine_once,["Breath Reverse"]=breath_return})
local bleed_off=scene("Bleed Off",{["Break Kick"]=kick_bleed,["Break Snare"]=snare_bleed,["Closed Hat"]=hats_bleed,["Floor Tom"]=tom_sparse,Bass=bass_bleed,["Low Drone"]=low_drone_air,Hook=hook_bleed,["Machine Room"]=machine_once})

local arrangement=sequence:timeline("Pressure Cycle 04")
arrangement:at_bar(1,pressure_front):at_bar(13,valve_noise):at_bar(25,first_weight)
  :at_bar(41,full_bore):at_bar(57,ratchet):at_bar(73,redline):at_bar(97,shear)
  :at_bar(113,vacuum):at_bar(129,primer):at_bar(141,reconnect):at_bar(153,return_pressure)
  :at_bar(169,release):at_bar(185,aftershock):at_bar(201,bleed_off):stop_at_bar(209)

local avl=meldritch.samples {source="remote",bank="AVL",cache=true,
  manifest="samples/pressure-system-avl.json",
  base_url="https://raw.githubusercontent.com/studiorack/avl-drumkits/b06cb2c27359cbc68a830a82bddced05bd73d1a0/",
  version="b06cb2c27359cbc68a830a82bddced05bd73d1a0"}
local found=meldritch.samples {source="remote",bank="Pressure",cache=true,
  manifest="samples/pressure-system-meta.json",
  base_url="https://raw.githubusercontent.com/facebookincubator/Meta-open-sound/251c0be57d5d1b3242b91f53e0cde62ab1cb42d9/",
  version="251c0be57d5d1b3242b91f53e0cde62ab1cb42d9"}
local tr=meldritch.samples {source="strudel",bank="RolandTR909",cache=true}

local acoustic=instrument:mixer {inputs=8,headroom=.24}
local function sampled_hit(track_name,sample,gain,slot,settings)
  settings=settings or {};settings.sample=sample;settings.gain=gain
  local player=instrument:sample_player(settings)
  local left=instrument:vca {};local right=instrument:vca {}
  instrument:connect(track[track_name].out,player.trigger)
  instrument:connect(player.left,left.input);instrument:connect(player.right,right.input)
  instrument:connect(track[track_name].velocity,left.cv);instrument:connect(track[track_name].velocity,right.cv)
  instrument:connect(left.out,acoustic.input[slot].left);instrument:connect(right.out,acoustic.input[slot].right)
  return player,left,right
end
local break_kick=sampled_hit("Break Kick",avl:load("kick",4),1.25,1,{finish=.82,rate=.92})
local break_snare=sampled_hit("Break Snare",avl:load("snare",4),1.12,2,{start=.015,finish=.92,rate=1.04})
local ghost_snare=sampled_hit("Ghost Snare",avl:load("snare_edge",1),.72,3,{start=.05,finish=.68,rate=1.18})
local closed_hat=sampled_hit("Closed Hat",avl:load("hat_closed",3),.72,4,{start=.03,finish=.66,rate=1.07})
local open_hat=sampled_hit("Open Hat",avl:load("hat_open",3),.54,5,{start=.02,finish=.86,rate=.92})
local floor_tom=sampled_hit("Floor Tom",avl:load("floor_tom",4),.9,6,{start=.02,finish=.88,rate=.78})

-- The negative rate makes every tom accent drag backward through the acoustic
-- transient; it is a separate parallel fragment rather than a fake synth hit.
local reverse_tom=instrument:sample_player {sample=avl:load("floor_tom",2),gain=.34,start=.08,finish=.72,rate=-.64}
instrument:connect(track["Floor Tom"].out,reverse_tom.trigger)
instrument:connect(reverse_tom.left,acoustic.input[7].left);instrument:connect(reverse_tom.right,acoustic.input[7].right)

local drum_bus=instrument:mixer {inputs=8,headroom=.40}
instrument:connect(acoustic.left,drum_bus.input[1].left);instrument:connect(acoustic.right,drum_bus.input[1].right)
local smash_l=instrument:drive {amount=4.8,output_gain=.42};local smash_r=instrument:drive {amount=4.8,output_gain=.42}
instrument:connect(acoustic.left,smash_l.input);instrument:connect(acoustic.right,smash_r.input)
instrument:connect(smash_l.out,drum_bus.input[2].left);instrument:connect(smash_r.out,drum_bus.input[2].right)

local function drum_sample(track_name,sample,gain,slot,settings)
  settings=settings or {};settings.sample=sample;settings.gain=gain
  local player=instrument:sample_player(settings)
  instrument:connect(track[track_name].out,player.trigger)
  instrument:connect(player.left,drum_bus.input[slot].left);instrument:connect(player.right,drum_bus.input[slot].right)
  return player
end
local electro=drum_sample("Electro Kick",tr:load("bd",0),.72,3,{rate=.82})
local clap=drum_sample("Clap",tr:load("cp",0),.42,4,{rate=.94})
local hydraulic=drum_sample("Hydraulic",found:load("hydraulic",0),.44,5,{start=.02,finish=.68,rate=.74})
local impact=drum_sample("Impact",found:load("impact",0),.5,6,{start=.0,finish=.92,rate=.68})

-- Bass: waveshaping main voice plus a clean sine foundation. Both share the
-- articulation, then meet before filtering, drive, and kick ducking.
local bass_main=instrument:plaits {model="waveshaping",harmonics=.62,timbre=.72,morph=.38}
local bass_sub=instrument:oscillator {waveform="sine",frequency=55}
local bass_env=instrument:adsr {attack=.003,decay=.17,sustain=.38,release=.11}
local bass_main_amp=instrument:vca {};local bass_sub_amp=instrument:vca {}
instrument:connect(track.Bass.pitch,bass_main.pitch);instrument:connect(track.Bass.gate,bass_main.gate);instrument:connect(track.Bass.velocity,bass_main.velocity)
instrument:connect(track.Bass.pitch,bass_sub.pitch);instrument:connect(track.Bass.gate,bass_env.gate)
instrument:connect(bass_main.main,bass_main_amp.input);instrument:connect(bass_sub.out,bass_sub_amp.input)
instrument:connect(bass_env.out,bass_main_amp.cv);instrument:connect(bass_env.out,bass_sub_amp.cv)
local bass_sum=instrument:mixer {inputs=2};instrument:connect(bass_main_amp.out,bass_sum.input[1]);instrument:connect(bass_sub_amp.out,bass_sum.input[2])
local bass_filter=instrument:svf {cutoff=620,resonance=.58};local bass_drive=instrument:drive {amount=4.2,output_gain=.7}
instrument:connect(bass_sum.out,bass_filter.input);instrument:connect(bass_filter.lowpass,bass_drive.input)
local bass_l=instrument:ducker {threshold=.025,depth=.74,attack=.002,release=.14};local bass_r=instrument:ducker {threshold=.025,depth=.74,attack=.002,release=.14}
instrument:connect(bass_drive.out,bass_l.input);instrument:connect(bass_drive.out,bass_r.input)
instrument:connect(break_kick.left,bass_l.sidechain);instrument:connect(break_kick.right,bass_r.sidechain)

local music=instrument:mixer {inputs=8,headroom=.27}
instrument:connect(bass_l.out,music.input[1].left);instrument:connect(bass_r.out,music.input[1].right)

-- Drone system one: a beating swarm constrained to the low-mid chamber and
-- ducked by the real kick so it can become huge without eating the groove.
local low=instrument:plaits {model="swarm",harmonics=.34,timbre=.48,morph=.62}
local low_env=instrument:adsr {attack=3.5,decay=2,sustain=.82,release=6}
local low_amp=instrument:vca {};local low_filter=instrument:svf {cutoff=420,resonance=.42};local low_drive=instrument:drive {amount=1.7,output_gain=.62}
instrument:connect(track["Low Drone"].pitch,low.pitch);instrument:connect(track["Low Drone"].gate,low.gate);instrument:connect(track["Low Drone"].velocity,low.velocity)
instrument:connect(track["Low Drone"].gate,low_env.gate);instrument:connect(low.main,low_amp.input);instrument:connect(low_env.out,low_amp.cv)
instrument:connect(low_amp.out,low_filter.input);instrument:connect(low_filter.bandpass,low_drive.input)
local drone_l=instrument:ducker {threshold=.022,depth=.58,attack=.004,release=.26};local drone_r=instrument:ducker {threshold=.022,depth=.58,attack=.004,release=.26}
instrument:connect(low_drive.out,drone_l.input);instrument:connect(low_drive.out,drone_r.input);instrument:connect(break_kick.left,drone_l.sidechain);instrument:connect(break_kick.right,drone_r.sidechain)
instrument:connect(drone_l.out,music.input[2].left);instrument:connect(drone_r.out,music.input[2].right)

local hook=instrument:plaits {model="phase_distortion",harmonics=.44,timbre=.58,morph=.68}
local hook_env=instrument:adsr {attack=.015,decay=.28,sustain=.38,release=.8};local hook_amp=instrument:vca {}
instrument:connect(track.Hook.pitch,hook.pitch);instrument:connect(track.Hook.gate,hook.gate);instrument:connect(track.Hook.velocity,hook.velocity)
instrument:connect(track.Hook.gate,hook_env.gate);instrument:connect(hook.main,hook_amp.input);instrument:connect(hook_env.out,hook_amp.cv)
local hook_filter=instrument:svf {cutoff=2600,resonance=.38};instrument:connect(hook_amp.out,hook_filter.input)
local hook_l=instrument:gain {gain=.74};local hook_r=instrument:gain {gain=.56};instrument:connect(hook_filter.lowpass,hook_l.input);instrument:connect(hook_filter.lowpass,hook_r.input)
instrument:connect(hook_l.out,music.input[3].left);instrument:connect(hook_r.out,music.input[3].right)

-- MicroLab remains a separate voice with independent note ownership.
local live=instrument:plaits {model="fm",harmonics=.58,timbre=.42,morph=.36};local live_env=instrument:adsr {attack=.008,decay=.2,sustain=.55,release=.75};local live_amp=instrument:vca {};local live_gain=instrument:gain {gain=.32}
instrument:connect(instrument.note.pitch,live.pitch);instrument:connect(instrument.note.gate,live.gate);instrument:connect(instrument.note.velocity,live.velocity)
instrument:connect(instrument.note.gate,live_env.gate);instrument:connect(live.main,live_amp.input);instrument:connect(live_env.out,live_amp.cv);instrument:connect(live_amp.out,live_gain.input)
instrument:connect(live_gain.out,music.input[4].left);instrument:connect(live_gain.out,music.input[4].right)

-- Drone system two is entirely found sound: machine-room air and a designed
-- industrial drone move through separate filters, then a tempo delay.
local texture=instrument:mixer {inputs=8,headroom=.3}
local machine=instrument:sample_player {sample=found:load("machine_room",0),gain=.48,start=.03,finish=.31,rate=.72}
local industrial=instrument:sample_player {sample=found:load("creepy_drone",0),gain=.42,start=.04,finish=.40,rate=.56}
instrument:connect(track["Machine Room"].out,machine.trigger);instrument:connect(track["Industrial Drone"].out,industrial.trigger)
local machine_l=instrument:svf {cutoff=1800,resonance=.34};local machine_r=instrument:svf {cutoff=2300,resonance=.28}
local industrial_l=instrument:svf {cutoff=1200,resonance=.46};local industrial_r=instrument:svf {cutoff=1550,resonance=.4}
instrument:connect(machine.left,machine_l.input);instrument:connect(machine.right,machine_r.input);instrument:connect(industrial.left,industrial_l.input);instrument:connect(industrial.right,industrial_r.input)
instrument:connect(machine_l.highpass,texture.input[1].left);instrument:connect(machine_r.highpass,texture.input[1].right)
instrument:connect(industrial_l.bandpass,texture.input[2].left);instrument:connect(industrial_r.bandpass,texture.input[2].right)

local breath_forward=instrument:sample_player {sample=found:load("breath",0),gain=.34,start=.08,finish=.82,rate=.74}
local breath_reverse=instrument:sample_player {sample=found:load("breath",0),gain=.4,start=.05,finish=.9,rate=-.62}
instrument:connect(track["Breath Forward"].out,breath_forward.trigger);instrument:connect(track["Breath Reverse"].out,breath_reverse.trigger)
instrument:connect(breath_forward.left,texture.input[3].left);instrument:connect(breath_forward.right,texture.input[3].right)
instrument:connect(breath_reverse.right,texture.input[4].left);instrument:connect(breath_reverse.left,texture.input[4].right)

local texture_delay=instrument:stereo_delay {left_beats=.75,right_beats=1.25,feedback=.42,crossfeed=.76,mix=.72,filter_cutoff=3100,feedback_drive=1.8}
instrument:connect(sequence.tempo,texture_delay.bpm);instrument:connect(texture.left,texture_delay.input.left);instrument:connect(texture.right,texture_delay.input.right)
local texture_dry_l=instrument:gain {gain=.52};local texture_dry_r=instrument:gain {gain=.52};local texture_wet_l=instrument:gain {gain=.42};local texture_wet_r=instrument:gain {gain=.42}
instrument:connect(texture.left,texture_dry_l.input);instrument:connect(texture.right,texture_dry_r.input);instrument:connect(texture_delay.left,texture_wet_l.input);instrument:connect(texture_delay.right,texture_wet_r.input)
instrument:connect(texture_dry_l.out,music.input[5].left);instrument:connect(texture_dry_r.out,music.input[5].right);instrument:connect(texture_wet_l.out,music.input[6].left);instrument:connect(texture_wet_r.out,music.input[6].right)

local hook_delay=instrument:stereo_delay {left_beats=.375,right_beats=.625,feedback=.34,crossfeed=.82,mix=1,filter_cutoff=4800,feedback_drive=1.35}
instrument:connect(sequence.tempo,hook_delay.bpm);instrument:connect(hook_l.out,hook_delay.input.left);instrument:connect(hook_r.out,hook_delay.input.right)
local hook_wet_l=instrument:gain {gain=.3};local hook_wet_r=instrument:gain {gain=.3};instrument:connect(hook_delay.left,hook_wet_l.input);instrument:connect(hook_delay.right,hook_wet_r.input)
instrument:connect(hook_wet_l.out,music.input[7].left);instrument:connect(hook_wet_r.out,music.input[7].right)

local master=instrument:mixer {inputs=8,headroom=.52}
instrument:connect(drum_bus.left,master.input[1].left);instrument:connect(drum_bus.right,master.input[1].right)
instrument:connect(music.left,master.input[2].left);instrument:connect(music.right,master.input[2].right)
local dc_l=instrument:svf {cutoff=24,resonance=.04};local dc_r=instrument:svf {cutoff=24,resonance=.04};instrument:connect(master.left,dc_l.input);instrument:connect(master.right,dc_r.input)
local comp=instrument:stereo_compressor {threshold=-18,ratio=3.2,attack=.018,release=.24,knee=5,makeup_gain=2,bypass=0}
local final_l=instrument:gain {gain=1};local final_r=instrument:gain {gain=1}
instrument:connect(dc_l.highpass,comp.input.left);instrument:connect(dc_r.highpass,comp.input.right)
instrument:connect(comp.left,final_l.input);instrument:connect(comp.right,final_r.input);instrument:connect(final_l.out,instrument.output.left);instrument:connect(final_r.out,instrument.output.right)

local function control(id,label,target,range,targets,curve)
  return instrument:control {id=id,label=label,target=target,targets=targets,range=range,curve=curve}
end
local break_smash=control("break_smash","Break parallel dirt",smash_l.amount,{.2,12},{smash_r.amount})
local bass_cutoff=control("bass_cutoff","Bass valve",bass_filter.cutoff,{70,7000},nil,"log")
local bass_drive_amount=control("bass_drive","Bass pressure",bass_drive.amount,{.2,12})
local drone_cutoff=control("drone_cutoff","Chamber tone",low_filter.cutoff,{80,4200},nil,"log")
local drone_intensity=control("drone_intensity","Chamber intensity",low_drive.output_gain,{0,.95})
local machine_tone=control("machine_tone","Machine air",machine_l.cutoff,{180,9000},{machine_r.cutoff},"log")
local texture_feedback=control("texture_feedback","Texture feedback",texture_delay.feedback,{0,.86})
local texture_wet=control("texture_wet","Texture space",texture_wet_l.gain,{0,.8},{texture_wet_r.gain})
local hook_tone=control("hook_tone","Signal tone",hook_filter.cutoff,{250,12000},nil,"log")
local hook_feedback=control("hook_feedback","Signal feedback",hook_delay.feedback,{0,.82})
local breath_rate=control("breath_rate","Breath speed",breath_forward.rate,{.25,1.7})
local master_level=control("master_level","Master volume",master.headroom,{.05,.72})
local ending_fade=control("ending_fade","Arrangement fade",final_l.gain,{0,1},{final_r.gain})
local master_threshold=control("master_threshold","Glue threshold",comp.threshold,{-34,-4})
local master_ratio=control("master_ratio","Glue ratio",comp.ratio,{1,8})
local master_attack=control("master_attack","Glue attack",comp.attack,{.002,.12},nil,"log")
local master_release=control("master_release","Glue release",comp.release,{.04,.8},nil,"log")
local master_makeup=control("master_makeup","Glue makeup",comp.makeup_gain,{-4,8})

-- Automation is the long-form pressure system: trajectories cross scene
-- boundaries and operate at independent rates rather than decorating cuts.
arrangement:automation {target=bass_cutoff,interpolation="smooth"}:at_bar(1,180):at_bar(25,420):at_bar(49,1050):at_bar(73,2600):at_bar(96,720):at_bar(113,150):at_bar(128,260):at_bar(153,1200):at_bar(176,3900):at_bar(193,1600):at_bar(209,120)
arrangement:automation {target=bass_drive_amount,interpolation="linear"}:at_bar(1,1.2):at_bar(25,2.8):at_bar(64,5.2):at_bar(88,8):at_bar(113,1.4):at_bar(141,3.4):at_bar(176,7.2):at_bar(193,4):at_bar(209,.8)
arrangement:automation {target=break_smash,interpolation="smooth"}:at_bar(1,.6):at_bar(25,2.4):at_bar(57,4.6):at_bar(80,8.4):at_bar(96,3):at_bar(113,.5):at_bar(141,2.2):at_bar(169,6.8):at_bar(184,9.2):at_bar(201,2.4):at_bar(209,.4)
arrangement:automation {target=drone_cutoff,interpolation="smooth"}:at_bar(1,220):at_bar(20,680):at_bar(48,380):at_bar(72,1300):at_bar(104,2600):at_bar(120,540):at_bar(136,180):at_bar(168,900):at_bar(190,2300):at_bar(209,140)
arrangement:automation {target=drone_intensity,interpolation="linear"}:at_bar(1,.38):at_bar(18,.72):at_bar(41,.34):at_bar(73,.68):at_bar(96,.28):at_bar(113,.82):at_bar(128,.18):at_bar(153,.52):at_bar(185,.24):at_bar(209,0)
arrangement:automation {target=machine_tone,interpolation="smooth"}:at_bar(1,340):at_bar(24,1700):at_bar(52,4200):at_bar(80,1100):at_bar(112,6200):at_bar(128,480):at_bar(168,2500):at_bar(200,7800):at_bar(209,300)
arrangement:automation {target=texture_feedback,interpolation="smooth"}:at_bar(1,.58):at_bar(25,.28):at_bar(64,.42):at_bar(89,.7):at_bar(112,.82):at_bar(129,.36):at_bar(160,.48):at_bar(184,.76):at_bar(185,.34):at_bar(208,.72)
arrangement:automation {target=texture_wet,interpolation="linear"}:at_bar(1,.58):at_bar(25,.3):at_bar(73,.46):at_bar(97,.62):at_bar(120,.76):at_bar(129,.32):at_bar(169,.48):at_bar(185,.38):at_bar(208,.68)
arrangement:automation {target=hook_tone,interpolation="smooth"}:at_bar(1,800):at_bar(41,1900):at_bar(72,6200):at_bar(97,1200):at_bar(141,900):at_bar(169,3400):at_bar(184,9800):at_bar(201,1800):at_bar(209,420)
arrangement:automation {target=hook_feedback,interpolation="linear"}:at_bar(1,.5):at_bar(41,.28):at_bar(72,.44):at_bar(96,.68):at_bar(113,.74):at_bar(129,.24):at_bar(168,.38):at_bar(184,.7):at_bar(185,.3):at_bar(208,.72)
arrangement:automation {target=breath_rate,interpolation="smooth"}:at_bar(1,.58):at_bar(48,.82):at_bar(96,.42):at_bar(128,1.18):at_bar(169,.68):at_bar(209,.34)
arrangement:automation {target=ending_fade,interpolation="smooth"}:at_bar(1,1):at_bar(201,1):at_bar(205,.84):at_bar(208,.28):at_bar(209,0)

instrument:performance {layout="step_grid",sequencer=sequence,project_file="04_pressure_system.lua",
  keys={Space="transport",s="start",x="stop",r="restart",Up="track:up",Down="track:down",Left="step:previous",Right="step:next",["["]="bank:previous",["]"]="bank:next",m="mute",o="solo",j="control:next",k="control:previous",h="control:decrease",l="control:increase",["1"]="toggle:1",["2"]="toggle:2",["3"]="toggle:3",["4"]="toggle:4",["5"]="toggle:5",["6"]="toggle:6",["7"]="toggle:7",["8"]="toggle:8"},
  apc={pages={"steps","performance"},performance_rows={"scene","mute","solo","track","restart","clip","",""},
    scenes={"Pressure Front","First Weight","Full Bore","Redline","Vacuum Pocket","Reconnect","Major Release","Bleed Off"},scene_quantization=16,
    faders={"break_smash","bass_cutoff","bass_drive","drone_cutoff","drone_intensity","machine_tone","texture_feedback","texture_wet"},
    fader_banks={{"break_smash","bass_cutoff","bass_drive","drone_cutoff","drone_intensity","machine_tone","texture_feedback","texture_wet"},{"hook_tone","hook_feedback","breath_rate","master_threshold","master_ratio","master_attack","master_release","master_makeup"}},
    scene_buttons={"transport","restart","faders:bank:0","faders:bank:1","","","",""},master="master_level"},
  focus={controls={"break_smash","bass_cutoff","bass_drive","drone_cutoff","drone_intensity","machine_tone","texture_feedback","texture_wet","hook_tone","hook_feedback","breath_rate"},midi_ccs={{channel=1,number=74},{channel=1,number=71},{channel=1,number=76},{channel=1,number=77}}}}

local composition=instrument:screen("composition","Pressure System")
composition:arrangement_status();composition:control_focus()
local clips=instrument:screen("clips","Break / Clip Inspector")
clips:pattern_grid();clips:control_focus()
local mastering=instrument:screen("master","Pressure / Master Glue")
mastering:control("Break parallel dirt","break_smash");mastering:control("Bass valve","bass_cutoff")
mastering:control("Chamber intensity","drone_intensity");mastering:control("Texture space","texture_wet")
mastering:control("Threshold (dB)","master_threshold");mastering:control("Ratio","master_ratio")
mastering:control("Attack (s)","master_attack");mastering:control("Release (s)","master_release")
mastering:control("Makeup (dB)","master_makeup")
local navigation=instrument:navigation {initial="composition",events={{name="composition",from="*",to="composition"},{name="clips",from="*",to="clips"},{name="master",from="*",to="master"}}}
navigation:bind("F9","composition");navigation:bind("F10","clips");navigation:bind("F11","master")

sequence:start()
return instrument
