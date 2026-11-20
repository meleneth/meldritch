-- ValueSequencer is a Board assembled exclusively from reusable primitives:
-- counter, constants (including all eight independently adjustable slots),
-- switches, and authored normalizations. No dedicated sequencer processor.
local values = {0.0, 0.25, 0.5, 0.75, 1.0, 0.75, 0.5, 0.25}
local board = meldritch.subpatch("value_sequencer")
local counter = board:module("counter", "counter")
local length = board:module("length", "constant", { value = #values })
local looped = board:module("looped", "constant", { value = 1.0 })
board:normalize(length.out, counter.length)
board:normalize(looped.out, counter.looped)
board:input("advance", counter.advance)
board:input("reset", counter.reset)
board:input("length", counter.length)
board:input("looped", counter.looped)
board:parameter("length", length, "value")
board:parameter("looped", looped, "value")
board:output("position", counter.position)
board:output("index", counter.index)

local selected
for index, value in ipairs(values) do
  local slot = board:module("slot_" .. index, "constant", {value = value})
  board:parameter("slot_" .. index, slot, "value")
  if not selected then
    selected = slot.out
  else
    local selector = board:module("select_" .. index, "switch", {threshold = index - 1.5})
    board:connect(selected, selector.left)
    board:connect(slot.out, selector.right)
    board:connect(counter.index, selector.select)
    selected = selector.out
  end
end
board:output("value", selected)

local graph = meldritch.graph()
local sequence = graph:instance("sequence", board)
local clock = graph:module("clock", "square", {frequency = 4.0})
local output = graph:module("output", "output")
graph:connect(clock.out, sequence.advance)
graph:connect(sequence.value, output.input)

-- Each slider manipulates a normal component. The Board itself has no fader
-- implementation or special-case sequencer parameter processing.
for i = 1, #values do
  graph:control("step_" .. i, sequence, "slot_" .. i,
    {min = 0.0, max = 1.0, label = "Step " .. i})
end
graph:control("length", sequence, "length",
  {min = 1.0, max = #values, label = "Active slots"})
graph:control("looped", sequence, "looped",
  {min = 0.0, max = 1.0, label = "Loop"})
return graph
