-- Session-mode grid notes run left to right, bottom row first (0..63).
-- Colors are Akai's fixed palette: 5 red, 9 orange, 13 yellow,
-- 21 green, 37 cyan, 45 blue, 49 purple, 53 pink. Brightness 0..6.
local palette = {5, 9, 13, 21, 37, 45, 49, 53}
local frames = {}
for step = 0, 7 do
  local pads = {}
  for cell = 0, 63 do
    pads[#pads + 1] = {
      cell = cell,
      color = palette[(cell + step) % #palette + 1],
      brightness = 2, -- 50% brightness
    }
  end
  local buttons = {}
  for column = 0, 7 do
    buttons[#buttons + 1] = {note = 100 + column, mode = column == step and "on" or "off"}
    buttons[#buttons + 1] = {note = 112 + column, mode = column == step and "on" or "off"}
  end
  frames[#frames + 1] = {hold_ms = 180, pads = pads, buttons = buttons}
end

-- Leave a useful map for example 19: four variation rows in lane colors,
-- mute row red, solo row yellow. These are static hints, not live state.
local pads = {}
local lane_colors = {9, 21, 45, 49}
for cell = 0, 63 do
  local row, column = cell // 8, cell % 8
  local color = 0
  if column < 4 then
    if row < 4 then color = lane_colors[column + 1]
    elseif row == 4 then color = 5
    elseif row == 5 then color = 13 end
  end
  pads[#pads + 1] = {cell = cell, color = color, brightness = 2}
end
local buttons = {}
for column = 0, 7 do
  buttons[#buttons + 1] = {note = 100 + column, mode = column < 4 and "on" or "off"}
  buttons[#buttons + 1] = {note = 112 + column, mode = column < 4 and "on" or "off"}
end
frames[#frames + 1] = {hold_ms = 0, pads = pads, buttons = buttons}
return {version = 1, frames = frames}
