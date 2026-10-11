return {
  version = 1, id = "invalid-color", song = ".", initial = "perform",
  surfaces = {{id = "apc-mini", kind = "apc_mini_mk2"}},
  widgets = {{id = "launch", dependencies = {"lanes"},
    interact = function() return {name = "toggle_lane_mute", lane = "beat-drum"} end,
    present = function() return {label = "invalid color", color = "puce", style = "led"} end}},
  states = {{id = "perform", transitions = {}, bindings = {{surface = "apc-mini", cell = 0, widget = "launch"}}}},
}
