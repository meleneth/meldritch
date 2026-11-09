-- A deliberately isolated Serum 2 playground. Serum's stereo outputs connect
-- directly to the host outputs: there is no mixer, gain stage, compressor,
-- limiter, effect, or other Meldritch DSP in the audio path.
local instrument=meldritch.instrument("Serum 2 Solo Playground")
local home=assert(os.getenv("HOME"),"HOME is required")
local SERUM="56534558667350736572756D20320000"
local catalog=os.getenv("MELDRITCH_SERUM_CATALOG") or home.."/.local/share/meldritch2/patches/serum2/factory"
instrument:voice_mode("mono")
local midi_device=os.getenv("MICROLAB_MIDI")
if midi_device then instrument:devices {midi_input=midi_device,midi_controls={midi_device}} end

local sequence=instrument:sequencer {name="Serum Audition",bpm=110,steps=16,steps_per_beat=4}
local track=sequence:note_track("Serum",{voices=3})
local single=sequence:pattern("Single notes")
single:notes(track,{{step=1,pitch="C3",velocity=96,duration=2},{step=5,pitch="E3",velocity=96,duration=2},{step=9,pitch="G3",velocity=96,duration=2},{step=13,pitch="C4",velocity=96,duration=2}},16)
local sustained=sequence:pattern("Sustained note")
sustained:notes(track,{{step=1,pitch="C3",velocity=92,duration=12}},16)
sequence:switch_pattern("Single notes")

local serum=instrument:vst3 {id="serum_solo",plugin=SERUM,paths={home.."/.vst3","/usr/lib/vst3"},preset_catalog=catalog,initial_preset=os.getenv("MELDRITCH_SERUM_PRESET") or "factory_lead_ld_silk",editor=true}:connect_track(track,sequence)
instrument:connect(instrument.note.pitch,serum.pitch_4)
instrument:connect(instrument.note.gate,serum.gate_4)
instrument:connect(instrument.note.velocity,serum.velocity_4)
instrument:connect(serum.out.left,instrument.output.left)
instrument:connect(serum.out.right,instrument.output.right)

instrument:control {id="serum_preset",label="Serum Preset",target=serum.preset,range={0,math.max(0,serum.preset_count-1)}}
instrument:control {id="serum_level",label="Serum Main Level",target=serum:parameter(0),range={0,1}}
instrument:control {id="serum_cutoff",label="Serum Filter Cutoff",target=serum:parameter(2000003),range={0,1}}
instrument:control {id="serum_resonance",label="Serum Filter Resonance",target=serum:parameter(2000004),range={0,1}}

local server=meldritch.server {host="127.0.0.1",port=8080}
server:websocket("/performance",{})
local performance=server:performance {name="Serum 2 Solo Playground"}
performance:role("musician",{controls="*"})
server:credential(os.getenv("MELDRITCH_SERUM_TOKEN") or "serum-playground","musician")
local plane=performance:plane {id="serum",label="Serum 2",roles={"musician"}}
plane:trigger {id="open_serum_editor",label="Open Serum UI on host",triggered=function() serum:open_editor() end}
plane:piano_roll {id="serum_notes",label="Serum Piano Roll",track=track,steps=16,pitch_min=24,pitch_max=84}
plane:pattern_selector {id="serum_patterns",label="Audition Pattern",sequencer=sequence,boundary="bar"}
plane:preset_selector {id="serum_preset",label="Serum Factory Presets",target="serum_preset",catalog=serum.preset_catalog}
plane:fader {id="serum_level",label="Serum Main Level",target="serum_level",min=0,max=1}
plane:knob {id="serum_cutoff",label="Serum Filter Cutoff",target="serum_cutoff",min=0,max=1}
plane:knob {id="serum_resonance",label="Serum Filter Resonance",target="serum_resonance",min=0,max=1}
return instrument
