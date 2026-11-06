local graph = meldritch.graph()

local bass = graph:module("bass", "sine", { frequency = 55.0 })
local bass_gain = graph:module("bass_gain", "gain", { gain = 0.18 })
local drop = graph:module("drop", "square", { frequency = 110.0 })
local drop_gain = graph:module("drop_gain", "gain", { gain = 0.0 })
local mix = graph:module("mix", "mix")
local output = graph:module("output", "output")

graph:connect(bass.out, bass_gain.input)
graph:connect(drop.out, drop_gain.input)
graph:connect(bass_gain.out, mix.left)
graph:connect(drop_gain.out, mix.right)
graph:connect(mix.out, output.input)

graph:control("bass_level", bass_gain, "gain", { min = 0.0, max = 0.4 })
graph:control("drop_layer", drop_gain, "gain", { min = 0.0, max = 0.35 })

local server = meldritch.server {
  host = "127.0.0.1",
  port = 8080,
  anonymous_role = "conductor",
}

server:get("/api/status", function(request)
  return { status = "running", request_id = request.id }
end)

server:websocket("/performance", {
  connected = function(client)
    print("browser connected", client.id)
  end,
  disconnected = function(client)
    print("browser disconnected", client.id)
  end,
})

local performance = server:performance { name = "The Ridiculous Red Button" }
performance:control("bass_level", {
  type = "continuous",
  label = "Bass Level",
  group = "Mix",
  target = "bass_level",
  min = 0.0,
  max = 0.4,
})
performance:control("DROP", {
  type = "trigger",
  label = "DROP",
  group = "Conductor",
  target = "drop_layer",
  min = 0.0,
  max = 0.35,
})
performance:role("conductor", { controls = "*" })

return graph
