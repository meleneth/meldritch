local function level(id, value)
  return {id = id, dependencies = {"controls"},
    interact = function() return {name = "set_control", id = id, value = value} end,
    present = function(state, surface)
      return {label = id .. " " .. tostring(state.controls[id]), color = "blue",
        style = surface == "apc_mini_mk2" and "led" or "lcd"}
    end}
end
return {
  version = 1, id = "midi-soft-takeover", song = ".", initial = "perform",
  surfaces = {{id = "apc-mini", kind = "apc_mini_mk2"}, {id = "deck", kind = "stream_deck_xl"}},
  widgets = {level("a-level", 0.5), level("b-level", 0.75)},
  states = {{id = "perform", transitions = {}, bindings = {
    {surface = "apc-mini", cell = 0, widget = "a-level"},
    {surface = "apc-mini", cell = 1, widget = "b-level"},
    {surface = "deck", cell = 0, widget = "a-level"},
    {surface = "deck", cell = 1, widget = "b-level"},
  }}},
}
