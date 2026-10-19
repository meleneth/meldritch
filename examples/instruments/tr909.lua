local drums = meldritch.instrument("TR-909")

-- Audio intentionally follows the system default (JBL or headphones). The
-- controller is selected by its stable ALSA name, not a command-line index.
drums:devices {
  apc_mini = "APC mini mk2:APC mini mk2 Control",
}

local kit = meldritch.samples {
  source = "strudel",
  bank = "RolandTR909",
  cache = true,
}

local sequence = drums:sequencer {
  bpm = 128,
  steps = 16,
  steps_per_beat = 4,
}

local mixer = drums:mixer {
  inputs = 8,
}

local tracks = {
  { name = "Kick",   sample = "bd", gain = 0.90 },
  { name = "Snare",  sample = "sd", gain = 0.75 },
  { name = "Clap",   sample = "cp", gain = 0.65 },
  { name = "CHat",   sample = "hh", gain = 0.45 },
  { name = "OHat",   sample = "oh", gain = 0.45 },
  { name = "LowTom", sample = "lt", gain = 0.65 },
  { name = "HiTom",  sample = "ht", gain = 0.65 },
  { name = "Ride",   sample = "rd", gain = 0.50 },
}

for index, track in ipairs(tracks) do
  local trigger = sequence:track(track.name)
  local player = drums:sample_player {
    sample = kit:load(track.sample, 0),
    gain = track.gain,
  }
  drums:connect(trigger.out, player.trigger)
  drums:connect(player.out, mixer.input[index])
end

drums:connect(mixer.left, drums.output.left)
drums:connect(mixer.right, drums.output.right)

sequence:steps("Kick",  { 1,0,0,0, 1,0,0,0, 1,0,0,0, 1,0,0,0 })
sequence:steps("Snare", { 0,0,0,0, 1,0,0,0, 0,0,0,0, 1,0,0,0 })
sequence:steps("CHat",  { 1,0,1,0, 1,0,1,0, 1,0,1,0, 1,0,1,0 })

drums:control("Tempo", sequence.bpm, { min = 60, max = 220 })
drums:action("Play", function() sequence:start() end)
drums:action("Stop", function() sequence:stop() end)
drums:action("Restart", function() sequence:restart() end)
drums:performance { sequencer = sequence, layout = "step_grid" }

return drums
