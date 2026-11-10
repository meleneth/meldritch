-- Rebuild the deterministic generator, then apply deliberate interactive edits.
local build=meldritch.include("generative_core.lua")
return build("generative-edits.lua")
