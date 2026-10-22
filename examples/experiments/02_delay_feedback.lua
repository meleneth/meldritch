-- Delay Feedback Laboratory: a playable bass sequencer and focused delay editor.
-- MicroLab mk3 knobs CC 74/71/76/77 all address the parameter selected from
-- Stream Deck or F1-F7. Change the CC declarations here for another profile.
local instrument = meldritch.instrument("Delay Feedback Laboratory")
instrument:devices { midi_controls={"MicroLab mk3:MicroLab mk3"},
  apc_mini="APC mini mk2:APC mini mk2 Control" }

local sequence=instrument:sequencer {bpm=118,steps=16,steps_per_beat=4}
local bass=sequence:track("Bassline")
local pattern=sequence:pattern("Feedback Bass")
pattern:notes("Bassline",{
  {step=1,note=36,velocity=.95,gate=.58},{step=3,note=36,velocity=.58,gate=.34},
  {step=5,note=43,velocity=.82,gate=.48},{step=7,note=39,velocity=.62,gate=.3},
  {step=9,note=36,velocity=1,gate=.68},{step=11,note=48,velocity=.7,gate=.3},
  {step=13,note=43,velocity=.86,gate=.52},{step=16,note=34,velocity=.7,gate=.24},
},16)

local source=instrument:oscillator {waveform="pulse",frequency=65.406,pulse_width=.32}
local envelope=instrument:adsr {attack=.002,decay=.12,sustain=.22,release=.09}
local impulse=instrument:constant {value=0}
local gates=instrument:mixer {inputs=2}
local amp=instrument:vca {}
local source_level=instrument:gain {gain=.72}
instrument:connect(bass.pitch,source.pitch)
instrument:connect(bass.gate,gates.input[1]);instrument:connect(impulse.out,gates.input[2])
instrument:connect(gates.out,envelope.gate)
instrument:connect(source.out,amp.input);instrument:connect(envelope.out,amp.cv)
instrument:connect(amp.out,source_level.input)

local modulation=instrument:lfo {waveform="sine",frequency=.17}
local mod_depth=instrument:constant {value=0}
local mod_scale=instrument:multiply {}
instrument:connect(modulation.out,mod_scale.a);instrument:connect(mod_depth.out,mod_scale.b)

local delay=instrument:stereo_delay {left_beats=.5,right_beats=.75,
  feedback=.68,crossfeed=.72,mix=.52,filter_cutoff=5200,feedback_drive=1.4}
instrument:connect(source_level.out,delay.input.left);instrument:connect(source_level.out,delay.input.right)
instrument:connect(mod_scale.out,delay.left_time_mod);instrument:connect(mod_scale.out,delay.right_time_mod)
instrument:connect(sequence.tempo,delay.bpm)
local master_left=instrument:gain {gain=.82};local master_right=instrument:gain {gain=.82}
instrument:connect(delay.left,master_left.input);instrument:connect(delay.right,master_right.input)
instrument:connect(master_left.out,instrument.output.left);instrument:connect(master_right.out,instrument.output.right)

instrument:control {id="impulse",label="Impulse",target=impulse.value,range={0,1}}
instrument:control {id="left_time",label="Left echo time",target=delay.left_beats,range={.0625,4}}
instrument:control {id="right_time",label="Right echo time",target=delay.right_beats,range={.0625,4}}
instrument:control {id="feedback",label="Feedback",target=delay.feedback,range={0,.94}}
instrument:control {id="feedback_drive",label="Feedback drive",target=delay.feedback_drive,range={.1,8}}
instrument:control {id="filter",label="Feedback filter",target=delay.filter_cutoff,range={80,16000},curve="log"}
instrument:control {id="pingpong",label="Ping pong",target=delay.crossfeed,range={0,.95}}
instrument:control {id="modulation",label="Delay modulation",target=mod_depth.value,range={0,.2}}
instrument:control {id="source_level",label="Bassline level",target=source_level.gain,range={0,1.25}}
instrument:control {id="master_volume",label="Master volume",target=master_left.gain,
  targets={master_right.gain},range={0,1.25}}

instrument:performance {layout="step_grid",sequencer=sequence,
  keys={Space="transport",s="start",x="stop",r="restart",Left="step:previous",Right="step:next",
    Up="velocity:up",Down="velocity:down",["["]="bank:previous",["]"]="bank:next",
    F1="control:select:left_time",F2="control:select:right_time",F3="control:select:feedback",
    F4="control:select:feedback_drive",F5="control:select:filter",F6="control:select:pingpong",
    F7="control:select:modulation",F8="control:select:master_volume",h="control:decrease",l="control:increase",
    ["1"]="toggle:1",["2"]="toggle:2",["3"]="toggle:3",["4"]="toggle:4",
    ["5"]="toggle:5",["6"]="toggle:6",["7"]="toggle:7",["8"]="toggle:8"},
  focus={controls={"left_time","right_time","feedback","feedback_drive","filter","pingpong","modulation","master_volume"},
    midi={{channel=1,cc=74},{channel=1,cc=71},{channel=1,cc=76},{channel=1,cc=77}}},
  apc={pages={"steps","performance"},performance_rows={"pattern","mute","solo","track","restart","","",""},
    faders={"source_level","left_time","right_time","feedback","feedback_drive","filter","pingpong","modulation"},
    master="master_volume"}}

instrument:control_surface {id="delay_feedback_deck",device="stream_deck_xl",initial_page="lab",pages={{id="lab",buttons={
  {id="start",index=0,label="START",icon="triangle",color={18,55,42},active_color={40,190,85},press={command="start"}},
  {id="stop",index=1,label="STOP",icon="square",color={60,20,28},active_color={235,45,63},press={command="stop"}},
  {id="restart",index=2,label="RESTART",color={50,35,16},active_color={243,168,51},press={command="restart"}},
  {id="hit",index=3,label="IMPULSE",icon="triangle",mode="momentary",color={45,12,8},active_color={255,90,25},press={control="impulse",value=1},release={control="impulse",value=0}},
  {id="left",index=8,label="LEFT TIME",color={10,35,45},active_color={54,197,244},press={command="control:select:left_time"}},
  {id="right",index=9,label="RIGHT TIME",color={10,35,45},active_color={54,197,244},press={command="control:select:right_time"}},
  {id="feedback",index=10,label="FEEDBACK",color={45,22,8},active_color={233,133,55},press={command="control:select:feedback"}},
  {id="drive",index=11,label="DRIVE",color={50,15,8},active_color={236,39,63},press={command="control:select:feedback_drive"}},
  {id="filter",index=16,label="FILTER",color={35,38,8},active_color={157,230,78},press={command="control:select:filter"}},
  {id="pingpong",index=17,label="PINGPONG",color={15,18,40},active_color={51,136,222},press={command="control:select:pingpong"}},
  {id="modulation",index=18,label="MOD",color={28,15,38},active_color={200,120,175},press={command="control:select:modulation"}},
  {id="master",index=19,label="MASTER",color={44,30,49},active_color={211,238,211},press={command="control:select:master_volume"}},
  {id="decay",index=24,label="DECAY",color={15,35,30},press={control="feedback",value=.48}},
  {id="build",index=25,label="BUILD",color={45,22,8},press={control="feedback",value=.9}},
  {id="ping_toggle",index=26,label="PING ON",mode="toggle",color={15,18,40},active_color={45,90,230},press={control="pingpong",value=.78},release={control="pingpong",value=0}},
  {id="mod_toggle",index=27,label="MOD ON",mode="toggle",color={28,15,38},active_color={170,50,210},press={control="modulation",value=.08},release={control="modulation",value=0}},
}}}}

local activity=meldritch.event_ui()
local screen=instrument:screen("laboratory","Delay Feedback Laboratory")
screen:performance_status();screen:pattern_grid();screen:control_focus()
screen:on(sequence,"step_advanced",function(event) activity:step(event.step,event.pattern,event.position) end)
sequence:start()
return instrument
