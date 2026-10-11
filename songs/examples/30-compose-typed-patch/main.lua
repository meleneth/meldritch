-- One bounded atomic batch; Lua declares the musical graph using typed operations.
local operations = {
  {operation="remove", id="osc", cables="disconnect"},
  {operation="remove", id="env", cables="disconnect"},
  {operation="remove", id="amp", cables="disconnect"},
  {operation="remove", id="filter", cables="disconnect"},
  {operation="remove", id="out", cables="disconnect"},
  {operation="instantiate", id="source", kind="oscillator"},
  {operation="instantiate", id="contour", kind="adsr"},
  {operation="instantiate", id="gain", kind="vca"},
  {operation="instantiate", id="tone", kind="low_pass"},
  {operation="instantiate", id="out", kind="audio_output"},
  {operation="set_parameter", module="source", parameter="waveform", value="saw"},
  {operation="set_parameter", module="contour", parameter="attack", value=0.01},
  {operation="set_parameter", module="contour", parameter="release", value=0.2},
  {operation="set_parameter", module="tone", parameter="cutoff_hz", value=800.0},
  {operation="set_parameter", module="tone", parameter="resonance", value=0.2},
  {operation="connect", from="input.pitch", to="source.pitch"},
  {operation="connect", from="input.gate", to="contour.gate"},
  {operation="connect", from="source.audio", to="gain.audio"},
  {operation="connect", from="contour.control", to="gain.level"},
  {operation="connect", from="gain.audio", to="tone.audio"},
  {operation="connect", from="tone.audio", to="out.audio"},
}
return {version=1, id="compose-patch", song=".", initial="edit",
  surfaces={{id="terminal", kind="tui"}, {id="apc-mini", kind="apc_mini_mk2"}},
  widgets={{id="compose", dependencies={"instrument", "audio"},
    interact=function() return {name="edit_patch", edit={synth="edited-bass", operations=operations}} end,
    present=function(view, surface)
      return {label="Compose " .. #view.instrument.graph.modules .. " modules",
        color=view.audio and view.audio.pending and "amber" or "blue",
        style=surface=="apc_mini_mk2" and "led" or "text"}
    end}},
  states={{id="edit", transitions={}, bindings={
    {surface="terminal", cell=0, widget="compose"},
    {surface="apc-mini", cell=0, widget="compose"},
  }}},
}
