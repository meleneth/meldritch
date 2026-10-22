local instrument = meldritch.instrument("Delay Feedback Laboratory")

local source = instrument:oscillator { waveform="pulse", frequency=110, pulse_width=.3 }
local envelope = instrument:adsr { attack=.001, decay=.035, sustain=0, release=.025 }
local gate = instrument:constant { value=0 }
local amp = instrument:vca {}
instrument:connect(gate.out, envelope.gate)
instrument:connect(source.out, amp.input)
instrument:connect(envelope.out, amp.cv)

local modulation = instrument:lfo { waveform="sine", frequency=.17 }
local mod_depth = instrument:constant { value=0 }
local mod_scale = instrument:multiply {}
instrument:connect(modulation.out, mod_scale.a)
instrument:connect(mod_depth.out, mod_scale.b)

local delay = instrument:stereo_delay { left_beats=.5, right_beats=.75,
  feedback=.68, crossfeed=.72, mix=.52, filter_cutoff=5200, feedback_drive=1.4 }
instrument:connect(amp.out, delay.input.left)
instrument:connect(amp.out, delay.input.right)
instrument:connect(mod_scale.out, delay.left_time_mod)
instrument:connect(mod_scale.out, delay.right_time_mod)
instrument:connect(delay.left, instrument.output.left)
instrument:connect(delay.right, instrument.output.right)

instrument:control { id="impulse", label="Impulse", target=gate.value, range={0,1} }
instrument:control { id="left_time", label="Left time", target=delay.left_beats, range={.0625,4} }
instrument:control { id="right_time", label="Right time", target=delay.right_beats, range={.0625,4} }
instrument:control { id="feedback", label="Feedback", target=delay.feedback, range={0,.94} }
instrument:control { id="feedback_drive", label="Feedback drive", target=delay.feedback_drive, range={.1,8} }
instrument:control { id="filter", label="Feedback filter", target=delay.filter_cutoff, range={80,16000}, curve="log" }
instrument:control { id="pingpong", label="Ping pong", target=delay.crossfeed, range={0,.95} }
instrument:control { id="modulation", label="Delay modulation", target=mod_depth.value, range={0,.2} }

instrument:control_surface {
  id="delay_feedback_deck", device="stream_deck_xl", initial_page="play",
  pages={
    { id="play", buttons={
      { id="hit", index=0, label="IMPULSE", icon="triangle", mode="momentary", color={45,12,8},
        active_color={255,90,25}, press={control="impulse",value=1}, release={control="impulse",value=0} },
      { id="feedback_low", index=1, label="DECAY", color={15,35,30}, active_color={25,180,110},
        press={control="feedback",value=.48} },
      { id="feedback_high", index=2, label="BUILD", color={45,22,8}, active_color={230,100,20},
        press={control="feedback",value=.9} },
      { id="pingpong", index=3, label="PINGPONG", icon="bars", mode="toggle", color={15,18,40},
        active_color={45,90,230}, press={control="pingpong",value=.78}, release={control="pingpong",value=0} },
      { id="modulation", index=4, label="MOD", mode="toggle", color={28,15,38},
        active_color={170,50,210}, press={control="modulation",value=.08}, release={control="modulation",value=0} },
      { id="times", index=31, label="TIMES", color={25,25,35}, active_color={80,80,130}, press={page="times"} },
    }},
    { id="times", buttons={
      { id="short_left", index=0, label="L 1/4", color={10,35,45}, press={control="left_time",value=.25} },
      { id="long_left", index=1, label="L 1", color={10,35,45}, press={control="left_time",value=1} },
      { id="short_right", index=8, label="R 3/8", color={35,15,45}, press={control="right_time",value=.375} },
      { id="long_right", index=9, label="R 3/2", color={35,15,45}, press={control="right_time",value=1.5} },
      { id="dark", index=16, label="DARK", color={35,20,8}, press={control="filter",value=1400} },
      { id="bright", index=17, label="BRIGHT", color={45,38,8}, press={control="filter",value=9000} },
      { id="clean", index=24, label="CLEAN", color={12,25,30}, press={control="feedback_drive",value=.8} },
      { id="drive", index=25, label="DRIVE", color={50,15,8}, press={control="feedback_drive",value=4} },
      { id="back", index=31, label="PLAY", color={25,25,35}, active_color={80,80,130}, press={page="play"} },
    }},
  },
}

return instrument
