# 09 — Curated Performance Controls

This separates the intentionally playable surface from the complete patch.

It should:

- load one curated `Echo Feedback` control from `main.mlperformance`
- resolve the control to `dsp:echo/delay.feedback`
- start the cockpit in performance mode
- show the control's label, binding, range, step, and current value
- hide the pattern grid, diagnostics, and complete parameter tree by default
- switch to all-parameters mode with `Ctrl-Tab`
- switch back without changing transport, selection, or published audio state
