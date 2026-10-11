-- Controller policy only. Rust owns notes, validation, identity and publication.
local page, low_pitch = 0, 48 -- C3; APC note 0 is bottom-left in Session mode.
local down, held, blocked = {}, nil, nil
local modifiers = { [112] = "delete", [113] = "velocity-down", [114] = "velocity-up" }
local names = { "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B" }
local function pitch_name(pitch) return names[pitch % 12 + 1] .. (pitch // 12 - 1) end
local function find(pattern, pitch, tick, starts_only)
  local found
  for _, n in ipairs(pattern.notes) do
    if n.note == pitch_name(pitch) and (n.start_tick == tick or
        (not starts_only and n.start_tick < tick and tick < n.start_tick + n.duration_ticks)) then
      if not found or n.id < found.id then found = n end
    end
  end
  return found
end
local function valid_hold(pattern)
  if not held then return end
  if held.pattern ~= pattern.id or pattern.selected_note ~= held.id then blocked, held = held, nil; return end
  local exists = false
  for _, n in ipairs(pattern.notes) do if n.id == held.id then exists = true end end
  if not exists then blocked, held = held, nil end
end
local function edit(pattern, operation, id, field, value)
  local change = { operation = operation, id = id }
  if field then change[field] = value end
  return { name = "edit_note", pattern = pattern.id, edit = change }
end
local function interact(view, event)
  if event.kind == "cancel" then down, held, blocked = {}, nil, nil; return end
  local key = event.source .. ":" .. event.cell
  if event.kind == "release" then
    down[key] = nil
    valid_hold(view.pattern)
    if held and held.source == event.source and held.cell == event.cell then
      local finished = held
      held = nil
      if finished.toggle then return edit(view.pattern, "delete", finished.id) end
    end
    if blocked and blocked.source == event.source and blocked.cell == event.cell then blocked = nil end
    return -- only a completed tap on an existing start deletes
  end
  if down[key] then return end -- repeated note-on is not another press
  down[key] = true
  local cell, pattern = event.cell, view.pattern
  valid_hold(pattern)
  if cell == 116 then
    held, page = nil, 0
    if pattern.id == "seed" then
      return { name = "create_note_pattern", track = "bass", id = "roll", length_ticks = 7680, looped = true }
    end
    return -- NEW creates the first pattern; never wipes the current definition
  end
  if cell == 115 then held = nil; return { name = "save_pattern" } end
  if cell == 106 then if page > 0 then page = page - 1 end; return end
  if cell == 107 then if (page + 1) * 3840 < pattern.length_ticks then page = page + 1 end; return end
  if cell == 104 then if held then blocked, held = held, nil end; if low_pitch >= 8 then low_pitch = low_pitch - 8 end; return end
  if cell == 105 then if held then blocked, held = held, nil end; if low_pitch <= 112 then low_pitch = low_pitch + 8 end; return end
  if modifiers[cell] then held = nil; return end
  if cell >= 64 then return end
  if blocked then return end -- interrupted starts must be released before another gesture
  local pitch, tick = low_pitch + cell // 8, (page * 8 + cell % 8) * 480
  local start = find(pattern, pitch, tick, true)
  local note = start or find(pattern, pitch, tick, false)
  if down[event.source .. ":112"] then
    held = nil
    if note then return edit(pattern, "delete", note.id) end
    return
  end
  if down[event.source .. ":113"] or down[event.source .. ":114"] then
    held = nil
    if note then
      local value = note.velocity + (down[event.source .. ":114"] and 0.1 or -0.1)
      if value < 0 then value = 0 elseif value > 1 then value = 1 end
      return edit(pattern, "velocity", note.id, "velocity", value)
    end
    return
  end
  if held then
    held.toggle = false -- a second pad consumes the tap, including invalid endpoints
    -- Invalid endpoints are consumed, never interpreted as note creation.
    if held.source ~= event.source or pitch ~= held.pitch or tick < held.tick then return end
    return edit(pattern, "duration", held.id, "duration_ticks", tick + 480 - held.tick)
  end
  if start then
    held = { id = start.id, pattern = pattern.id, source = event.source, cell = cell, pitch = pitch, tick = tick, toggle = true }
    return { name = "select_note", pattern = pattern.id, id = start.id }
  end
  if note then return { name = "select_note", pattern = pattern.id, id = note.id } end
  if tick >= pattern.length_ticks then return end
  held = { id = "note-" .. pattern.next_note_id, pattern = pattern.id, source = event.source, cell = cell, pitch = pitch, tick = tick }
  return { name = "edit_note", pattern = pattern.id,
    edit = { operation = "insert", start_tick = tick, duration_ticks = 480, pitch = pitch, velocity = 0.8 } }
end
local function present(view, surface, state, cell)
  local pattern = view.pattern
  valid_hold(pattern)
  if cell >= 64 then
    local labels = { [104] = "Pitch -8", [105] = "Pitch +8", [106] = "Time <", [107] = "Time >",
      [112] = "Hold DELETE", [113] = "Hold velocity -", [114] = "Hold velocity +", [115] = "Save Lua", [116] = "New pattern" }
    return { label = labels[cell], color = "green", style = surface == "apc_mini_mk2" and "led" or "text" }
  end
  local pitch, tick = low_pitch + cell // 8, (page * 8 + cell % 8) * 480
  local start = find(pattern, pitch, tick, true)
  local note = start or find(pattern, pitch, tick, false)
  local count = 0
  for _, n in ipairs(pattern.notes) do
    if n.note == pitch_name(pitch) and n.start_tick <= tick and tick < n.start_tick + n.duration_ticks then count = count + 1 end
  end
  local selected = note and note.id == pattern.selected_note
  local playing = view.playing and pattern.playhead_tick // 480 == tick // 480
  local color = count > 1 and "red" or start and "green" or note and "blue" or "off"
  if selected then color = "yellow" end
  if playing then color = note and "white" or "gray" end
  return { label = (start and "start " or note and "body " or "empty ") .. pitch_name(pitch) .. " t" .. tick ..
      (selected and " SELECT " or " ") .. (note and note.id or "") .. (count > 1 and " overlap" or "") .. (playing and " PLAY" or ""),
    color = color, style = surface == "apc_mini_mk2" and "led" or "text" }
end
local bindings = {}
for _, surface in ipairs({"apc-mini", "terminal"}) do
  for cell = 0, 63 do bindings[#bindings + 1] = { surface = surface, cell = cell, widget = "roll" } end
  for _, cell in ipairs({104,105,106,107,112,113,114,115,116}) do bindings[#bindings + 1] = { surface = surface, cell = cell, widget = "roll" } end
end
return { version = 1, id = "piano-roll", song = ".", initial = "edit",
  surfaces = {{id = "apc-mini", kind = "apc_mini_mk2"}, {id = "terminal", kind = "tui"}},
  widgets = {{id = "roll", events = true, dependencies = {"pattern", "transport", "audio"}, interact = interact, present = present}},
  states = {{id = "edit", transitions = {}, bindings = bindings}},
}
