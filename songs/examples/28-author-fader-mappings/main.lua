-- The physical addresses and page meanings belong to this Lua composition.
local assignments = {
  pages = {"main", "tone"},
  targets = {
    {id="cutoff",label="Filter cutoff",target={kind="synth",id="bass",module="filter",parameter="cutoff_hz"},minimum=80,maximum=6000,step=10},
    {id="resonance",label="Filter resonance",target={kind="synth",id="bass",module="filter",parameter="resonance"},minimum=0,maximum=1,step=.01},
    {id="volume",label="Track volume",target={kind="track",id="bass",parameter="volume"},minimum=0,maximum=1,step=.01},
    {id="pan",label="Track pan",target={kind="track",id="bass",parameter="pan"},minimum=-1,maximum=1,step=.01},
    {id="delay-mix",label="Delay mix",target={kind="dsp",id="echo",module="delay",parameter="mix"},minimum=0,maximum=1,step=.01},
    {id="delay-feedback",label="Delay feedback",target={kind="dsp",id="echo",module="delay",parameter="feedback"},minimum=0,maximum=.99,step=.01},
  },
  faders = {},
}
for _,page in ipairs(assignments.pages) do
  local defaults = page == "main" and {"cutoff","volume","pan","resonance","delay-mix"} or {"resonance","cutoff","delay-mix","volume","pan"}
  for i=1,9 do assignments.faders[#assignments.faders+1]={id=page.."-"..i,device="apc-mini",channel=1,cc=47+i,page=page,target=defaults[i]} end
end
local function current(view,index)
  local selected = 0
  for _,f in ipairs(view.assignments.definition.faders) do
    if f.page == view.assignments.page then selected=selected+1; if selected==index then return f end end
  end
end
local widgets,bindings = {},{}
for i=1,9 do
  local index=i
  local function present(view,surface)
    local f=current(view,index)
    local observed=view.assignments.observations[f.id]
    local value=view.controls["assignment:"..f.id]
    return {label=f.id.." -> "..(type(f.target)=="string" and f.target or "unassigned").." = "..tostring(value)..
      " raw "..tostring(observed and observed.raw).." "..(observed and observed.acquired and "acquired" or "pickup pending"),
      color=observed and observed.acquired and "green" or "yellow",style=surface=="apc_mini_mk2" and "led" or "text"}
  end
  widgets[#widgets+1]={id="assign-"..i,dependencies={"assignments","controls"},present=present,
    interact=function(view)
      local f=current(view,index);local targets=view.assignments.definition.targets;local next=1
      for j,t in ipairs(targets) do if t.id==f.target then next=j%#targets+1 end end
      return {name="assign_fader",id=f.id,target=targets[next].id}
    end}
  widgets[#widgets+1]={id="remove-"..i,dependencies={"assignments"},present=present,
    interact=function(view) return {name="assign_fader",id=current(view,index).id} end}
  for _,surface in ipairs({"apc-mini","terminal"}) do
    bindings[#bindings+1]={surface=surface,cell=i-1,widget="assign-"..i}
    bindings[#bindings+1]={surface=surface,cell=i+15,widget="remove-"..i}
  end
end
widgets[#widgets+1]={id="page",dependencies={"assignments"},present=function(view,surface) return {label="Page "..view.assignments.page,color="blue",style=surface=="apc_mini_mk2" and "led" or "text"} end,
  interact=function(view) return {name="assignment_page",id=view.assignments.page=="main" and "tone" or "main"} end}
widgets[#widgets+1]={id="save",dependencies={"assignments"},present=function(view,surface) return {label="Save controls Lua",color="green",style=surface=="apc_mini_mk2" and "led" or "text"} end,interact=function() return {name="save_assignments"} end}
for _,surface in ipairs({"apc-mini","terminal"}) do
  bindings[#bindings+1]={surface=surface,cell=106,widget="page"}
  bindings[#bindings+1]={surface=surface,cell=115,widget="save"}
end
return {version=1,id="control-authoring",song=".",initial="edit",assignments=assignments,
  surfaces={{id="apc-mini",kind="apc_mini_mk2"},{id="terminal",kind="tui"}},
  widgets=widgets,states={{id="edit",transitions={},bindings=bindings}}}
