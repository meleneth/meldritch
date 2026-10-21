-- Launch a deliberately saved performance without modifying the shipped patterns.
local build=meldritch.include("groovebox_core.lua")
return build(false,"groovebox-project.json")
