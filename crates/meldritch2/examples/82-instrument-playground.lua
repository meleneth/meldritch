-- Instrument Playground: two independent PULSE machines and four hot-swappable
-- Serum 2 instruments. Rust owns clocks, notes, plugin instances and preset state.
local instrument=meldritch.instrument("Meldritch Instrument Playground")
local home=assert(os.getenv("HOME"),"HOME is required")
local SERUM="56534558667350736572756D20320000"
local PULSE="ABCDEF019182FAEB506C7365506C6473"
local paths={home.."/.vst3","/usr/lib/vst3"}
local catalog=os.getenv("MELDRITCH_SERUM_CATALOG") or home.."/.local/share/meldritch2/patches/serum2/factory"
instrument:voice_mode("poly")
instrument:devices {midi_input=os.getenv("MICROLAB_MIDI") or "MicroLab mk3:MicroLab mk3",
  midi_controls={os.getenv("MICROLAB_MIDI") or "MicroLab mk3:MicroLab mk3"},
  apc_mini=os.getenv("APC_MINI_MIDI") or "APC mini mk2:APC mini mk2 Control"}

local function sequence(name,steps)
  return instrument:sequencer {name=name,bpm=122,steps=steps or 32,steps_per_beat=4}
end
local seq808,seq909=sequence("PULSE 808",32),sequence("PULSE 909",32)
local seqbass,seqdrone,seql,seqarp=sequence("Serum Bass",32),sequence("Serum Drone",64),sequence("Serum Lead",32),sequence("Serum Arp",32)
local function kit(seq,name)
  return seq:drum_kit {name=name,voices={{id="kick",label="Kick",note=36,channel=10},{id="snare",label="Snare",note=38,channel=10},{id="hat",label="Hat",note=42,channel=10},{id="perc",label="Perc / Rim",note=46,channel=10}}}
end
local kit808,kit909=kit(seq808,"808-inspired"),kit(seq909,"909-inspired")
local function drums(seq,kit,name,lanes)
  local pattern=seq:pattern(name);pattern:drums(kit,lanes,32);return pattern
end
local patterns808={};for _,definition in ipairs({
  {"Foundation",{kick={{step=1,velocity=127},{step=9,velocity=112},{step=17,velocity=124},{step=25,velocity=116}},snare={{step=9,velocity=108},{step=25,velocity=116}},hat={{step=1,velocity=58},{step=5,velocity=48},{step=9,velocity=66},{step=13,velocity=51},{step=17,velocity=62},{step=21,velocity=48},{step=25,velocity=72},{step=29,velocity=52}},perc={{step=15,velocity=65},{step=31,velocity=79}}}},
  {"Syncopated",{kick={{step=1,velocity=127},{step=7,velocity=91},{step=12,velocity=114},{step=17,velocity=123},{step=23,velocity=96},{step=28,velocity=111}},snare={{step=9,velocity=115},{step=25,velocity=121}},hat={{step=3,velocity=52},{step=5,velocity=69},{step=11,velocity=56},{step=13,velocity=74},{step=19,velocity=54},{step=21,velocity=72},{step=27,velocity=58},{step=29,velocity=78}},perc={{step=8,velocity=72},{step=16,velocity=66},{step=24,velocity=81},{step=32,velocity=91}}}},
  {"Sparse",{kick={{step=1,velocity=119},{step=18,velocity=103}},snare={{step=25,velocity=89}},hat={{step=5,velocity=45},{step=13,velocity=51},{step=21,velocity=49},{step=29,velocity=57}},perc={{step=16,velocity=61}}}},
}) do patterns808[#patterns808+1]=drums(seq808,kit808,definition[1],definition[2]) end
local patterns909={};for _,definition in ipairs({
  {"Drive",{kick={{step=1,velocity=124},{step=5,velocity=101},{step=9,velocity=119},{step=13,velocity=106},{step=17,velocity=127},{step=21,velocity=99},{step=25,velocity=121},{step=29,velocity=109}},snare={{step=9,velocity=125},{step=25,velocity=127}},hat={{step=1,velocity=67},{step=3,velocity=48},{step=5,velocity=72},{step=7,velocity=52},{step=9,velocity=78},{step=11,velocity=51},{step=13,velocity=73},{step=15,velocity=55},{step=17,velocity=69},{step=19,velocity=50},{step=21,velocity=75},{step=23,velocity=54},{step=25,velocity=82},{step=27,velocity=56},{step=29,velocity=77},{step=31,velocity=62}},perc={{step=7,velocity=74},{step=15,velocity=86},{step=23,velocity=79},{step=31,velocity=94}}}},
  {"Broken",{kick={{step=1,velocity=127},{step=6,velocity=96},{step=14,velocity=112},{step=17,velocity=120},{step=22,velocity=101},{step=27,velocity=117}},snare={{step=9,velocity=121},{step=16,velocity=72},{step=25,velocity=126}},hat={{step=2,velocity=55},{step=4,velocity=71},{step=7,velocity=61},{step=10,velocity=79},{step=12,velocity=53},{step=15,velocity=75},{step=18,velocity=58},{step=20,velocity=73},{step=23,velocity=65},{step=26,velocity=82},{step=28,velocity=59},{step=31,velocity=77}},perc={{step=4,velocity=81},{step=12,velocity=69},{step=20,velocity=88},{step=28,velocity=76}}}},
  {"Half Time",{kick={{step=1,velocity=127},{step=12,velocity=104},{step=21,velocity=117}},snare={{step=17,velocity=127}},hat={{step=1,velocity=53},{step=5,velocity=61},{step=9,velocity=56},{step=13,velocity=68},{step=17,velocity=59},{step=21,velocity=66},{step=25,velocity=58},{step=29,velocity=73}},perc={{step=15,velocity=78},{step=31,velocity=91}}}},
}) do patterns909[#patterns909+1]=drums(seq909,kit909,definition[1],definition[2]) end
seq808:switch_pattern("Foundation");seq909:switch_pattern("Drive")

local bass_track=seqbass:note_track("Bass",{voices=4});local drone_track=seqdrone:note_track("Drone",{voices=4})
local lead_track=seql:note_track("Lead",{voices=3});local arp_track=seqarp:note_track("Arp",{voices=4})
local function notes(seq,track,name,events,length) local p=seq:pattern(name);p:notes(track,events,length);return p end
local bass1=notes(seqbass,bass_track,"Night Drive",{{step=1,pitch="C1",velocity=120,duration=3},{step=7,pitch="G1",velocity=96,duration=2},{step=9,pitch="A#1",velocity=111,duration=3},{step=15,pitch="G1",velocity=99,duration=2},{step=17,pitch="C1",velocity=124,duration=4},{step=25,pitch="D#1",velocity=108,duration=3},{step=30,pitch="G1",velocity=94,duration=2}},32)
local bass2=notes(seqbass,bass_track,"Climb",{{step=1,pitch="C1",velocity=121,duration=4},{step=9,pitch="D#1",velocity=108,duration=4},{step=17,pitch="F1",velocity=114,duration=4},{step=25,pitch="G1",velocity=124,duration=4}},32)
local drone1=notes(seqdrone,drone_track,"Atmosphere",{{step=1,pitch="C2",velocity=65,duration=64},{step=1,pitch="G2",velocity=52,duration=64},{step=33,pitch="D#3",velocity=43,duration=31}},64)
local drone2=notes(seqdrone,drone_track,"Suspended",{{step=1,pitch="A#1",velocity=58,duration=64},{step=1,pitch="F2",velocity=48,duration=64},{step=1,pitch="C3",velocity=41,duration=64}},64)
local lead1=notes(seql,lead_track,"Signal",{{step=3,pitch="G4",velocity=101,duration=2},{step=7,pitch="A#4",velocity=112,duration=2},{step=11,pitch="C5",velocity=119,duration=3},{step=16,pitch="D#5",velocity=108,duration=2},{step=21,pitch="G5",velocity=122,duration=3},{step=29,pitch="D#5",velocity=96,duration=2}},32)
local lead2=notes(seql,lead_track,"Answer",{{step=1,pitch="C5",velocity=112,duration=3},{step=9,pitch="G4",velocity=98,duration=4},{step=17,pitch="A#4",velocity=107,duration=3},{step=25,pitch="D#5",velocity=119,duration=5}},32)
local arp=meldritch.arp.new {mode="up_down",octaves=2,clock_division=2,gate=.65,velocity_shape={.82,.62,.91,.68}}
local arp_events={};for _,event in ipairs(arp:events(32,meldritch.arp.from_pattern({{step=1,notes={48,51,55}},{step=17,notes={46,50,55}}},32))) do arp_events[#arp_events+1]={step=event.step,pitch=event.note,velocity=math.floor(event.velocity*127),duration=1} end
local arp1=notes(seqarp,arp_track,"Glass Circuit",arp_events,32)
local arp2_engine=meldritch.arp.new {mode="down",octaves=2,clock_division=2,gate=.55,velocity_shape={.7,.9,.62,.84}};local arp2_events={};for _,event in ipairs(arp2_engine:events(32,{46,50,53,58})) do arp2_events[#arp2_events+1]={step=event.step,pitch=event.note,velocity=math.floor(event.velocity*127),duration=1} end
local arp2=notes(seqarp,arp_track,"Descending Texture",arp2_events,32)
seqbass:switch_pattern("Night Drive");seqdrone:switch_pattern("Atmosphere");seql:switch_pattern("Signal");seqarp:switch_pattern("Glass Circuit")
seq808:timeline("808 Arrangement"):at_bar(1,patterns808[1]):at_bar(9,patterns808[2]):at_bar(17,patterns808[1]):at_bar(25,patterns808[3]):at_bar(33,patterns808[2])
seq909:timeline("909 Arrangement"):at_bar(1,patterns909[1]):at_bar(9,patterns909[2]):at_bar(17,patterns909[3]):at_bar(25,patterns909[1]):at_bar(33,patterns909[2])
seqbass:timeline("Bass Arrangement"):at_bar(1,bass1):at_bar(17,bass2):at_bar(33,bass1)
seqdrone:timeline("Drone Arrangement"):at_bar(1,drone1):at_bar(17,drone2):at_bar(33,drone1)
seql:timeline("Lead Arrangement"):at_bar(1,lead1):at_bar(9,lead2):at_bar(17,lead1):at_bar(25,lead2):at_bar(33,lead1)
seqarp:timeline("Arp Arrangement"):at_bar(1,arp1):at_bar(17,arp2):at_bar(33,arp1)

local K={freq=1469782605,klevel=1709986221,knoise=1387348517,kvol=1107180885,kdecay=337125320,sfreq=1273269100,snoise=1727377702,stone=2009332974,svol=1108104406,sdecay=2079936616,hfilter=1812396582,hnoise=2067406887,htone=1812819469,hvol=1109027927,hdecay=1675263303,pfreq=880242090,pwave=880732715,pnoise=259952424,pvol=1109951448,pdecay=1270589029}
local function pulse_preset(name,v) return {name=name,parameters={[K.freq]=v[1],[K.klevel]=v[2],[K.knoise]=v[3],[K.kvol]=v[4],[K.kdecay]=v[5],[K.sfreq]=v[6],[K.snoise]=v[7],[K.stone]=v[8],[K.svol]=v[9],[K.sdecay]=v[10],[K.hfilter]=v[11],[K.hnoise]=v[12],[K.htone]=v[13],[K.hvol]=v[14],[K.hdecay]=v[15],[K.pfreq]=v[16],[K.pwave]=v[17],[K.pnoise]=v[18],[K.pvol]=v[19],[K.pdecay]=v[20]}} end
local bank808=instrument:vst3_managed_bank {id="pulse_808",plugin=PULSE,paths=paths,midi_channels={10,10,10,10},preset_bank="808-inspired",preset_category="Drums",presets={
  pulse_preset("Deep 808",{.16,.94,.02,.72,.82,.34,.68,.48,.55,.40,.63,.67,.58,.42,.24,.48,.52,.20,.43,.35}),pulse_preset("Tight 808",{.23,.88,.03,.68,.38,.42,.61,.61,.59,.25,.72,.64,.70,.45,.13,.57,.64,.18,.47,.20}),pulse_preset("Distorted 808",{.18,1,.16,.84,.67,.48,.79,.72,.70,.46,.78,.78,.81,.52,.31,.41,.83,.39,.61,.42}),pulse_preset("Soft 808",{.14,.73,.01,.59,.71,.29,.48,.42,.45,.52,.55,.51,.49,.35,.27,.38,.43,.12,.37,.39}),pulse_preset("Long Decay 808",{.13,.96,.025,.74,.96,.35,.63,.53,.53,.67,.61,.62,.57,.40,.56,.46,.55,.19,.42,.70})}}
local bank909=instrument:vst3_managed_bank {id="pulse_909",plugin=PULSE,paths=paths,midi_channels={10,10,10,10},preset_bank="909-inspired",preset_category="Drums",presets={
  pulse_preset("Classic 909-ish",{.31,.88,.08,.70,.43,.55,.72,.68,.68,.38,.75,.75,.74,.53,.36,.61,.68,.29,.55,.40}),pulse_preset("Punchy 909",{.38,.96,.06,.78,.28,.63,.78,.74,.75,.25,.81,.77,.80,.58,.24,.69,.72,.24,.60,.27}),pulse_preset("Industrial 909",{.29,1,.22,.86,.49,.71,.91,.85,.82,.58,.88,.89,.91,.67,.51,.54,.92,.56,.72,.61}),pulse_preset("Tight 909",{.42,.91,.04,.73,.19,.66,.70,.77,.69,.18,.84,.72,.82,.55,.15,.73,.75,.21,.57,.18}),pulse_preset("Aggressive 909",{.36,1,.18,.90,.34,.76,.94,.89,.88,.33,.92,.93,.94,.72,.28,.67,.98,.48,.76,.37})}}
for _,bank in ipairs({bank808,bank909}) do for _,instance in ipairs(bank.instances) do for _,info in ipairs(instance:parameter_info()) do if info.name:match(" S[12] Level$") then instance:set_parameter(info.id,0) end end end end
bank808:connect_drum_kit(kit808);bank909:connect_drum_kit(kit909)

local function serum(id,initial,track,seq) return instrument:vst3 {id=id,plugin=SERUM,paths=paths,preset_catalog=catalog,initial_preset=initial}:connect_track(track,seq) end
local bass=serum("serum_bass","factory_bass_reese_ba_gnarly_reese",bass_track,seqbass)
local drone=serum("serum_drone","factory_pad_pd_analog_silk_sweep",drone_track,seqdrone)
local lead=serum("serum_lead","factory_lead_ld_classic_techno_lead",lead_track,seql)
local texture=serum("serum_arp","factory_arp_arp_aardvark",arp_track,seqarp)
instrument:connect(instrument.note.pitch,lead.pitch_4);instrument:connect(instrument.note.gate,lead.gate_4);instrument:connect(instrument.note.velocity,lead.velocity_4)

local controls={};local function bind(id,label,target,min,max,targets) instrument:control {id=id,label=label,target=target,targets=targets or {},range={min or 0,max or 1}};controls[#controls+1]=id end
bind("pulse808_preset","PULSE 808 Preset",bank808.preset,0,4);bind("pulse909_preset","PULSE 909 Preset",bank909.preset,0,4)
for _,entry in ipairs({{"pulse808",bank808},{"pulse909",bank909}}) do bind(entry[1].."_kick_pitch","Kick Pitch",entry[2]:parameter(K.freq),0,1);bind(entry[1].."_kick_decay","Kick Decay",entry[2]:parameter(K.kdecay),0,1);bind(entry[1].."_snare_tone","Snare Tone",entry[2]:parameter(K.stone),0,1);bind(entry[1].."_hat_filter","Hat Filter",entry[2]:parameter(K.hfilter),0,1);bind(entry[1].."_perc_tone","Perc Tone",entry[2]:parameter(K.pwave),0,1) end
for _,entry in ipairs({{"bass",bass},{"drone",drone},{"lead",lead},{"arp",texture}}) do bind(entry[1].."_preset",entry[1].." preset",entry[2].preset,0,363);bind(entry[1].."_tone",entry[1].." filter",entry[2]:parameter(2000003),0,1);bind(entry[1].."_attack",entry[1].." attack",entry[2]:parameter(3000000),0,1);bind(entry[1].."_release",entry[1].." release",entry[2]:parameter(3000004),0,1) end
local sources={{"pulse808",bank808.out,.72},{"pulse909",bank909.out,.62},{"bass",bass.out,.56},{"drone",drone.out,.32},{"lead",lead.out,.38},{"arp",texture.out,.31}}
local mix=instrument:mixer {inputs=8,headroom=.42}
local solo_targets={{},{},{},{},{},{}}
for index,source in ipairs(sources) do
  local gl=instrument:gain {gain=source[3]};local gr=instrument:gain {gain=source[3]};local ml=instrument:mute {muted=0};local mr=instrument:mute {muted=0}
  instrument:connect(source[2].left,gl.input);instrument:connect(source[2].right,gr.input);instrument:connect(gl.out,ml.input);instrument:connect(gr.out,mr.input)
  local left,right=ml.out,mr.out
  for selected=1,#sources do if selected~=index then local sl=instrument:mute {muted=0};local sr=instrument:mute {muted=0};instrument:connect(left,sl.input);instrument:connect(right,sr.input);left,right=sl.out,sr.out;solo_targets[selected][#solo_targets[selected]+1]=sl.muted;solo_targets[selected][#solo_targets[selected]+1]=sr.muted end end
  instrument:connect({left=left,right=right,direction="stereo_output"},mix.input[index]);bind(source[1].."_level",source[1].." level",gl.gain,0,1,{gr.gain});bind(source[1].."_mute",source[1].." mute",ml.muted,0,1,{mr.muted})
end
for selected,source in ipairs(sources) do local targets=solo_targets[selected];local first=table.remove(targets,1);bind(source[1].."_solo",source[1].." solo",first,0,1,targets) end
local comp=instrument:stereo_compressor {threshold=-14,ratio=3.5,attack=.012,release=.18,knee=6,makeup_gain=0};instrument:connect(mix.left,comp.input.left);instrument:connect(mix.right,comp.input.right);instrument:connect(comp.out.left,instrument.output.left);instrument:connect(comp.out.right,instrument.output.right)

local live=instrument:midi_input {channel=1};local livearp=instrument:arpeggiator {notes=live.notes,tempo=seqarp,mode="up",octaves=2,rate=4,gate=.68};instrument:note_source(livearp.notes)
local server=meldritch.server {host="127.0.0.1",port=8080};server:websocket("/performance",{});local performance=server:performance {name="Instrument Playground"};performance:role("performer",{controls="*"});server:credential(os.getenv("MELDRITCH_PLAYGROUND_TOKEN") or "instrument-playground","performer")
local function pulse_plane(id,label,seq,kit,bank,target)
  local plane=performance:plane {id=id,label=label,roles={"performer"}};plane:drum_grid {id=id.."_grid",label=label.." Grid",kit=kit,steps=32};plane:pattern_selector {id=id.."_patterns",label="Patterns",sequencer=seq,boundary="bar"};plane:preset_selector {id=target,label="Preset Bank",target=target,catalog=bank.preset_catalog};for _,control in ipairs({{"kick_pitch","Kick Pitch"},{"kick_decay","Kick Decay"},{"snare_tone","Snare Tone"},{"hat_filter","Hat Filter"},{"perc_tone","Perc Tone"},{"level","Level"}}) do plane:knob {id=id.."_"..control[1],label=control[2],target=id.."_"..control[1],min=0,max=1} end;plane:toggle {id=id.."_mute",label="Mute",target=id.."_mute",min=0,max=1};plane:toggle {id=id.."_solo",label="Solo",target=id.."_solo",min=0,max=1}
end
pulse_plane("pulse808","PULSE 808",seq808,kit808,bank808,"pulse808_preset");pulse_plane("pulse909","PULSE 909",seq909,kit909,bank909,"pulse909_preset")
local function serum_plane(id,label,seq,track,plugin,pitch_min,pitch_max)
  local plane=performance:plane {id=id,label=label,roles={"performer"}};plane:piano_roll {id=id.."_roll",label="Sequence",track=track,steps=seq==seqdrone and 64 or 32,pitch_min=pitch_min,pitch_max=pitch_max};plane:pattern_selector {id=id.."_patterns",label="Patterns",sequencer=seq,boundary="bar"};plane:preset_selector {id=id.."_preset",label="Preset Browser",target=id.."_preset",catalog=plugin.preset_catalog};plane:knob {id=id.."_tone",label="Filter",target=id.."_tone",min=0,max=1};plane:knob {id=id.."_level",label="Level",target=id.."_level",min=0,max=1};plane:toggle {id=id.."_mute",label="Mute",target=id.."_mute",min=0,max=1};plane:toggle {id=id.."_solo",label="Solo",target=id.."_solo",min=0,max=1};plane:adsr {id=id.."_adsr",label="Envelope",normalized=true,attack=id.."_attack",decay=id.."_attack",sustain=id.."_tone",release=id.."_release"}
end
serum_plane("bass","Serum Bass",seqbass,bass_track,bass,24,48);serum_plane("drone","Serum Drone",seqdrone,drone_track,drone,24,60);serum_plane("lead","Serum Lead",seql,lead_track,lead,48,84);serum_plane("arp","Serum Arp / Texture",seqarp,arp_track,texture,36,84)
instrument:control_surface {id="stream_deck_playground",device="stream_deck_xl",initial_page="instruments",pages={{id="instruments",buttons={{id="p808",index=0,label="808",press={control="pulse808_mute",value=0}},{id="p909",index=1,label="909",press={control="pulse909_mute",value=0}},{id="bass",index=2,label="BASS",press={control="bass_mute",value=0}},{id="drone",index=3,label="DRONE",press={control="drone_mute",value=0}},{id="lead",index=4,label="LEAD",press={control="lead_mute",value=0}},{id="arp",index=5,label="ARP",press={control="arp_mute",value=0}}}}}}
return instrument
