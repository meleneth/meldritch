# Edit duration with a held pad

Status: `design`; no release-aware Lua authoring interaction exists yet. Extend
example 26, using [shared rules](../../../AUTHORING.md#acceptance-execution-contract).

Hold an occupied onset pad; pressing another pad on the same pitch row chooses
an inclusive endpoint. With an eighth-note grid, holding C3 step 0 and pressing
step 3 sets a half-note duration (four steps). Commit on the endpoint press,
once; release only clears the transient hold. This gesture changes one event,
not four retriggers and not the legacy step gate. Plain short press retains the
example 26 toggle after release if no endpoint was chosen. Note-on-zero is a
release. The equivalent TUI onset/endpoint selection produces identical music.

Reject earlier/cross-row endpoints and ends beyond the bar without editing.
No implicit wrap/tie schema. Disconnect, overflow, page/pattern switch or deleted
selection cancels the hold; later releases cannot edit a different event. Rewind
and playback loop must not make interaction ordering invalid.

Persist onset, pitch and explicit musical duration only. Reload gates for the
chosen duration with ADSR release afterward; transport/held buttons are absent.
Physical acceptance separately confirms hold/second press/release, LED/TUI
span feedback and a sustained sound without intermediate attacks.
