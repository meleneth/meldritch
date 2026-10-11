return {version = 1, id = "unknown-audio-dependency", song = ".", initial = "perform",
  surfaces = {{id = "apc-mini", kind = "apc_mini_mk2"}},
  widgets = {{id = "bad", dependencies = {"sound"},
    interact = function() return {name = "play"} end,
    present = function() return {label = "bad", color = "red", style = "led"} end}},
  states = {{id = "perform", transitions = {}, bindings = {{surface = "apc-mini", cell = 0, widget = "bad"}}}},
}
