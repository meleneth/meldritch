# Edit an existing synth

Status: `design`. [acceptance.feature](acceptance.feature) specifies the future
automated tests; its step bindings and authoring runtime are unimplemented.
Baseline: `../02-synth-parameter-pattern`; inspect the loaded patch rather than
the showcase bass settings. The shared execution/persistence rules are in
[AUTHORING.md](../../../AUTHORING.md#acceptance-execution-contract).

The TUI must show module identity/type, current source defaults, parameter
units/ranges and explicit cables. Edit low-pass cutoff and ADSR release while
playing, with the parameter automation disabled through the example setup so
the authored cutoff change is measurable. All surfaces show the same source
revision. Changed sound becomes available through worker publication, and
callback consumption is reported separately. No direct callback mutation.

Persist absolute parameter values and stable synth/module IDs as readable Lua
edits over the validated base, not a temporary curated override or key history.
Reload without session replay reconstructs the edited defaults and equal audio.
Reject unknown modules, nonfinite cutoff and negative release atomically.
No new engine, arbitrary modulation graph or complete editor framework is needed.
