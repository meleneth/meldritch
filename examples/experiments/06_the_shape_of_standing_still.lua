-- The Shape of Standing Still
-- 160 bars / 94 BPM / 6:48.5. Continuous drone, pulse, and three-note landmark.
-- No external samples are used.
local instrument=meldritch.instrument("The Shape of Standing Still")
instrument:voice_mode("mono")
instrument:devices {midi_input="MicroLab mk3:MicroLab mk3",
  midi_controls={"MicroLab mk3:MicroLab mk3"},
  apc_mini="APC mini mk2:APC mini mk2 Control"}

local sequence=instrument:sequencer {bpm=94,steps=128,steps_per_beat=4}
local names={"Low Pulse","Dust","Harmonic Gate","Three Notes"}
local track={};for _,name in ipairs(names) do track[name]=sequence:track(name) end

local function ev(step,velocity,offset) return {step=step,velocity=velocity,offset=offset or 0} end
local function drum(track_name,name,length,events)
  return track[track_name]:clip(name,{length=length,events=events})
end
local function notes(track_name,name,length,events)
  return track[track_name]:clip(name,{length=length,notes=events})
end

-- Percussion does not invent the pulse: it eventually names cycles already
-- audible in the drone. The 48-step tension clips realign every three bars.
local low_hint=drum("Low Pulse","Pulse: barely named",64,{ev(1,.48),ev(33,.42)})
local low_drift=drum("Low Pulse","Pulse: phase drift",48,{ev(1,.72),ev(13,.48),ev(25,.68),ev(37,.52)})
local low_open=drum("Low Pulse","Pulse: open interior",64,{ev(1,.64),ev(17,.5),ev(33,.62),ev(49,.46)})

local dust_sparse=drum("Dust","Dust: glints",64,{ev(12,.2),ev(30,.24),ev(44,.18),ev(62,.28)})
local dust_motion=drum("Dust","Dust: visible current",64,{
  ev(4,.18),ev(12,.28),ev(20,.2),ev(28,.34),ev(36,.2),ev(44,.3),ev(52,.22),ev(60,.38)})
local dust_drift=drum("Dust","Dust: three against four",48,{
  ev(7,.2),ev(13,.34),ev(19,.22),ev(25,.38),ev(31,.24),ev(37,.36),ev(43,.28),ev(48,.42)})
local dust_after=drum("Dust","Dust: afterimage",128,{ev(16,.2),ev(48,.16),ev(80,.14),ev(112,.12)})

local harmonic_sparse=notes("Harmonic Gate","Harmonic: hidden breath",96,{
  {step=1,note=50,velocity=.4,gate=7},{step=33,note=50,velocity=.34,gate=5},{step=65,note=45,velocity=.38,gate=7}})
local harmonic_pulse=notes("Harmonic Gate","Harmonic: one-bar lungs",64,{
  {step=1,note=50,velocity=.48,gate=6},{step=17,note=50,velocity=.42,gate=5},
  {step=33,note=45,velocity=.46,gate=6},{step=49,note=50,velocity=.44,gate=5}})
local harmonic_cross=notes("Harmonic Gate","Harmonic: phase window",48,{
  {step=1,note=50,velocity=.5,gate=5},{step=13,note=45,velocity=.42,gate=4},
  {step=25,note=50,velocity=.48,gate=5},{step=37,note=45,velocity=.44,gate=4}})
local harmonic_open=notes("Harmonic Gate","Harmonic: long light",128,{
  {step=1,note=50,velocity=.48,gate=14},{step=33,note=45,velocity=.4,gate=12},
  {step=65,note=50,velocity=.44,gate=18},{step=105,note=45,velocity=.34,gate=14}})

-- One landmark, always D/A/C. Its rhythm and articulation change; its notes do not.
local motif_fragment=notes("Three Notes","Landmark: distant",128,{
  {step=35,note=62,velocity=.34,gate=2.5},{step=40,note=57,velocity=.28,gate=1},
  {step=46,note=60,velocity=.32,gate=4}})
local motif_clear=notes("Three Notes","Landmark: recognized",64,{
  {step=3,note=62,velocity=.48,gate=2},{step=8,note=57,velocity=.4,gate=.8},
  {step=14,note=60,velocity=.46,gate=3},{step=35,note=62,velocity=.44,gate=2},
  {step=40,note=57,velocity=.36,gate=.8},{step=46,note=60,velocity=.42,gate=3}})
local motif_drift=notes("Three Notes","Landmark: seen sideways",96,{
  {step=7,note=60,velocity=.44,gate=3},{step=13,note=62,velocity=.5,gate=1.5},
  {step=20,note=57,velocity=.38,gate=1},{step=55,note=60,velocity=.42,gate=4},
  {step=61,note=62,velocity=.48,gate=2},{step=68,note=57,velocity=.36,gate=1}})
local motif_open=notes("Three Notes","Landmark: inside out",128,{
  {step=1,note=60,velocity=.46,gate=9},{step=23,note=62,velocity=.52,gate=7},
  {step=49,note=57,velocity=.4,gate=11},{step=81,note=60,velocity=.42,gate=13},
  {step=113,note=62,velocity=.38,gate=9}})

local function scene(name,values)
  local explicit={};for _,track_name in ipairs(names) do explicit[track_name]=false end
  for key,value in pairs(values) do explicit[key]=value end
  return sequence:scene(name,explicit)
end
local field=scene("Static Field",{})
local hidden=scene("Hidden Pulse",{["Harmonic Gate"]=harmonic_sparse,["Three Notes"]=motif_fragment})
local recognized=scene("Recognized Motion",{["Low Pulse"]=low_hint,Dust=dust_sparse,["Harmonic Gate"]=harmonic_pulse,["Three Notes"]=motif_clear})
local drift=scene("Phase Drift",{["Low Pulse"]=low_drift,Dust=dust_drift,["Harmonic Gate"]=harmonic_cross,["Three Notes"]=motif_drift})
local interior=scene("Open Interior",{["Low Pulse"]=low_open,Dust=dust_motion,["Harmonic Gate"]=harmonic_open,["Three Notes"]=motif_open})
local afterimage=scene("Afterimage",{Dust=dust_after,["Harmonic Gate"]=harmonic_sparse,["Three Notes"]=motif_fragment})

local arrangement=sequence:timeline("Standing Still / Moving 06")
arrangement:at_bar(1,field):at_bar(25,hidden):at_bar(49,recognized)
  :at_bar(81,drift):at_bar(113,interior):at_bar(145,afterimage):stop_at_bar(161)

-- FOUNDATION: a centered D1/A1 identity. Two very slow, bar-related frequency
-- movements create beating without allowing the sub image to wander sideways.
local sub_d=instrument:oscillator {waveform="sine",frequency=36.708}
local sub_a=instrument:oscillator {waveform="triangle",frequency=55}
local foundation_drift_a=instrument:lfo {frequency=.0122396}
local foundation_drift_b=instrument:lfo {frequency=.0139881}
local detune_a=instrument:affine {scale=.035,offset=0};local detune_b=instrument:affine {scale=.055,offset=0}
instrument:connect(foundation_drift_a.out,detune_a.input);instrument:connect(detune_a.out,sub_d.fm)
instrument:connect(foundation_drift_b.out,detune_b.input);instrument:connect(detune_b.out,sub_a.fm)
local foundation_mix=instrument:mixer {inputs=2};instrument:connect(sub_d.out,foundation_mix.input[1]);instrument:connect(sub_a.out,foundation_mix.input[2])
local foundation_filter=instrument:svf {cutoff=145,resonance=.18};local foundation_drive=instrument:drive {amount=1.25,output_gain=.78}
instrument:connect(foundation_mix.out,foundation_filter.input);instrument:connect(foundation_filter.lowpass,foundation_drive.input)
local foundation_breath=instrument:lfo {frequency=.0244792};local foundation_cv=instrument:affine {scale=.08,offset=.88}
instrument:connect(foundation_breath.out,foundation_cv.input)
local foundation_l=instrument:gain {gain=.62};local foundation_r=instrument:gain {gain=.62}
instrument:connect(foundation_drive.out,foundation_l.input);instrument:connect(foundation_drive.out,foundation_r.input)
instrument:connect(foundation_cv.out,foundation_l.mod);instrument:connect(foundation_cv.out,foundation_r.mod)

-- BODY: two continuously sounding interpretations of D2. Their spectral
-- cycles take 20 and 28 bars, while a subtle one-bar amplitude wave becomes
-- perceptible before the sequenced pulse confirms it.
local body_left=instrument:plaits {model="harmonic",frequency=73.416,harmonics=.3,timbre=.38,morph=.24}
local body_right=instrument:plaits {model="wavetable",frequency=73.57,harmonics=.34,timbre=.42,morph=.31}
local body_slow_a=instrument:lfo {frequency=.0195833};local body_slow_b=instrument:lfo {frequency=.0139881}
body_left:modulate("harmonics",body_slow_a,{depth=.17});body_left:modulate("timbre",body_slow_b,{depth=.14})
body_right:modulate("timbre",body_slow_a,{depth=.16});body_right:modulate("morph",body_slow_b,{depth=.2})
local body_filter_l=instrument:svf {cutoff=620,resonance=.38};local body_filter_r=instrument:svf {cutoff=680,resonance=.35}
body_filter_l:modulate("cutoff",body_slow_b,{depth=260});body_filter_r:modulate("cutoff",body_slow_a,{depth=310})
instrument:connect(body_left.main,body_filter_l.input);instrument:connect(body_right.main,body_filter_r.input)
local body_drive_l=instrument:drive {amount=1.7,output_gain=.62};local body_drive_r=instrument:drive {amount=1.7,output_gain=.62}
instrument:connect(body_filter_l.lowpass,body_drive_l.input);instrument:connect(body_filter_r.lowpass,body_drive_r.input)
local body_direct_l=instrument:gain {gain=.88};local body_direct_r=instrument:gain {gain=.88};local body_cross_l=instrument:gain {gain=.24};local body_cross_r=instrument:gain {gain=.24}
instrument:connect(body_drive_l.out,body_direct_l.input);instrument:connect(body_drive_r.out,body_direct_r.input)
instrument:connect(body_drive_r.out,body_cross_l.input);instrument:connect(body_drive_l.out,body_cross_r.input)
local body_mix_l=instrument:mixer {inputs=2};local body_mix_r=instrument:mixer {inputs=2}
instrument:connect(body_direct_l.out,body_mix_l.input[1]);instrument:connect(body_cross_l.out,body_mix_l.input[2]);instrument:connect(body_direct_r.out,body_mix_r.input[1]);instrument:connect(body_cross_r.out,body_mix_r.input[2])
local body_pulse=instrument:lfo {frequency=.3916667};local body_pulse_cv=instrument:affine {scale=.035,offset=.78}
instrument:connect(body_pulse.out,body_pulse_cv.input)
local body_level_l=instrument:gain {gain=.34};local body_level_r=instrument:gain {gain=.34}
instrument:connect(body_mix_l.out,body_level_l.input);instrument:connect(body_mix_r.out,body_level_r.input);instrument:connect(body_pulse_cv.out,body_level_l.mod);instrument:connect(body_pulse_cv.out,body_level_r.mod)

local body_space=instrument:stereo_delay {left_beats=1.5,right_beats=2.25,feedback=.34,crossfeed=.82,mix=1,filter_cutoff=2800,feedback_drive=1.25}
local delay_drift_l=instrument:lfo {frequency=.0122396};local delay_drift_r=instrument:lfo {frequency=.0195833}
body_space:modulate("left_time",delay_drift_l,{depth=.006});body_space:modulate("right_time",delay_drift_r,{depth=.009})
instrument:connect(sequence.tempo,body_space.bpm);instrument:connect(body_level_l.out,body_space.input.left);instrument:connect(body_level_r.out,body_space.input.right)
local body_wet_l=instrument:gain {gain=.28};local body_wet_r=instrument:gain {gain=.28};instrument:connect(body_space.left,body_wet_l.input);instrument:connect(body_space.right,body_wet_r.input)

-- AIR: filtered inharmonic grains and a very quiet audio-rate FM halo. A
-- three-beat breathing cycle moves against the body's four-beat cycle without
-- jitter; fixed oscillators keep offline and realtime renders deterministic.
local air_filter_l=instrument:svf {cutoff=2400,resonance=.62};local air_filter_r=instrument:svf {cutoff=3100,resonance=.58}
local air_slow_l=instrument:lfo {frequency=.0326389};local air_slow_r=instrument:lfo {frequency=.0244792}
local air_grain_a=instrument:oscillator {waveform="saw",frequency=997.13};local air_grain_b=instrument:oscillator {waveform="pulse",frequency=1481.7,pulse_width=.37};local air_fm=instrument:oscillator {waveform="sine",frequency=71}
air_grain_a:modulate("frequency",air_fm,{depth=43});air_grain_b:modulate("pulse_width",air_slow_l,{depth=.18})
local air_source=instrument:mixer {inputs=2};instrument:connect(air_grain_a.out,air_source.input[1]);instrument:connect(air_grain_b.out,air_source.input[2])
air_filter_l:modulate("cutoff",air_slow_l,{depth=950});air_filter_r:modulate("cutoff",air_slow_r,{depth=1200})
instrument:connect(air_source.out,air_filter_l.input);instrument:connect(air_source.out,air_filter_r.input)
local halo=instrument:oscillator {waveform="sine",frequency=1174.66};local halo_mod=instrument:oscillator {waveform="sine",frequency=67}
halo:modulate("frequency",halo_mod,{depth=135});local halo_filter=instrument:svf {cutoff=1700,resonance=.3};instrument:connect(halo.out,halo_filter.input)
local air_mix_l=instrument:mixer {inputs=2};local air_mix_r=instrument:mixer {inputs=2};local halo_l=instrument:gain {gain=.07};local halo_r=instrument:gain {gain=.05}
instrument:connect(halo_filter.highpass,halo_l.input);instrument:connect(halo_filter.highpass,halo_r.input)
instrument:connect(air_filter_l.bandpass,air_mix_l.input[1]);instrument:connect(halo_l.out,air_mix_l.input[2]);instrument:connect(air_filter_r.bandpass,air_mix_r.input[1]);instrument:connect(halo_r.out,air_mix_r.input[2])
local air_pulse=instrument:lfo {frequency=.5222222};local air_cv=instrument:affine {scale=.04,offset=.72};instrument:connect(air_pulse.out,air_cv.input)
local air_level_l=instrument:gain {gain=.16};local air_level_r=instrument:gain {gain=.16};instrument:connect(air_mix_l.out,air_level_l.input);instrument:connect(air_mix_r.out,air_level_r.input);instrument:connect(air_cv.out,air_level_l.mod);instrument:connect(air_cv.out,air_level_r.mod)
local air_space=instrument:stereo_delay {left_beats=.75,right_beats=1.25,feedback=.38,crossfeed=.9,mix=1,filter_cutoff=5200,feedback_drive=1.18}
instrument:connect(sequence.tempo,air_space.bpm);instrument:connect(air_level_l.out,air_space.input.left);instrument:connect(air_level_r.out,air_space.input.right)

-- Sparse rhythm and the only pitched landmark.
local low=instrument:plaits {model="bass_drum",frequency=48,harmonics=.34,timbre=.42,morph=.24};local low_gain_l=instrument:gain {gain=.46};local low_gain_r=instrument:gain {gain=.46}
instrument:connect(track["Low Pulse"].gate,low.gate);instrument:connect(track["Low Pulse"].velocity,low.velocity);instrument:connect(low.main,low_gain_l.input);instrument:connect(low.main,low_gain_r.input)
local dust=instrument:plaits {model="hihat",frequency=4200,harmonics=.28,timbre=.46,morph=.34};local dust_l=instrument:gain {gain=.2};local dust_r=instrument:gain {gain=.27}
instrument:connect(track.Dust.gate,dust.gate);instrument:connect(track.Dust.velocity,dust.velocity);instrument:connect(dust.main,dust_l.input);instrument:connect(dust.aux,dust_r.input)
local harmonic=instrument:plaits {model="string_machine",harmonics=.4,timbre=.38,morph=.28};local harmonic_env=instrument:adsr {attack=.08,decay=1.4,sustain=.5,release=2.6};local harmonic_amp=instrument:vca {};local harmonic_filter=instrument:svf {cutoff=1300,resonance=.38}
instrument:connect(track["Harmonic Gate"].pitch,harmonic.pitch);instrument:connect(track["Harmonic Gate"].gate,harmonic.gate);instrument:connect(track["Harmonic Gate"].velocity,harmonic.velocity);instrument:connect(track["Harmonic Gate"].gate,harmonic_env.gate);instrument:connect(harmonic.main,harmonic_amp.input);instrument:connect(harmonic_env.out,harmonic_amp.cv);instrument:connect(harmonic_amp.out,harmonic_filter.input)
local motif=instrument:plaits {model="phase_distortion",harmonics=.34,timbre=.42,morph=.3};local motif_env=instrument:adsr {attack=.025,decay=.6,sustain=.42,release=1.8};local motif_amp=instrument:vca {};local motif_filter=instrument:svf {cutoff=1900,resonance=.3}
instrument:connect(track["Three Notes"].pitch,motif.pitch);instrument:connect(track["Three Notes"].gate,motif.gate);instrument:connect(track["Three Notes"].velocity,motif.velocity);instrument:connect(track["Three Notes"].gate,motif_env.gate);instrument:connect(motif.main,motif_amp.input);instrument:connect(motif_env.out,motif_amp.cv);instrument:connect(motif_amp.out,motif_filter.input)
local motif_delay=instrument:stereo_delay {left_beats=.75,right_beats=1.5,feedback=.42,crossfeed=.76,mix=1,filter_cutoff=3400,feedback_drive=1.2};instrument:connect(sequence.tempo,motif_delay.bpm);instrument:connect(motif_filter.lowpass,motif_delay.input.left);instrument:connect(motif_filter.lowpass,motif_delay.input.right)
local motif_dry_l=instrument:gain {gain=.38};local motif_dry_r=instrument:gain {gain=.34};local motif_wet_l=instrument:gain {gain=.26};local motif_wet_r=instrument:gain {gain=.3}
instrument:connect(motif_filter.lowpass,motif_dry_l.input);instrument:connect(motif_filter.lowpass,motif_dry_r.input);instrument:connect(motif_delay.left,motif_wet_l.input);instrument:connect(motif_delay.right,motif_wet_r.input)

local live=instrument:plaits {model="harmonic",harmonics=.36,timbre=.4,morph=.3};local live_env=instrument:adsr {attack=.02,decay=.5,sustain=.46,release=1.8};local live_amp=instrument:vca {};local live_level=instrument:gain {gain=.24}
instrument:connect(instrument.note.pitch,live.pitch);instrument:connect(instrument.note.gate,live.gate);instrument:connect(instrument.note.velocity,live.velocity);instrument:connect(instrument.note.gate,live_env.gate);instrument:connect(live.main,live_amp.input);instrument:connect(live_env.out,live_amp.cv);instrument:connect(live_amp.out,live_level.input)

local master=instrument:mixer {inputs=8,headroom=.32}
instrument:connect(foundation_l.out,master.input[1].left);instrument:connect(foundation_r.out,master.input[1].right)
instrument:connect(body_level_l.out,master.input[2].left);instrument:connect(body_level_r.out,master.input[2].right);instrument:connect(body_wet_l.out,master.input[3].left);instrument:connect(body_wet_r.out,master.input[3].right)
instrument:connect(air_level_l.out,master.input[4].left);instrument:connect(air_level_r.out,master.input[4].right);instrument:connect(air_space.left,master.input[5].left);instrument:connect(air_space.right,master.input[5].right)
instrument:connect(low_gain_l.out,master.input[6].left);instrument:connect(low_gain_r.out,master.input[6].right);instrument:connect(dust_l.out,master.input[7].left);instrument:connect(dust_r.out,master.input[7].right)
instrument:connect(harmonic_filter.lowpass,master.input[8].left);instrument:connect(harmonic_filter.lowpass,master.input[8].right)
-- The motif and live voice enter through the body bus so they share its sense of space.
instrument:connect(motif_dry_l.out,body_level_l.input);instrument:connect(motif_dry_r.out,body_level_r.input);instrument:connect(motif_wet_l.out,body_wet_l.input);instrument:connect(motif_wet_r.out,body_wet_r.input);instrument:connect(live_level.out,body_level_l.input);instrument:connect(live_level.out,body_level_r.input)

local dc_l=instrument:svf {cutoff=22,resonance=.03};local dc_r=instrument:svf {cutoff=22,resonance=.03};instrument:connect(master.left,dc_l.input);instrument:connect(master.right,dc_r.input)
local glue=instrument:stereo_compressor {threshold=-18,ratio=1.8,attack=.035,release=.32,knee=7,makeup_gain=2,bypass=0};instrument:connect(dc_l.highpass,glue.input.left);instrument:connect(dc_r.highpass,glue.input.right)
local final_l=instrument:gain {gain=1};local final_r=instrument:gain {gain=1};instrument:connect(glue.left,final_l.input);instrument:connect(glue.right,final_r.input);instrument:connect(final_l.out,instrument.output.left);instrument:connect(final_r.out,instrument.output.right)

local function control(id,label,target,range,targets,curve) return instrument:control {id=id,label=label,target=target,targets=targets,range=range,curve=curve} end
local foundation_level=control("foundation_level","Foundation weight",foundation_l.gain,{0,.9},{foundation_r.gain})
local foundation_motion=control("foundation_motion","Foundation beating",detune_a.scale,{0,.24},{detune_b.scale})
local body_level=control("body_level","Body presence",body_level_l.gain,{0,.8},{body_level_r.gain})
local body_motion=control("body_motion","Breathing depth",body_pulse_cv.scale,{0,.28})
local body_color=control("body_color","Body harmonics",body_left.harmonics,{0,1},{body_right.timbre})
local body_spread=control("body_spread","Body crossfeed",body_cross_l.gain,{0,.7},{body_cross_r.gain})
local air_level=control("air_level","Air presence",air_level_l.gain,{0,.55},{air_level_r.gain})
local air_motion=control("air_motion","Air three-beat depth",air_cv.scale,{0,.3})
local space=control("space","Room memory",body_space.feedback,{0,.82},{air_space.feedback,motif_delay.feedback})
local motif_tone=control("motif_tone","Landmark color",motif_filter.cutoff,{420,6200},nil,"log")
local tension=control("tension","Body tension",body_drive_l.amount,{.2,7},{body_drive_r.amount})
local master_level=control("master_level","Master volume",master.headroom,{.06,.72})
local master_threshold=control("master_threshold","Glue threshold",glue.threshold,{-32,-5});local master_ratio=control("master_ratio","Glue ratio",glue.ratio,{1,6});local master_attack=control("master_attack","Glue attack",glue.attack,{.005,.15},nil,"log");local master_release=control("master_release","Glue release",glue.release,{.06,1.2},nil,"log");local ending_fade=control("ending_fade","Afterimage fade",final_l.gain,{0,1},{final_r.gain})

-- Different timescales deliberately cross phase; these points describe one
-- continuous process and do not merely initialize values at scene boundaries.
arrangement:automation {target=foundation_level,interpolation="smooth"}:at_bar(1,.54):at_bar(17,.64):at_bar(41,.58):at_bar(65,.7):at_bar(93,.5):at_bar(121,.68):at_bar(145,.48):at_bar(160,.22):at_bar(161,0)
arrangement:automation {target=foundation_motion,interpolation="smooth"}:at_bar(1,.025):at_bar(29,.06):at_bar(61,.1):at_bar(89,.2):at_bar(109,.23):at_bar(129,.08):at_bar(145,.045):at_bar(161,.015)
arrangement:automation {target=body_level,interpolation="smooth"}:at_bar(1,.12):at_bar(25,.24):at_bar(49,.42):at_bar(73,.58):at_bar(97,.66):at_bar(113,.74):at_bar(137,.56):at_bar(145,.32):at_bar(161,0)
arrangement:automation {target=body_motion,interpolation="smooth"}:at_bar(1,.018):at_bar(33,.045):at_bar(57,.11):at_bar(81,.2):at_bar(105,.26):at_bar(121,.14):at_bar(145,.055):at_bar(161,.01)
arrangement:automation {target=body_color,interpolation="smooth"}:at_bar(1,.22):at_bar(21,.38):at_bar(45,.28):at_bar(69,.62):at_bar(93,.82):at_bar(113,.46):at_bar(133,.72):at_bar(149,.3):at_bar(161,.18)
arrangement:automation {target=body_spread,interpolation="linear"}:at_bar(1,.38):at_bar(41,.25):at_bar(73,.12):at_bar(97,.46):at_bar(121,.18):at_bar(145,.4):at_bar(161,.28)
arrangement:automation {target=air_level,interpolation="smooth"}:at_bar(1,.055):at_bar(29,.12):at_bar(57,.24):at_bar(85,.42):at_bar(105,.5):at_bar(121,.38):at_bar(137,.46):at_bar(145,.18):at_bar(161,0)
arrangement:automation {target=air_motion,interpolation="smooth"}:at_bar(1,.025):at_bar(49,.07):at_bar(81,.18):at_bar(101,.26):at_bar(121,.1):at_bar(145,.045):at_bar(161,.01)
arrangement:automation {target=space,interpolation="smooth"}:at_bar(1,.24):at_bar(33,.38):at_bar(65,.52):at_bar(97,.7):at_bar(113,.46):at_bar(129,.62):at_bar(145,.5):at_bar(157,.68):at_bar(161,.22)
arrangement:automation {target=motif_tone,interpolation="smooth"}:at_bar(1,760):at_bar(25,1100):at_bar(49,1800):at_bar(81,3200):at_bar(105,5200):at_bar(113,1400):at_bar(137,3600):at_bar(145,820):at_bar(161,480)
arrangement:automation {target=tension,interpolation="smooth"}:at_bar(1,.8):at_bar(37,1.4):at_bar(69,2.3):at_bar(97,4.8):at_bar(109,5.6):at_bar(113,1.6):at_bar(133,2.8):at_bar(145,1.1):at_bar(161,.5)
arrangement:automation {target=ending_fade,interpolation="smooth"}:at_bar(1,1):at_bar(145,1):at_bar(153,.82):at_bar(157,.5):at_bar(160,.12):at_bar(161,0)

instrument:performance {layout="step_grid",sequencer=sequence,project_file="06_the_shape_of_standing_still.lua",
  keys={Space="transport",s="start",x="stop",r="restart",Up="track:up",Down="track:down",Left="step:previous",Right="step:next",["["]="bank:previous",["]"]="bank:next",m="mute",o="solo",j="control:next",k="control:previous",h="control:decrease",l="control:increase",["1"]="toggle:1",["2"]="toggle:2",["3"]="toggle:3",["4"]="toggle:4",["5"]="toggle:5",["6"]="toggle:6",["7"]="toggle:7",["8"]="toggle:8"},
  apc={pages={"steps","performance"},performance_rows={"scene","mute","solo","track","restart","clip","",""},scenes={"Static Field","Hidden Pulse","Recognized Motion","Phase Drift","Open Interior","Afterimage"},scene_quantization=16,
    faders={"foundation_level","foundation_motion","body_level","body_motion","body_color","air_level","air_motion","space"},fader_banks={{"foundation_level","foundation_motion","body_level","body_motion","body_color","air_level","air_motion","space"},{"body_spread","tension","motif_tone","master_threshold","master_ratio","master_attack","master_release","space"}},scene_buttons={"transport","restart","faders:bank:0","faders:bank:1","","","",""},master="master_level"},
  focus={controls={"foundation_level","foundation_motion","body_level","body_motion","body_color","body_spread","air_level","air_motion","space","motif_tone","tension"},midi_ccs={{channel=1,number=74},{channel=1,number=71},{channel=1,number=76},{channel=1,number=77}}}}

local composition=instrument:screen("composition","The Shape of Standing Still");composition:arrangement_status();composition:control_focus()
local clips=instrument:screen("clips","Pulse / Landmark Inspector");clips:pattern_grid();clips:control_focus()
local mastering=instrument:screen("master","Space / Master Glue");mastering:control("Foundation","foundation_level");mastering:control("Body","body_level");mastering:control("Air","air_level");mastering:control("Room memory","space");mastering:control("Threshold (dB)","master_threshold");mastering:control("Ratio","master_ratio");mastering:control("Attack (s)","master_attack");mastering:control("Release (s)","master_release")
local navigation=instrument:navigation {initial="composition",events={{name="composition",from="*",to="composition"},{name="clips",from="*",to="clips"},{name="master",from="*",to="master"}}};navigation:bind("F9","composition");navigation:bind("F10","clips");navigation:bind("F11","master")
sequence:start()
return instrument
