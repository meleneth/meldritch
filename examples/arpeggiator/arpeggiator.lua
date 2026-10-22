local instrument = meldritch.instrument("Arpeggiator Playground")
local project = meldritch.musical.project(instrument, {
  id = "arpeggiator_playground", bpm = 108, max_steps = 64,
  steps_per_beat = 4, persistence = "arpeggiator-project.lua",
})
local sequence = project.sequence
local mixer = instrument:mixer {inputs = 8, headroom = 0.20}

local voices = {
  {id="Crystal", model="harmonic", length=15, pan={0.40,1.00}, mode="up", octaves=2, division=1, seed=11},
  {id="Undertow", model="fm", length=20, pan={1.00,0.44}, mode="down", octaves=2, division=2, seed=23},
  {id="Firefly", model="modal", length=28, pan={0.78,0.92}, mode="random", octaves=3, division=1, seed=1949},
}
local arps = {}

for index, spec in ipairs(voices) do
  local track = project:track {id=spec.id, kind="melodic", instrument="plaits:" .. spec.model}
  local voice = instrument:plaits {
    model=spec.model, harmonics=0.28 + index * 0.13,
    timbre=0.30 + index * 0.09, morph=0.18 + index * 0.08,
  }
  local envelope = instrument:adsr {
    attack=0.002 + index * 0.002, decay=0.10 + index * 0.035,
    sustain=0.42, release=0.16 + index * 0.08,
  }
  local amp = instrument:vca {}
  local level = instrument:gain {gain=0.72 - index * 0.08}
  local left = instrument:gain {gain=spec.pan[1]}
  local right = instrument:gain {gain=spec.pan[2]}
  instrument:connect(track.signal.pitch, voice.pitch)
  instrument:connect(track.signal.gate, voice.gate)
  instrument:connect(track.signal.velocity, voice.velocity)
  instrument:connect(track.signal.gate, envelope.gate)
  instrument:connect(voice.main, amp.input)
  instrument:connect(envelope.out, amp.cv)
  instrument:connect(amp.out, level.input)
  instrument:connect(track.signal.enabled, level.mod)
  instrument:connect(level.out, left.input)
  instrument:connect(level.out, right.input)
  instrument:connect(left.out, mixer.input[index].left)
  instrument:connect(right.out, mixer.input[index].right)
  instrument:control {id="tone_"..index,label=spec.id.." tone",target=voice.timbre,range={0,1}}
  instrument:control {id="shape_"..index,label=spec.id.." shape",target=voice.morph,range={0,1}}
  instrument:control {id="release_"..index,label=spec.id.." release",target=envelope.release,range={0.01,2},curve="log"}
  arps[index] = meldritch.arp.new {
    mode=spec.mode, octaves=spec.octaves, clock_division=spec.division,
    gate=index==2 and 0.62 or 0.72, phase=index==3 and "free" or "restart",
    velocity_shape=index==1 and {1.0,0.64,0.82,0.70}
      or (index==2 and {0.92,0.68} or {0.52,0.76,0.61,0.88}),
    seed=spec.seed,
  }
end

instrument:connect(mixer.left, instrument.output.left)
instrument:connect(mixer.right, instrument.output.right)
instrument:control {id="tempo",label="Tempo",target=sequence.bpm,range={72,150}}
instrument:control {id="master",label="Master",target=mixer.headroom,range={0.04,0.45}}

local progressions = {
  Glass = {
    {{step=1,notes={48,52,55,59}},{step=9,notes={50,53,57,60}}},
    {{step=1,notes={36,43,47}},{step=11,notes={38,45,48}}},
    {{step=1,notes={60,64,67}},{step=15,notes={62,65,69}}},
  },
  Tides = {
    {{step=1,notes={53,57,60,64}},{step=8,notes={48,52,55,59}}},
    {{step=1,notes={41,48,52}},{step=13,notes={43,50,53}}},
    {{step=1,notes={65,69,72}},{step=10,notes={60,64,67}},{step=20,notes={62,67,71}}},
  },
  Sparks = {
    {{step=1,notes={45,52,57,60}},{step=6,notes={47,54,59,62}},{step=11,notes={48,55,60,64}}},
    {{step=1,notes={33,40,45}},{step=9,notes={35,42,47}}},
    {{step=1,notes={57,60,64,67}},{step=8,notes={59,62,66,69}},{step=17,notes={60,64,67,71}}},
  },
  Home = {
    {{step=1,notes={48,52,55,59}},{step=12,notes={48,53,57,60}}},
    {{step=1,notes={36,43,48}},{step=15,notes={36,41,45}}},
    {{step=1,notes={60,67,71}},{step=14,notes={60,65,69}}},
  },
}

for _, name in ipairs({"Glass","Tides","Sparks","Home"}) do
  local pattern = project:pattern(name)
  for index, spec in ipairs(voices) do
    local source = meldritch.arp.from_pattern(progressions[name][index], spec.length)
    arps[index]:render(pattern, spec.id, spec.length, source, {section=name,voice=spec.id})
  end
end

-- 40 polymetric cycles: 28 steps * 40 at 108 BPM = 155.56 seconds.
project:arrangement {
  {name="Glass",pattern="Glass",repeats=10},
  {name="Tides",pattern="Tides",repeats=10},
  {name="Sparks",pattern="Sparks",repeats=10},
  {name="Home",pattern="Home",repeats=10},
}
sequence:start()
instrument:performance {layout="step_grid",sequencer=sequence,project_file="arpeggiator-project.lua",
  keys={Space="transport",s="start",x="stop",r="restart",Up="track:up",Down="track:down",
    Left="step:previous",Right="step:next",["["]="bank:previous",["]"]="bank:next",
    n="pattern:next",N="pattern:previous",["+"]="velocity:up",["-"]="velocity:down",
    p="pitch:up",P="pitch:down",g="gate:up",G="gate:down",S="save",R="reload"}}
local activity = meldritch.event_ui()
local screen = instrument:screen("performance","Arpeggiator Playground")
screen:performance_status(); screen:event_activity("Arpeggiated voices",activity)
screen:pattern_grid(); screen:performance_controls()
screen:on(sequence,"step_advanced",function(event) activity:step(event.step,event.pattern,event.position) end)
screen:on(sequence,"note_on",function(event) activity:trigger(event.track,event.pitch,event.velocity) end)
return instrument
