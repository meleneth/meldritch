-- Reusable Lua-owned musical model. Mutable event data lives in the native
-- prepared sequencer reached through these stable track and pattern IDs.
local M = {}

function M.project(instrument, spec)
  assert(type(spec) == "table" and type(spec.id) == "string", "project requires a stable id")
  local sequence = instrument:sequencer {
    bpm = assert(spec.bpm, "project requires bpm"),
    steps = spec.max_steps or 64,
    steps_per_beat = spec.steps_per_beat or 4,
  }
  local project = {
    id = spec.id, instrument = instrument, sequence = sequence,
    tracks = {}, track_order = {}, patterns = {}, pattern_order = {},
    persistence = spec.persistence,
  }

  function project:track(track)
    assert(type(track) == "table" and type(track.id) == "string" and track.id ~= "",
      "track requires a stable id")
    assert(not self.tracks[track.id], "duplicate track: " .. track.id)
    track.kind = track.kind or "melodic"
    assert(track.kind == "drum" or track.kind == "melodic", "track kind must be drum or melodic")
    track.signal = sequence:track(track.id)
    self.tracks[track.id] = track
    self.track_order[#self.track_order + 1] = track
    return track
  end

  function project:pattern(id)
    assert(type(id) == "string" and id ~= "", "pattern requires a stable id")
    local existing = self.patterns[id]
    if existing then return existing end
    local native = sequence:pattern(id)
    local pattern = { id = id, native = native, project = self }
    function pattern:triggers(track_id, velocities)
      assert(self.project.tracks[track_id], "unknown track: " .. tostring(track_id))
      self.native:steps(track_id, velocities)
      return self
    end
    function pattern:notes(track_id, length, events)
      assert(self.project.tracks[track_id], "unknown track: " .. tostring(track_id))
      self.native:notes(track_id, events, length)
      return self
    end
    function pattern:length(track_id, length)
      self.native:length(track_id, length)
      return self
    end
    self.patterns[id] = pattern
    self.pattern_order[#self.pattern_order + 1] = pattern
    return pattern
  end

  function project:view_pattern(id) sequence:view_pattern(id) end
  function project:play_pattern(id) sequence:switch_pattern(id) end
  function project:duplicate_pattern(from, to)
    assert(not self.patterns[to], "duplicate pattern: " .. tostring(to))
    local native = sequence:duplicate_pattern(from, to)
    local pattern = { id = to, native = native, project = self }
    function pattern:triggers(track_id, velocities)
      assert(self.project.tracks[track_id], "unknown track: " .. tostring(track_id));self.native:steps(track_id, velocities);return self
    end
    function pattern:notes(track_id, length, events)
      assert(self.project.tracks[track_id], "unknown track: " .. tostring(track_id));self.native:notes(track_id, events, length);return self
    end
    function pattern:length(track_id, length) self.native:length(track_id, length);return self end
    self.patterns[to] = pattern
    self.pattern_order[#self.pattern_order + 1] = pattern
    return pattern
  end
  function project:edit_pattern(id, edit)
    assert(type(edit) == "function", "edit_pattern requires a function")
    edit(assert(self.patterns[id], "missing edit target pattern: " .. tostring(id)))
  end
  function project:arrangement(sections)
    local compiled = {}
    for index, section in ipairs(sections) do
      assert(type(section) == "table" and (section.pattern or section.patterns),
        "arrangement section requires pattern or per-track patterns")
      local pattern = section.pattern
      if section.patterns then
        local id = "__section_" .. (section.name or tostring(index))
        sequence:compose_pattern(id, section.patterns)
        pattern = id
      end
      compiled[#compiled + 1] = { pattern = pattern, repeats = section.repeats or 1 }
    end
    sequence:arrangement(compiled)
  end
  function project:save(path) sequence:save_project(path or assert(self.persistence, "no persistence path")) end
  function project:save_as(path) assert(type(path) == "string" and path ~= ""); sequence:save_project(path) end
  function project:reload(path) sequence:load_project(path or assert(self.persistence, "no persistence path")) end
  function project:command(command) sequence:command(command) end
  return project
end

return M
