return function(arrangement_mode, project_path)
local instrument = meldritch.instrument("Delay Lab: Overdrive")
instrument:devices {apc_mini="APC mini mk2:APC mini mk2 Control"}
local kit = meldritch.samples {source="strudel",bank="RolandTR909",cache=true}
local project = meldritch.musical.project(instrument, {
  id="delay_lab_overdrive",bpm=112,max_steps=64,steps_per_beat=4,
  persistence="overdrive-project.lua",
})
local sequence = project.sequence
local names = {"Kick","SnareClap","CHat","OHat","Perc","Pulse","Chime","Lead"}
local kinds = {"drum","drum","drum","drum","drum","melodic","melodic","melodic"}
local tracks = {}
for index,name in ipairs(names) do
  tracks[index] = project:track {
    id=name,kind=kinds[index],
    instrument=index<=5 and "tr909_sample" or "plaits",
    routing=index<=5 and "dry_drums" or (index<=7 and "delay_lab_bus" or "lead_ambience"),
  }
end

local drum_bus = instrument:mixer {inputs=8,headroom=.32}
local synth_bus = instrument:mixer {inputs=8,headroom=.34}
local master = instrument:mixer {inputs=8,headroom=.52}
local volume_controls, track_controls = {}, {}

local function level_track(index,left_source,right_source,gain,destination)
  local left=instrument:gain {gain=gain};local right=instrument:gain {gain=gain}
  instrument:connect(left_source,left.input);instrument:connect(right_source,right.input)
  instrument:connect(tracks[index].signal.enabled,left.mod)
  instrument:connect(tracks[index].signal.enabled,right.mod)
  instrument:connect(left.out,destination.left);instrument:connect(right.out,destination.right)
  local id="volume_"..index;volume_controls[index]=id
  instrument:control {id=id,label=names[index].." volume",target=left.gain,
    targets={right.gain},range={0,1.5}}
  return left.out,right.out
end

local kick_player=instrument:sample_player {sample=kit:load("bd",0),gain=1}
instrument:connect(tracks[1].signal.out,kick_player.trigger)
local kick_left=instrument:drive {amount=2.8,output_gain=.78}
local kick_right=instrument:drive {amount=2.8,output_gain=.78}
instrument:connect(kick_player.left,kick_left.input);instrument:connect(kick_player.right,kick_right.input)
local kick_out_left,kick_out_right=level_track(1,kick_left.out,kick_right.out,.92,drum_bus.input[1])
instrument:control {id="kick_drive",label="Kick drive",target=kick_left.amount,
  targets={kick_right.amount},range={.1,10}}
track_controls[1]={"kick_drive"}

local snare=instrument:sample_player {sample=kit:load("sd",0),gain=1}
local clap=instrument:sample_player {sample=kit:load("cp",0),gain=.72}
instrument:connect(tracks[2].signal.out,snare.trigger);instrument:connect(tracks[2].signal.out,clap.trigger)
local snare_left=instrument:mixer {inputs=2};local snare_right=instrument:mixer {inputs=2}
instrument:connect(snare.left,snare_left.input[1]);instrument:connect(clap.left,snare_left.input[2])
instrument:connect(snare.right,snare_right.input[1]);instrument:connect(clap.right,snare_right.input[2])
local snare_color_left=instrument:drive {amount=1.5,output_gain=.68}
local snare_color_right=instrument:drive {amount=1.5,output_gain=.68}
instrument:connect(snare_left.out,snare_color_left.input);instrument:connect(snare_right.out,snare_color_right.input)
level_track(2,snare_color_left.out,snare_color_right.out,.72,drum_bus.input[2])
instrument:control {id="snare_drive",label="Snare/clap drive",target=snare_color_left.amount,
  targets={snare_color_right.amount},range={.1,10}}
track_controls[2]={"snare_drive"}

local function filtered_drum(index,sample_name,cutoff,gain,bus_slot)
  local player=instrument:sample_player {sample=kit:load(sample_name,0),gain=1}
  instrument:connect(tracks[index].signal.out,player.trigger)
  local left=instrument:svf {cutoff=cutoff,resonance=.18};local right=instrument:svf {cutoff=cutoff,resonance=.18}
  instrument:connect(player.left,left.input);instrument:connect(player.right,right.input)
  level_track(index,left.highpass,right.highpass,gain,drum_bus.input[bus_slot])
  local id="filter_"..index
  instrument:control {id=id,label=names[index].." brightness",target=left.cutoff,
    targets={right.cutoff},range={1200,16000},curve="log"}
  track_controls[index]={id}
end
filtered_drum(3,"hh",7200,.58,3)
filtered_drum(4,"oh",5900,.62,4)

local percussion=instrument:sample_player {sample=kit:load("lt",0),gain=1}
instrument:connect(tracks[5].signal.out,percussion.trigger)
local perc_left=instrument:drive {amount=1.7,output_gain=.72};local perc_right=instrument:drive {amount=1.7,output_gain=.72}
instrument:connect(percussion.left,perc_left.input);instrument:connect(percussion.right,perc_right.input)
level_track(5,perc_left.out,perc_right.out,.60,drum_bus.input[5])
instrument:control {id="perc_drive",label="Percussion drive",target=perc_left.amount,
  targets={perc_right.amount},range={.1,12}}
track_controls[5]={"perc_drive"}

local function plaits_voice(index,model,harmonics,timbre,morph,pan)
  local voice=instrument:plaits {model=model,harmonics=harmonics,timbre=timbre,morph=morph}
  local envelope=instrument:adsr {attack=.003,decay=.16,sustain=.40,release=.24}
  local amp=instrument:vca {};local color=instrument:drive {amount=index==6 and 2.6 or 1.25,output_gain=.72}
  instrument:connect(tracks[index].signal.pitch,voice.pitch);instrument:connect(tracks[index].signal.gate,voice.gate)
  instrument:connect(tracks[index].signal.velocity,voice.velocity);instrument:connect(tracks[index].signal.gate,envelope.gate)
  instrument:connect(voice.main,amp.input);instrument:connect(envelope.out,amp.cv);instrument:connect(amp.out,color.input)
  local left=instrument:gain {gain=pan[1]};local right=instrument:gain {gain=pan[2]}
  instrument:connect(color.out,left.input);instrument:connect(color.out,right.input)
  local out_left,out_right=level_track(index,left.out,right.out,index==6 and .70 or .55,synth_bus.input[index-5])
  instrument:control {id="tone_"..index,label=names[index].." tone",target=voice.timbre,range={0,1}}
  instrument:control {id="shape_"..index,label=names[index].." shape",target=voice.morph,range={0,1}}
  instrument:control {id="drive_"..index,label=names[index].." drive",target=color.amount,range={.1,10}}
  track_controls[index]={"tone_"..index,"shape_"..index,"drive_"..index}
  return out_left,out_right,voice
end
plaits_voice(6,"virtual_analog_vcf",.38,.48,.30,{1,.50})
plaits_voice(7,"modal",.60,.42,.44,{.48,1})

-- The Delay Lab bus has explicit dry and 100%-wet branches. Both are keyed
-- from the audible kick, while the drum bus never passes through a ducker.
local delay=instrument:stereo_delay {left_beats=.75,right_beats=1.25,feedback=.68,
  crossfeed=1,mix=1,filter_cutoff=3600,feedback_drive=1.8}
local drift_left=instrument:lfo {frequency=.037};local drift_right=instrument:lfo {frequency=.051}
local drift_scale_left=instrument:affine {scale=.009,offset=0};local drift_scale_right=instrument:affine {scale=.013,offset=0}
instrument:connect(drift_left.out,drift_scale_left.input);instrument:connect(drift_right.out,drift_scale_right.input)
instrument:connect(drift_scale_left.out,delay.left_time_mod);instrument:connect(drift_scale_right.out,delay.right_time_mod)
instrument:connect(sequence.tempo,delay.bpm);instrument:connect(synth_bus.left,delay.input.left);instrument:connect(synth_bus.right,delay.input.right)

local dry_duck_left=instrument:ducker {threshold=.045,depth=.58,attack=.002,release=.16}
local dry_duck_right=instrument:ducker {threshold=.045,depth=.58,attack=.002,release=.16}
local wet_duck_left=instrument:ducker {threshold=.035,depth=.84,attack=.0015,release=.27}
local wet_duck_right=instrument:ducker {threshold=.035,depth=.84,attack=.0015,release=.27}
instrument:connect(synth_bus.left,dry_duck_left.input);instrument:connect(synth_bus.right,dry_duck_right.input)
instrument:connect(delay.left,wet_duck_left.input);instrument:connect(delay.right,wet_duck_right.input)
for _,ducker in ipairs({dry_duck_left,dry_duck_right,wet_duck_left,wet_duck_right}) do
  instrument:connect(kick_out_left,ducker.sidechain)
end
instrument:connect(dry_duck_left.out,master.input[2].left);instrument:connect(dry_duck_right.out,master.input[2].right)
instrument:connect(wet_duck_left.out,master.input[3].left);instrument:connect(wet_duck_right.out,master.input[3].right)
instrument:control {id="dry_duck",label="Dry synth duck",target=dry_duck_left.depth,
  targets={dry_duck_right.depth},range={0,1}}
instrument:control {id="wet_duck",label="Delay return duck",target=wet_duck_left.depth,
  targets={wet_duck_right.depth},range={0,1}}
instrument:control {id="duck_attack",label="Duck attack",target=dry_duck_left.attack,
  targets={dry_duck_right.attack,wet_duck_left.attack,wet_duck_right.attack},range={.0001,.1},curve="log"}
instrument:control {id="duck_release",label="Duck release",target=dry_duck_left.release,
  targets={dry_duck_right.release,wet_duck_left.release,wet_duck_right.release},range={.01,1.2},curve="log"}

-- The lead is a separate instrument/effect path. Its short diffuse echo is a
-- deliberately modest ambience network, independent of the Delay Lab bus.
local lead=instrument:plaits {model="harmonic",harmonics=.72,timbre=.58,morph=.36}
local lead_env=instrument:adsr {attack=.006,decay=.22,sustain=.48,release=.52};local lead_amp=instrument:vca {}
instrument:connect(tracks[8].signal.pitch,lead.pitch);instrument:connect(tracks[8].signal.gate,lead.gate)
instrument:connect(tracks[8].signal.velocity,lead.velocity);instrument:connect(tracks[8].signal.gate,lead_env.gate)
instrument:connect(lead.main,lead_amp.input);instrument:connect(lead_env.out,lead_amp.cv)
local lead_pre=instrument:gain {gain=.62};instrument:connect(lead_amp.out,lead_pre.input);instrument:connect(tracks[8].signal.enabled,lead_pre.mod)
local lead_left=instrument:gain {gain=.62};local lead_right=instrument:gain {gain=1}
instrument:connect(lead_pre.out,lead_left.input);instrument:connect(lead_pre.out,lead_right.input)
instrument:connect(lead_left.out,master.input[4].left);instrument:connect(lead_right.out,master.input[4].right)
local ambience=instrument:stereo_delay {left_beats=.1875,right_beats=.3125,feedback=.42,
  crossfeed=.82,mix=1,filter_cutoff=7800,feedback_drive=1.25}
instrument:connect(sequence.tempo,ambience.bpm);instrument:connect(lead_left.out,ambience.input.left);instrument:connect(lead_right.out,ambience.input.right)
local ambience_level_left=instrument:gain {gain=.46};local ambience_level_right=instrument:gain {gain=.46}
instrument:connect(ambience.left,ambience_level_left.input);instrument:connect(ambience.right,ambience_level_right.input)
instrument:connect(ambience_level_left.out,master.input[5].left);instrument:connect(ambience_level_right.out,master.input[5].right)
volume_controls[8]="volume_8"
instrument:control {id="volume_8",label="Lead volume",target=lead_pre.gain,range={0,1.5}}
instrument:control {id="lead_tone",label="Lead tone",target=lead.timbre,range={0,1}}
instrument:control {id="lead_shape",label="Lead shape",target=lead.morph,range={0,1}}
instrument:control {id="lead_space",label="Lead ambience",target=ambience_level_left.gain,
  targets={ambience_level_right.gain},range={0,1}}
track_controls[8]={"lead_tone","lead_shape","lead_space"}

instrument:connect(drum_bus.left,master.input[1].left);instrument:connect(drum_bus.right,master.input[1].right)
local master_compressor=instrument:stereo_compressor {threshold=-16,ratio=2,attack=.025,
  release=.18,knee=6,makeup_gain=1,bypass=0}
instrument:connect(master.left,master_compressor.input.left);instrument:connect(master.right,master_compressor.input.right)
instrument:connect(master_compressor.left,instrument.output.left);instrument:connect(master_compressor.right,instrument.output.right)
instrument:control {id="tempo",label="Tempo",target=sequence.bpm,range={72,160}}
instrument:control {id="delay_feedback",label="Delay feedback",target=delay.feedback,range={0,.94}}
instrument:control {id="delay_filter",label="Delay filter",target=delay.filter_cutoff,range={80,16000},curve="log"}
instrument:control {id="master_level",label="Master level",target=master.headroom,range={.05,.9}}
instrument:control {id="master_threshold",label="Master threshold",target=master_compressor.threshold,range={-36,0}}
instrument:control {id="master_ratio",label="Master ratio",target=master_compressor.ratio,range={1,8}}
instrument:control {id="master_attack",label="Master attack",target=master_compressor.attack,range={.001,.2},curve="log"}
instrument:control {id="master_release",label="Master release",target=master_compressor.release,range={.02,1},curve="log"}
instrument:control {id="master_bypass",label="Master compressor bypass",target=master_compressor.bypass,range={0,1}}

local function row(length,hits)
  local values={};for step=1,length do values[step]=0 end
  for _,hit in ipairs(hits or {}) do values[hit[1]]=hit[2] end
  return values
end
local drum_patterns = {
  Atmosphere={32,{}, {}, {}, {}, {{8,.28},{23,.36}}},
  Percussion={32,{}, {{5,.52},{13,.62},{21,.56},{29,.72}},
    {{1,.35},{3,.42},{5,.38},{7,.52},{9,.36},{11,.46},{13,.40},{15,.58},{17,.38},{19,.45},{21,.42},{23,.62},{25,.40},{27,.48},{29,.44},{31,.72}},
    {{7,.54},{15,.62},{23,.58},{31,.76}},{{4,.45},{10,.58},{14,.42},{20,.66},{27,.52},{31,.78}}},
  Groove={32,{{1,1},{5,.9},{9,.98},{13,.9},{17,1},{21,.92},{25,.98},{29,.94}},
    {{5,.75},{13,.9},{21,.78},{29,1}},{{1,.48},{3,.62},{5,.52},{7,.68},{9,.48},{11,.64},{13,.54},{15,.75},{17,.5},{19,.66},{21,.54},{23,.7},{25,.5},{27,.64},{29,.58},{31,.82}},
    {{7,.56},{15,.70},{23,.60},{31,.82}},{{4,.42},{10,.54},{14,.38},{20,.62},{26,.48},{30,.72}}},
  Poly={48,{{1,1},{5,.88},{9,.96},{13,.9},{17,1},{21,.9},{25,.98},{29,.92},{33,1},{37,.88},{41,.96},{45,.9}},
    {{5,.72},{13,.9},{21,.76},{29,.94},{37,.78},{45,1}},{{1,.48},{3,.62},{5,.5},{7,.72},{9,.46},{11,.66},{13,.54},{15,.76},{17,.5},{19,.64},{21,.52},{23,.74},{25,.48},{27,.68},{29,.58},{31,.82},{33,.5},{35,.66},{37,.54},{39,.72},{41,.5},{43,.68},{45,.58},{47,.88}},
    {{7,.55},{15,.68},{23,.62},{31,.80},{39,.66},{47,.9}},{{4,.46},{10,.58},{14,.40},{20,.66},{27,.54},{31,.72},{36,.48},{42,.64},{46,.82}}},
  Breakdown={32,{},{{13,.42},{29,.56}},{{3,.25},{11,.32},{19,.28},{27,.38}},
    {{15,.42},{31,.52}},{{8,.30},{24,.44}}},
  Overdrive={64,{{1,1},{5,.94},{9,1},{13,.92},{17,1},{21,.96},{25,1},{29,.94},{33,1},{37,.96},{41,1},{45,.94},{49,1},{53,.96},{57,1},{59,.55},{61,.96},{63,.72}},
    {{5,.82},{13,1},{21,.86},{29,1},{37,.88},{45,1},{53,.9},{57,.55},{59,.62},{61,.78},{63,1}},
    {{1,.56},{3,.72},{5,.6},{7,.78},{9,.58},{11,.76},{13,.62},{15,.84},{17,.58},{19,.74},{21,.62},{23,.82},{25,.58},{27,.78},{29,.66},{31,.9},{33,.62},{35,.78},{37,.66},{39,.84},{41,.62},{43,.8},{45,.68},{47,.88},{49,.64},{51,.82},{53,.7},{55,.9},{57,.76},{58,.48},{59,.8},{60,.52},{61,.86},{62,.58},{63,1},{64,.72}},
    {{7,.66},{15,.78},{23,.72},{31,.92},{39,.76},{47,.94},{55,.82},{59,.58},{63,1}},
    {{4,.52},{10,.66},{14,.48},{20,.74},{27,.62},{31,.84},{36,.58},{42,.78},{46,.9},{52,.66},{56,.8},{58,.55},{60,.7},{62,.82},{64,1}}},
  Outro={32,{{1,.9},{9,.78},{17,.72},{25,.62}},{{5,.58},{13,.64},{21,.5},{29,.58}},
    {{1,.38},{5,.42},{9,.36},{13,.44},{17,.34},{21,.38},{25,.3},{29,.36}},{{15,.46},{31,.5}},{{8,.34},{24,.38}}},
  Tail={32,{}, {}, {}, {}, {}},
}

local chords = {
  Atmosphere={{{step=1,notes={41,48,53}},{step=17,notes={43,50,55}}},{{step=1,notes={65,69,72,76}},{step=25,notes={60,64,67,71}}},nil},
  Percussion={{{step=1,notes={41,48,53}},{step=17,notes={43,50,55}}},{{step=1,notes={65,69,72}},{step=25,notes={60,64,67}}},nil},
  Groove={{{step=1,notes={45,52,57}},{step=17,notes={48,55,60}}},{{step=1,notes={60,63,67}},{step=25,notes={62,65,69}}},{{step=1,notes={69,72,76}},{step=33,notes={67,71,74}}}},
  Poly={{{step=1,notes={45,52,57}},{step=13,notes={48,55,60}},{step=25,notes={43,50,55}}},{{step=1,notes={60,64,67,71}},{step=17,notes={62,65,69,72}},{step=33,notes={65,69,72,76}}},{{step=1,notes={72,76,79}},{step=21,notes={74,77,81}},{step=41,notes={71,74,79}}}},
  Breakdown={{{step=1,notes={41,48,53}},{step=25,notes={36,43,48}}},{{step=1,notes={65,72,76}},{step=33,notes={60,67,71}}},{{step=1,notes={69,76,81}},{step=33,notes={67,74,79}}}},
  Overdrive={{{step=1,notes={45,52,57,60}},{step=17,notes={47,54,59,62}}},{{step=1,notes={60,64,67,71}},{step=25,notes={62,66,69,74}}},{{step=1,notes={72,75,79,82}},{step=17,notes={74,77,81,84}},{step=41,notes={70,74,77,82}}}},
  Outro={{{step=1,notes={45,52,57}},{step=17,notes={41,48,53}}},{{step=1,notes={60,67,71}},{step=25,notes={65,69,72}}},{{step=1,notes={72,76,79}},{step=33,notes={69,72,76}}}},
  Tail={nil,nil,nil},
}

local section_order={"Atmosphere","Percussion","Groove","Poly","Breakdown","Overdrive","Outro","Tail"}
for _,name in ipairs(section_order) do
  local pattern=project:pattern(name);local drums=drum_patterns[name];local length=drums[1]
  for track=1,5 do pattern:triggers(names[track],row(length,drums[track+1])) end
  local sources=chords[name]
  if sources[1] then
    meldritch.arp.new {mode="up_down",octaves=2,clock_division=name=="Atmosphere" and 2 or 1,
      gate=.58,velocity_shape={1,.64,.82,.70}}:render(pattern,"Pulse",32,
      meldritch.arp.from_pattern(sources[1],32),{section=name})
  else pattern:notes("Pulse",32,{}) end
  if sources[2] then
    meldritch.arp.new {mode="played",octaves=3,clock_division=3,gate=.72,phase="free",
      velocity_shape={.68,1,.58}}:render(pattern,"Chime",48,
      meldritch.arp.from_pattern(sources[2],48),{section=name})
  else pattern:notes("Chime",48,{}) end
  if sources[3] then
    meldritch.arp.new {mode=name=="Overdrive" and "random" or "up",octaves=2,
      clock_division=name=="Breakdown" and 4 or 2,gate=.64,seed=0x0DDBA11,
      velocity_shape={.72,1,.62,.84}}:render(pattern,"Lead",64,
      meldritch.arp.from_pattern(sources[3],64),{section=name})
  else pattern:notes("Lead",64,{}) end
end

if project_path then project:reload(project_path) end
if arrangement_mode=="showcase" then
  project:arrangement {{name="Showcase",pattern="Overdrive",repeats=2}}
else
  -- 49 64-step cycles at 112 BPM = exactly 420 seconds. The former
  -- seven-cycle atmospheric opening deferred the first kick for almost two
  -- minutes, which made a working drum graph sound absent. Percussion now
  -- enters after two cycles and the kick establishes the groove after five.
  project:arrangement {{name="Atmosphere",pattern="Atmosphere",repeats=2},
    {name="Percussion",pattern="Percussion",repeats=3},{name="Groove",pattern="Groove",repeats=8},
    {name="Poly",pattern="Poly",repeats=8},{name="Breakdown",pattern="Breakdown",repeats=7},
    {name="Overdrive",pattern="Overdrive",repeats=12},{name="Outro",pattern="Outro",repeats=6},
    {name="Tail",pattern="Tail",repeats=3}}
end
sequence:start()

instrument:performance {layout="step_grid",sequencer=sequence,project_file="overdrive-project.lua",
  keys={F1="pattern:0",F2="pattern:1",F3="pattern:2",F4="pattern:3",F5="pattern:4",F6="pattern:5",
    Space="transport",s="start",x="stop",r="restart",R="reload",Up="track:up",Down="track:down",
    Left="step:previous",Right="step:next",["["]="bank:previous",["]"]="bank:next",
    n="pattern:next",N="pattern:previous",m="mute",o="solo",d="duplicate",C="clear",
    j="control:next",k="control:previous",h="control:decrease",l="control:increase",
    ["+"]="velocity:up",["-"]="velocity:down",p="pitch:up",P="pitch:down",
    g="gate:up",G="gate:down",S="save",["1"]="toggle:1",["2"]="toggle:2",
    ["3"]="toggle:3",["4"]="toggle:4",["5"]="toggle:5",["6"]="toggle:6",
    ["7"]="toggle:7",["8"]="toggle:8"},
  apc={pages={"steps","performance"},performance_rows={"pattern","mute","solo","track","restart","","",""},
    faders={volume_controls[1],volume_controls[2],volume_controls[3],volume_controls[4],volume_controls[5],
      volume_controls[6],volume_controls[7],volume_controls[8],"master_level"}}}
-- Put shared bus controls on the Pulse page without teaching the UI what a bus is.
track_controls[6]={"tone_6","shape_6","drive_6","dry_duck","wet_duck","duck_attack","duck_release",
  "delay_feedback","delay_filter","master_threshold","master_ratio","master_attack","master_release","master_bypass"}
local ui_tracks={};local colors={"red","orange","yellow","green","pink","blue","cyan","violet"}
for index,name in ipairs(names) do ui_tracks[index]={label=name,volume=volume_controls[index],
  controls=track_controls[index],color=colors[index],melodic=index>=6} end
local events=meldritch.event_ui();local screen=instrument:screen("overdrive","Delay Lab: Overdrive")
screen:performance_surface {title="OVERDRIVE",theme="lospec500",tracks=ui_tracks,events=events}
screen:on(sequence,"step_advanced",function(event) events:step(event.step,event.pattern,event.position) end)
screen:on(sequence,"voice_triggered",function(event) if event.track<=5 then events:trigger(event.track,event.pitch,event.velocity) end end)
screen:on(sequence,"note_on",function(event) if event.track>=6 then events:trigger(event.track,event.pitch,event.velocity) end end)
screen:on(sequence,"pattern_changed",function(event) events:pattern(event.pattern) end)
screen:on_mount(function() local state=sequence:state();events:step(state.step,state.pattern,state.position) end)
local editor=instrument:screen("track_editor","Overdrive editor");editor:pattern_grid();editor:performance_controls()
local navigation=instrument:navigation {initial="overdrive",events={{name="performance",from="*",to="overdrive"},
  {name="editor",from="*",to="track_editor"}}}
navigation:bind("F9","performance");navigation:bind("F10","editor")
return instrument
end
