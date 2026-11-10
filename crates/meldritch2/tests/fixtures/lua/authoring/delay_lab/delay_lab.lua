local instrument = meldritch.instrument("Delay Laboratory")
local project = meldritch.musical.project(instrument, {
  id="delay_laboratory", bpm=96, max_steps=64, steps_per_beat=4,
  persistence="delay-lab-project.lua",
})
local sequence = project.sequence
local dry = instrument:mixer {inputs=8,headroom=0.22}
local definitions = {
  {id="Pulse",model="virtual_analog_vcf",length=32,pan={1.0,0.42},root=45},
  {id="Chime",model="modal",length=48,pan={0.45,1.0},root=60},
}
local arps = {}
for index,spec in ipairs(definitions) do
  local track=project:track {id=spec.id,kind="melodic",instrument="plaits:"..spec.model}
  local voice=instrument:plaits {model=spec.model,harmonics=0.38+index*.15,timbre=.42,morph=.28+index*.12}
  local envelope=instrument:adsr {attack=.003,decay=.13+index*.08,sustain=.32,release=.18+index*.17}
  local amp=instrument:vca {};local level=instrument:gain {gain=index==1 and .74 or .58}
  local left=instrument:gain {gain=spec.pan[1]};local right=instrument:gain {gain=spec.pan[2]}
  instrument:connect(track.signal.pitch,voice.pitch);instrument:connect(track.signal.gate,voice.gate)
  instrument:connect(track.signal.velocity,voice.velocity);instrument:connect(track.signal.gate,envelope.gate)
  instrument:connect(voice.main,amp.input);instrument:connect(envelope.out,amp.cv)
  instrument:connect(amp.out,level.input);instrument:connect(track.signal.enabled,level.mod)
  instrument:connect(level.out,left.input);instrument:connect(level.out,right.input)
  instrument:connect(left.out,dry.input[index].left);instrument:connect(right.out,dry.input[index].right)
  instrument:control {id="timbre_"..index,label=spec.id.." timbre",target=voice.timbre,range={0,1}}
  instrument:control {id="morph_"..index,label=spec.id.." morph",target=voice.morph,range={0,1}}
  arps[index]=meldritch.arp.new {mode=index==1 and "up_down" or "played",octaves=index==1 and 2 or 3,
    clock_division=index==1 and 1 or 3,gate=index==1 and .58 or .74,phase=index==1 and "restart" or "free",
    velocity_shape=index==1 and {1,.65,.78,.58} or {0.72,1,.61},seed=700+index}
end

local echoes=instrument:stereo_delay {
  left_beats=.75,right_beats=1.25,feedback=.68,crossfeed=1.0,mix=.48,
  filter_cutoff=3600,feedback_drive=1.8,
}
local drift_left=instrument:lfo {frequency=.037};local drift_right=instrument:lfo {frequency=.051}
local left_depth=instrument:affine {scale=.009,offset=0};local right_depth=instrument:affine {scale=.013,offset=0}
instrument:connect(drift_left.out,left_depth.input);instrument:connect(drift_right.out,right_depth.input)
instrument:connect(left_depth.out,echoes.left_time_mod);instrument:connect(right_depth.out,echoes.right_time_mod)
instrument:connect(sequence.tempo,echoes.bpm)
instrument:connect(dry.left,echoes.input.left);instrument:connect(dry.right,echoes.input.right)
instrument:connect(echoes.left,instrument.output.left);instrument:connect(echoes.right,instrument.output.right)
instrument:control {id="tempo",label="Tempo",target=sequence.bpm,range={60,150}}
instrument:control {id="left_time",label="Left beats",target=echoes.left_beats,range={.0625,4}}
instrument:control {id="right_time",label="Right beats",target=echoes.right_beats,range={.0625,4}}
instrument:control {id="feedback",label="Feedback",target=echoes.feedback,range={0,.94}}
instrument:control {id="crossfeed",label="Ping pong",target=echoes.crossfeed,range={0,1}}
instrument:control {id="delay_mix",label="Echo mix",target=echoes.mix,range={0,1}}
instrument:control {id="feedback_filter",label="Feedback filter",target=echoes.filter_cutoff,range={80,16000},curve="log"}
instrument:control {id="feedback_drive",label="Echo saturation",target=echoes.feedback_drive,range={.1,8}}

local sections = {
  Dry={{{step=1,notes={45,48,52}},{step=17,notes={43,47,50}}},{{step=1,notes={60,64,67}}}},
  Bounce={{{step=1,notes={45,52,57}},{step=13,notes={48,55,60}}},{{step=1,notes={60,63,67}},{step=25,notes={62,65,69}}}},
  Smear={{{step=1,notes={41,48,53}},{step=9,notes={43,50,55}},{step=21,notes={45,52,57}}},{{step=1,notes={65,69,72,76}},{step=31,notes={60,64,67,71}}}},
  Orbit={{{step=1,notes={45,48,52,55}},{step=17,notes={38,45,50,53}}},{{step=1,notes={60,67,71}},{step=19,notes={62,69,72}},{step=37,notes={64,71,74}}}},
}
for _,name in ipairs({"Dry","Bounce","Smear","Orbit"}) do
  local pattern=project:pattern(name)
  for index,spec in ipairs(definitions) do
    arps[index]:render(pattern,spec.id,spec.length,
      meldritch.arp.from_pattern(sections[name][index],spec.length),{section=name})
  end
end
-- 32 cycles * 48 steps at 96 BPM = 240 seconds.
project:arrangement {{name="Dry",pattern="Dry",repeats=8},{name="Bounce",pattern="Bounce",repeats=8},
  {name="Smear",pattern="Smear",repeats=8},{name="Orbit",pattern="Orbit",repeats=8}}
sequence:start()
instrument:performance {layout="step_grid",sequencer=sequence,project_file="delay-lab-project.lua",
  keys={Space="transport",s="start",x="stop",r="restart",Up="track:up",Down="track:down",
    Left="step:previous",Right="step:next",["["]="bank:previous",["]"]="bank:next",
    n="pattern:next",N="pattern:previous",["+"]="velocity:up",["-"]="velocity:down",
    p="pitch:up",P="pitch:down",g="gate:up",G="gate:down",S="save",R="reload"}}
local activity=meldritch.event_ui();local screen=instrument:screen("laboratory","Delay Laboratory")
screen:performance_status();screen:event_activity("Echo sources",activity);screen:pattern_grid();screen:performance_controls()
screen:on(sequence,"step_advanced",function(event) activity:step(event.step,event.pattern,event.position) end)
screen:on(sequence,"note_on",function(event) activity:trigger(event.track,event.pitch,event.velocity) end)
return instrument
