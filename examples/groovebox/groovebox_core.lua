return function(render_arrangement,project_path)
local box=meldritch.instrument("Meldritch Groovebox")
box:devices { apc_mini="APC mini mk2:APC mini mk2 Control" }
local kit=meldritch.samples {source="strudel",bank="RolandTR909",cache=true}
local seq=box:sequencer {bpm=128,steps=16,steps_per_beat=4}
local mixer=box:mixer {inputs=8,headroom=.18}
local names={"Kick","Snare","CHat","OHat","Clap","Perc","Bass","Lead"}
local pans={{1,1},{.82,.82},{.62,1},{1,.58},{.78,.88},{1,.42},{1,1},{.58,1}}
local tracks={};for i,name in ipairs(names) do tracks[i]=seq:track(name) end
local volume_controls={}
local function finish_track(index,audio,voice_parameter,voice_label,voice_range,voice_curve)
  local level=box:gain {gain=index<=2 and .9 or (index<=6 and .62 or .75)}
  box:connect(audio,level.input);box:connect(tracks[index].enabled,level.mod)
  local left=box:gain {gain=pans[index][1]};local right=box:gain {gain=pans[index][2]}
  box:connect(level.out,left.input);box:connect(level.out,right.input)
  box:connect(left.out,mixer.input[index].left);box:connect(right.out,mixer.input[index].right)
  local id="volume_"..index;volume_controls[index]=id
  box:control {id=id,label=names[index].." volume",target=level.gain,range={0,1.5}}
  box:control {id="voice_"..index,label=names[index].." "..voice_label,target=voice_parameter,range=voice_range,curve=voice_curve}
end
local sample_names={"bd","sd","hh","oh","cp","lt"} -- Low tom is the explicit percussion substitute.
for i,sample_name in ipairs(sample_names) do
  local player=box:sample_player {sample=kit:load(sample_name,0),gain=1};box:connect(tracks[i].out,player.trigger)
  if i==3 or i==4 then local filter=box:svf {cutoff=i==3 and 8500 or 6200,resonance=.2};box:connect(player.left,filter.input);finish_track(i,filter.highpass,filter.cutoff,"filter",{80,12000},"log")
  else local color=box:drive {amount=i==1 and 2.5 or 1.3,output_gain=.8};box:connect(player.left,color.input);finish_track(i,color.out,color.amount,"drive",{.1,12}) end
end
local bass=box:plaits {model="virtual_analog_vcf",harmonics=.35,timbre=.62,morph=.28}
local bass_filter=box:svf {cutoff=780,resonance=.7};local bass_env=box:adsr {attack=.002,decay=.12,sustain=.5,release=.09};local bass_amp=box:vca {};local bass_drive=box:drive {amount=4.5,output_gain=.65}
box:connect(tracks[7].pitch,bass.pitch);box:connect(tracks[7].gate,bass.gate);box:connect(tracks[7].velocity,bass.velocity);box:connect(tracks[7].gate,bass_env.gate);box:connect(bass.main,bass_filter.input);box:connect(bass_filter.lowpass,bass_drive.input);box:connect(bass_drive.out,bass_amp.input);box:connect(bass_env.out,bass_amp.cv);finish_track(7,bass_amp.out,bass_filter.cutoff,"filter",{80,12000},"log")
local lead=box:plaits {model="harmonic",harmonics=.7,timbre=.55,morph=.4};local lead_delay=box:delay {time=.28125,feedback=.52,mix=.4};local lead_env=box:adsr {attack=.008,decay=.2,sustain=.62,release=.45};local lead_amp=box:vca {}
box:connect(tracks[8].pitch,lead.pitch);box:connect(tracks[8].gate,lead.gate);box:connect(tracks[8].velocity,lead.velocity);box:connect(tracks[8].gate,lead_env.gate);box:connect(lead.main,lead_amp.input);box:connect(lead_env.out,lead_amp.cv);box:connect(lead_amp.out,lead_delay.input);finish_track(8,lead_delay.out,lead.harmonics,"harmonics",{0,1})
box:connect(mixer.left,box.output.left);box:connect(mixer.right,box.output.right)
box:control {id="master_level",label="Master level",target=mixer.headroom,range={.01,1}}
box:control {id="master_effect",label="Lead delay mix",target=lead_delay.mix,range={0,1}}
box:control {id="Tempo",label="Tempo",target=seq.bpm,range={60,220}}
local authored=meldritch.include("patterns.lua")
for _,pattern_name in ipairs({"Foundation","Broken","Acid","Atmosphere"}) do local pattern=seq:pattern(pattern_name);local data=authored[pattern_name];for i=1,6 do pattern:steps(names[i],data[names[i]]) end;pattern:notes("Bass",data.Bass);pattern:notes("Lead",data.Lead) end
if project_path then seq:load_project(project_path) end
if render_arrangement then seq:arrangement {{pattern="Foundation",repeats=4},{pattern="Broken",repeats=4},{pattern="Acid",repeats=4},{pattern="Atmosphere",repeats=4},{pattern="Foundation",repeats=4}} end
seq:start()
box:performance {layout="step_grid",sequencer=seq,project_file="groovebox-project.json",
  keys={F1="pattern:0",F2="pattern:1",F3="pattern:2",F4="pattern:3",Space="transport",s="start",x="stop",r="restart",Up="track:up",Down="track:down",["["]="bank:0",["]"]="bank:1",m="mute",o="solo",c="copy:next",C="clear",j="control:next",k="control:previous",h="control:decrease",l="control:increase",["+"]="velocity:up",["-"]="velocity:down",p="pitch:up",P="pitch:down",g="gate:up",G="gate:down",S="save",["1"]="toggle:1",["2"]="toggle:2",["3"]="toggle:3",["4"]="toggle:4",["5"]="toggle:5",["6"]="toggle:6",["7"]="toggle:7",["8"]="toggle:8"},
  apc={pages={"steps","performance"},performance_rows={"pattern","mute","solo","track","restart","","",""},faders={volume_controls[1],volume_controls[2],volume_controls[3],volume_controls[4],volume_controls[5],volume_controls[6],volume_controls[7],volume_controls[8],"master_level"}}}
local screen=box:screen("groovebox","Groovebox");screen:performance_status();screen:pattern_grid();screen:performance_controls()
return box
end
