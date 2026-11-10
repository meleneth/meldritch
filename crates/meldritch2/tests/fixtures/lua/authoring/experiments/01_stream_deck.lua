local instrument = meldritch.instrument("Stream Deck Hardware Playground")

-- A tiny audible state makes the hardware experiment useful without making
-- the Stream Deck part of the signal graph.
local tone = instrument:oscillator { waveform="sine", frequency=220 }
local level = instrument:gain { gain=0 }
instrument:connect(tone.out, level.input)
instrument:connect(level.out, instrument.output.left)
instrument:connect(level.out, instrument.output.right)
instrument:control { id="tone", label="Tone", target=level.gain, range={0,.2} }
instrument:control { id="pitch", label="Pitch", target=tone.frequency, range={110,880}, curve="log" }

instrument:control_surface {
  id="stream_deck_playground", device="stream_deck_xl", initial_page="main",
  pages={
    { id="main", buttons={
      { id="tone_toggle", index=0, label="TONE", icon="circle", mode="toggle",
        color={20,20,28}, active_color={0,150,80},
        press={control="tone",value=.12}, release={control="tone",value=0} },
      { id="tone_hit", index=1, label="HOLD", icon="triangle", mode="momentary",
        color={30,18,18}, active_color={220,70,30},
        press={control="tone",value=.2}, release={control="tone",value=0} },
      { id="high", index=2, label="HIGH", mode="toggle",
        color={18,20,36}, active_color={60,100,230},
        press={control="pitch",value=440}, release={control="pitch",value=220} },
      { id="to_colors", index=31, label="COLORS", color={35,20,60},
        active_color={100,40,180}, press={page="colors"} },
    }},
    { id="colors", buttons={
      { id="red", index=0, label="RED", mode="toggle", color={55,8,8},
        active_color={230,25,25}, press={refresh=true}, release={refresh=true} },
      { id="green", index=1, label="GREEN", mode="toggle", color={8,45,12},
        active_color={20,220,60}, press={refresh=true}, release={refresh=true} },
      { id="blue", index=2, label="BLUE", mode="toggle", color={8,15,55},
        active_color={30,80,230}, press={refresh=true}, release={refresh=true} },
      { id="to_main", index=31, label="MAIN", color={30,30,30},
        active_color={100,100,100}, press={page="main"} },
    }},
  },
}

return instrument
