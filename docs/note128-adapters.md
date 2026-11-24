# Note/128 adapters

The musical storage convention for a discrete note is **MIDI note number / 128**.
Thus 0/128 is note zero, 69/128 is A4, and 127/128 is note 127.
All 128 values are exact in binary floating point. **1.0 is not a legal
note/128 code**; conversions saturate it to MIDI note 127 rather than wrapping.

Every adapter is a regular, allocation-free Rust contract module with one
numeric `input` and one numeric `out`:

| Component | Input | Output |
| --- | --- | --- |
| `note128_to_midi` | note/128 | MIDI note integer 0..127 as f64 |
| `midi_to_note128` | MIDI note integer | note/128 |
| `note128_to_hz` | note/128 | Hz, using A4 = 440 Hz and 12-TET |
| `hz_to_note128` | Hz | nearest MIDI note/128 |

Mappings quantize to nearest integer note with Rust's `round` and clamp to
0..127. Invalid floating inputs produce finite fallback values.

The Lua graph exposes `graph:note128_to_hz(id, source_port)`,
`graph:hz_to_note128(id, source_port)`, and the two MIDI-number equivalents.
These are wiring helpers that instantiate ordinary adapter modules. For
instance:

```lua
local notes = graph:value_sequencer {
  id = "notes", values = {60/128, 64/128, 67/128, 72/128}
}
local typed_note = graph:as_note128("as_note", notes.value)
local pitch_hz = graph:note128_to_hz("to_hz", typed_note)
graph:connect(pitch_hz, synth.pitch)
```

No ValueSequencer implementation is aware of MIDI note numbers, Hz, or VST3.

## Type safety boundary

The existing graph compiler validates **port domains**, fan-in, module kinds,
parameter names/ranges, and connection endpoints at preparation time. It also distinguishes the `Note128`, `MidiNote`, and `FrequencyHz`
port domains and rejects mismatched connections. Use `as_note128` to
explicitly interpret a generic numeric source, then a conversion adapter.
A VST3 pitch input is `FrequencyHz`, so it rejects generic or note/128
values until explicitly adapted. A legacy generic Hz source can be
declared with `as_frequency_hz`. A generic `MonoAudio` sink can observe
a typed numeric signal, but a generic source cannot enter a typed input.
The compiler erases these semantic types when it constructs the f64
execution plan.

The VST3 host's existing `pitch` port expects Hz, while `gate` and
`velocity` use their existing numeric semantics. The adapters do not emit
MIDI NoteOn/NoteOff events: they transform sample values. The VST3 host
continues to translate Hz/gate to its own MIDI events.

## Rust values are not interchangeable

The shared interface crate now has dedicated `Note128`, `MidiNote`
and `FrequencyHz` newtypes. They validate their payloads on construction;
`MidiNote::new(69)` and `Note128::new(69.0 / 128.0)` represent the
same pitch without being assignment-compatible Rust types.

`From<MidiNote> for Note128` and `TryFrom<Note128> for MidiNote`
provide explicit conversion operations. Rust does not invoke these traits
implicitly during assignment or function argument passing. The normal
DSP signal storage remains `f64` and the graph's port metadata is
validated before the executable machine is prepared.
