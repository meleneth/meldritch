sr = 48000
ksmps = 32
nchnls = 1
0dbfs = 1

instr Kick
  iDur = p3
  aEnv expon 1, iDur, 0.001
  kPitch expon 115, 0.08, 42
  aSig poscil 0.95 * aEnv, kPitch
  aClick expon 1, 0.012, 0.001
  out tanh(aSig * 2.4 + rand:a(0.08) * aClick)
endin

instr Snare
  iDur = p3
  aBodyEnv expon 1, 0.18, 0.001
  aNoiseEnv expon 1, iDur, 0.001
  aBody poscil 0.32 * aBodyEnv, 185
  aNoise butterhp rand:a(0.55), 1600
  out tanh((aBody + aNoise * aNoiseEnv) * 1.8)
endin

instr Hat
  iDur = p3
  aEnv expon 1, iDur, 0.001
  aNoise butterhp rand:a(0.45), 6500
  out aNoise * aEnv
endin

instr Sub
  iDur = p3
  aEnv linsegr 0, 0.01, 1, iDur - 0.03, 0.8, 0.02, 0
  aSig poscil 0.75 * aEnv, 55
  out tanh(aSig * 1.2)
endin
