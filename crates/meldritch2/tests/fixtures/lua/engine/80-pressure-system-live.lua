-- Pressure System Live
--
-- Eight minutes at 128 BPM (256 bars), then the final patterns keep looping for
-- live performance. Rust remains the clock and state authority; this file only
-- declares musical intent and the projected performance stations.
local instrument = meldritch.instrument("Pressure System Live")
local home = assert(os.getenv("HOME"), "HOME is required")
local SERUM = "56534558667350736572756D20320000"
local PULSE = "ABCDEF019182FAEB506C7365506C6473"
local LSP_COMPRESSOR = "647370206B31732020202020756E7363"
local LSP_LIMITER = "64737020623173202020202072667563"
local plugin_paths = { home .. "/.vst3", "/usr/lib/vst3" }
local serum_bank_root = os.getenv("PRESSURE_SERUM_BANK")
  or (home .. "/.local/share/meldritch2/patches/serum2/pressure-system")

local function notes(sequence, track, name, events, length)
  local pattern = sequence:pattern(name)
  pattern:notes(track, events, length or 16)
  return pattern
end

local function make_note_patterns(sequence, track, definitions)
  local result = {}
  for _, definition in ipairs(definitions) do
    result[#result + 1] = notes(sequence, track, definition[1], definition[2], definition[3])
  end
  return result
end

-- Four independent sequencers share the same Rust engine clock rate. Their
-- pattern selectors remain independent without introducing another scheduler.
local bass_sequence = instrument:sequencer { bpm=128, steps=16, steps_per_beat=4 }
local lead_sequence = instrument:sequencer { bpm=128, steps=16, steps_per_beat=4 }
local drone_sequence = instrument:sequencer { bpm=128, steps=64, steps_per_beat=4 }
local drum_sequence = instrument:sequencer { bpm=128, steps=16, steps_per_beat=4 }
local bass_track = bass_sequence:note_track("Pressure Bass", {voices=4})
local lead_track = lead_sequence:note_track("Pressure Lead", {voices=4})
local drone_track = drone_sequence:note_track("Pressure Drone", {voices=4})

local bass_patterns = make_note_patterns(bass_sequence, bass_track, {
  {"Foundation", {{step=1,pitch="C1",velocity=122,duration=3},{step=5,pitch="C2",velocity=96,duration=2},{step=9,pitch="C1",velocity=116,duration=3},{step=14,pitch="D#1",velocity=106,duration=2}}},
  {"Teeth", {{step=1,pitch="C1",velocity=127,duration=2},{step=4,pitch="G1",velocity=91,duration=1},{step=7,pitch="C2",velocity=118,duration=2},{step=10,pitch="A#1",velocity=101,duration=1},{step=13,pitch="D#2",velocity=124,duration=2},{step=16,pitch="C1",velocity=92,duration=1}}},
  {"Acid Pressure", {{step=1,pitch="C2",velocity=120,duration=1},{step=3,pitch="D#2",velocity=92,duration=2},{step=6,pitch="F2",velocity=108,duration=1},{step=8,pitch="G2",velocity=126,duration=2},{step=11,pitch="C2",velocity=96,duration=1},{step=13,pitch="A#1",velocity=114,duration=3}}},
  {"Broken Machine", {{step=1,pitch="C1",velocity=127,duration=4},{step=6,pitch="F#1",velocity=84,duration=1},{step=8,pitch="G1",velocity=112,duration=2},{step=12,pitch="D#1",velocity=103,duration=1},{step=15,pitch="C2",velocity=123,duration=2}}},
  {"Subduction", {{step=1,pitch="C1",velocity=118,duration=7},{step=9,pitch="G0",velocity=112,duration=5},{step=15,pitch="D#1",velocity=91,duration=2}}},
  {"Overdrive", {{step=1,pitch="C1",velocity=127,duration=2},{step=3,pitch="C2",velocity=109,duration=1},{step=5,pitch="D#1",velocity=121,duration=2},{step=8,pitch="G1",velocity=97,duration=1},{step=9,pitch="C1",velocity=127,duration=2},{step=12,pitch="A#1",velocity=110,duration=1},{step=14,pitch="D#2",velocity=124,duration=2},{step=16,pitch="G1",velocity=89,duration=1}}},
  {"Empty Space", {{step=1,pitch="C1",velocity=112,duration=3},{step=12,pitch="G0",velocity=93,duration=2}}},
  {"Final Transmission", {{step=1,pitch="C1",velocity=112,duration=6},{step=9,pitch="D#1",velocity=91,duration=3},{step=14,pitch="G0",velocity=72,duration=2}}},
})

local lead_patterns = make_note_patterns(lead_sequence, lead_track, {
  {"Signal", {{step=3,pitch="G4",velocity=104,duration=2},{step=7,pitch="A#4",velocity=116,duration=1},{step=11,pitch="C5",velocity=98,duration=3},{step=16,pitch="D#5",velocity=120,duration=1}}},
  {"Razor", {{step=1,pitch="C5",velocity=124,duration=1},{step=4,pitch="F#5",velocity=108,duration=1},{step=6,pitch="G5",velocity=122,duration=2},{step=10,pitch="D#5",velocity=94,duration=1},{step=13,pitch="A#5",velocity=127,duration=2}}},
  {"Voltage", {{step=2,pitch="C4",velocity=94,duration=4},{step=7,pitch="D#5",velocity=120,duration=2},{step=11,pitch="G4",velocity=103,duration=4}}},
  {"Fracture", {{step=1,pitch="C5",velocity=119,duration=1},{step=3,pitch="D5",velocity=88,duration=1},{step=8,pitch="G5",velocity=127,duration=1},{step=9,pitch="F#5",velocity=102,duration=2},{step=14,pitch="D#5",velocity=113,duration=1},{step=16,pitch="A#4",velocity=95,duration=1}}},
  {"Ascension", {{step=1,pitch="C4",velocity=91,duration=3},{step=5,pitch="G4",velocity=100,duration=3},{step=9,pitch="C5",velocity=112,duration=3},{step=13,pitch="G5",velocity=124,duration=3}}},
  {"Transmission", {{step=1,pitch="C5",velocity=106,duration=6},{step=9,pitch="A#4",velocity=88,duration=4},{step=14,pitch="G4",velocity=72,duration=2}}},
})

local drone_patterns = make_note_patterns(drone_sequence, drone_track, {
  {"Reactor", {{step=1,pitch="C2",velocity=82,duration=64},{step=1,pitch="G2",velocity=62,duration=64}},64},
  {"Subterranean", {{step=1,pitch="C2",velocity=78,duration=64},{step=17,pitch="D#2",velocity=58,duration=40}},64},
  {"Ghost Signal", {{step=1,pitch="G2",velocity=52,duration=48},{step=17,pitch="C3",velocity=65,duration=40},{step=33,pitch="F#3",velocity=38,duration=24}},64},
  {"Unstable Core", {{step=1,pitch="C2",velocity=82,duration=32},{step=25,pitch="C#2",velocity=74,duration=32},{step=41,pitch="G2",velocity=59,duration=23}},64},
  {"Blackout", {{step=1,pitch="C2",velocity=42,duration=16},{step=49,pitch="G1",velocity=48,duration=12}},64},
  {"Event Horizon", {{step=1,pitch="C2",velocity=71,duration=64},{step=1,pitch="D#3",velocity=45,duration=64},{step=33,pitch="F#2",velocity=41,duration=31}},64},
})

local kit = drum_sequence:drum_kit {
  name="PULSE Pressure Kit",
  voices={
    {id="kick",label="Kick",note=36,channel=10,duration=1},
    {id="snare",label="Snare",note=38,channel=10,duration=1},
    {id="hat",label="Hat",note=42,channel=10,duration=1},
    {id="percussion",label="Metal",note=46,channel=10,duration=1},
  },
}
local function drum_pattern(name, lanes)
  local pattern = drum_sequence:pattern(name)
  pattern:drums(kit, lanes, 16)
  return pattern
end
local drum_patterns = {
  drum_pattern("Iron March",{kick={{step=1,velocity=127},{step=5,velocity=116},{step=9,velocity=124},{step=13,velocity=118}},snare={{step=5,velocity=112},{step=13,velocity=123}},hat={{step=1,velocity=72},{step=3,velocity=52},{step=5,velocity=82},{step=7,velocity=55},{step=9,velocity=74},{step=11,velocity=50},{step=13,velocity=88},{step=15,velocity=58}},percussion={{step=4,velocity=58},{step=12,velocity=72}}}),
  drum_pattern("Broken Conveyor",{kick={{step=1,velocity=127},{step=4,velocity=82},{step=7,velocity=119},{step=11,velocity=105},{step=15,velocity=121}},snare={{step=5,velocity=124},{step=10,velocity=74},{step=13,velocity=117},{step=16,velocity=96}},hat={{step=2,velocity=48},{step=3,velocity=71},{step=6,velocity=52},{step=8,velocity=83},{step=11,velocity=68},{step=14,velocity=76},{step=16,velocity=55}},percussion={{step=3,velocity=76},{step=8,velocity=91},{step=14,velocity=84}}}),
  drum_pattern("Hydraulic Press",{kick={{step=1,velocity=127},{step=5,velocity=127},{step=9,velocity=127},{step=13,velocity=127}},snare={{step=5,velocity=118},{step=13,velocity=126}},hat={{step=1,velocity=60},{step=2,velocity=44},{step=3,velocity=67},{step=4,velocity=48},{step=5,velocity=75},{step=6,velocity=45},{step=7,velocity=66},{step=8,velocity=51},{step=9,velocity=62},{step=10,velocity=43},{step=11,velocity=70},{step=12,velocity=47},{step=13,velocity=79},{step=14,velocity=48},{step=15,velocity=72},{step=16,velocity=56}},percussion={{step=7,velocity=78},{step=15,velocity=96}}}),
  drum_pattern("Empty Factory",{kick={{step=1,velocity=118},{step=11,velocity=91}},snare={{step=13,velocity=84}},hat={{step=3,velocity=45},{step=9,velocity=53},{step=15,velocity=42}},percussion={{step=8,velocity=62},{step=16,velocity=71}}}),
  drum_pattern("Overpressure",{kick={{step=1,velocity=127},{step=4,velocity=101},{step=5,velocity=119},{step=8,velocity=96},{step=9,velocity=127},{step=12,velocity=105},{step=13,velocity=121},{step=16,velocity=113}},snare={{step=3,velocity=72},{step=5,velocity=127},{step=10,velocity=77},{step=13,velocity=127},{step=16,velocity=104}},hat={{step=1,velocity=78},{step=2,velocity=51},{step=3,velocity=73},{step=4,velocity=49},{step=5,velocity=88},{step=6,velocity=52},{step=7,velocity=76},{step=8,velocity=56},{step=9,velocity=80},{step=10,velocity=53},{step=11,velocity=79},{step=12,velocity=58},{step=13,velocity=93},{step=14,velocity=54},{step=15,velocity=84},{step=16,velocity=65}},percussion={{step=2,velocity=65},{step=6,velocity=76},{step=10,velocity=83},{step=14,velocity=96}}}),
  drum_pattern("Terminal Velocity",{kick={{step=1,velocity=124},{step=5,velocity=112},{step=9,velocity=119},{step=14,velocity=101}},snare={{step=5,velocity=110},{step=13,velocity=118}},hat={{step=1,velocity=61},{step=3,velocity=48},{step=5,velocity=70},{step=7,velocity=45},{step=9,velocity=65},{step=11,velocity=46},{step=13,velocity=72},{step=15,velocity=49}},percussion={{step=4,velocity=58},{step=8,velocity=67},{step=12,velocity=75},{step=16,velocity=91}}}),
}

bass_sequence:switch_pattern("Foundation")
lead_sequence:switch_pattern("Signal")
drone_sequence:switch_pattern("Reactor")
drum_sequence:switch_pattern("Iron March")

-- Authentic Serum states captured from the native editor. Only the selected resident runs
-- steady-state DSP; selection flushes outgoing notes and reconstructs held incoming notes
-- without opaque plugin state mutation in the real-time callback.
local bass = instrument:vst3 {id="serum_bass",plugin=SERUM,paths=plugin_paths,
  patch_bank=serum_bank_root.."/bass",initial_patch="bass_reese"}:connect_track(bass_track,bass_sequence)
local lead = instrument:vst3 {id="serum_lead",plugin=SERUM,paths=plugin_paths,
  patch_bank=serum_bank_root.."/lead",initial_patch="lead_cutting"}:connect_track(lead_track,lead_sequence)
local drone = instrument:vst3 {id="serum_drone",plugin=SERUM,paths=plugin_paths,
  patch_bank=serum_bank_root.."/drone",initial_patch="drone_evolving"}:connect_track(drone_track,drone_sequence)

local pulse = instrument:vst3 {id="pulse_drums",plugin=PULSE,paths=plugin_paths,midi_channels={10,10,10,10},state_file=os.getenv("PULSE_STATE")}
pulse:connect_drum_kit(kit)
local pulse_info = {}; for _, info in ipairs(pulse:parameter_info()) do pulse_info[info.name]=info end
local function pulse_id(name, expected)
  local info=assert(pulse_info[name],"PULSE parameter unavailable: "..name)
  assert(info.id==expected,"PULSE parameter ID changed: "..name)
  return info.id
end
local P={
  kick_freq=pulse_id("KICK Synth Freq",1469782605),kick_level=pulse_id("KICK Synth Level",1709986221),kick_noise=pulse_id("KICK Noise Level",1387348517),kick_volume=pulse_id("KICK Volume",1107180885),
  snare_freq=pulse_id("SNARE Synth Freq",1273269100),snare_noise=pulse_id("SNARE Noise Level",1727377702),snare_tone=pulse_id("SNARE Noise Tone",2009332974),snare_volume=pulse_id("SNARE Volume",1108104406),
  hat_filter=pulse_id("HAT Noise Filter",1812396582),hat_noise=pulse_id("HAT Noise Level",2067406887),hat_tone=pulse_id("HAT Noise Tone",1812819469),hat_volume=pulse_id("HAT Volume",1109027927),
  perc_freq=pulse_id("PERC Synth Freq",880242090),perc_wave=pulse_id("PERC Synth Wave",880732715),perc_noise=pulse_id("PERC Noise Level",259952424),perc_volume=pulse_id("PERC Volume",1109951448),
}
for _,part in ipairs({"KICK","SNARE","HAT","PERC"}) do
  pulse:set_parameter(assert(pulse_info[part.." S1 Level"]).id,0)
  pulse:set_parameter(assert(pulse_info[part.." S2 Level"]).id,0)
end
local pulse_initial={kick_freq=.18,kick_level=.88,kick_noise=.035,kick_volume=.70,snare_freq=.38,snare_noise=.67,snare_tone=.54,snare_volume=.61,hat_filter=.61,hat_noise=.70,hat_tone=.66,hat_volume=.48,perc_freq=.49,perc_wave=.58,perc_noise=.24,perc_volume=.50}
for name,value in pairs(pulse_initial) do pulse:set_parameter(P[name],value) end

-- Public controls are ordinary compiled graph controls. Browser, MIDI,
-- automation, and Lua all converge on the same handles.
local control_ids={bass={},lead={},drone={},drums={},master={},macros={}}
local function bind(group,id,label,target,min,max,targets)
  instrument:control {id=id,label=label,target=target,targets=targets or {},range={min or 0,max or 1}}
  group[#group+1]=id
end
local function serum_controls(prefix,plugin,group)
  local function patch_parameter(id) return plugin:parameter_targets(id) end
  local function patch_bind(id,label,parameter)
    local first,rest=patch_parameter(parameter); bind(group,id,label,first,0,1,rest)
  end
  patch_bind(prefix.."_level","Level",0)
  patch_bind(prefix.."_a_position","Osc A Position",1000039)
  patch_bind(prefix.."_b_position","Osc B Position",1001039)
  patch_bind(prefix.."_a_warp","Osc A Warp",1000034)
  patch_bind(prefix.."_b_warp","Osc B Warp",1001034)
  patch_bind(prefix.."_cutoff","Filter Cutoff",2000003)
  patch_bind(prefix.."_resonance","Filter Resonance",2000004)
  patch_bind(prefix.."_drive","Filter Drive",2000005)
  patch_bind(prefix.."_attack","Attack",3000000)
  patch_bind(prefix.."_decay","Decay",3000002)
  patch_bind(prefix.."_sustain","Sustain",3000003)
  patch_bind(prefix.."_release","Release",3000004)
  patch_bind(prefix.."_portamento","Portamento",3)
  patch_bind(prefix.."_macro1","Macro 1",7000000)
  patch_bind(prefix.."_macro2","Macro 2",7001000)
end
serum_controls("bass",bass,control_ids.bass)
serum_controls("lead",lead,control_ids.lead)
serum_controls("drone",drone,control_ids.drone)
bind(control_ids.bass,"bass_patch","Serum Patch",bass.preset,0,#bass.patch_names-1)
bind(control_ids.lead,"lead_patch","Serum Patch",lead.preset,0,#lead.patch_names-1)
bind(control_ids.drone,"drone_patch","Serum Patch",drone.preset,0,#drone.patch_names-1)
for name,id in pairs(P) do bind(control_ids.drums,"pulse_"..name,name,pulse:parameter(id),0,1) end

-- Kick-triggered deterministic bass ducking is explicit and bounded. PULSE is
-- still the sole percussion generator.
local duck_l=instrument:ducker {threshold=.04,depth=.68,attack=.0015,release=.115}
local duck_r=instrument:ducker {threshold=.04,depth=.68,attack=.0015,release=.115}
instrument:connect(bass.left,duck_l.input); instrument:connect(bass.right,duck_r.input)
instrument:connect(kit.voices[1].track.gate,duck_l.sidechain); instrument:connect(kit.voices[1].track.gate,duck_r.sidechain)
bind(control_ids.bass,"duck_depth","Kick Duck",duck_l.depth,0,1,{duck_r.depth})
bind(control_ids.bass,"duck_release","Duck Release",duck_l.release,.001,3,{duck_r.release})

local function part_mute(id,label,source)
  local left=instrument:mute {muted=0}
  local right=instrument:mute {muted=0}
  instrument:connect(source.left,left.input);instrument:connect(source.right,right.input)
  bind(control_ids.master,"master_mute_"..id,label.." Mute",left.muted,0,1,{right.muted})
  return {left=left.out,right=right.out,direction="stereo_output"}
end
local muted_drums=part_mute("drums","DRUMS",pulse.out)
local muted_bass=part_mute("bass","BASS",{left=duck_l.out,right=duck_r.out,direction="stereo_output"})
local muted_lead=part_mute("lead","LEAD",lead.out)
local muted_drone=part_mute("drone","DRONE",drone.out)

local mix=instrument:mixer {inputs=8,headroom=.55}
instrument:connect(muted_drums,mix.input[1])
instrument:connect(muted_bass,mix.input[2])
instrument:connect(muted_lead,mix.input[3]); instrument:connect(muted_drone,mix.input[4])

local compressor=instrument:vst3_effect {id="master_compressor",plugin=LSP_COMPRESSOR,paths=plugin_paths}
local limiter=instrument:vst3_effect {id="master_limiter",plugin=LSP_LIMITER,paths=plugin_paths}
local C={bypass=490312032,threshold=420714220,attack=421003508,release=493070452,ratio=429409650,knee=463178862,makeup=471548843}
local L={bypass=490312032,threshold=501114984,lookahead=467309611,release=493070452,gain_boost=701868353}
compressor:set_parameter(C.bypass,0);compressor:set_parameter(C.threshold,.79);compressor:set_parameter(C.ratio,.30);compressor:set_parameter(C.attack,.70);compressor:set_parameter(C.release,.76);compressor:set_parameter(C.knee,.75);compressor:set_parameter(C.makeup,.50)
limiter:set_parameter(L.bypass,0);limiter:set_parameter(L.threshold,.985);limiter:set_parameter(L.lookahead,.74);limiter:set_parameter(L.release,.72);limiter:set_parameter(L.gain_boost,0)
instrument:connect(mix.left,compressor.input.left);instrument:connect(mix.right,compressor.input.right)
instrument:connect(compressor.out.left,limiter.input.left);instrument:connect(compressor.out.right,limiter.input.right)
instrument:connect(limiter.out.left,instrument.output.left);instrument:connect(limiter.out.right,instrument.output.right)
for name,id in pairs(C) do bind(control_ids.master,"compressor_"..name,"Compressor "..name,compressor:parameter(id),0,1) end
for name,id in pairs(L) do bind(control_ids.master,"limiter_"..name,"Limiter "..name,limiter:parameter(id),0,1) end

-- Cross-instrument macros fan out to every resident patch so switching never reveals stale
-- control values. Preset-dependent modulation routings can still make a macro less audible.
local function macro_bind(id, assignments)
  local targets={}
  for _,assignment in ipairs(assignments) do
    local first,rest=assignment[1]:parameter_targets(assignment[2])
    targets[#targets+1]=first
    for _,target in ipairs(rest) do targets[#targets+1]=target end
  end
  local first=table.remove(targets,1)
  bind(control_ids.macros,id,id,first,0,1,targets)
end
macro_bind("PRESSURE",{{bass,1000001},{bass,1001001},{bass,2000006}})
macro_bind("HEAT",{{lead,1000001},{lead,1001001},{lead,2000006}})
macro_bind("MOTION",{{drone,1000031},{drone,1001031},{drone,7002000}})
macro_bind("SPACE",{{drone,2000007},{lead,2000007},{drone,2000001}})

-- Ten arrangement sections. Bar 257 intentionally has no stop command: live
-- playback settles into the final patterns, while an offline render chooses
-- its explicit duration (480 seconds plus tail).
local bass_timeline=bass_sequence:timeline("Bass Arrangement")
bass_timeline:at_bar(1,bass_patterns[7]):at_bar(17,bass_patterns[1]):at_bar(33,bass_patterns[3]):at_bar(65,bass_patterns[2]):at_bar(97,bass_patterns[7]):at_bar(113,bass_patterns[1]):at_bar(145,bass_patterns[4]):at_bar(177,bass_patterns[5]):at_bar(193,bass_patterns[6]):at_bar(225,bass_patterns[8])
bass_timeline:automation {target="bass_cutoff",interpolation="smooth"}:at_bar(1,.20):at_bar(33,.43):at_bar(64,.70):at_bar(97,.16):at_bar(144,.58):at_bar(176,.34):at_bar(224,.78):at_bar(256,.18)
bass_timeline:automation {target="bass_drive",interpolation="linear"}:at_bar(1,.18):at_bar(65,.52):at_bar(112,.28):at_bar(193,.72):at_bar(256,.16)

local lead_timeline=lead_sequence:timeline("Lead Arrangement")
lead_timeline:at_bar(1,lead_patterns[6]):at_bar(33,lead_patterns[1]):at_bar(65,lead_patterns[2]):at_bar(97,lead_patterns[6]):at_bar(113,lead_patterns[3]):at_bar(145,lead_patterns[4]):at_bar(177,lead_patterns[6]):at_bar(193,lead_patterns[5]):at_bar(225,lead_patterns[6])
lead_timeline:automation {target="lead_level",interpolation="smooth"}:at_bar(1,.04):at_bar(33,.16):at_bar(65,.31):at_bar(97,.03):at_bar(113,.22):at_bar(193,.34):at_bar(225,.18):at_bar(256,.02)
lead_timeline:automation {target="lead_cutoff",interpolation="smooth"}:at_bar(1,.22):at_bar(64,.66):at_bar(96,.82):at_bar(112,.18):at_bar(224,.76):at_bar(256,.24)

local drone_timeline=drone_sequence:timeline("Drone Arrangement")
drone_timeline:at_bar(1,drone_patterns[1]):at_bar(17,drone_patterns[2]):at_bar(65,drone_patterns[4]):at_bar(97,drone_patterns[3]):at_bar(113,drone_patterns[1]):at_bar(145,drone_patterns[4]):at_bar(177,drone_patterns[5]):at_bar(193,drone_patterns[6]):at_bar(225,drone_patterns[2])
drone_timeline:automation {target="drone_a_position",interpolation="smooth"}:at_bar(1,.12):at_bar(64,.62):at_bar(112,.31):at_bar(176,.80):at_bar(224,.47):at_bar(256,.08)
drone_timeline:automation {target="drone_cutoff",interpolation="smooth"}:at_bar(1,.20):at_bar(96,.44):at_bar(112,.14):at_bar(224,.52):at_bar(256,.12)

local drum_timeline=drum_sequence:timeline("Drum Arrangement")
drum_timeline:at_bar(1,drum_patterns[4]):at_bar(9,drum_patterns[1]):at_bar(17,drum_patterns[1]):at_bar(33,drum_patterns[2]):at_bar(65,drum_patterns[3]):at_bar(97,drum_patterns[4]):at_bar(113,drum_patterns[1]):at_bar(145,drum_patterns[2]):at_bar(177,drum_patterns[4]):at_bar(193,drum_patterns[5]):at_bar(225,drum_patterns[6]):at_bar(249,drum_patterns[4])
drum_timeline:automation {target="pulse_hat_filter",interpolation="smooth"}:at_bar(1,.20):at_bar(64,.72):at_bar(112,.24):at_bar(176,.66):at_bar(224,.84):at_bar(256,.14)
drum_timeline:automation {target="pulse_perc_freq",interpolation="linear"}:at_bar(1,.31):at_bar(96,.62):at_bar(144,.22):at_bar(224,.78):at_bar(256,.30)

local server=meldritch.server {host=os.getenv("PRESSURE_HOST") or "127.0.0.1",port=tonumber(os.getenv("PRESSURE_PORT") or "8080")}
server:websocket("/performance",{})
local performance=server:performance {name="Pressure System Live"}
performance:role("conductor",{controls="*"})
performance:role("bass",{controls={"bass_patch","bass_level","bass_a_position","bass_b_position","bass_a_warp","bass_b_warp","bass_cutoff","bass_resonance","bass_drive","bass_portamento","duck_depth","duck_release"}})
performance:role("lead",{controls={"lead_patch","lead_level","lead_a_position","lead_b_position","lead_a_warp","lead_cutoff","lead_resonance","lead_drive","lead_portamento","lead_macro1","lead_macro2"}})
performance:role("drone",{controls={"drone_patch","drone_level","drone_a_position","drone_b_position","drone_a_warp","drone_b_warp","drone_cutoff","drone_resonance","drone_drive","drone_macro1","drone_macro2"}})
performance:role("drummer",{controls={"pulse_kick_freq","pulse_kick_level","pulse_kick_noise","pulse_kick_volume","pulse_snare_freq","pulse_snare_noise","pulse_snare_tone","pulse_hat_filter","pulse_hat_tone","pulse_perc_freq","pulse_perc_wave","pulse_perc_noise"}})
performance:role("observer",{controls={}})
server:credential("pressure-conductor","conductor")
server:credential("pressure-bass","bass")
server:credential("pressure-lead","lead")
server:credential("pressure-drone","drone")
server:credential("pressure-drums","drummer")
server:credential("pressure-observer","observer")

local bass_plane=performance:plane {id="bass",label="SERUM // BASS",roles={"bass","conductor"},read_only_roles={"observer"}}
bass_plane:piano_roll {id="bass_roll",label="Bass Phrase",track=bass_track,steps=16,pitch_min=24,pitch_max=48}
bass_plane:pattern_selector {id="bass_patterns",label="Bass Pattern",sequencer=bass_sequence,boundary="bar"}
bass_plane:preset_selector {id="bass_patch",label="Serum Patch",target="bass_patch",presets=bass.patch_names}
for _,item in ipairs({{"bass_level","Level"},{"bass_a_position","A Position"},{"bass_b_position","B Position"},{"bass_a_warp","A Warp"},{"bass_b_warp","B Warp"},{"bass_cutoff","Cutoff"},{"bass_resonance","Resonance"},{"bass_drive","Drive"},{"bass_portamento","Portamento"},{"duck_depth","Kick Duck"},{"duck_release","Duck Release"}}) do bass_plane:knob {id=item[1],label=item[2],target=item[1],min=item[1]=="duck_release" and .001 or 0,max=item[1]=="duck_release" and 3 or 1} end
bass_plane:adsr {id="bass_adsr",label="Amp Envelope",normalized=true,attack="bass_attack",decay="bass_decay",sustain="bass_sustain",release="bass_release"}

local lead_plane=performance:plane {id="lead",label="SERUM // LEAD",roles={"lead","conductor"},read_only_roles={"observer"}}
lead_plane:piano_roll {id="lead_roll",label="Lead Phrase",track=lead_track,steps=16,pitch_min=48,pitch_max=84}
lead_plane:pattern_selector {id="lead_patterns",label="Lead Pattern",sequencer=lead_sequence,boundary="bar"}
lead_plane:preset_selector {id="lead_patch",label="Serum Patch",target="lead_patch",presets=lead.patch_names}
for _,item in ipairs({{"lead_level","Level"},{"lead_a_position","A Position"},{"lead_b_position","B Position"},{"lead_a_warp","A Warp"},{"lead_cutoff","Cutoff"},{"lead_resonance","Resonance"},{"lead_drive","Drive"},{"lead_portamento","Portamento"},{"lead_macro1","Macro 1"},{"lead_macro2","Macro 2"}}) do lead_plane:knob {id=item[1],label=item[2],target=item[1],min=0,max=1} end
lead_plane:adsr {id="lead_adsr",label="Amp Envelope",normalized=true,attack="lead_attack",decay="lead_decay",sustain="lead_sustain",release="lead_release"}

local drone_plane=performance:plane {id="drone",label="SERUM // DRONE",roles={"drone","conductor"},read_only_roles={"observer"}}
drone_plane:piano_roll {id="drone_roll",label="Drone Voicing",track=drone_track,steps=16,pitch_min=24,pitch_max=60}
drone_plane:pattern_selector {id="drone_modes",label="Drone State",sequencer=drone_sequence,boundary="bar"}
drone_plane:preset_selector {id="drone_patch",label="Serum Patch",target="drone_patch",presets=drone.patch_names}
for _,item in ipairs({{"drone_level","Level"},{"drone_a_position","A Position"},{"drone_b_position","B Position"},{"drone_a_warp","A Warp"},{"drone_b_warp","B Warp"},{"drone_cutoff","Cutoff"},{"drone_resonance","Resonance"},{"drone_drive","Drive"},{"drone_macro1","Motion"},{"drone_macro2","Width"}}) do drone_plane:knob {id=item[1],label=item[2],target=item[1],min=0,max=1} end
drone_plane:adsr {id="drone_adsr",label="Long Envelope",normalized=true,attack="drone_attack",decay="drone_decay",sustain="drone_sustain",release="drone_release"}

local drum_plane=performance:plane {id="drums",label="PULSE // DRUM MACHINE",roles={"drummer","conductor"},read_only_roles={"observer"}}
drum_plane:drum_grid {id="pulse_grid",label="PULSE Grid",kit=kit,steps=16}
drum_plane:pattern_selector {id="drum_patterns",label="Groove",sequencer=drum_sequence,boundary="bar"}
for _,item in ipairs({{"pulse_kick_freq","Kick Pitch"},{"pulse_kick_level","Kick Body"},{"pulse_kick_noise","Kick Noise"},{"pulse_kick_volume","Kick Level"},{"pulse_snare_freq","Snare Tone"},{"pulse_snare_noise","Snare Noise"},{"pulse_snare_tone","Snare Color"},{"pulse_hat_filter","Hat Filter"},{"pulse_hat_tone","Hat Color"},{"pulse_perc_freq","Metal Pitch"},{"pulse_perc_wave","Metal Wave"},{"pulse_perc_noise","Metal Noise"}}) do drum_plane:knob {id=item[1],label=item[2],target=item[1],min=0,max=1} end
drum_plane:trigger {id="IMPACT",label="IMPACT / FILL",triggered=function() drum_sequence:command("pattern:4") end}

local master_plane=performance:plane {id="master",label="MASTER // LSP DYNAMICS",roles={"conductor"},read_only_roles={"observer"}}
for _,item in ipairs({{"master_mute_drums","DRUMS"},{"master_mute_bass","BASS"},{"master_mute_lead","LEAD"},{"master_mute_drone","DRONE"}}) do master_plane:toggle {id=item[1],label=item[2].." MUTE",target=item[1],min=0,max=1} end
for _,item in ipairs({{"compressor_threshold","Compressor Threshold"},{"compressor_ratio","Ratio"},{"compressor_attack","Attack"},{"compressor_release","Release"},{"compressor_knee","Knee"},{"compressor_makeup","Makeup"},{"compressor_bypass","Comp Bypass"},{"limiter_threshold","Limiter Ceiling"},{"limiter_lookahead","Lookahead"},{"limiter_release","Limiter Release"},{"limiter_bypass","Limiter Bypass"}}) do master_plane:knob {id=item[1],label=item[2],target=item[1],min=0,max=1} end

local macro_plane=performance:plane {id="macros",label="PRESSURE SYSTEM // MACROS",roles={"conductor"},read_only_roles={"observer"}}
for _,id in ipairs({"PRESSURE","HEAT","MOTION","SPACE"}) do macro_plane:fader {id=id,label=id,target=id,min=0,max=1} end
macro_plane:trigger {id="ADVANCE",label="ADVANCE / OVERDRIVE",triggered=function() bass_sequence:switch_pattern("Overdrive","bar");lead_sequence:switch_pattern("Ascension","bar");drone_sequence:switch_pattern("Event Horizon","bar");drum_sequence:switch_pattern("Overpressure","bar") end}
macro_plane:trigger {id="STOP",label="STOP",triggered=function() bass_sequence:stop();lead_sequence:stop();drone_sequence:stop();drum_sequence:stop() end}
macro_plane:trigger {id="RESTART",label="RESTART",triggered=function() bass_sequence:restart();lead_sequence:restart();drone_sequence:restart();drum_sequence:restart() end}

return instrument
