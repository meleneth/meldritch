return {
  version = 1, id = "cycle", song = ".", initial = "home",
  surfaces = {{id = "deck", kind = "stream_deck_xl"}}, widgets = {},
  states = {{id = "home", parent = "parent", transitions = {}},
    {id = "parent", parent = "home", transitions = {}}},
}
