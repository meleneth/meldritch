# Lua APC Mini MK2 piano roll

The first piano-roll slice creates and edits a synth-backed note pattern using
the existing note compiler, background worker and audio publication. Rust owns
the validated song source and stable note identities; Lua owns grid orientation,
pages, holds, modifiers and semantic LED presentation. The TUI reads the same
pattern and selected note. No new engine or DAW editor framework is introduced.

```sh
cargo run -- tui-song songs/examples/26-apc-note-pattern --lua main.lua
```

This opens the ordinary song host and declared APC Control input/output. Playback
autostarts; the seed is deliberately blank. Press **NEW** (MIDI note 116) to
create and assign `roll`, a two-bar looping pattern, or author the loaded seed
directly. NEW creates once and subsequently returns to time page 0 without
erasing notes. Two bars give a small example of crossing an eight-step page.

The Session grid has eight time columns and eight chromatic pitches, bottom to
top. The initial bottom row is C3, the top row G3. Each column is an eighth note
(480 ticks at the loader's 960 PPQ). These choices live in `main.lua`.

- Press an empty pad to create a note with velocity 0.8 and one-step duration.
- Press a note start to select it. It stays present when released.
- Hold its start and press another pad on the same pitch row to set its end
  through that endpoint step. Release either pad first; releases never edit.
  A newly created start can also be held immediately for extension.
- Press a sustained body to select its note without starting a duration gesture.
  Invalid earlier or cross-row endpoints are consumed without creating notes.

| Control (MIDI note) | Example policy |
| --- | --- |
| 106 / 107 | Previous / next eight-step time page |
| 104 / 105 | Shift the pitch range down / up eight semitones |
| Hold 112, press a note | Delete that note |
| Hold 113 / 114, press a note | Velocity −0.1 / +0.1, bounded to [0,1] |
| 115 | Save the selected pattern as a new Lua definition |
| 116 | Create the initial two-bar pattern; never overwrite an existing pattern |

These MIDI addresses name the script-declared controls. Their physical labels,
orientation and gestures still need observation on the connected APC. Rust's
device adapter validates pad/button addresses; it does not choose their musical
meaning. Buttons use on/off feedback; the grid uses semantic RGB states:
green starts, blue bodies, yellow selected notes, red overlapping bodies, white
notes under the playhead and gray empty playhead cells. The playhead takes visual
priority for its current column. Other columns retain starts/bodies/selection.

Press **Ctrl-N** to open/close the native pattern inspector, **Ctrl-S** to save,
**Ctrl-E** to inspect the instrument, p to play/stop, r to rewind and q to quit.
Note edits automatically show the inspector. It displays pattern/track identity,
selected note, pitch, onset, exclusive end, velocity, source fingerprint and
pending/published audio. It keeps a long selection visible by scrolling its
note list. Grid gestures belong to Lua; native TUI keyboard input in this slice
inspects/saves/transports and does not modify the legacy backing step grid.
The logical `terminal` surface in headless tests uses the same Lua grid policy
and projects the same note labels/colors as the APC.

## Duration and interruption semantics

Rust stores positive durations with **exclusive ends**. An endpoint pad chooses
an **inclusive step**: start 0 plus endpoint 3 gives `[0,4)` steps, duration 1920
ticks / 48000 frames at 120 BPM and 48 kHz. It is one note with one gate, followed
by the existing ADSR release; body pads do not retrigger it.

Adjacent notes, such as `[0,4)` and `[4,5)`, are distinct starts with separate
identities. Release tails and overlaps follow the existing voice/polyphony
semantics. Overlapping notes remain separate events, including same-pitch notes;
the grid prefers actual starts, otherwise the lowest lexical note ID covering
the cell. Overlap labels and red body cells expose multiple events. Exact
coincident starts can be inspected/edited by ID through typed commands; the
initial hardware policy selects the lowest ID and does not cycle them.

Time navigation retains a valid held start, so hold a start on page 0, navigate
to page 1 and select an endpoint there. Durations do not wrap or tie implicitly
at a pattern boundary. The endpoint must remain within the pattern. A physical
pad already held cannot be a second simultaneous endpoint on another page;
choose another column or release and start a fresh gesture.

Pitch changes, changed/deleted selections and pattern replacement interrupt
the gesture; the original start must be released before new grid edits.
Disconnect and explicit overflow cancellation clear holds/modifiers. Queued pad
observations are discarded on overflow and late releases make no edits. Duplicate
note-ons are suppressed while a pad is down. Velocity-zero note-on and note-off
are both releases. Gestures use ordered observations and musical coordinates,
not a monotonically advancing playback position, so rewind/loop does not reject
physical releases or endpoints. No held/modifier state is persisted.

## Ownership, commands and persistence

The additive Lua composition v1 request vocabulary is `create_note_pattern`
(track, ID, length in ticks, loop flag), `edit_note` (pattern, typed operation),
`select_note` (pattern, note ID) and `save_pattern`. Note operations are `insert`,
`duration`, `velocity`, and `delete`. The `pattern` dependency supplies a deeply
read-only projection with authored notes, identity allocation counter, selected
note, fingerprint and playhead tick. Lua may retain transient gestures, never a
second note collection. Eventful widgets explicitly declare `events = true`;
`interact(view,event)` receives press/release/cancel and the cell/source.
`present(view,surface,state,cell)` receives the presentation address. Existing
press-only widgets retain their behavior and are not invoked on release/cancel.

Inserts allocate stable `note-N` identities. Deletion never reuses them, and the
next identity counter survives reload. All edits validate ownership, pitch,
velocity, positive duration and pattern boundaries, then compile before source
acceptance. Dirty ranges begin at the first affected onset and extend through
the bounded song horizon because voice allocation, filters, tails and later
loops can depend on the edit. The current worker still renders whole immutable
snapshots; it publishes only the latest generation while playback continues.
Publication and callback consumption are separate from physical sound.

Saving writes a unique, atomic `authored/pattern-*.lua` and reports its path.
It preserves TOML, controller scripts, previous exports and
`performances/*.mlperformance` journals. Failures retain edits. The explicit
version-1 definition returns `kind = "pattern"`, `version`, `base_fingerprint`
and a `pattern` containing stable ID/track, length/loop, identity counter and
notes with readable pitch names, absolute musical ticks and `f64` velocities.
See the checked-in `edited-pattern.lua` in example 26.

The base is the unchanged song root's TOML entry point and references. Repeated
exports have identical text. Missing/changed bases, unknown versions/fields,
duplicate IDs, bad counters, unsupported ownership/sample events, invalid note
types/ranges or times not losslessly representable at 960 PPQ reject. Creation
and assignment currently require synth tracks without performance lane banks.
Existing patterns need a single synth-track owner. A selected-pattern export
rejects unrelated authored source changes; composed instrument/pattern exports,
multiple new patterns per export and lane-bank authoring remain future work.

```sh
cargo run -- validate-song songs/examples/26-apc-note-pattern --pattern edited-pattern.lua
cargo run -- render-pattern songs/examples/26-apc-note-pattern edited-pattern.lua --output /tmp/piano-roll.wav
cargo run -- tui-song songs/examples/26-apc-note-pattern --lua main.lua --pattern edited-pattern.lua
```

Use the saved `authored/...lua` relative path instead of the checked-in example.
Reload independently validates and compiles the pattern without journal replay,
and retains the normal TOML entrypoint when no `--pattern` is supplied.

## Executable headless acceptance

```sh
cargo test -p meldritch-control --test piano_roll
cargo run -p meldritch-control --example piano_roll
cargo run -p meldritch-control --example piano_roll -- songs/examples/27-held-pad-duration/interrupted.pads
```

The scripts run the actual Lua policy, MIDI packet decoder, app commands,
performance capture, worker and publication in a temporary fixture copy. They
check creation, extension, page navigation, modifiers, release ordering,
interruptions and deterministic independent reload/audio. The runner prints
readable Lua to stdout and its acceptance result to stderr, then cleans up the
temporary copy. Tests also compare APC/logical TUI output, native inspection,
actual callback consumption, rejection atomicity and historical compatibility.
No physical pad/LED/hold/listening acceptance is claimed. The next iteration
should follow observation of these gestures on the connected APC.
