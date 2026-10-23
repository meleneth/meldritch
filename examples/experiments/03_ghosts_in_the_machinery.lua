-- Ghosts in the Machinery
-- 96 bars / 128 BPM / progressive electro-breakbeat performance arrangement.
local instrument=meldritch.instrument("Ghosts in the Machinery")
instrument:voice_mode("mono")
instrument:devices {midi_input="MicroLab mk3:MicroLab mk3",
  midi_controls={"MicroLab mk3:MicroLab mk3"},
  apc_mini="APC mini mk2:APC mini mk2 Control"}

local sequence=instrument:sequencer {bpm=128,steps=128,steps_per_beat=4}
local names={"Kick","Snare + Clap","Hats","Break","Bass","Theme","Arp","Atmosphere"}
local track={}
for _,name in ipairs(names) do track[name]=sequence:track(name) end

local function hits(length,events)
  local out={};for i=1,length do out[i]=0 end
  for _,e in ipairs(events) do out[e[1]]=e[2] end
  return out
end
local function repeat_hits(length,period,events)
  local out={};for i=1,length do out[i]=0 end
  for base=0,length-1,period do for _,e in ipairs(events) do
    if base+e[1]<=length then out[base+e[1]]=e[2] end
  end end
  return out
end
local function drum(name,track_name,length,values)
  return track[track_name]:clip(name,{length=length,steps=values})
end
local function notes(name,track_name,length,events)
  return track[track_name]:clip(name,{length=length,notes=events})
end

-- The short cycles lock hard while the longer phrases keep changing above them.
local kick_pulse=drum("Kick Pulse","Kick",16,hits(16,{{1,1},{7,.72},{9,.94},{12,.50}}))
local kick_contact=drum("Kick Contact","Kick",16,hits(16,{{1,1},{4,.48},{7,.78},{9,.96},{12,.62},{15,.74}}))
local kick_machine=drum("Kick Machine","Kick",32,repeat_hits(32,16,{{1,1},{4,.54},{7,.82},{9,.96},{11,.46},{14,.72}}))
local kick_overload=drum("Kick Overload","Kick",32,hits(32,{{1,1},{4,.58},{7,.88},{9,1},{11,.55},{14,.82},{17,1},{20,.62},{23,.9},{25,1},{27,.58},{29,.82},{31,.74}}))
local kick_shutdown=drum("Kick Shutdown","Kick",128,hits(128,{{1,1},{7,.74},{9,.92},{14,.66},{17,.94},{23,.72},{25,.9},{30,.62},{33,.9},{39,.68},{41,.86},{46,.58},{49,.82},{57,.78},{65,.74},{73,.68},{81,.62},{97,.52}}))

local snare_ghost=drum("Snare Ghost","Snare + Clap",32,hits(32,{{5,.30},{13,.54},{21,.32},{29,.62}}))
local snare_break=drum("Snare Break","Snare + Clap",32,hits(32,{{5,.86},{12,.28},{13,1},{20,.35},{21,.82},{27,.30},{29,1},{32,.44}}))
local snare_overload=drum("Snare Overload","Snare + Clap",32,hits(32,{{3,.20},{5,.94},{8,.28},{12,.38},{13,1},{16,.42},{19,.25},{21,.96},{24,.35},{27,.32},{29,1},{31,.52},{32,.72}}))
local snare_shutdown=drum("Snare Shutdown","Snare + Clap",128,hits(128,{{5,.9},{13,1},{21,.86},{29,1},{37,.82},{45,.9},{53,.76},{61,.86},{69,.70},{77,.78},{85,.62},{93,.68},{109,.50}}))

local hats_filtered=drum("Hats Filtered","Hats",32,repeat_hits(32,8,{{1,.26},{3,.18},{5,.34},{7,.22}}))
local hats_contact=drum("Hats Contact","Hats",32,repeat_hits(32,8,{{1,.52},{3,.38},{5,.62},{7,.44},{8,.24}}))
local hats_machine=drum("Hats Machine","Hats",32,hits(32,{{1,.58},{3,.46},{5,.7},{7,.52},{9,.62},{11,.5},{13,.76},{15,.58},{17,.62},{19,.48},{21,.72},{23,.54},{25,.66},{27,.52},{29,.82},{30,.32},{31,.7},{32,.42}}))
local hats_vacuum=drum("Hats Vacuum","Hats",64,hits(64,{{7,.18},{15,.24},{23,.20},{31,.32},{39,.22},{47,.28},{55,.34},{63,.48}}))
local hats_shutdown=drum("Hats Shutdown","Hats",128,hits(128,{{1,.55},{3,.4},{5,.62},{7,.44},{9,.56},{11,.4},{13,.66},{15,.48},{17,.54},{21,.6},{25,.52},{29,.58},{33,.48},{37,.54},{41,.44},{45,.5},{49,.40},{57,.44},{65,.36},{73,.32},{81,.28},{97,.22}}))

-- A hand-rearranged two-bar break: low/high tom fragments and syncopated accents.
local break_frag=drum("Break Fragment","Break",32,hits(32,{{4,.42},{7,.62},{10,.38},{12,.54},{15,.72},{18,.40},{20,.66},{23,.48},{26,.72},{28,.46},{30,.82},{32,.58}}))
local break_machine=drum("Break Machine","Break",64,hits(64,{{2,.36},{4,.58},{7,.72},{10,.44},{12,.66},{15,.82},{18,.42},{20,.72},{23,.54},{26,.78},{28,.48},{30,.9},{32,.62},{34,.48},{36,.70},{39,.84},{42,.46},{44,.76},{47,.6},{50,.86},{52,.54},{55,.74},{58,.62},{60,.88},{61,.44},{62,.58},{63,.76},{64,1}}))
local break_overload=drum("Break Overload","Break",64,repeat_hits(64,16,{{2,.42},{4,.68},{7,.82},{10,.48},{12,.76},{14,.44},{15,.9},{16,.58}}))
local break_vacuum=drum("Vacuum Accents","Break",128,hits(128,{{1,.68},{31,.36},{48,.48},{63,.72},{80,.42},{95,.68},{111,.54},{120,.64},{124,.76},{126,.86},{127,.94},{128,1}}))
local break_shutdown=drum("Break Shutdown","Break",128,hits(128,{{4,.62},{10,.46},{15,.72},{20,.52},{28,.68},{31,.82},{36,.58},{44,.7},{52,.62},{60,.76},{68,.54},{76,.66},{84,.48},{92,.58},{108,.44},{124,.36}}))

-- E Dorian harmony: Em9 | G6 | Dadd9 | A7sus, with a chromatic machine-tone turn.
local bass_primary=notes("Bass Primary","Bass",64,{
  {step=1,note=40,velocity=1,gate=2.6},{step=5,note=40,velocity=.62,gate=1},{step=7,note=47,velocity=.85,gate=1.7},{step=11,note=43,velocity=.72,gate=1},{step=14,note=38,velocity=.84,gate=2},
  {step=17,note=43,velocity=.96,gate=2.5},{step=21,note=50,velocity=.68,gate=1},{step=23,note=47,velocity=.78,gate=1.5},{step=27,note=45,velocity=.7,gate=1},{step=30,note=42,velocity=.82,gate=2},
  {step=33,note=38,velocity=.98,gate=2.5},{step=37,note=45,velocity=.7,gate=1},{step=40,note=47,velocity=.8,gate=1.5},{step=43,note=36,velocity=.58,gate=1},{step=46,note=38,velocity=.9,gate=2},
  {step=49,note=45,velocity=1,gate=2.5},{step=53,note=40,velocity=.68,gate=1},{step=56,note=43,velocity=.8,gate=1.5},{step=59,note=47,velocity=.72,gate=1},{step=62,note=39,velocity=.9,gate=2}})
local bass_alternate=notes("Bass Reactor","Bass",64,{
  {step=1,note=28,velocity=1,gate=3},{step=6,note=40,velocity=.7,gate=1},{step=8,note=47,velocity=.9,gate=1},{step=11,note=43,velocity=.76,gate=1},{step=15,note=39,velocity=.92,gate=1},
  {step=17,note=31,velocity=1,gate=3},{step=22,note=43,velocity=.74,gate=1},{step=24,note=50,velocity=.88,gate=1},{step=27,note=47,velocity=.78,gate=1},{step=31,note=42,velocity=.94,gate=1},
  {step=33,note=26,velocity=1,gate=3},{step=38,note=38,velocity=.72,gate=1},{step=40,note=45,velocity=.9,gate=1},{step=43,note=36,velocity=.72,gate=1},{step=47,note=41,velocity=.86,gate=1},
  {step=49,note=33,velocity=1,gate=3},{step=54,note=45,velocity=.75,gate=1},{step=56,note=52,velocity=.92,gate=1},{step=59,note=47,velocity=.8,gate=1},{step=63,note=39,velocity=.96,gate=1}})
local bass_vacuum=notes("Bass Vacuum","Bass",128,{{step=1,note=28,velocity=.42,gate=12},{step=49,note=26,velocity=.38,gate=12},{step=97,note=33,velocity=.46,gate=15}})
local bass_shutdown=notes("Bass Shutdown","Bass",128,{{step=1,note=40,velocity=.9,gate=3},{step=7,note=47,velocity=.72,gate=1},{step=14,note=38,velocity=.78,gate=2},{step=17,note=43,velocity=.84,gate=3},{step=23,note=47,velocity=.68,gate=1},{step=30,note=42,velocity=.72,gate=2},{step=33,note=38,velocity=.78,gate=3},{step=40,note=45,velocity=.62,gate=1},{step=46,note=38,velocity=.68,gate=2},{step=49,note=33,velocity=.72,gate=4},{step=62,note=40,velocity=.56,gate=2},{step=65,note=28,velocity=.65,gate=6},{step=81,note=28,velocity=.50,gate=8},{step=97,note=28,velocity=.34,gate=12}})

local motif={{1,76,.92,2},{5,71,.66,1},{7,74,.8,2},{11,78,.92,1},{13,76,.76,3},{17,79,.96,2},{21,74,.68,1},{23,71,.82,2},{27,69,.72,1},{29,71,.88,3}}
local function theme_events(high,fragment,ending)
  local e={};local roots={0,3,-2,5,0,3,-2,5}
  for bar=0,7 do if not fragment or bar%2==0 then
    for _,m in ipairs(motif) do local step=bar*16+m[1];if step<=128 then e[#e+1]={step=step,note=m[2]+roots[bar+1]+high,velocity=m[3],gate=m[4]} end end
  end end
  if ending then e[#e+1]={step=121,note=76+high,velocity=1,gate=7} end
  return e
end
local theme_frag=notes("Theme Fragments","Theme",128,theme_events(-12,true,false))
local theme_main=notes("Theme Main","Theme",128,theme_events(0,false,false))
local theme_high=notes("Theme Reactor","Theme",128,theme_events(12,false,true))
local theme_vacuum=notes("Theme Vacuum","Theme",128,{{step=17,note=76,velocity=.42,gate=3},{step=45,note=74,velocity=.38,gate=4},{step=73,note=79,velocity=.46,gate=5},{step=105,note=78,velocity=.52,gate=7}})
local theme_shutdown=notes("Theme Shutdown","Theme",128,{{step=1,note=76,velocity=.78,gate=2},{step=5,note=71,velocity=.54,gate=1},{step=7,note=74,velocity=.66,gate=2},{step=17,note=79,velocity=.72,gate=2},{step=23,note=71,velocity=.6,gate=2},{step=33,note=74,velocity=.64,gate=3},{step=49,note=71,velocity=.56,gate=4},{step=65,note=64,velocity=.45,gate=8},{step=97,note=64,velocity=.30,gate=16}})

local arp_sparse=notes("Arp Sparse","Arp",64,{{step=3,note=64,velocity=.48,gate=.5},{step=7,note=71,velocity=.58,gate=.5},{step=11,note=74,velocity=.52,gate=.5},{step=15,note=78,velocity=.64,gate=.5},{step=19,note=67,velocity=.5,gate=.5},{step=23,note=74,velocity=.62,gate=.5},{step=27,note=79,velocity=.56,gate=.5},{step=31,note=83,velocity=.7,gate=.5},{step=35,note=62,velocity=.48,gate=.5},{step=39,note=69,velocity=.58,gate=.5},{step=43,note=76,velocity=.54,gate=.5},{step=47,note=81,velocity=.66,gate=.5},{step=51,note=69,velocity=.5,gate=.5},{step=55,note=76,velocity=.62,gate=.5},{step=59,note=79,velocity=.58,gate=.5},{step=63,note=83,velocity=.74,gate=.5}})
local arp_busy=notes("Arp Machine","Arp",64,(function() local e={};local scale={64,67,71,74,78,81,83,86};for s=1,64,2 do local i=((s*5+s//8)%#scale)+1;e[#e+1]={step=s,note=scale[i],velocity=.42+(i%3)*.12,gate=.42} end return e end)())
local arp_shutdown=notes("Arp Shutdown","Arp",128,(function() local e={};local scale={64,67,71,74,78};for s=1,80,4 do e[#e+1]={step=s,note=scale[((s//4)%5)+1],velocity=.46*(1-s/160),gate=.7} end return e end)())

local pad_awake=notes("Pad Awakening","Atmosphere",128,{{step=1,note=52,velocity=.34,gate=30},{step=33,note=55,velocity=.32,gate=30},{step=65,note=50,velocity=.34,gate=30},{step=97,note=57,velocity=.36,gate=30}})
local pad_motion=notes("Pad Motion","Atmosphere",128,{{step=1,note=52,velocity=.30,gate=28},{step=33,note=55,velocity=.28,gate=28},{step=65,note=50,velocity=.30,gate=28},{step=97,note=57,velocity=.32,gate=28}})
local pad_vacuum=notes("Pad Vacuum","Atmosphere",128,{{step=1,note=40,velocity=.50,gate=31},{step=33,note=43,velocity=.46,gate=31},{step=65,note=38,velocity=.50,gate=31},{step=97,note=45,velocity=.56,gate=31}})

local awakening=sequence:scene("Awakening",{Kick=false,["Snare + Clap"]=snare_ghost,Hats=hats_filtered,Break=false,Bass=false,Theme=theme_frag,Arp=false,Atmosphere=pad_awake})
local contact=sequence:scene("First Contact",{Kick=kick_contact,["Snare + Clap"]=snare_break,Hats=hats_contact,Break=break_frag,Bass=bass_primary,Theme=theme_main,Arp=arp_sparse,Atmosphere=pad_motion})
local machine=sequence:scene("Machine Room",{Kick=kick_machine,["Snare + Clap"]=snare_break,Hats=hats_machine,Break=break_machine,Bass=bass_primary,Theme=theme_frag,Arp=arp_busy,Atmosphere=pad_motion})
local overload=sequence:scene("Overload",{Kick=kick_overload,["Snare + Clap"]=snare_overload,Hats=hats_machine,Break=break_overload,Bass=bass_alternate,Theme=theme_main,Arp=arp_busy,Atmosphere=pad_motion})
local vacuum=sequence:scene("Vacuum",{Kick=false,["Snare + Clap"]=false,Hats=hats_vacuum,Break=break_vacuum,Bass=bass_vacuum,Theme=theme_vacuum,Arp=false,Atmosphere=pad_vacuum})
local reactor=sequence:scene("Reactor",{Kick=kick_overload,["Snare + Clap"]=snare_overload,Hats=hats_machine,Break=break_overload,Bass=bass_alternate,Theme=theme_high,Arp=arp_busy,Atmosphere=pad_motion})
local shutdown=sequence:scene("Shutdown",{Kick=kick_shutdown,["Snare + Clap"]=snare_shutdown,Hats=hats_shutdown,Break=break_shutdown,Bass=bass_shutdown,Theme=theme_shutdown,Arp=arp_shutdown,Atmosphere=pad_vacuum})

local arrangement=sequence:timeline("Main Reactor Sequence")
arrangement:at_bar(1,awakening):at_bar(9,contact):at_bar(25,machine):at_bar(41,overload)
  :at_bar(57,vacuum):at_bar(65,reactor):at_bar(81,shutdown):stop_at_bar(97)

local kit=meldritch.samples {source="strudel",bank="RolandTR909",cache=true}
local drums=instrument:mixer {inputs=8,headroom=.25};local music=instrument:mixer {inputs=8,headroom=.23}
local function stereo_sample(track_name,sample_name,gain,slot)
  local p=instrument:sample_player {sample=kit:load(sample_name,0),gain=gain}
  instrument:connect(track[track_name].out,p.trigger)
  instrument:connect(p.left,drums.input[slot].left);instrument:connect(p.right,drums.input[slot].right)
  return p
end
local kick=stereo_sample("Kick","bd",1,1)
local snare=stereo_sample("Snare + Clap","sd",.82,2);local clap=instrument:sample_player {sample=kit:load("cp",0),gain=.48}
instrument:connect(track["Snare + Clap"].out,clap.trigger);instrument:connect(clap.left,drums.input[3].left);instrument:connect(clap.right,drums.input[3].right)
local hat=stereo_sample("Hats","hh",.52,4);local openhat=instrument:sample_player {sample=kit:load("oh",0),gain=.22}
instrument:connect(track.Hats.out,openhat.trigger);instrument:connect(openhat.left,drums.input[5].left);instrument:connect(openhat.right,drums.input[5].right)
local break_low=stereo_sample("Break","lt",.55,6);local break_high=instrument:sample_player {sample=kit:load("ht",0),gain=.32}
instrument:connect(track.Break.out,break_high.trigger);instrument:connect(break_high.left,drums.input[7].left);instrument:connect(break_high.right,drums.input[7].right)

local bass=instrument:oscillator {waveform="pulse",frequency=82.4,pulse_width=.28};local bass_env=instrument:adsr {attack=.002,decay=.16,sustain=.42,release=.10}
local bass_filter=instrument:svf {cutoff=720,resonance=.66};local bass_amp=instrument:vca {};local bass_drive=instrument:drive {amount=3.2,output_gain=.72}
instrument:connect(track.Bass.pitch,bass.pitch);instrument:connect(track.Bass.gate,bass_env.gate)
instrument:connect(bass.out,bass_filter.input);instrument:connect(bass_filter.lowpass,bass_amp.input);instrument:connect(bass_env.out,bass_amp.cv);instrument:connect(bass_amp.out,bass_drive.input)
local bass_l=instrument:ducker {threshold=.035,depth=.70,attack=.002,release=.15};local bass_r=instrument:ducker {threshold=.035,depth=.70,attack=.002,release=.15}
instrument:connect(bass_drive.out,bass_l.input);instrument:connect(bass_drive.out,bass_r.input);instrument:connect(kick.left,bass_l.sidechain);instrument:connect(kick.right,bass_r.sidechain)
instrument:connect(bass_l.out,music.input[1].left);instrument:connect(bass_r.out,music.input[1].right)

local function voice(track_name,model,harmonics,timbre,morph,attack,release,slot,pan_l,pan_r)
  local v=instrument:plaits {model=model,harmonics=harmonics,timbre=timbre,morph=morph};local e=instrument:adsr {attack=attack,decay=.22,sustain=.42,release=release};local a=instrument:vca {}
  instrument:connect(track[track_name].pitch,v.pitch);instrument:connect(track[track_name].gate,v.gate);instrument:connect(track[track_name].velocity,v.velocity);instrument:connect(track[track_name].gate,e.gate);instrument:connect(v.main,a.input);instrument:connect(e.out,a.cv)
  local l=instrument:gain {gain=pan_l};local r=instrument:gain {gain=pan_r};instrument:connect(a.out,l.input);instrument:connect(a.out,r.input);instrument:connect(l.out,music.input[slot].left);instrument:connect(r.out,music.input[slot].right)
  return v,l,r
end
local lead,lead_l,lead_r=voice("Theme","harmonic",.72,.58,.36,.006,.48,2,.72,1)
local arp,arp_l,arp_r=voice("Arp","modal",.56,.66,.30,.002,.12,3,1,.48)
local pad,pad_l,pad_r=voice("Atmosphere","virtual_analog_vcf",.36,.32,.42,.8,2.8,4,.72,.88)

-- MicroLab owns a separate live lead voice; scene changes cannot steal its notes.
local live=instrument:plaits {model="fm",harmonics=.62,timbre=.48,morph=.38};local live_env=instrument:adsr {attack=.008,decay=.2,sustain=.56,release=.7};local live_amp=instrument:vca {};local live_level=instrument:gain {gain=.34}
instrument:connect(instrument.note.pitch,live.pitch);instrument:connect(instrument.note.gate,live.gate);instrument:connect(instrument.note.velocity,live.velocity);instrument:connect(instrument.note.gate,live_env.gate);instrument:connect(live.main,live_amp.input);instrument:connect(live_env.out,live_amp.cv);instrument:connect(live_amp.out,live_level.input);instrument:connect(live_level.out,music.input[5].left);instrument:connect(live_level.out,music.input[5].right)

local delay=instrument:stereo_delay {left_beats=.375,right_beats=.625,feedback=.32,crossfeed=.78,mix=1,filter_cutoff=5200,feedback_drive=1.5}
instrument:connect(sequence.tempo,delay.bpm);instrument:connect(music.left,delay.input.left);instrument:connect(music.right,delay.input.right)
local dry_l=instrument:gain {gain=.82};local dry_r=instrument:gain {gain=.82};local wet_l=instrument:gain {gain=.34};local wet_r=instrument:gain {gain=.34}
instrument:connect(music.left,dry_l.input);instrument:connect(music.right,dry_r.input);instrument:connect(delay.left,wet_l.input);instrument:connect(delay.right,wet_r.input)
local master=instrument:mixer {inputs=8,headroom=.58};instrument:connect(drums.left,master.input[1].left);instrument:connect(drums.right,master.input[1].right);instrument:connect(dry_l.out,master.input[2].left);instrument:connect(dry_r.out,master.input[2].right);instrument:connect(wet_l.out,master.input[3].left);instrument:connect(wet_r.out,master.input[3].right)
-- The synth models are intentionally driven hard; remove subsonic bias before
-- the linked master compressor so scene changes and the final release settle
-- around true zero rather than carrying a false DC "tail".
local dc_l=instrument:svf {cutoff=25,resonance=.05};local dc_r=instrument:svf {cutoff=25,resonance=.05}
instrument:connect(master.left,dc_l.input);instrument:connect(master.right,dc_r.input)
local comp=instrument:stereo_compressor {threshold=-20,ratio=3.5,attack=.012,release=.22,knee=6,makeup_gain=2.5,bypass=0};instrument:connect(dc_l.highpass,comp.input.left);instrument:connect(dc_r.highpass,comp.input.right);instrument:connect(comp.left,instrument.output.left);instrument:connect(comp.right,instrument.output.right)

local function control(id,label,target,range,targets,curve) return instrument:control {id=id,label=label,target=target,targets=targets,range=range,curve=curve} end
local bass_cutoff=control("bass_cutoff","Bass cutoff",bass_filter.cutoff,{80,8000},nil,"log")
local bass_res=control("bass_resonance","Bass resonance",bass_filter.resonance,{0,.92})
local bass_dirt=control("bass_drive","Bass drive",bass_drive.amount,{.1,10})
local lead_tone=control("lead_tone","Theme tone",lead.timbre,{0,1})
local arp_tone=control("arp_tone","Arp tone",arp.timbre,{0,1})
local delay_feedback=control("delay_feedback","Ghost feedback",delay.feedback,{0,.88})
local delay_color=control("delay_color","Ghost color",delay.filter_cutoff,{300,14000},nil,"log")
local wet_level=control("wet_level","Ghost level",wet_l.gain,{0,.8},{wet_r.gain})
local master_level=control("master_level","Master volume",master.headroom,{.05,.8})
local master_threshold=control("master_threshold","Glue threshold",comp.threshold,{-36,0})
local master_ratio=control("master_ratio","Glue ratio",comp.ratio,{1,10})
local master_attack=control("master_attack","Glue attack",comp.attack,{.001,.15},nil,"log")
local master_release=control("master_release","Glue release",comp.release,{.02,1.2},nil,"log")
local master_knee=control("master_knee","Glue knee",comp.knee,{0,18})
local master_makeup=control("master_makeup","Glue makeup",comp.makeup_gain,{-6,12})

arrangement:automation {target=bass_cutoff,interpolation="smooth"}:at_bar(1,260):at_bar(9,760):at_bar(25,1250):at_bar(41,2300):at_bar(57,180):at_bar(60,420):at_bar(62,1200):at_bar(64,4200):at_bar(65,1100):at_bar(81,900):at_bar(97,180)
arrangement:automation {target=delay_feedback,interpolation="smooth"}:at_bar(1,.50):at_bar(9,.30):at_bar(41,.42):at_bar(57,.48):at_bar(64,.78):at_bar(65,.32):at_bar(81,.52):at_bar(96,.76)
arrangement:automation {target=wet_level,interpolation="linear"}:at_bar(1,.46):at_bar(9,.24):at_bar(41,.34):at_bar(57,.48):at_bar(64,.68):at_bar(65,.28):at_bar(81,.40):at_bar(96,.62)
arrangement:automation {target=bass_dirt,interpolation="step"}:at_bar(1,1.4):at_bar(9,2.8):at_bar(41,5.4):at_bar(57,1.2):at_bar(65,4.2):at_bar(81,2.2)

instrument:performance {layout="step_grid",sequencer=sequence,project_file="ghosts-in-the-machinery.lua",
  keys={Space="transport",s="start",x="stop",r="restart",Up="track:up",Down="track:down",Left="step:previous",Right="step:next",["["]="bank:previous",["]"]="bank:next",m="mute",o="solo",j="control:next",k="control:previous",h="control:decrease",l="control:increase",["1"]="toggle:1",["2"]="toggle:2",["3"]="toggle:3",["4"]="toggle:4",["5"]="toggle:5",["6"]="toggle:6",["7"]="toggle:7",["8"]="toggle:8"},
  apc={pages={"steps","performance"},performance_rows={"scene","mute","solo","track","restart","clip","",""},scenes={"Awakening","First Contact","Machine Room","Overload","Vacuum","Reactor","Shutdown"},scene_quantization=16,
    faders={"bass_cutoff","bass_resonance","bass_drive","lead_tone","arp_tone","delay_feedback","delay_color","wet_level"},
    fader_banks={{"bass_cutoff","bass_resonance","bass_drive","lead_tone","arp_tone","delay_feedback","delay_color","wet_level"},{"master_threshold","master_ratio","master_attack","master_release","master_knee","master_makeup","wet_level","delay_feedback"}},
    scene_buttons={"transport","restart","faders:bank:0","faders:bank:1","","","",""},master="master_level"},
  focus={controls={"bass_cutoff","bass_resonance","bass_drive","lead_tone","arp_tone","delay_feedback","delay_color","wet_level"},midi_ccs={{channel=1,number=74},{channel=1,number=71},{channel=1,number=76},{channel=1,number=77}}}}

local screen=instrument:screen("composition","Ghosts in the Machinery")
screen:arrangement_status();screen:control_focus()
local editor=instrument:screen("clips","Clip Inspector");editor:pattern_grid();editor:control_focus()
local mastering=instrument:screen("master","Master Glue")
mastering:label("Linked stereo bus compressor — all six parameters are live controls")
mastering:control("Threshold (dB)","master_threshold");mastering:control("Ratio","master_ratio")
mastering:control("Attack (s)","master_attack");mastering:control("Release (s)","master_release")
mastering:control("Soft knee (dB)","master_knee");mastering:control("Makeup (dB)","master_makeup")
local navigation=instrument:navigation {initial="composition",events={{name="composition",from="*",to="composition"},{name="clips",from="*",to="clips"},{name="master",from="*",to="master"}}}
navigation:bind("F9","composition");navigation:bind("F10","clips");navigation:bind("F11","master")

sequence:start()
return instrument
