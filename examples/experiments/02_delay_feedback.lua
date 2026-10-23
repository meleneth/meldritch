-- Delay Feedback Laboratory: dry 909 rhythm, key-locked APC piano roll,
-- kick-ducked bass delay, and pickup-safe focused hardware controls.
local instrument = meldritch.instrument("Delay Feedback Laboratory")
instrument:devices { midi_controls={"MicroLab mk3:MicroLab mk3"},
  apc_mini="APC mini mk2:APC mini mk2 Control" }

local sequence=instrument:sequencer {bpm=118,steps=64,steps_per_beat=4}
local kick_track=sequence:track("Kick")
local snare_track=sequence:track("Snare")
local hat_track=sequence:track("Closed Hat")
local bass_track=sequence:track("Bassline")
local pattern=sequence:pattern("Feedback Bass")
pattern:steps("Kick",{1,0,0,0, 1,0,0,0, 1,0,0,0, 1,0,1,0})
pattern:steps("Snare",{0,0,0,0, 1,0,0,0, 0,0,0,0, 1,0,0,.45})
pattern:steps("Closed Hat",{.8,0,.55,0, .72,0,.9,0, .8,0,.55,0, .72,0,1,0})
pattern:notes("Bassline",{
  {step=1,note=36,velocity=.95,gate=2},{step=4,note=36,velocity=.58,gate=1},
  {step=6,note=43,velocity=.82,gate=2},{step=9,note=39,velocity=.62,gate=1},
  {step=11,note=36,velocity=1,gate=3},{step=15,note=41,velocity=.7,gate=1},
  {step=18,note=43,velocity=.86,gate=2},{step=23,note=38,velocity=.7,gate=2},
  {step=27,note=39,velocity=.8,gate=1},{step=30,note=36,velocity=.92,gate=3},
},32)

local kit=meldritch.samples {source="strudel",bank="RolandTR909",cache=true}
local drum_bus=instrument:mixer {inputs=8,headroom=.34}
local kick=instrument:sample_player {sample=kit:load("bd",0),gain=1}
local snare=instrument:sample_player {sample=kit:load("sd",0),gain=.84}
local hat=instrument:sample_player {sample=kit:load("hh",0),gain=.62}
instrument:connect(kick_track.out,kick.trigger);instrument:connect(snare_track.out,snare.trigger)
instrument:connect(hat_track.out,hat.trigger)
instrument:connect(kick.left,drum_bus.input[1].left);instrument:connect(kick.right,drum_bus.input[1].right)
instrument:connect(snare.left,drum_bus.input[2].left);instrument:connect(snare.right,drum_bus.input[2].right)
instrument:connect(hat.left,drum_bus.input[3].left);instrument:connect(hat.right,drum_bus.input[3].right)

local source=instrument:oscillator {waveform="pulse",frequency=65.406,pulse_width=.32}
local envelope=instrument:adsr {attack=.002,decay=.12,sustain=.22,release=.09}
local impulse=instrument:constant {value=0}
local gates=instrument:mixer {inputs=2}
local amp=instrument:vca {};local source_level=instrument:gain {gain=.72}
instrument:connect(bass_track.pitch,source.pitch)
instrument:connect(bass_track.gate,gates.input[1]);instrument:connect(impulse.out,gates.input[2])
instrument:connect(gates.out,envelope.gate);instrument:connect(source.out,amp.input)
instrument:connect(envelope.out,amp.cv);instrument:connect(amp.out,source_level.input)

local modulation=instrument:lfo {waveform="sine",frequency=.17}
local mod_depth=instrument:constant {value=0};local mod_scale=instrument:multiply {}
instrument:connect(modulation.out,mod_scale.a);instrument:connect(mod_depth.out,mod_scale.b)
local delay=instrument:stereo_delay {left_beats=.5,right_beats=.75,
  feedback=.68,crossfeed=.72,mix=.52,filter_cutoff=5200,feedback_drive=1.4}
instrument:connect(source_level.out,delay.input.left);instrument:connect(source_level.out,delay.input.right)
instrument:connect(mod_scale.out,delay.left_time_mod);instrument:connect(mod_scale.out,delay.right_time_mod)
instrument:connect(sequence.tempo,delay.bpm)

-- Drums stay dry. The audible kick is only the detector for the delayed bass.
local duck_left=instrument:ducker {threshold=.035,depth=.72,attack=.002,release=.22}
local duck_right=instrument:ducker {threshold=.035,depth=.72,attack=.002,release=.22}
instrument:connect(delay.left,duck_left.input);instrument:connect(delay.right,duck_right.input)
instrument:connect(kick.left,duck_left.sidechain);instrument:connect(kick.left,duck_right.sidechain)
local final_mix=instrument:mixer {inputs=8,headroom=.72}
instrument:connect(drum_bus.left,final_mix.input[1].left);instrument:connect(drum_bus.right,final_mix.input[1].right)
instrument:connect(duck_left.out,final_mix.input[2].left);instrument:connect(duck_right.out,final_mix.input[2].right)
local master_left=instrument:gain {gain=.82};local master_right=instrument:gain {gain=.82}
instrument:connect(final_mix.left,master_left.input);instrument:connect(final_mix.right,master_right.input)
instrument:connect(master_left.out,instrument.output.left);instrument:connect(master_right.out,instrument.output.right)

local function control(id,label,target,range,targets,curve)
  instrument:control {id=id,label=label,target=target,targets=targets,range=range,curve=curve}
end
control("impulse","Impulse",impulse.value,{0,1})
control("source_level","Bass level",source_level.gain,{0,1.25})
control("attack","Bass attack",envelope.attack,{.001,2})
control("decay","Bass decay",envelope.decay,{.005,3})
control("sustain","Bass sustain",envelope.sustain,{0,1})
control("release","Bass release",envelope.release,{.005,4})
control("left_time","Left echo",delay.left_beats,{.0625,4})
control("right_time","Right echo",delay.right_beats,{.0625,4})
control("feedback","Feedback",delay.feedback,{0,.94})
control("feedback_drive","Feedback drive",delay.feedback_drive,{.1,8})
control("filter","Feedback filter",delay.filter_cutoff,{80,16000},nil,"log")
control("pingpong","Ping pong",delay.crossfeed,{0,.95})
control("modulation","Delay modulation",mod_depth.value,{0,.2})
control("delay_mix","Delay mix",delay.mix,{0,1})
control("duck_depth","Duck depth",duck_left.depth,{0,1},{duck_right.depth})
control("duck_attack","Duck attack",duck_left.attack,{.00001,1},{duck_right.attack},"log")
control("duck_release","Duck release",duck_left.release,{.001,3},{duck_right.release},"log")
control("duck_threshold","Duck threshold",duck_left.threshold,{0,.99},{duck_right.threshold})
control("master_volume","Master volume",master_left.gain,{0,1.25},{master_right.gain})

local focused={"source_level","attack","decay","sustain","release","left_time","right_time",
  "feedback","feedback_drive","filter","pingpong","modulation","delay_mix",
  "duck_depth","duck_attack","duck_release","duck_threshold","master_volume"}
instrument:performance {layout="step_grid",sequencer=sequence,
  keys={Space="transport",s="start",x="stop",r="restart",Left="step:previous",Right="step:next",
    Up="velocity:up",Down="velocity:down",["["]="view:previous",["]"]="view:next",
    z="zoom:in",Z="zoom:out",[";"]="length:decrease",["'"]="length:increase",
    k="key:down",K="key:up",F1="control:select:source_level",F2="control:select:attack",
    F3="control:select:decay",F4="control:select:sustain",F5="control:select:release",
    F6="control:select:feedback",F7="control:select:delay_mix",F8="control:select:duck_depth",
    h="control:decrease",l="control:increase",
    ["1"]="toggle:1",["2"]="toggle:2",["3"]="toggle:3",["4"]="toggle:4",
    ["5"]="toggle:5",["6"]="toggle:6",["7"]="toggle:7",["8"]="toggle:8"},
  focus={controls=focused,midi={{channel=1,cc=74},{channel=1,cc=71},{channel=1,cc=76},{channel=1,cc=77}}},
  apc={pages={"steps","performance"},performance_rows={"pattern","mute","solo","track","restart","","",""},
    piano_roll={drums={"Kick","Snare","Closed Hat"},track="Bassline",root=36,scale={0,2,3,5,7}},
    track_buttons={"view:previous","view:next","zoom:in","zoom:out","key:down","key:up","length:decrease","length:increase"},
    faders={"source_level","attack","decay","sustain","release","feedback","duck_depth","delay_mix"},
    fader_banks={
      {"source_level","attack","decay","sustain","release","feedback","duck_depth","delay_mix"},
      {"left_time","right_time","feedback","feedback_drive","filter","pingpong","modulation","delay_mix"},
      {"duck_depth","duck_attack","duck_release","duck_threshold","source_level","feedback","filter","delay_mix"},
    },
    scene_buttons={"transport","restart","faders:bank:0","faders:bank:1","faders:bank:2","","",""},
    master="master_volume"}}

local common={
  {id="start",index=0,label="START",icon="triangle",color={18,55,42},active_color={40,190,85},press={command="start"}},
  {id="stop",index=1,label="STOP",icon="square",color={60,20,28},active_color={235,45,63},press={command="stop"}},
  {id="restart",index=2,label="RESTART",color={50,35,16},active_color={243,168,51},press={command="restart"}},
  {id="hit",index=3,label="IMPULSE",icon="triangle",mode="momentary",color={45,12,8},active_color={255,90,25},press={control="impulse",value=1},release={control="impulse",value=0}},
}
instrument:control_surface {id="delay_feedback_deck",device="stream_deck_xl",initial_page="delay",pages={
 {id="delay",buttons={common[1],common[2],common[3],common[4],
  {id="left",index=8,label="LEFT TIME",press={command="control:select:left_time"}},
  {id="right",index=9,label="RIGHT TIME",press={command="control:select:right_time"}},
  {id="feedback",index=10,label="FEEDBACK",press={command="control:select:feedback"}},
  {id="drive",index=11,label="DRIVE",press={command="control:select:feedback_drive"}},
  {id="filter",index=16,label="FILTER",press={command="control:select:filter"}},
  {id="ping",index=17,label="PING PONG",press={command="control:select:pingpong"}},
  {id="mod",index=18,label="MOD",press={command="control:select:modulation"}},
  {id="mix",index=19,label="DELAY MIX",press={command="control:select:delay_mix"}},
  {id="bass_faders_env",index=24,label="FADERS MAIN",press={command="faders:bank:0"}},
  {id="bass_faders_delay",index=25,label="FADERS DELAY",press={command="faders:bank:1"}},
  {id="bass_faders_duck",index=26,label="FADERS DUCK",press={command="faders:bank:2"}},
  {id="bass_page",index=31,label="BASS / DUCK",color={44,30,49},press={page="bass"}},}},
 {id="bass",buttons={
  {id="play",index=0,label="PLAY/STOP",press={command="transport"}},
  {id="prev",index=1,label="VIEW -",press={command="view:previous"}},
  {id="next",index=2,label="VIEW +",press={command="view:next"}},
  {id="zin",index=3,label="ZOOM IN",press={command="zoom:in"}},
  {id="zout",index=4,label="ZOOM OUT",press={command="zoom:out"}},
  {id="keydn",index=5,label="KEY -",press={command="key:down"}},
  {id="keyup",index=6,label="KEY +",press={command="key:up"}},
  {id="lendn",index=7,label="LENGTH -",press={command="length:decrease"}},
  {id="lenup",index=8,label="LENGTH +",press={command="length:increase"}},
  {id="level",index=9,label="BASS LEVEL",press={command="control:select:source_level"}},
  {id="attack",index=10,label="ATTACK",press={command="control:select:attack"}},
  {id="decay",index=11,label="DECAY",press={command="control:select:decay"}},
  {id="sustain",index=12,label="SUSTAIN",press={command="control:select:sustain"}},
  {id="release",index=13,label="RELEASE",press={command="control:select:release"}},
  {id="depth",index=16,label="DUCK DEPTH",press={command="control:select:duck_depth"}},
  {id="dattack",index=17,label="DUCK ATTACK",press={command="control:select:duck_attack"}},
  {id="drelease",index=18,label="DUCK RELEASE",press={command="control:select:duck_release"}},
  {id="threshold",index=19,label="THRESHOLD",press={command="control:select:duck_threshold"}},
  {id="master",index=20,label="MASTER",press={command="control:select:master_volume"}},
  {id="faders_env",index=24,label="FADERS MAIN",press={command="faders:bank:0"}},
  {id="faders_delay",index=25,label="FADERS DELAY",press={command="faders:bank:1"}},
  {id="faders_duck",index=26,label="FADERS DUCK",press={command="faders:bank:2"}},
  {id="delay_page",index=31,label="DELAY",color={10,35,45},press={page="delay"}},}},
}}

local activity=meldritch.event_ui()
local screen=instrument:screen("laboratory","Delay Feedback Laboratory")
screen:performance_status();screen:pattern_grid()
screen:label("APC: top 3 = kick/snare/hat | bottom 5 = key-locked bass | red = view/zoom/key/length")
screen:label("Pickup: ↑ raise physical control  ↓ lower it  ◆ captured")
screen:control_focus()
screen:on(sequence,"step_advanced",function(event) activity:step(event.step,event.pattern,event.position) end)
sequence:start()
return instrument
