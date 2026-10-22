-- Reusable author-side arpeggiator. It produces ordinary pitched pattern
-- events, so transport, polymetric wrapping, note-off scheduling,
-- arrangements, editing, and persistence remain owned by meldritch.musical.
local M = {}

local MODES = { up=true, down=true, up_down=true, played=true, random=true }

local function copy_note(value)
  if type(value) == "number" then return {note=value, velocity=1.0} end
  assert(type(value) == "table", "arpeggiator notes must be MIDI numbers or note tables")
  assert(type(value.note) == "number" or type(value.frequency) == "number",
    "arpeggiator note requires note or frequency")
  local note = {note=value.note, frequency=value.frequency, velocity=value.velocity or 1.0}
  assert(type(note.velocity) == "number" and note.velocity >= 0 and note.velocity <= 1,
    "arpeggiator note velocity must be in 0..1")
  return note
end

local function normalize_notes(values)
  assert(type(values) == "table", "note set must be a table")
  local result = {}
  for index,value in ipairs(values) do result[index] = copy_note(value) end
  return result
end

local function pitch(note)
  return note.note or 69 + 12 * math.log(note.frequency / 440, 2)
end

local function source(kind, resolve)
  return {_arp_source=true, kind=kind, resolve=resolve}
end

function M.note_set(notes)
  local captured = normalize_notes(notes)
  return source("note_set", function() return captured end)
end

-- `changes` is a pattern-like list of `{step=..., notes={...}}` chord events.
-- The last chord at or before a step remains active, wrapping at `length`.
function M.from_pattern(changes, length)
  assert(type(changes) == "table" and #changes > 0, "arp pattern requires chord events")
  local captured = {}
  for index,change in ipairs(changes) do
    assert(type(change.step) == "number" and change.step >= 1 and change.step % 1 == 0,
      "arp chord step must be a positive integer")
    captured[index] = {step=change.step, notes=normalize_notes(change.notes)}
  end
  table.sort(captured, function(a,b) return a.step < b.step end)
  assert(captured[1].step == 1, "arp chord pattern must establish a note set at step 1")
  length = length or captured[#captured].step
  assert(type(length) == "number" and length >= captured[#captured].step and length % 1 == 0,
    "arp chord-pattern length must contain every chord event")
  return source("pattern", function(step)
    local local_step = (step - 1) % length + 1
    local selected = captured[#captured].notes
    for _,change in ipairs(captured) do
      if change.step <= local_step then selected = change.notes else break end
    end
    return selected
  end)
end

-- Generative and control-side live adapters use the same note-set snapshot
-- representation. The producer runs while authoring/preparing a pattern,
-- never from the audio callback.
function M.producer(producer)
  assert(type(producer) == "function", "arp producer requires a function")
  return source("producer", function(step, context)
    return normalize_notes(producer(step, context))
  end)
end

function M.live(snapshot)
  assert(type(snapshot) == "function", "live arp source requires a snapshot function")
  return source("live_snapshot", function(step, context)
    return normalize_notes(snapshot(step, context))
  end)
end

local function note_key(notes)
  local fields = {}
  for _,note in ipairs(notes) do
    fields[#fields+1] = string.format("%.9g:%.9g:%.9g", note.note or -1,
      note.frequency or -1, note.velocity)
  end
  return table.concat(fields, "|")
end

local function expanded(notes, octaves, mode)
  local base = {}
  for index,note in ipairs(notes) do base[index] = note end
  if mode ~= "played" and mode ~= "random" then
    table.sort(base, function(a,b) return pitch(a) < pitch(b) end)
  end
  local result = {}
  for octave=0,octaves-1 do
    for _,note in ipairs(base) do
      result[#result+1] = {
        note=note.note and note.note + octave * 12 or nil,
        frequency=note.frequency and note.frequency * 2^octave or nil,
        velocity=note.velocity,
      }
    end
  end
  if mode == "down" then
    local reversed = {}; for index=#result,1,-1 do reversed[#reversed+1]=result[index] end
    return reversed
  end
  if mode == "up_down" and #result > 1 then
    for index=#result-1,2,-1 do result[#result+1]=result[index] end
  end
  return result
end

local function reset_state(self)
  self._cursor = 1
  self._random = self.seed
  self._source_key = nil
end

function M.new(spec)
  spec = spec or {}
  local arp = {
    mode=spec.mode or "up",
    octaves=spec.octaves or 1,
    clock_division=spec.clock_division or 1,
    gate=spec.gate or 0.72,
    velocity_shape=spec.velocity_shape,
    phase=spec.phase or "restart",
    restart_on_change=spec.restart_on_change ~= false,
    seed=spec.seed or 1,
  }
  assert(MODES[arp.mode], "arp mode must be up, down, up_down, played, or random")
  assert(type(arp.octaves)=="number" and arp.octaves>=1 and arp.octaves%1==0,
    "arp octaves must be a positive integer")
  assert(type(arp.clock_division)=="number" and arp.clock_division>=1 and arp.clock_division%1==0,
    "arp clock division must be a positive integer number of pattern steps")
  assert(arp.phase=="restart" or arp.phase=="free", "arp phase must be restart or free")
  assert(type(arp.seed)=="number" and arp.seed%1==0, "arp random seed must be an integer")
  reset_state(arp)

  function arp:restart() reset_state(self) end

  local function select_note(self, pool)
    if self.mode == "random" then
      self._random = (self._random * 1103515245 + 12345) % 2147483648
      return pool[self._random % #pool + 1]
    end
    local selected = pool[(self._cursor - 1) % #pool + 1]
    self._cursor = self._cursor + 1
    return selected
  end

  function arp:events(length, note_source, context)
    assert(type(length)=="number" and length>=1 and length<=64 and length%1==0,
      "arp pattern length must be 1..64")
    if not (type(note_source)=="table" and note_source._arp_source) then
      note_source = M.note_set(note_source)
    end
    if self.phase == "restart" then reset_state(self) end
    local events = {}
    local ordinal = 0
    for step=1,length,self.clock_division do
      local notes = normalize_notes(note_source.resolve(step, context))
      if #notes > 0 then
        local key = note_key(notes)
        if self._source_key and key ~= self._source_key and self.restart_on_change then
          self._cursor = 1
          if self.phase == "restart" then self._random = self.seed end
        end
        self._source_key = key
        local pool = expanded(notes, self.octaves, self.mode)
        local selected = select_note(self, pool)
        ordinal = ordinal + 1
        local velocity = selected.velocity
        if type(self.velocity_shape)=="number" then
          velocity = velocity * self.velocity_shape
        elseif type(self.velocity_shape)=="table" and #self.velocity_shape>0 then
          velocity = velocity * self.velocity_shape[(ordinal-1)%#self.velocity_shape+1]
        elseif type(self.velocity_shape)=="function" then
          velocity = self.velocity_shape(velocity, ordinal, step, selected, context)
        end
        local gate = type(self.gate)=="function" and self.gate(ordinal,step,selected,context) or self.gate
        assert(type(velocity)=="number" and velocity>=0 and velocity<=1,
          "shaped arp velocity must be in 0..1")
        assert(type(gate)=="number" and gate>=0 and gate<=1,
          "arp gate must be in 0..1")
        events[#events+1] = {step=step,note=selected.note,frequency=selected.frequency,
          velocity=velocity,gate=gate}
      end
    end
    return events
  end

  function arp:render(pattern, track_id, length, note_source, context)
    local events = self:events(length, note_source, context)
    pattern:notes(track_id, length, events)
    return events
  end

  return arp
end

return M
