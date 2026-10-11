-- Drum interaction policy. Rust owns note/sample-slot identity and playback.
local page, down = 0, {}
local slots = {"kick", "snare", "clap", "closed-hat", "open-hat", "low-tom", "high-tom", "ride"}
local pitches = {36, 38, 39, 42, 46, 45, 50, 51}
local function hit(pattern, slot, tick)
  local found
  for _, note in ipairs(pattern.notes) do
    if note.slot == slot and note.start_tick == tick and (not found or note.id < found.id) then found = note end
  end
  return found
end
local function interact(view, event)
  if event.kind == "cancel" then down = {}; return end
  local key = event.source .. ":" .. event.cell
  if event.kind == "release" then down[key] = nil; return end
  if down[key] then return end
  down[key] = true
  local cell, pattern = event.cell, view.pattern
  if cell == 116 then
    page = 0
    if pattern.id == "seed" then return {name = "create_note_pattern", track = "drums", id = "roll", length_ticks = 3840, looped = true} end
    return
  end
  if cell == 115 then return {name = "save_pattern"} end
  if cell == 106 then if page > 0 then page = page - 1 end; return end
  if cell == 107 then if (page + 1) * 1920 < pattern.length_ticks then page = page + 1 end; return end
  if cell >= 64 then return end
  local row, tick = cell // 8 + 1, (page * 8 + cell % 8) * 240
  local note = hit(pattern, slots[row], tick)
  if down[event.source .. ":113"] or down[event.source .. ":114"] then
    if note then
      local velocity = note.velocity + (down[event.source .. ":114"] and .1 or -.1)
      if velocity < 0 then velocity = 0 elseif velocity > 1 then velocity = 1 end
      return {name = "edit_note", pattern = pattern.id, edit = {operation = "velocity", id = note.id, velocity = velocity}}
    end
    return
  end
  if note then return {name = "edit_note", pattern = pattern.id, edit = {operation = "delete", id = note.id}} end
  if down[event.source .. ":112"] or tick >= pattern.length_ticks then return end
  return {name = "edit_note", pattern = pattern.id, edit = {operation = "insert_sample",
    slot = slots[row], pitch = pitches[row], start_tick = tick, duration_ticks = 240, velocity = .8}}
end
local function present(view, surface, state, cell)
  local style = surface == "apc_mini_mk2" and "led" or "text"
  if cell >= 64 then
    local labels = {[106] = "Time <", [107] = "Time >", [112] = "Hold DELETE", [113] = "Velocity -", [114] = "Velocity +", [115] = "Save Lua", [116] = "New drums"}
    return {label = labels[cell], color = "green", style = style}
  end
  local slot, tick = slots[cell // 8 + 1], (page * 8 + cell % 8) * 240
  local note = hit(view.pattern, slot, tick)
  local color = note and (note.id == view.pattern.selected_note and "yellow" or "green") or "off"
  if view.playing and view.pattern.playhead_tick // 240 == tick // 240 then color = note and "white" or "gray" end
  return {label = slot .. " t" .. tick .. (note and " " .. note.id or " empty"), color = color, style = style}
end
local bindings = {}
for _, surface in ipairs({"apc-mini", "terminal"}) do
  for cell = 0, 63 do bindings[#bindings + 1] = {surface = surface, cell = cell, widget = "drums"} end
  for _, cell in ipairs({106,107,112,113,114,115,116}) do bindings[#bindings + 1] = {surface = surface, cell = cell, widget = "drums"} end
end
return {version = 1, id = "drum-editor", song = ".", initial = "edit",
  surfaces = {{id = "apc-mini", kind = "apc_mini_mk2"}, {id = "terminal", kind = "tui"}},
  widgets = {{id = "drums", events = true, dependencies = {"pattern", "transport", "audio"}, interact = interact, present = present}},
  states = {{id = "edit", transitions = {}, bindings = bindings}},
}
