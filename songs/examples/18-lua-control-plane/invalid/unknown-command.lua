return {
  version = 1, id = "unknown-command", song = ".", initial = "home",
  surfaces = {{id = "deck", kind = "stream_deck_xl"}},
  widgets = {{id = "bad", dependencies = {"mode"},
    interact = function() return {name = "explode"} end,
    present = function() return {label = "bad", color = "red", style = "lcd"} end}},
  states = {{id = "home", transitions = {},
    bindings = {{surface = "deck", cell = 0, widget = "bad"}}}},
}
