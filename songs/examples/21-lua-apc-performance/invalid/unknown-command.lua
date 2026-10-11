return {
  version = 1, id = "invalid-command", song = ".", initial = "perform",
  surfaces = {{id = "apc-mini", kind = "apc_mini_mk2"}},
  widgets = {{id = "launch", dependencies = {"lanes"},
    interact = function() return {name = "invented_launch_command"} end,
    present = function() return {label = "invalid command", color = "red", style = "led"} end}},
  states = {{id = "perform", transitions = {}, bindings = {{surface = "apc-mini", cell = 0, widget = "launch"}}}},
}
