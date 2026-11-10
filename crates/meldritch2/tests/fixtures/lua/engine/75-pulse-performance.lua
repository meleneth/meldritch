-- PULSE Drum Synth + Serum 2 multi-performer electronic rhythm section.
-- PULSE is required at ~/.vst3/PULSE Drum Synth.vst3. Serum is optional:
-- when it is absent the synthesis plane uses a native Plaits voice.
local instrument = meldritch.instrument("PULSE Performance")
local home = assert(os.getenv("HOME"), "HOME is required")
local pulse_path = os.getenv("PULSE_VST3_PATH") or (home .. "/.vst3")
local PULSE = "ABCDEF019182FAEB506C7365506C6473"
local SERUM = "56534558667350736572756D20320000"

local sequence = instrument:sequencer { bpm=124, steps=16, steps_per_beat=4 }
local kit = sequence:drum_kit {
  name="PULSE",
  voices={
    {id="kick",label="Kick",note=36,channel=10,duration=1},
    {id="snare",label="Snare",note=38,channel=10,duration=1},
    {id="hat",label="Hat",note=42,channel=10,duration=1},
    {id="percussion",label="Percussion",note=46,channel=10,duration=1},
  },
}
local synth_track = sequence:note_track("Synth", {voices=4})

local floor = sequence:pattern("Pressure Floor")
floor:drums(kit, {
  kick={{step=1,velocity=127},{step=5,velocity=112},{step=9,velocity=124},{step=13,velocity=116}},
  snare={{step=5,velocity=115},{step=13,velocity=124}},
  hat={{step=1,velocity=80},{step=3,velocity=55},{step=5,velocity=88},{step=7,velocity=60},{step=9,velocity=78},{step=11,velocity=57},{step=13,velocity=92},{step=15,velocity=64}},
  percussion={{step=4,velocity=62},{step=8,velocity=70},{step=11,velocity=54},{step=16,velocity=82}},
},16)
floor:notes(synth_track, {
  {id=1,step=1,pitch="C2",velocity=108,duration=3},
  {id=2,step=5,pitch="C2",velocity=94,duration=2},
  {id=3,step=9,pitch="D#2",velocity=104,duration=3},
  {id=4,step=13,pitch="G2",velocity=98,duration=2},
},16)

local breakbeat = sequence:pattern("Pressure Break")
breakbeat:drums(kit, {
  kick={{step=1,velocity=127},{step=4,velocity=88},{step=7,velocity=118},{step=11,velocity=108},{step=15,velocity=122}},
  snare={{step=5,velocity=122},{step=10,velocity=78},{step=13,velocity=127},{step=16,velocity=98}},
  hat={{step=1,velocity=66},{step=2,velocity=49},{step=3,velocity=72},{step=5,velocity=90},{step=6,velocity=52},{step=7,velocity=76},{step=9,velocity=68},{step=11,velocity=84},{step=12,velocity=54},{step=13,velocity=94},{step=15,velocity=74},{step=16,velocity=58}},
  percussion={{step=3,velocity=76},{step=6,velocity=64},{step=8,velocity=91},{step=12,velocity=70},{step=14,velocity=84},{step=16,velocity=104}},
},16)
breakbeat:notes(synth_track, {
  {id=1,step=1,pitch="F2",velocity=112,duration=3},
  {id=2,step=4,pitch="G2",velocity=98,duration=2},
  {id=3,step=7,pitch="G#2",velocity=108,duration=3},
  {id=4,step=11,pitch="C3",velocity=116,duration=2},
  {id=5,step=14,pitch="G2",velocity=102,duration=2},
},16)
sequence:switch_pattern("Pressure Floor")

-- PULSE's runtime parameter IDs are JUCE's stable VST3 IDs. The assertions
-- deliberately fail if a future PULSE release changes the inventory.
local pulse = instrument:vst3 {
  plugin=PULSE, paths={pulse_path}, midi_channels={10,10,10,10},
  state_file=os.getenv("PULSE_STATE"),
}
pulse:connect_drum_kit(kit)
local info_by_name = {}
for _, info in ipairs(pulse:parameter_info()) do info_by_name[info.name] = info end
local function require_parameter(name, expected)
  local info = assert(info_by_name[name], "PULSE parameter is unavailable: " .. name)
  assert(info.id == expected, "PULSE parameter ID changed for " .. name)
  assert(info.automatable and not info.read_only, "PULSE parameter is not writable: " .. name)
  return info.id
end

local ids = {
  kick_freq=require_parameter("KICK Synth Freq",1469782605),
  kick_level=require_parameter("KICK Synth Level",1709986221),
  kick_noise=require_parameter("KICK Noise Level",1387348517),
  kick_volume=require_parameter("KICK Volume",1107180885),
  kick_a=require_parameter("KICK Env2 A",337125317), kick_d=require_parameter("KICK Env2 D",337125320),
  kick_s=require_parameter("KICK Env2 S",337125335), kick_r=require_parameter("KICK Env2 R",337125334),
  snare_freq=require_parameter("SNARE Synth Freq",1273269100),
  snare_noise=require_parameter("SNARE Noise Level",1727377702),
  snare_tone=require_parameter("SNARE Noise Tone",2009332974),
  snare_volume=require_parameter("SNARE Volume",1108104406),
  snare_a=require_parameter("SNARE Env3 A",2079936613), snare_d=require_parameter("SNARE Env3 D",2079936616),
  snare_s=require_parameter("SNARE Env3 S",2079936631), snare_r=require_parameter("SNARE Env3 R",2079936630),
  hat_filter=require_parameter("HAT Noise Filter",1812396582),
  hat_noise=require_parameter("HAT Noise Level",2067406887),
  hat_tone=require_parameter("HAT Noise Tone",1812819469),
  hat_volume=require_parameter("HAT Volume",1109027927),
  hat_a=require_parameter("HAT Env3 A",1675263300), hat_d=require_parameter("HAT Env3 D",1675263303),
  hat_s=require_parameter("HAT Env3 S",1675263318), hat_r=require_parameter("HAT Env3 R",1675263317),
  perc_freq=require_parameter("PERC Synth Freq",880242090),
  perc_wave=require_parameter("PERC Synth Wave",880732715),
  perc_noise=require_parameter("PERC Noise Level",259952424),
  perc_volume=require_parameter("PERC Volume",1109951448),
  perc_a=require_parameter("PERC Env2 A",1270589026), perc_d=require_parameter("PERC Env2 D",1270589029),
  perc_s=require_parameter("PERC Env2 S",1270589044), perc_r=require_parameter("PERC Env2 R",1270589043),
}

-- Synthesis-only initialization avoids external sample dependencies.
for _, part in ipairs({"KICK","SNARE","HAT","PERC"}) do
  pulse:set_parameter(assert(info_by_name[part .. " S1 Level"]).id,0)
  pulse:set_parameter(assert(info_by_name[part .. " S2 Level"]).id,0)
end
local variant = os.getenv("PULSE_TONE_VARIANT") or "bright"
local initial = variant == "dark" and {
  kick_freq=.10,kick_level=.78,kick_noise=.03,kick_volume=.72,
  snare_freq=.24,snare_noise=.58,snare_tone=.28,snare_volume=.67,
  hat_filter=.25,hat_noise=.62,hat_tone=.30,hat_volume=.55,
  perc_freq=.19,perc_wave=.33,perc_noise=.20,perc_volume=.58,
} or {
  kick_freq=.24,kick_level=.88,kick_noise=.08,kick_volume=.76,
  snare_freq=.42,snare_noise=.72,snare_tone=.61,snare_volume=.70,
  hat_filter=.68,hat_noise=.78,hat_tone=.72,hat_volume=.58,
  perc_freq=.56,perc_wave=.66,perc_noise=.28,perc_volume=.62,
}
for name, value in pairs(initial) do pulse:set_parameter(ids[name],value) end

local drum_control_ids = {}
local function pulse_control(name,label,public)
  instrument:control {id=name,label=label,target=pulse:parameter(ids[name]),range={0,1}}
  if public ~= false then drum_control_ids[#drum_control_ids+1]=name end
end
for _, control in ipairs({
  {"kick_freq","Kick Pitch"},{"kick_level","Kick Oscillator"},{"kick_noise","Kick Noise"},{"kick_volume","Kick Level"},
  {"snare_freq","Snare Tone"},{"snare_noise","Snare Noise"},{"snare_tone","Snare Color"},{"snare_volume","Snare Level"},
  {"hat_filter","Hat Filter"},{"hat_noise","Hat Noise"},{"hat_tone","Hat Color"},{"hat_volume","Hat Level"},
  {"perc_freq","Perc Pitch"},{"perc_wave","Perc Waveform"},{"perc_noise","Perc Noise"},{"perc_volume","Perc Level"},
}) do pulse_control(control[1],control[2]) end
for _, part in ipairs({"kick","snare","hat","perc"}) do
  for _, suffix in ipairs({"a","d","s","r"}) do
    -- ADSR commands are authorized through their public plane controls below;
    -- these target IDs are internal compiled controls, not role-visible IDs.
    pulse_control(part .. "_" .. suffix,part .. " envelope " .. suffix,false)
  end
end

-- Serum coexists when installed; a native voice preserves the complete demo
-- and the synth-performer role on systems that only have PULSE.
local serum_enabled = os.getenv("PULSE_DISABLE_SERUM") ~= "1"
local serum_ok, serum = false, "disabled by PULSE_DISABLE_SERUM"
if serum_enabled then serum_ok, serum = pcall(function()
  local plugin = instrument:vst3 {plugin=SERUM}
  plugin:connect_track(synth_track,sequence)
  plugin:set_parameter(0,.44)
  plugin:set_parameter(2000003,.52)
  plugin:set_parameter(2000004,.26)
  return plugin
end)
end
local synth_output
if serum_ok then
  instrument:control {id="synth_tone",label="Serum Cutoff",target=serum:parameter(2000003),range={0,1}}
  instrument:control {id="synth_color",label="Serum Resonance",target=serum:parameter(2000004),range={0,1}}
  synth_output=serum.out
else
  io.stderr:write("Using native Plaits fallback: " .. tostring(serum) .. "\n")
  local voice=instrument:plaits {model="fm",frequency=110,harmonics=.45,timbre=.55,morph=.35}
  local env=instrument:adsr {attack=.004,decay=.18,sustain=.45,release=.12}
  local amp=instrument:vca {}
  instrument:connect(synth_track.voices[1].pitch,voice.pitch)
  instrument:connect(synth_track.voices[1].gate,voice.gate)
  instrument:connect(synth_track.voices[1].velocity,voice.velocity)
  instrument:connect(synth_track.voices[1].gate,env.gate)
  instrument:connect(voice.out,amp.input); instrument:connect(env.out,amp.cv)
  instrument:control {id="synth_tone",label="Native Timbre",target=voice.timbre,range={0,1}}
  instrument:control {id="synth_color",label="Native Morph",target=voice.morph,range={0,1}}
  synth_output={left=amp.out,right=amp.out,direction="stereo_output"}
end

local mix=instrument:mixer {inputs=8,headroom=.42}
instrument:connect(pulse.out,mix.input[1]); instrument:connect(synth_output,mix.input[2])
instrument:connect(mix.left,instrument.output.left); instrument:connect(mix.right,instrument.output.right)

-- The offline arrangement switches patterns and drives real VST3 process-time
-- parameter changes. Live browser changes use the same control path.
local arrangement=sequence:timeline("PULSE Evolution")
arrangement:at_bar(1,floor):at_bar(7,breakbeat):at_bar(11,floor):stop_at_bar(14)
arrangement:automation {target="kick_freq",interpolation="smooth"}:at_bar(1,initial.kick_freq):at_bar(6,.48):at_bar(10,.16):at_bar(13,.34)
arrangement:automation {target="hat_d",interpolation="linear"}:at_bar(1,.42):at_bar(5,.76):at_bar(9,.18):at_bar(13,.55)

local server=meldritch.server {host="127.0.0.1",port=8080}
server:websocket("/performance",{})
local performance=server:performance {name="PULSE Performance"}
performance:role("drummer",{controls=drum_control_ids})
performance:role("synth",{controls={"synth_tone","synth_color"}})
performance:role("leader",{controls="*"})
performance:role("observer",{controls={}})
server:credential("drummer-demo-token","drummer")
server:credential("synth-demo-token","synth")
server:credential("leader-demo-token","leader")
server:credential("observer-demo-token","observer")

local rhythm=performance:plane {id="rhythm",label="PULSE Rhythm",roles={"drummer","leader"},read_only_roles={"observer"}}
rhythm:drum_grid {id="pulse_grid",label="Four-Part Drum Grid",kit=kit,steps=16}
rhythm:pattern_selector {id="pulse_patterns",label="Drum Patterns",sequencer=sequence,boundary="bar"}

local sound=performance:plane {id="pulse_sound",label="PULSE Synthesis",roles={"drummer","leader"},read_only_roles={"observer"}}
for _, part in ipairs({
  {"kick","Kick",{"freq","level","noise","volume"}},
  {"snare","Snare",{"freq","noise","tone","volume"}},
  {"hat","Hat",{"filter","noise","tone","volume"}},
  {"perc","Percussion",{"freq","wave","noise","volume"}},
}) do
  for _, suffix in ipairs(part[3]) do
    local id=part[1] .. "_" .. suffix
    sound:knob {id=id,label=part[2] .. " " .. suffix,target=id,min=0,max=1,unit="normalized"}
  end
  sound:adsr {id=part[1] .. "_envelope",label=part[2] .. " Envelope",normalized=true,
    attack=part[1] .. "_a",decay=part[1] .. "_d",sustain=part[1] .. "_s",release=part[1] .. "_r"}
end

local synth_plane=performance:plane {id="synth",label=serum_ok and "Serum 2" or "Native Synth",roles={"synth","leader"},read_only_roles={"observer"}}
synth_plane:piano_roll {id="synth_notes",label="Bass Notes",track=synth_track,steps=16,pitch_min=24,pitch_max=60}
synth_plane:pattern_selector {id="synth_patterns",label="Synth Patterns",sequencer=sequence,boundary="bar"}
synth_plane:fader {id="synth_tone",label="Tone",target="synth_tone",min=0,max=1}
synth_plane:knob {id="synth_color",label="Color",target="synth_color",min=0,max=1}

local leader=performance:plane {id="leader",label="Arrangement",roles={"leader"},read_only_roles={"observer"}}
leader:pattern_selector {id="arrangement",label="Pattern",sequencer=sequence,boundary="bar"}
leader:trigger {id="restart",label="RESTART",triggered=function() sequence:restart() end}
leader:trigger {id="stop",label="STOP",triggered=function() sequence:stop() end}
return instrument
