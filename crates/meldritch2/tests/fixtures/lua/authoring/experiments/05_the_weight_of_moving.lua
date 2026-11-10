-- The Weight of Moving
-- 176 bars / 116 BPM / 6:04. Sample-driven groove composition.
--
-- Acoustic drums: AVL Drumkits by Glen MacArthur, CC BY-SA 3.0.
-- https://github.com/studiorack/avl-drumkits/tree/b06cb2c27359cbc68a830a82bddced05bd73d1a0
-- Performed rowing, typing, and breath textures: Oculus Audio Pack 01
-- by Oculus VR, LLC, CC BY 4.0.
-- https://github.com/facebookincubator/Meta-open-sound/tree/251c0be57d5d1b3242b91f53e0cde62ab1cb42d9/corpora/oculus-audio-pack
-- Required credit: "Oculus Audio Pack 01" by Oculus VR, LLC
-- (https://developers.meta.com/vr/downloads/package/oculus-audio-pack-1/).
-- OGG previews were transcoded from the original WAV files.
local instrument=meldritch.instrument("The Weight of Moving")
instrument:voice_mode("mono")
instrument:devices {midi_input="MicroLab mk3:MicroLab mk3",
  midi_controls={"MicroLab mk3:MicroLab mk3"},
  apc_mini="APC mini mk2:APC mini mk2 Control"}

local sequence=instrument:sequencer {bpm=116,steps=128,steps_per_beat=4}
local names={"Performed Break A","Performed Break B","Oar Chop A","Oar Chop B","Reverse Oar",
  "Kick","Snare","Ghost","Hats","Open Hat","Tambourine","Bass","Breath Hook",
  "Reverse Breath","Typing","Two Note"}
local track={};for _,name in ipairs(names) do track[name]=sequence:track(name) end

-- Fractions of a sixteenth note. These are stable relationships, not jitter.
sequence:groove("deep",{0,.05,.13,.03,-.05,.07,.15,.02,-.02,.06,.12,.01,-.06,.08,.16,.025})
sequence:groove("forward",{0,.025,.075,-.01,-.085,.035,.09,-.025,-.035,.025,.07,-.02,-.09,.035,.10,-.015})
sequence:groove("broken",{0,.08,-.04,.03,-.06,.10,-.03,.07,-.04,.09,-.05,.04,-.07,.11,-.02,.08})
sequence:groove("straight",{0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0})

local function ev(step,velocity,offset) return {step=step,velocity=velocity,offset=offset or 0} end
local function drum(track_name,name,length,groove,events)
  return track[track_name]:clip(name,{length=length,groove=groove,events=events})
end
local function notes(track_name,name,length,groove,events)
  return track[track_name]:clip(name,{length=length,groove=groove,notes=events})
end
local function repeat_events(length,period,source)
  local out={};for base=0,length-1,period do for _,event in ipairs(source) do
    if base+event.step<=length then out[#out+1]={step=base+event.step,velocity=event.velocity,offset=event.offset} end
  end end;return out
end

-- Every trigger plays a complete recorded oar-stroke performance. At .856x
-- and .883x the two recordings occupy two bars at 116 BPM; internal stroke,
-- splash, tail, and room relationships remain untouched.
local performed_a=drum("Performed Break A","Rowing performance A",32,"straight",{ev(1,1)})
local performed_b=drum("Performed Break B","Rowing performance B",32,"straight",{ev(1,1)})
local performed_sparse=drum("Performed Break A","Rowing omissions",64,"straight",{ev(1,1),ev(49,.86)})

-- Chops reorganize groups from the same performance. They never replace the
-- complete layer in the pocket; their syncopations answer its natural strokes.
local oar_deep=drum("Oar Chop A","Oar answer: deep",64,"deep",{
  ev(7,.38),ev(16,.52),ev(23,.32),ev(31,.58),ev(39,.34),ev(48,.5),ev(55,.3),ev(63,.62)})
local oar_forward=drum("Oar Chop A","Oar answer: forward",64,"forward",{
  ev(4,.34),ev(7,.48),ev(12,.4),ev(16,.58),ev(20,.36),ev(23,.54),ev(28,.42),ev(31,.68),
  ev(36,.38),ev(39,.56),ev(44,.4),ev(48,.62),ev(52,.42),ev(55,.58),ev(60,.5),ev(63,.76)})
local oar_broken=drum("Oar Chop A","Oar answer: broken",64,"broken",{
  ev(7,.42),ev(16,.58),ev(23,.38),ev(31,.64),ev(39,.44),ev(48,.62),ev(55,.4),ev(63,.7)})
local oar_low=drum("Oar Chop B","Oar body",64,"deep",{ev(11,.34),ev(27,.46),ev(43,.38),ev(59,.52)})
local oar_low_forward=drum("Oar Chop B","Oar body: push",64,"forward",{
  ev(11,.4),ev(15,.28),ev(27,.52),ev(31,.34),ev(43,.46),ev(47,.3),ev(59,.58),ev(63,.4)})
local reverse_sparse=drum("Reverse Oar","Reverse water",128,"broken",{ev(30,.42),ev(62,.5),ev(94,.46),ev(126,.62)})
local reverse_busy=drum("Reverse Oar","Reverse water: lift",64,"broken",{ev(30,.38),ev(62,.54)})

local kick_deep=drum("Kick","Kick: deep pocket",64,"deep",repeat_events(64,16,{ev(1,1),ev(7,.68),ev(11,.42),ev(16,.78,-.03)}))
local kick_forward=drum("Kick","Kick: forward motion",64,"forward",{
  ev(1,1),ev(7,.72),ev(11,.5),ev(15,.56),ev(16,.8),ev(17,1),ev(23,.74),ev(27,.48),ev(31,.86),
  ev(33,1),ev(39,.7),ev(43,.52),ev(47,.62),ev(49,.96),ev(55,.76),ev(59,.48),ev(63,.84)})
local kick_broken=drum("Kick","Kick: broken pocket",64,"broken",{
  ev(1,1),ev(7,.66),ev(11,.5),ev(16,.78),ev(17,1),ev(23,.7),ev(27,.52),ev(31,.62),ev(32,.82),
  ev(33,1),ev(39,.68),ev(43,.54),ev(46,.48),ev(48,.8),ev(49,1),ev(55,.72),ev(59,.56),ev(62,.64),ev(64,.88)})
local kick_exit=drum("Kick","Kick: footsteps out",128,"deep",{
  ev(1,1),ev(7,.66),ev(16,.76),ev(17,.94),ev(23,.62),ev(32,.7),ev(33,.86),ev(39,.58),
  ev(48,.64),ev(49,.78),ev(64,.58),ev(65,.68),ev(81,.56),ev(97,.42)})

local snare_deep=drum("Snare","Snare: behind the shoulder",64,"deep",repeat_events(64,16,{ev(5,.92),ev(13,1,.07)}))
local snare_forward=drum("Snare","Snare: pulling ahead",64,"forward",repeat_events(64,16,{ev(5,.96,-.025),ev(13,1,-.02),ev(16,.3)}))
local snare_broken=drum("Snare","Snare: wrong doorway",64,"broken",{
  ev(5,.96),ev(13,1),ev(21,.96),ev(29,1),ev(37,1),ev(45,1),ev(53,1),ev(61,1),ev(63,.34)})
local snare_exit=drum("Snare","Snare: last shoulders",128,"deep",{ev(5,.9),ev(13,1),ev(21,.88),ev(29,.96),ev(37,.82),ev(45,.9),ev(53,.74),ev(61,.82),ev(77,.68),ev(93,.54)})

local ghost_deep=drum("Ghost","Ghost: conversation",64,"deep",{
  ev(4,.18),ev(10,.24),ev(12,.15),ev(16,.3),ev(20,.2),ev(26,.28),ev(28,.16),ev(32,.34),
  ev(36,.2),ev(42,.26),ev(47,.18),ev(52,.24),ev(58,.3),ev(60,.18),ev(63,.38)})
local ghost_forward=drum("Ghost","Ghost: elbows",64,"forward",repeat_events(64,16,{ev(3,.18),ev(4,.3),ev(8,.16),ev(10,.26),ev(12,.2),ev(15,.4),ev(16,.28)}))
local ghost_broken=drum("Ghost","Ghost: tripwire",64,"broken",{
  ev(4,.18),ev(10,.3),ev(12,.16),ev(16,.36),ev(20,.2),ev(26,.32),ev(28,.18),ev(32,.4),
  ev(36,.2),ev(42,.34),ev(44,.18),ev(48,.42),ev(52,.22),ev(58,.36),ev(60,.2),ev(63,.46)})

local hats_deep=drum("Hats","Hats: lazy hinge",64,"deep",repeat_events(64,8,{ev(1,.42),ev(3,.3),ev(5,.52),ev(7,.34)}))
local hats_forward=drum("Hats","Hats: rolling hinge",64,"forward",repeat_events(64,8,{ev(1,.5),ev(3,.38),ev(5,.62),ev(7,.44),ev(8,.22)}))
local hats_broken=drum("Hats","Hats: dropped teeth",64,"broken",{
  ev(1,.48),ev(3,.32),ev(5,.58),ev(7,.36),ev(9,.46),ev(11,.34),ev(13,.6),ev(15,.4),
  ev(17,.5),ev(19,.34),ev(21,.62),ev(23,.38),ev(25,.48),ev(27,.36),ev(29,.64),ev(31,.46),ev(32,.24),
  ev(33,.52),ev(35,.36),ev(37,.64),ev(39,.4),ev(41,.5),ev(43,.38),ev(45,.66),ev(47,.44),
  ev(49,.54),ev(51,.38),ev(53,.68),ev(55,.42),ev(57,.52),ev(59,.4),ev(61,.7),ev(63,.5),ev(64,.3)})
local hats_exit=drum("Hats","Hats: evaporating",128,"deep",{ev(1,.44),ev(3,.3),ev(5,.52),ev(7,.34),ev(9,.42),ev(11,.28),ev(13,.5),ev(15,.32),ev(17,.4),ev(21,.46),ev(25,.38),ev(29,.44),ev(33,.36),ev(41,.4),ev(49,.32),ev(65,.26),ev(81,.2)})
local open_deep=drum("Open Hat","Open hat: exhale",64,"deep",{ev(12,.34),ev(28,.42),ev(44,.36),ev(60,.48)})
local open_forward=drum("Open Hat","Open hat: shoulders",64,"forward",{ev(8,.28),ev(16,.42),ev(24,.32),ev(32,.5),ev(40,.3),ev(48,.46),ev(56,.36),ev(64,.56)})
local tamb_forward=drum("Tambourine","Tambourine: shoes",64,"forward",repeat_events(64,8,{ev(3,.26),ev(7,.4)}))
local tamb_broken=drum("Tambourine","Tambourine: running feet",64,"broken",{ev(3,.28),ev(7,.42),ev(11,.3),ev(15,.46),ev(19,.3),ev(23,.44),ev(27,.34),ev(31,.5),ev(35,.32),ev(39,.46),ev(43,.34),ev(47,.52),ev(51,.34),ev(55,.5),ev(59,.4),ev(63,.58)})

-- Bass answers the kick: roots reinforce steps 1/17, fifths avoid the snare,
-- and steps 16/32 anticipate the next bar. Gates deliberately cross selected
-- snares while other phrases leave them completely exposed.
local bass_deep=notes("Bass","Bass: deep pocket",128,"deep",{
  {step=1,note=38,velocity=1,gate=2.8},{step=7,note=38,velocity=.68,gate=.8},{step=11,note=45,velocity=.76,gate=1.4},{step=16,note=36,velocity=.72,gate=.65,offset=-.04},
  {step=17,note=38,velocity=.96,gate=3.4},{step=23,note=50,velocity=.62,gate=.75},{step=27,note=41,velocity=.74,gate=1.2},{step=32,note=45,velocity=.68,gate=.6,offset=-.03},
  {step=33,note=38,velocity=1,gate=2.6},{step=39,note=38,velocity=.6,gate=.7},{step=43,note=45,velocity=.78,gate=1.5},{step=48,note=36,velocity=.72,gate=.6,offset=-.05},
  {step=49,note=34,velocity=.94,gate=3.2},{step=55,note=41,velocity=.64,gate=.7},{step=59,note=45,velocity=.76,gate=1.2},{step=64,note=36,velocity=.7,gate=.65,offset=-.04},
  {step=65,note=38,velocity=1,gate=2.8},{step=71,note=38,velocity=.66,gate=.8},{step=75,note=45,velocity=.78,gate=1.2},{step=80,note=36,velocity=.7,gate=.6,offset=-.03},
  {step=81,note=38,velocity=.96,gate=3.5},{step=87,note=50,velocity=.62,gate=.7},{step=91,note=41,velocity=.76,gate=1.1},{step=96,note=45,velocity=.68,gate=.6,offset=-.04},
  {step=97,note=41,velocity=.94,gate=2.8},{step=103,note=48,velocity=.64,gate=.8},{step=107,note=38,velocity=.76,gate=1.3},{step=112,note=36,velocity=.68,gate=.6,offset=-.05},
  {step=113,note=36,velocity=1,gate=3.2},{step=119,note=43,velocity=.66,gate=.7},{step=123,note=41,velocity=.8,gate=1.2},{step=128,note=33,velocity=.72,gate=.6,offset=-.04}})
local bass_forward=notes("Bass","Bass: forward motion",128,"forward",{
  {step=1,note=38,velocity=1,gate=2.1},{step=6,note=38,velocity=.62,gate=.55},{step=11,note=45,velocity=.82,gate=.9},{step=15,note=50,velocity=.66,gate=.55},{step=16,note=36,velocity=.8,gate=.5,offset=-.05},
  {step=17,note=38,velocity=1,gate=2},{step=23,note=45,velocity=.72,gate=.6},{step=27,note=41,velocity=.86,gate=.8},{step=31,note=45,velocity=.68,gate=.5},{step=32,note=36,velocity=.78,gate=.5,offset=-.06},
  {step=33,note=38,velocity=1,gate=2.3},{step=39,note=50,velocity=.66,gate=.55},{step=43,note=45,velocity=.84,gate=.9},{step=47,note=41,velocity=.7,gate=.5},{step=48,note=36,velocity=.82,gate=.45,offset=-.05},
  {step=49,note=34,velocity=1,gate=2.2},{step=55,note=41,velocity=.7,gate=.55},{step=59,note=45,velocity=.86,gate=.8},{step=63,note=36,velocity=.76,gate=.5},{step=64,note=33,velocity=.82,gate=.45,offset=-.06},
  {step=65,note=38,velocity=1,gate=2},{step=71,note=45,velocity=.7,gate=.55},{step=75,note=50,velocity=.88,gate=.8},{step=80,note=36,velocity=.82,gate=.45,offset=-.06},
  {step=81,note=38,velocity=1,gate=2.2},{step=87,note=41,velocity=.68,gate=.55},{step=91,note=45,velocity=.86,gate=.8},{step=96,note=36,velocity=.8,gate=.45,offset=-.05},
  {step=97,note=41,velocity=.96,gate=2},{step=103,note=48,velocity=.7,gate=.55},{step=107,note=45,velocity=.84,gate=.8},{step=112,note=36,velocity=.82,gate=.45,offset=-.06},
  {step=113,note=36,velocity=1,gate=2.4},{step=119,note=43,velocity=.72,gate=.55},{step=123,note=41,velocity=.88,gate=.9},{step=127,note=33,velocity=.72,gate=.5},{step=128,note=36,velocity=.84,gate=.45,offset=-.05}})
local bass_broken=notes("Bass","Bass: broken pocket",128,"broken",{
  {step=1,note=38,velocity=1,gate=2.2},{step=7,note=38,velocity=.68,gate=.6},{step=11,note=45,velocity=.82,gate=.9},{step=15,note=50,velocity=.76,gate=.45},{step=16,note=36,velocity=.72,gate=.4},
  {step=17,note=38,velocity=1,gate=1.8},{step=23,note=45,velocity=.72,gate=.55},{step=27,note=41,velocity=.84,gate=.8},{step=31,note=50,velocity=.8,gate=.45},{step=32,note=45,velocity=.72,gate=.4},
  {step=33,note=38,velocity=1,gate=2},{step=39,note=50,velocity=.72,gate=.55},{step=43,note=45,velocity=.86,gate=.8},{step=46,note=53,velocity=.7,gate=.4},{step=48,note=50,velocity=.82,gate=.45},
  {step=49,note=34,velocity=1,gate=2.2},{step=55,note=41,velocity=.72,gate=.55},{step=59,note=46,velocity=.86,gate=.8},{step=62,note=53,velocity=.74,gate=.4},{step=64,note=46,velocity=.82,gate=.45},
  {step=65,note=38,velocity=1,gate=1.8},{step=71,note=45,velocity=.72,gate=.55},{step=75,note=50,velocity=.88,gate=.8},{step=78,note=53,velocity=.72,gate=.4},{step=80,note=50,velocity=.84,gate=.45},
  {step=81,note=38,velocity=1,gate=1.7},{step=87,note=50,velocity=.74,gate=.55},{step=91,note=45,velocity=.88,gate=.75},{step=94,note=53,velocity=.74,gate=.4},{step=96,note=50,velocity=.86,gate=.45},
  {step=97,note=41,velocity=1,gate=1.8},{step=103,note=48,velocity=.74,gate=.55},{step=107,note=53,velocity=.9,gate=.75},{step=110,note=55,velocity=.74,gate=.4},{step=112,note=53,velocity=.86,gate=.45},
  {step=113,note=36,velocity=1,gate=2},{step=119,note=43,velocity=.76,gate=.55},{step=123,note=48,velocity=.9,gate=.75},{step=126,note=55,velocity=.78,gate=.4},{step=127,note=52,velocity=.72,gate=.35},{step=128,note=50,velocity=.88,gate=.4}})
local bass_exit=notes("Bass","Bass: weight leaving",128,"deep",{{step=1,note=38,velocity=.9,gate=3},{step=16,note=36,velocity=.7,gate=.6,offset=-.04},{step=17,note=38,velocity=.84,gate=3},{step=33,note=34,velocity=.78,gate=4},{step=49,note=38,velocity=.7,gate=5},{step=65,note=26,velocity=.6,gate=8},{step=97,note=26,velocity=.38,gate=16}})

local breath_deep=drum("Breath Hook","Breath: two syllables",128,"deep",{ev(15,.44),ev(47,.5),ev(79,.46),ev(111,.56)})
local breath_forward=drum("Breath Hook","Breath: call",64,"forward",{ev(7,.38),ev(15,.52),ev(23,.4),ev(31,.58),ev(39,.42),ev(47,.56),ev(55,.46),ev(63,.64)})
local breath_reverse=drum("Reverse Breath","Breath: answer backwards",128,"broken",{ev(30,.42),ev(62,.5),ev(94,.46),ev(126,.6)})
local typing_deep=drum("Typing","Typing: side room",128,"deep",{ev(1,.34),ev(65,.3)})
local typing_broken=drum("Typing","Typing: nervous hands",64,"broken",{ev(3,.3),ev(11,.42),ev(18,.34),ev(27,.46),ev(35,.32),ev(42,.44),ev(51,.38),ev(59,.5)})
local two_note=notes("Two Note","Two notes, enough",128,"deep",{{step=13,note=55,velocity=.52,gate=2},{step=45,note=58,velocity=.58,gate=3},{step=77,note=55,velocity=.5,gate=2},{step=109,note=53,velocity=.62,gate=4}})
local two_note_push=notes("Two Note","Two notes, insist",128,"forward",{{step=7,note=55,velocity=.58,gate=1.5},{step=29,note=58,velocity=.66,gate=2},{step=39,note=55,velocity=.56,gate=1.5},{step=61,note=53,velocity=.68,gate=3},{step=71,note=55,velocity=.6,gate=1.5},{step=93,note=58,velocity=.7,gate=2},{step=103,note=55,velocity=.58,gate=1.5},{step=125,note=53,velocity=.74,gate=3}})

local function scene(name,values)
  local explicit={};for _,track_name in ipairs(names) do explicit[track_name]=false end
  for key,value in pairs(values) do explicit[key]=value end
  return sequence:scene(name,explicit)
end
local crank=scene("Crank",{["Performed Break A"]=performed_sparse,["Oar Chop A"]=oar_deep,["Ghost"]=ghost_deep,["Breath Hook"]=breath_deep})
local pocket_a=scene("Deep Pocket A",{["Performed Break A"]=performed_a,["Oar Chop A"]=oar_deep,["Oar Chop B"]=oar_low,Kick=kick_deep,Snare=snare_deep,Ghost=ghost_deep,Hats=hats_deep,["Open Hat"]=open_deep,Bass=bass_deep,["Breath Hook"]=breath_deep})
local pocket_b=scene("Deep Pocket B",{["Performed Break B"]=performed_b,["Oar Chop A"]=oar_deep,["Oar Chop B"]=oar_low,Kick=kick_deep,Snare=snare_deep,Ghost=ghost_deep,Hats=hats_deep,["Open Hat"]=open_deep,Bass=bass_deep,["Breath Hook"]=breath_deep,Typing=typing_deep,["Two Note"]=two_note})
local forward_a=scene("Forward Motion A",{["Performed Break B"]=performed_b,["Oar Chop A"]=oar_forward,["Oar Chop B"]=oar_low_forward,Kick=kick_forward,Snare=snare_forward,Ghost=ghost_forward,Hats=hats_forward,["Open Hat"]=open_forward,Tambourine=tamb_forward,Bass=bass_forward,["Breath Hook"]=breath_forward,["Two Note"]=two_note})
local forward_b=scene("Forward Motion B",{["Performed Break A"]=performed_a,["Oar Chop A"]=oar_forward,["Oar Chop B"]=oar_low_forward,["Reverse Oar"]=reverse_sparse,Kick=kick_forward,Snare=snare_forward,Ghost=ghost_forward,Hats=hats_forward,["Open Hat"]=open_forward,Tambourine=tamb_forward,Bass=bass_forward,["Breath Hook"]=breath_forward,Typing=typing_deep,["Two Note"]=two_note_push})
local broken_entry=scene("Stroke Ignition",{["Performed Break B"]=performed_b,["Oar Chop A"]=oar_broken,["Reverse Oar"]=reverse_busy,Kick=kick_broken,Snare=snare_broken,Ghost=ghost_broken,Hats=hats_broken,["Open Hat"]=open_forward,Tambourine=tamb_broken,Bass=bass_broken,["Breath Hook"]=breath_forward,["Two Note"]=two_note_push})
local broken_pocket=scene("Rower's High",{["Performed Break A"]=performed_a,["Oar Chop A"]=oar_broken,["Oar Chop B"]=oar_low_forward,["Reverse Oar"]=reverse_busy,Kick=kick_broken,Snare=snare_broken,Ghost=ghost_broken,Hats=hats_broken,["Open Hat"]=open_forward,Tambourine=tamb_broken,Bass=bass_broken,["Breath Hook"]=breath_forward,Typing=typing_deep,["Two Note"]=two_note_push})
local bare=scene("Bare Hands",{["Performed Break A"]=performed_a,["Oar Chop A"]=oar_deep,Kick=kick_deep,Snare=snare_deep,Bass=bass_deep,["Breath Hook"]=breath_deep})
local reassembly=scene("Reassembly",{["Performed Break A"]=performed_a,["Oar Chop A"]=oar_deep,["Oar Chop B"]=oar_low,Kick=kick_deep,Snare=snare_deep,Ghost=ghost_deep,Hats=hats_deep,["Open Hat"]=open_deep,Bass=bass_deep,["Breath Hook"]=breath_deep,Typing=typing_deep})
local release_a=scene("Full Release A",{["Performed Break B"]=performed_b,["Oar Chop A"]=oar_forward,["Oar Chop B"]=oar_low_forward,Kick=kick_forward,Snare=snare_forward,Ghost=ghost_deep,Hats=hats_forward,["Open Hat"]=open_forward,Tambourine=tamb_forward,Bass=bass_forward,["Breath Hook"]=breath_forward,Typing=typing_deep,["Two Note"]=two_note_push})
local release_b=scene("Full Release B",{["Performed Break A"]=performed_a,["Oar Chop A"]=oar_forward,["Oar Chop B"]=oar_low_forward,["Reverse Oar"]=reverse_sparse,Kick=kick_forward,Snare=snare_forward,Ghost=ghost_forward,Hats=hats_forward,["Open Hat"]=open_forward,Tambourine=tamb_forward,Bass=bass_broken,["Breath Hook"]=breath_forward,["Reverse Breath"]=breath_reverse,Typing=typing_deep,["Two Note"]=two_note_push})
local exit=scene("Walk Away",{["Performed Break A"]=performed_sparse,["Oar Chop A"]=oar_deep,Kick=kick_exit,Snare=snare_exit,Hats=hats_exit,Bass=bass_exit,["Breath Hook"]=breath_deep})

local arrangement=sequence:timeline("Weight Transfer 05")
arrangement:at_bar(1,crank):at_bar(9,pocket_a):at_bar(25,pocket_b):at_bar(41,forward_a)
  :at_bar(57,forward_b):at_bar(73,broken_entry):at_bar(85,broken_pocket):at_bar(105,bare)
  :at_bar(117,reassembly):at_bar(137,release_a):at_bar(153,release_b):at_bar(169,exit):stop_at_bar(177)

local avl=meldritch.samples {source="remote",bank="Weight",cache=true,
  manifest="samples/weight-avl.json",
  base_url="https://raw.githubusercontent.com/studiorack/avl-drumkits/b06cb2c27359cbc68a830a82bddced05bd73d1a0/",
  version="b06cb2c27359cbc68a830a82bddced05bd73d1a0"}
local found=meldritch.samples {source="remote",bank="Weight",cache=true,
  manifest="samples/weight-meta.json",
  base_url="https://raw.githubusercontent.com/facebookincubator/Meta-open-sound/251c0be57d5d1b3242b91f53e0cde62ab1cb42d9/",
  version="251c0be57d5d1b3242b91f53e0cde62ab1cb42d9"}

local performance=instrument:mixer {inputs=8,headroom=.28}
local function sample_to(track_name,sample,settings,mixer,slot,velocity)
  settings=settings or {};settings.sample=sample
  local player=instrument:sample_player(settings);instrument:connect(track[track_name].out,player.trigger)
  if velocity then
    local l=instrument:vca {};local r=instrument:vca {};instrument:connect(player.left,l.input);instrument:connect(player.right,r.input)
    instrument:connect(track[track_name].velocity,l.cv);instrument:connect(track[track_name].velocity,r.cv)
    instrument:connect(l.out,mixer.input[slot].left);instrument:connect(r.out,mixer.input[slot].right)
  else instrument:connect(player.left,mixer.input[slot].left);instrument:connect(player.right,mixer.input[slot].right) end
  return player
end
local row_a=sample_to("Performed Break A",found:load("rowing",4),{gain=.66,rate=.856},performance,1,false)
local row_b=sample_to("Performed Break B",found:load("rowing",3),{gain=.62,rate=.883},performance,2,false)
local chop_a=sample_to("Oar Chop A",found:load("rowing",4),{gain=.48,start=.0,finish=.31,rate=.92},performance,3,true)
local chop_b=sample_to("Oar Chop B",found:load("rowing",4),{gain=.42,start=.31,finish=.61,rate=.78},performance,4,true)
local reverse_oar=sample_to("Reverse Oar",found:load("rowing",4),{gain=.36,start=.05,finish=.36,rate=-.7},performance,5,true)
local performance_l=instrument:svf {cutoff=180,resonance=.2};local performance_r=instrument:svf {cutoff=220,resonance=.18}
instrument:connect(performance.left,performance_l.input);instrument:connect(performance.right,performance_r.input)

local drums=instrument:mixer {inputs=8,headroom=.28}
local kick=sample_to("Kick",avl:load("kick",4),{gain=1.12,finish=.86,rate=.96},drums,1,true)
local snare=sample_to("Snare",avl:load("snare",4),{gain=1.04,start=.015,finish=.9,rate=1.02},drums,2,true)
local ghost=sample_to("Ghost",avl:load("snare_edge",1),{gain=.62,start=.04,finish=.62,rate=1.12},drums,3,true)
local hats=sample_to("Hats",avl:load("hat_closed",3),{gain=.54,start=.03,finish=.58,rate=1.04},drums,4,true)
local open_hat=sample_to("Open Hat",avl:load("hat_open",3),{gain=.38,start=.02,finish=.78,rate=.92},drums,5,true)
local tamb=sample_to("Tambourine",avl:load("tambourine",3),{gain=.38,start=.02,finish=.76,rate=.96},drums,6,true)
local drum_drive_l=instrument:drive {amount=3.2,output_gain=.34};local drum_drive_r=instrument:drive {amount=3.2,output_gain=.34}
instrument:connect(drums.left,drum_drive_l.input);instrument:connect(drums.right,drum_drive_r.input)
local drum_sum=instrument:mixer {inputs=8,headroom=.44};instrument:connect(drums.left,drum_sum.input[1].left);instrument:connect(drums.right,drum_sum.input[1].right);instrument:connect(drum_drive_l.out,drum_sum.input[2].left);instrument:connect(drum_drive_r.out,drum_sum.input[2].right);instrument:connect(performance_l.highpass,drum_sum.input[3].left);instrument:connect(performance_r.highpass,drum_sum.input[3].right)

local bass_main=instrument:plaits {model="virtual_analog",harmonics=.34,timbre=.58,morph=.28};local bass_sub=instrument:oscillator {waveform="sine",frequency=55}
local bass_env=instrument:adsr {attack=.004,decay=.2,sustain=.3,release=.09};local bass_a=instrument:vca {};local sub_a=instrument:vca {}
instrument:connect(track.Bass.pitch,bass_main.pitch);instrument:connect(track.Bass.gate,bass_main.gate);instrument:connect(track.Bass.velocity,bass_main.velocity);instrument:connect(track.Bass.pitch,bass_sub.pitch);instrument:connect(track.Bass.gate,bass_env.gate)
instrument:connect(bass_main.main,bass_a.input);instrument:connect(bass_sub.out,sub_a.input);instrument:connect(bass_env.out,bass_a.cv);instrument:connect(bass_env.out,sub_a.cv)
local bass_sum=instrument:mixer {inputs=2};instrument:connect(bass_a.out,bass_sum.input[1]);instrument:connect(sub_a.out,bass_sum.input[2])
local bass_filter=instrument:svf {cutoff=760,resonance=.48};local bass_drive=instrument:drive {amount=2.6,output_gain=.72};instrument:connect(bass_sum.out,bass_filter.input);instrument:connect(bass_filter.lowpass,bass_drive.input)
local bass_l=instrument:ducker {threshold=.025,depth=.62,attack=.003,release=.115};local bass_r=instrument:ducker {threshold=.025,depth=.62,attack=.003,release=.115};instrument:connect(bass_drive.out,bass_l.input);instrument:connect(bass_drive.out,bass_r.input);instrument:connect(kick.left,bass_l.sidechain);instrument:connect(kick.right,bass_r.sidechain)

local details=instrument:mixer {inputs=8,headroom=.3}
local breath=sample_to("Breath Hook",found:load("breath",0),{gain=.42,start=.18,finish=.43,rate=.82},details,1,true)
local breath_r=sample_to("Reverse Breath",found:load("breath",0),{gain=.38,start=.15,finish=.48,rate=-.7},details,2,true)
local typing=sample_to("Typing",found:load("typing",0),{gain=.36,start=.3,finish=.46,rate=1.06},details,3,true)
local detail_filter_l=instrument:svf {cutoff=520,resonance=.32};local detail_filter_r=instrument:svf {cutoff=660,resonance=.28};instrument:connect(details.left,detail_filter_l.input);instrument:connect(details.right,detail_filter_r.input)
local detail_delay=instrument:stereo_delay {left_beats=.375,right_beats=.625,feedback=.32,crossfeed=.76,mix=.72,filter_cutoff=3600,feedback_drive=1.35};instrument:connect(sequence.tempo,detail_delay.bpm);instrument:connect(detail_filter_l.highpass,detail_delay.input.left);instrument:connect(detail_filter_r.highpass,detail_delay.input.right)

local hook=instrument:plaits {model="string",harmonics=.36,timbre=.42,morph=.32};local hook_env=instrument:adsr {attack=.012,decay=.22,sustain=.34,release=.65};local hook_amp=instrument:vca {};local hook_filter=instrument:svf {cutoff=2100,resonance=.3}
instrument:connect(track["Two Note"].pitch,hook.pitch);instrument:connect(track["Two Note"].gate,hook.gate);instrument:connect(track["Two Note"].velocity,hook.velocity);instrument:connect(track["Two Note"].gate,hook_env.gate);instrument:connect(hook.main,hook_amp.input);instrument:connect(hook_env.out,hook_amp.cv);instrument:connect(hook_amp.out,hook_filter.input)

local live=instrument:plaits {model="waveshaping",harmonics=.42,timbre=.46,morph=.34};local live_env=instrument:adsr {attack=.008,decay=.2,sustain=.5,release=.6};local live_amp=instrument:vca {};local live_gain=instrument:gain {gain=.3};instrument:connect(instrument.note.pitch,live.pitch);instrument:connect(instrument.note.gate,live.gate);instrument:connect(instrument.note.velocity,live.velocity);instrument:connect(instrument.note.gate,live_env.gate);instrument:connect(live.main,live_amp.input);instrument:connect(live_env.out,live_amp.cv);instrument:connect(live_amp.out,live_gain.input)

local music=instrument:mixer {inputs=8,headroom=.32};instrument:connect(bass_l.out,music.input[1].left);instrument:connect(bass_r.out,music.input[1].right);instrument:connect(hook_filter.lowpass,music.input[2].left);instrument:connect(hook_filter.lowpass,music.input[2].right);instrument:connect(detail_filter_l.highpass,music.input[3].left);instrument:connect(detail_filter_r.highpass,music.input[3].right);instrument:connect(detail_delay.left,music.input[4].left);instrument:connect(detail_delay.right,music.input[4].right);instrument:connect(live_gain.out,music.input[5].left);instrument:connect(live_gain.out,music.input[5].right)

local master=instrument:mixer {inputs=8,headroom=.75};instrument:connect(drum_sum.left,master.input[1].left);instrument:connect(drum_sum.right,master.input[1].right);instrument:connect(music.left,master.input[2].left);instrument:connect(music.right,master.input[2].right)
local dc_l=instrument:svf {cutoff=24,resonance=.04};local dc_r=instrument:svf {cutoff=24,resonance=.04};instrument:connect(master.left,dc_l.input);instrument:connect(master.right,dc_r.input)
local comp=instrument:stereo_compressor {threshold=-17,ratio=2.7,attack=.022,release=.18,knee=5,makeup_gain=5,bypass=0};local final_l=instrument:gain {gain=1};local final_r=instrument:gain {gain=1};instrument:connect(dc_l.highpass,comp.input.left);instrument:connect(dc_r.highpass,comp.input.right);instrument:connect(comp.left,final_l.input);instrument:connect(comp.right,final_r.input);instrument:connect(final_l.out,instrument.output.left);instrument:connect(final_r.out,instrument.output.right)

local function control(id,label,target,range,targets,curve) return instrument:control {id=id,label=label,target=target,targets=targets,range=range,curve=curve} end
local break_level=control("break_level","Performed break",row_a.gain,{0,1.1},{row_b.gain})
local chop_level=control("chop_level","Oar fragments",chop_a.gain,{0,.9},{chop_b.gain,reverse_oar.gain})
local drum_dirt=control("drum_dirt","Parallel dirt",drum_drive_l.amount,{.2,10},{drum_drive_r.amount})
local bass_cutoff=control("bass_cutoff","Bass pocket",bass_filter.cutoff,{90,5200},nil,"log")
local bass_decay=control("bass_decay","Bass articulation",bass_env.decay,{.04,.65})
local detail_tone=control("detail_tone","Fragment tone",detail_filter_l.cutoff,{180,8000},{detail_filter_r.cutoff},"log")
local detail_feedback=control("detail_feedback","Fragment feedback",detail_delay.feedback,{0,.82})
local hook_tone=control("hook_tone","Two-note color",hook_filter.cutoff,{320,7000},nil,"log")
local master_level=control("master_level","Master volume",master.headroom,{.05,.75})
local master_threshold=control("master_threshold","Glue threshold",comp.threshold,{-32,-5});local master_ratio=control("master_ratio","Glue ratio",comp.ratio,{1,8});local master_attack=control("master_attack","Glue attack",comp.attack,{.003,.12},nil,"log");local master_release=control("master_release","Glue release",comp.release,{.04,.8},nil,"log");local ending_fade=control("ending_fade","Arrangement fade",final_l.gain,{0,1},{final_r.gain})

arrangement:automation {target=break_level,interpolation="smooth"}:at_bar(1,.44):at_bar(9,.68):at_bar(41,.76):at_bar(72,.66):at_bar(73,.72):at_bar(85,.8):at_bar(104,.76):at_bar(105,.54):at_bar(137,.76):at_bar(169,.48):at_bar(177,.18)
arrangement:automation {target=chop_level,interpolation="linear"}:at_bar(1,.34):at_bar(25,.48):at_bar(57,.66):at_bar(73,.48):at_bar(85,.56):at_bar(104,.62):at_bar(105,.3):at_bar(137,.62):at_bar(153,.76):at_bar(169,.32)
arrangement:automation {target=drum_dirt,interpolation="smooth"}:at_bar(1,.8):at_bar(9,2.2):at_bar(41,3.8):at_bar(65,5.4):at_bar(73,3.2):at_bar(96,4.8):at_bar(104,5.4):at_bar(105,1.6):at_bar(137,4.2):at_bar(160,6):at_bar(169,2):at_bar(177,.6)
arrangement:automation {target=bass_cutoff,interpolation="smooth"}:at_bar(1,280):at_bar(9,620):at_bar(41,1100):at_bar(72,2200):at_bar(73,1200):at_bar(85,1800):at_bar(104,3000):at_bar(105,420):at_bar(137,1200):at_bar(160,2600):at_bar(169,680):at_bar(177,180)
arrangement:automation {target=bass_decay,interpolation="linear"}:at_bar(1,.3):at_bar(9,.22):at_bar(41,.14):at_bar(73,.16):at_bar(96,.11):at_bar(105,.26):at_bar(137,.13):at_bar(169,.28)
arrangement:automation {target=detail_tone,interpolation="smooth"}:at_bar(1,480):at_bar(40,1800):at_bar(72,5200):at_bar(96,860):at_bar(120,2600):at_bar(152,6200):at_bar(169,1200):at_bar(177,320)
arrangement:automation {target=detail_feedback,interpolation="linear"}:at_bar(1,.42):at_bar(41,.28):at_bar(72,.5):at_bar(96,.46):at_bar(105,.26):at_bar(137,.34):at_bar(168,.62):at_bar(169,.3):at_bar(176,.68)
arrangement:automation {target=hook_tone,interpolation="smooth"}:at_bar(1,900):at_bar(41,1800):at_bar(72,3600):at_bar(96,4600):at_bar(104,5200):at_bar(105,760):at_bar(137,2200):at_bar(168,4800):at_bar(177,520)
arrangement:automation {target=ending_fade,interpolation="smooth"}:at_bar(1,1):at_bar(169,1):at_bar(173,.78):at_bar(176,.24):at_bar(177,0)

instrument:performance {layout="step_grid",sequencer=sequence,project_file="05_the_weight_of_moving.lua",
  keys={Space="transport",s="start",x="stop",r="restart",Up="track:up",Down="track:down",Left="step:previous",Right="step:next",["["]="bank:previous",["]"]="bank:next",m="mute",o="solo",j="control:next",k="control:previous",h="control:decrease",l="control:increase",["1"]="toggle:1",["2"]="toggle:2",["3"]="toggle:3",["4"]="toggle:4",["5"]="toggle:5",["6"]="toggle:6",["7"]="toggle:7",["8"]="toggle:8"},
  apc={pages={"steps","performance"},performance_rows={"scene","mute","solo","track","restart","clip","",""},scenes={"Crank","Deep Pocket A","Forward Motion A","Rower's High","Bare Hands","Reassembly","Full Release A","Walk Away"},scene_quantization=16,
    faders={"break_level","chop_level","drum_dirt","bass_cutoff","bass_decay","detail_tone","detail_feedback","hook_tone"},fader_banks={{"break_level","chop_level","drum_dirt","bass_cutoff","bass_decay","detail_tone","detail_feedback","hook_tone"},{"master_threshold","master_ratio","master_attack","master_release","break_level","bass_cutoff","drum_dirt","detail_feedback"}},scene_buttons={"transport","restart","faders:bank:0","faders:bank:1","","","",""},master="master_level"},
  focus={controls={"break_level","chop_level","drum_dirt","bass_cutoff","bass_decay","detail_tone","detail_feedback","hook_tone"},midi_ccs={{channel=1,number=74},{channel=1,number=71},{channel=1,number=76},{channel=1,number=77}}}}

local composition=instrument:screen("composition","The Weight of Moving");composition:arrangement_status();composition:control_focus()
local clips=instrument:screen("clips","Groove / Clip Inspector");clips:pattern_grid();clips:control_focus()
local mastering=instrument:screen("master","Pocket / Master Glue");mastering:control("Performed break","break_level");mastering:control("Oar fragments","chop_level");mastering:control("Parallel dirt","drum_dirt");mastering:control("Bass articulation","bass_decay");mastering:control("Threshold (dB)","master_threshold");mastering:control("Ratio","master_ratio");mastering:control("Attack (s)","master_attack");mastering:control("Release (s)","master_release")
local navigation=instrument:navigation {initial="composition",events={{name="composition",from="*",to="composition"},{name="clips",from="*",to="clips"},{name="master",from="*",to="master"}}};navigation:bind("F9","composition");navigation:bind("F10","clips");navigation:bind("F11","master")
sequence:start()
return instrument
