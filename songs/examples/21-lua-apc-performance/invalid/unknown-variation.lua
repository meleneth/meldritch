return {
  version = 1, id = "invalid-reference", song = ".", initial = "perform",
  surfaces = {{id = "apc-mini", kind = "apc_mini_mk2"}},
  widgets = {{id = "launch", dependencies = {"lanes"},
    interact = function() return {name = "select_lane_variation", lane = "beat-drum", variation = "missing"} end,
    present = function() return {label = "invalid reference", color = "red", style = "led"} end}},
  states = {{id = "perform", transitions = {}, bindings = {{surface = "apc-mini", cell = 0, widget = "launch"}}}},
}
