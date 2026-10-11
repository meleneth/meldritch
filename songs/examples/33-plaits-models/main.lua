-- Controller policy declares a navigation order; Rust validates the typed model.
-- Cell 0 cycles the source model while the instrument editor observes the same source.
local models = {"fm", "harmonic", "virtual_analog", "waveshaping", "grain", "wavetable", "chord", "speech", "swarm", "noise", "particle", "string", "modal", "bass_drum", "snare_drum", "hihat", "virtual_analog_vcf", "phase_distortion", "six_op_a", "six_op_b", "six_op_c", "wave_terrain", "string_machine", "chiptune"}
local function selected(state)
  for _, parameter in ipairs(state.instrument.parameters) do
    if parameter.id.module == "osc" and parameter.id.parameter == "model" then
      return parameter.value
    end
  end
end
return {version = 1, id = "instrument-editor", song = ".", initial = "edit",
  surfaces = {{id = "apc-mini", kind = "apc_mini_mk2"}, {id = "terminal", kind = "tui"}},
  widgets = {{id = "model", dependencies = {"instrument", "audio"},
    interact = function(state)
      local current = selected(state)
      for index, model in ipairs(models) do
        if current == model then
          return {name = "edit_instrument", edit = {
            target = {synth = "edited-bass", module = "osc", parameter = "model"},
            value = models[index % #models + 1]}}
        end
      end
      error("selected model missing from authored navigation order")
    end,
    present = function(state, surface)
      return {label = "Model " .. tostring(selected(state)),
        color = state.audio and state.audio.pending and "amber" or "blue",
        style = surface == "apc_mini_mk2" and "led" or "text"}
    end}},
  states = {{id = "edit", transitions = {}, bindings = {
    {surface = "apc-mini", cell = 0, widget = "model"},
    {surface = "terminal", cell = 0, widget = "model"},
  }}},
}
