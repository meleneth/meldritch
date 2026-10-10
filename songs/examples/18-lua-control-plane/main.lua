-- Version 1: compose existing validated song data; keep behavior reusable.
local mode = {
  id = "mode", dependencies = {"mode"},
  interact = function(state)
    return {name = "set_mode", mode = state.mode == "performance"
      and "all_parameters" or "performance"}
  end,
  present = function(state, surface, page)
    return {label = page .. ": " .. state.mode,
      color = state.mode == "performance" and "green" or "amber",
      style = surface == "stream_deck_xl" and "lcd"
        or surface == "apc_mini_mk2" and "led" or "text"}
  end,
}
local function bindings(extra)
  local result = {
    {surface = "apc", cell = 0, widget = "mode"},
    {surface = "deck", cell = 0, widget = "mode"},
    {surface = "terminal", cell = 0, widget = "mode"},
    {surface = "deck", cell = 31, event = "next"},
    {surface = "deck", cell = 30, event = "back"},
  }
  if extra then table.insert(result, {surface = "deck", cell = 1, widget = "mode"}) end
  return result
end
return {
  version = 1, id = "lua-control-plane", song = ".", initial = "home",
  surfaces = {
    {id = "apc", kind = "apc_mini_mk2"},
    {id = "deck", kind = "stream_deck_xl"},
    {id = "terminal", kind = "tui"},
  },
  widgets = {mode},
  states = {
    {id = "online", transitions = {"home", "detail"},
      handle = function(state, event)
        if event.name == "back" then return {transition = "home"} end
      end,
      entry = function() return "online entered" end,
      exit = function() return "online exited" end},
    {id = "home", parent = "online", transitions = {"detail"},
      bindings = bindings(true),
      handle = function(state, event)
        if event.name == "next" then
          if state.mode ~= "all_parameters" then return {reject = "inspect mode required"} end
          return {transition = "detail"}
        end
      end,
      entry = function() return "home entered" end,
      exit = function() return "home exited" end},
    {id = "detail", parent = "online", transitions = {}, bindings = bindings(),
      entry = function() return "detail entered" end,
      exit = function() return "detail exited" end},
  },
}
