return function(edit_path)
local instrument=meldritch.instrument("Seeded Constellation")
local project=meldritch.musical.project(instrument,{id="seeded_constellation",bpm=104,max_steps=64,steps_per_beat=4,persistence="generative-edits.lua"})
local sequence=project.sequence
local mixer=instrument:mixer {inputs=8,headroom=.22}
local definitions={
  {id="Orbit",length=24,model="harmonic",root=48,color="cyan"},
  {id="Dust",length=32,model="modal",root=60,color="violet"},
  {id="Pulse",length=48,model="virtual_analog_vcf",root=36,color="orange"},
}
local tracks={}
for index,spec in ipairs(definitions) do
  local track=project:track {id=spec.id,kind="melodic",instrument="plaits:"..spec.model,controls={"tone_"..index,"volume_"..index}}
  tracks[index]=track
  local voice=instrument:plaits {model=spec.model,harmonics=.35+.12*index,timbre=.38,morph=.25}
  local envelope=instrument:adsr {attack=.005,decay=.16,sustain=.55,release=.25}
  local amp=instrument:vca {}
  instrument:connect(track.signal.pitch,voice.pitch);instrument:connect(track.signal.gate,voice.gate);instrument:connect(track.signal.velocity,voice.velocity)
  instrument:connect(track.signal.gate,envelope.gate);instrument:connect(voice.main,amp.input);instrument:connect(envelope.out,amp.cv)
  local level=instrument:gain {gain=.7};instrument:connect(amp.out,level.input);instrument:connect(track.signal.enabled,level.mod)
  instrument:connect(level.out,mixer.input[index].left);instrument:connect(level.out,mixer.input[index].right)
  instrument:control {id="tone_"..index,label=spec.id.." tone",target=voice.timbre,range={0,1}}
  instrument:control {id="volume_"..index,label=spec.id.." volume",target=level.gain,range={0,1.4}}
end
instrument:connect(mixer.left,instrument.output.left);instrument:connect(mixer.right,instrument.output.right)
instrument:control {id="Tempo",label="Tempo",target=sequence.bpm,range={50,180}}

-- Deterministic authored programming remains ordinary Lua. Interactive edits
-- are saved separately and applied after this generator runs again.
local state=0x51a7
local function random(limit) state=(state*1103515245+12345)%2147483648;return state%limit end
local scale={0,2,3,7,10}
local generated=project:pattern("Generated")
for _,spec in ipairs(definitions) do
  local events={}
  for step=1,spec.length do
    if random(100)<24 then events[#events+1]={step=step,note=spec.root+scale[random(#scale)+1]+12*random(2),velocity=.45+random(50)/100,gate=.25+random(65)/100} end
  end
  generated:notes(spec.id,spec.length,events)
end
local sparse=project:pattern("Sparse")
for _,spec in ipairs(definitions) do sparse:notes(spec.id,spec.length,{{step=1,note=spec.root,velocity=.75,gate=.9},{step=math.floor(spec.length/2)+1,note=spec.root+7,velocity=.62,gate=.7}}) end
if edit_path then project:reload(edit_path) end
project:arrangement {{name="Constellation",pattern="Generated",repeats=3},{name="Breath",pattern="Sparse",repeats=1}}
sequence:start()
instrument:performance {layout="step_grid",sequencer=sequence,project_file="generative-edits.lua",keys={Space="transport",Up="track:up",Down="track:down",Left="step:previous",Right="step:next",["["]="bank:previous",["]"]="bank:next",n="pattern:next",N="pattern:previous",d="duplicate",[","]="rotate:left",["."]="rotate:right",["+"]="velocity:up",["-"]="velocity:down",p="pitch:up",P="pitch:down",g="gate:up",G="gate:down",C="clear",S="save",R="reload",["1"]="toggle:1",["2"]="toggle:2",["3"]="toggle:3",["4"]="toggle:4",["5"]="toggle:5",["6"]="toggle:6",["7"]="toggle:7",["8"]="toggle:8"}}
local activity=meldritch.event_ui()
local performance=instrument:screen("performance","Generative performance")
performance:performance_status();performance:event_activity("Latest authored note",activity);performance:performance_controls()
performance:on(sequence,"step_advanced",function(event) activity:step(event.step,event.pattern,event.position) end)
performance:on(sequence,"note_on",function(event) activity:trigger(event.track,event.pitch,event.velocity) end)
local editor=instrument:screen("editor","Generative track editor");editor:pattern_grid();editor:performance_controls()
local navigation=instrument:navigation {initial="performance",events={{name="performance",from="*",to="performance"},{name="editor",from="*",to="editor"}}}
navigation:bind("F9","performance");navigation:bind("F10","editor")
return instrument
end
