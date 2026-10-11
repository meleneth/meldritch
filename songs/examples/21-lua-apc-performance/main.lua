-- Widgets derive musical state from Rust's immutable projection, never a local
-- copy. A second interface sees the same variation/mute/solo after each command.
local widgets, bindings = {}, {}
local function bind(cell, id)
  bindings[#bindings + 1] = {surface = "apc-mini", cell = cell, widget = id}
  -- LCD keys use contiguous addresses; transport/image encoding stays in Rust.
  bindings[#bindings + 1] = {surface = "deck", cell = #widgets - 1, widget = id}
  if #widgets <= 6 then
    bindings[#bindings + 1] = {surface = "terminal", cell = #widgets - 1, widget = id}
  end
end
local function widget(lane, variation, kind, cell)
  local id = lane .. "-" .. (variation or kind)
  widgets[#widgets + 1] = {
    id = id, dependencies = {"lanes", "transport"},
    interact = function(state)
      if kind == "launch" then
        return {name = "select_lane_variation", lane = lane, variation = lane .. "-" .. variation}
      end
      return {name = kind == "mute" and "toggle_lane_mute" or "toggle_lane_solo", lane = lane}
    end,
    present = function(state, surface)
      local value = state.lanes[lane]
      local color, label = "gray", lane .. " unavailable"
      if value then
        if kind == "launch" then
          local selected = value.variation == lane .. "-" .. variation
          label = lane .. " " .. variation .. (selected and " selected" or " idle")
          color = selected and (value.audible and (state.playing and "green" or "amber") or "red") or "blue"
        elseif kind == "mute" then
          label = lane .. (value.muted and " muted" or " unmuted")
          color = value.muted and "red" or "gray"
        else
          label = lane .. (value.soloed and " solo" or " solo off")
          color = value.soloed and "yellow" or "gray"
        end
      end
      return {label = label, color = color,
        style = surface == "apc_mini_mk2" and "led" or surface == "stream_deck_xl" and "lcd" or "text"}
    end,
  }
  bind(cell, id)
end
for column, lane in ipairs({"beat-drum", "rhythm-drum-a", "bass-a", "pad"}) do
  for row, variation in ipairs({"a", "b", "c", "d"}) do
    widget(lane, variation, "launch", (row - 1) * 8 + column - 1)
  end
  widget(lane, nil, "mute", 32 + column - 1)
  widget(lane, nil, "solo", 40 + column - 1)
end
return {
  version = 1, id = "lua-apc-performance", song = ".", initial = "perform",
  surfaces = {
    {id = "apc-mini", kind = "apc_mini_mk2"},
    {id = "deck", kind = "stream_deck_xl"},
    {id = "terminal", kind = "tui"},
  },
  widgets = widgets,
  states = {
    {id = "online", transitions = {}, entry = function() return "surface online" end,
      exit = function() return "surface offline" end},
    {id = "perform", parent = "online", transitions = {}, bindings = bindings,
      entry = function() return "performance entered" end,
      exit = function() return "performance exited" end},
  },
}
