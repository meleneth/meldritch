local instrument = meldritch.instrument("Control Plane Session")

local sequence = instrument:sequencer {
  bpm = 118,
  steps = 16,
  steps_per_beat = 4,
}
local kick_track = sequence:track("kick")
local main = sequence:pattern("Main")
main:steps("kick", { 1,0,0,0, 1,0,0,0, 1,0,0,0, 1,0,0,0 })
sequence:switch_pattern("Main")

local kick = instrument:oscillator { waveform = "sine", frequency = 52.0 }
local kick_amp = instrument:vca {}
instrument:connect(kick.out, kick_amp.input)
instrument:connect(kick_track.out, kick_amp.cv)

local synth = instrument:oscillator { waveform = "saw", frequency = 110.0 }
local filter = instrument:svf { cutoff = 900.0, resonance = 0.25 }
local synth_gain = instrument:gain { gain = 0.16 }
instrument:connect(synth.out, filter.input)
instrument:connect(filter.lowpass, synth_gain.input)

local mix = instrument:mixer { inputs = 2 }
instrument:connect(kick_amp.out, mix.input[1])
instrument:connect(synth_gain.out, mix.input[2])
instrument:connect(mix.out, instrument.output.left)
instrument:connect(mix.out, instrument.output.right)

instrument:control {
  id = "filter_cutoff", label = "Cutoff", target = filter.cutoff,
  range = { 80.0, 8000.0 }, curve = "log",
}
instrument:control {
  id = "filter_resonance", label = "Resonance", target = filter.resonance,
  range = { 0.0, 0.95 },
}

local server = meldritch.server { host = "127.0.0.1", port = 8080 }
server:websocket("/performance", {})
local performance = server:performance { name = "Two Performer Control Planes" }

performance:role("drummer", { controls = {} })
performance:role("synth", { controls = { "filter_cutoff", "filter_resonance" } })
performance:role("leader", { controls = "*" })
performance:role("observer", { controls = {} })

server:credential("drummer-demo-token", "drummer")
server:credential("synth-demo-token", "synth")
server:credential("leader-demo-token", "leader")
server:credential("observer-demo-token", "observer")

local rhythm = performance:plane {
  id = "rhythm", label = "Rhythm", roles = { "drummer", "leader" },
  read_only_roles = { "observer" },
}
rhythm:step_sequencer {
  id = "kick_pattern", label = "Kick", steps = 16,
  track = kick_track, pattern = main,
}

local fill = false
rhythm:trigger {
  id = "kick_fill", label = "FILL",
  triggered = function()
    fill = not fill
    if fill then
      main:steps("kick", { 1,0,0,1, 1,0,1,0, 1,0,0,1, 1,1,1,1 })
    else
      main:steps("kick", { 1,0,0,0, 1,0,0,0, 1,0,0,0, 1,0,0,0 })
    end
  end,
}

local synthesis = performance:plane {
  id = "synthesis", label = "Synthesizer", roles = { "synth", "leader" },
  read_only_roles = { "observer" },
}
synthesis:fader {
  id = "filter_cutoff", label = "Cutoff", target = "filter_cutoff",
  min = 80.0, max = 8000.0, scale = "log", unit = "Hz",
}
synthesis:knob {
  id = "filter_resonance", label = "Resonance", target = "filter_resonance",
  min = 0.0, max = 0.95,
}

return instrument
