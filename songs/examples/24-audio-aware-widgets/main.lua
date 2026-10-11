local function pattern(id)
  return {id = "lead-" .. id, dependencies = {"lanes", "transport", "audio"},
    interact = function() return {name = "select_lane_variation", lane = "lead", variation = id} end,
    present = function(state, surface)
      local lane, audio = state.lanes.lead, state.audio
      local status, color = "unavailable", "gray"
      local actual = audio and audio.lanes.lead
      if actual then
        local selected = lane.variation == id
        if selected and audio.error then status, color = "error", "red"
        elseif selected and audio.pending then status, color = "queued", "amber"
        elseif state.playing and actual.playing_variation == id then status, color = "playing", "green"
        elseif actual.published_variation == id then status, color = "ready", "blue"
        elseif lane.audible then status, color = "idle", "gray" end
      end
      return {label = id .. " " .. status, color = color,
        style = surface == "apc_mini_mk2" and "led" or surface == "stream_deck_xl" and "lcd" or "text"}
    end}
end
return {version = 1, id = "audio-aware-widgets", song = ".", initial = "perform",
  surfaces = {{id = "apc-mini", kind = "apc_mini_mk2"},
    {id = "deck", kind = "stream_deck_xl"}, {id = "terminal", kind = "tui"}},
  widgets = {pattern("a"), pattern("b")},
  states = {{id = "online", transitions = {}},
    {id = "perform", parent = "online", transitions = {}, bindings = {
      {surface = "apc-mini", cell = 0, widget = "lead-a"},
      {surface = "apc-mini", cell = 8, widget = "lead-b"},
      {surface = "deck", cell = 0, widget = "lead-a"},
      {surface = "deck", cell = 1, widget = "lead-b"},
      {surface = "terminal", cell = 0, widget = "lead-a"},
      {surface = "terminal", cell = 1, widget = "lead-b"},
    }}},
}
