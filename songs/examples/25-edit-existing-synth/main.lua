-- Same authoritative source edit from APC, LCD or logical TUI cells.
local function release_widget()
  return {id = "release", dependencies = {"instrument", "audio"},
    interact = function(state)
      return {name = "edit_instrument", edit = {
        target = {synth = "edited-bass", module = "env", parameter = "release"},
        value = 0.25}}
    end,
    present = function(state, surface)
      local value = "unknown"
      for _, parameter in ipairs(state.instrument.parameters) do
        if parameter.id.module == "env" and parameter.id.parameter == "release" then
          value = tostring(parameter.value)
        end
      end
      return {label = "Release " .. value,
        color = state.audio and state.audio.pending and "amber" or "blue",
        style = surface == "apc_mini_mk2" and "led" or surface == "stream_deck_xl" and "lcd" or "text"}
    end}
end
return {version = 1, id = "instrument-editor", song = ".", initial = "edit",
  surfaces = {{id = "apc-mini", kind = "apc_mini_mk2"},
    {id = "deck", kind = "stream_deck_xl"}, {id = "terminal", kind = "tui"}},
  widgets = {release_widget()},
  states = {{id = "edit", transitions = {}, bindings = {
    {surface = "apc-mini", cell = 0, widget = "release"},
    {surface = "deck", cell = 0, widget = "release"},
    {surface = "terminal", cell = 0, widget = "release"},
  }}},
}
