# Bind faders to synth and mixer parameters

Status: `design`. Baseline: the existing levels/filters in
`../22-midi-soft-takeover`; new binding authoring is unimplemented. Use
[shared rules](../../../AUTHORING.md#acceptance-execution-contract).

In the TUI choose a declared APC device/channel/CC, inspect target units/range,
and assign fader 1 (CC48) to synth low-pass cutoff, 80..6000 Hz, and fader 2
(CC49) to the selected lane level, 0..1. Each has a linear mapping in this
example. Bindings must be authored data. Show the mapping and pickup state;
another surface's edit or remapping rearms pickup. Distant observations create
no edit until crossing/near-target acquisition. Accepted fader edits update
shared source values, render and record once.

Persist device binding, target identity, range/curve and latest authored cutoff/
lane level as Lua. Reload reconstructs mappings without hardware. A zero lane
level silences that lane; unity retains the existing mixed-render normalization.
Pan and buses are explicitly future capabilities, not invented targets here.
Ambiguous same-context bindings, nonexistent targets and invalid ranges must
diagnose and leave old mappings intact. Physical testing must verify both
faders, pickup, remapping, feedback and sound independently of headless tests.
