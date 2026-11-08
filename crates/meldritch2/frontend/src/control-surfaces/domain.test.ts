import { describe, expect, it } from "vitest";
import { mapNormalized, unmapValue } from "./mapping";
import {
  constrainEnvelope,
  mapEnvelopeValue,
  normalizeEnvelopeValue,
  type EnvelopeRanges,
} from "./envelope";
import {
  createNote,
  deleteNotes,
  interpolatePatterns,
  quantizePattern,
  rotatePattern,
  snapPitch,
  transpose,
  updateNotes,
  type PatternModel,
} from "./pattern";

describe("XY mappings", () => {
  it("maps linear, inverted, snapped, log, and bipolar ranges", () => {
    expect(mapNormalized(0.25, { target: "x", min: 0, max: 100 })).toBe(25);
    expect(mapNormalized(0.25, { target: "x", min: 100, max: 0 })).toBe(75);
    expect(
      mapNormalized(0.24, { target: "x", min: 0, max: 10, quantize: 1 }),
    ).toBe(2);
    expect(
      mapNormalized(0.5, { target: "x", min: 20, max: 20000, scale: "log" }),
    ).toBeCloseTo(Math.sqrt(20 * 20000));
    expect(
      mapNormalized(0.5, {
        target: "x",
        min: -2,
        max: 6,
        scale: "bipolar",
        center: 0,
      }),
    ).toBe(0);
    const mapping = {
      target: "x",
      min: 1,
      max: 100,
      scale: "log" as const,
      invert: true,
    };
    expect(unmapValue(mapNormalized(0.3, mapping), mapping)).toBeCloseTo(0.3);
  });
  it("rejects invalid logarithmic ranges", () => {
    expect(() =>
      mapNormalized(0.5, { target: "x", min: 0, max: 1, scale: "log" }),
    ).toThrow();
  });
});

describe("pattern data operations", () => {
  const pattern: PatternModel = {
    length: 16,
    resolution: 1,
    notes: [
      { id: 1, start: 0, duration: 4, pitch: 60, velocity: 100 },
      { id: 2, start: 0, duration: 2, pitch: 64, velocity: 90 },
    ],
  };
  it("creates, moves, resizes, deletes, and preserves polyphony", () => {
    const created = createNote(pattern, {
      id: 3,
      start: 0,
      duration: 3,
      pitch: 67,
      velocity: 80,
    });
    expect(created.notes.filter((note) => note.start === 0)).toHaveLength(3);
    const edited = updateNotes(created, new Set([3]), (note) => ({
      ...note,
      start: 5,
      duration: 6,
    }));
    expect(edited.notes.find((note) => note.id === 3)).toMatchObject({
      start: 5,
      duration: 6,
    });
    expect(
      deleteNotes(edited, new Set([2])).notes.map((note) => note.id),
    ).toEqual([1, 3]);
  });
  it("locks new pitches without rewriting existing out-of-scale notes", () => {
    const lock = {
      tonic: 0,
      mode: "major" as const,
      pitchMin: 48,
      pitchMax: 72,
    };
    expect(snapPitch(61, lock)).toBe(60);
    const result = createNote(
      pattern,
      { id: 3, start: 2, duration: 1, pitch: 61, velocity: 70 },
      lock,
    );
    expect(result.notes[0].pitch).toBe(60);
    expect(result.notes[2].pitch).toBe(60);
  });
  it("transforms deterministically without depending on zoom", () => {
    expect(transpose(pattern, 12).notes[0].pitch).toBe(72);
    expect(rotatePattern(pattern, -1).notes[0].start).toBe(15);
    expect(
      quantizePattern({
        ...pattern,
        notes: [{ ...pattern.notes[0], start: 2.6 }],
      }).notes[0].start,
    ).toBe(3);
  });
  it("interpolates mismatched note counts without discarding notes", () => {
    const right: PatternModel = {
      ...pattern,
      notes: [
        { id: 1, start: 8, duration: 2, pitch: 72, velocity: 80 },
        { id: 7, start: 4, duration: 1, pitch: 55, velocity: 60 },
        { id: 8, start: 12, duration: 2, pitch: 67, velocity: 100 },
      ],
    };
    const halfway = interpolatePatterns(pattern, right, 0.5);
    expect(halfway.notes).toHaveLength(3);
    expect(halfway.notes.find((note) => note.id === 1)).toMatchObject({
      start: 4,
      pitch: 66,
    });
  });
});

describe("ADSR data", () => {
  const ranges: EnvelopeRanges = {
    attack: { min: 0.001, max: 5, scale: "log" },
    decay: { min: 0.001, max: 5, scale: "log" },
    sustain: { min: 0, max: 1, scale: "linear" },
    release: { min: 0.001, max: 10, scale: "log" },
  };
  it("keeps durations and sustain level independently constrained", () => {
    expect(
      constrainEnvelope(
        { attack: -1, decay: 9, sustain: 2, release: 0 },
        ranges,
      ),
    ).toEqual({ attack: 0.001, decay: 5, sustain: 1, release: 0.001 });
    const time = mapEnvelopeValue(0.5, ranges.attack);
    expect(normalizeEnvelopeValue(time, ranges.attack)).toBeCloseTo(0.5);
    expect(mapEnvelopeValue(0.25, ranges.sustain)).toBe(0.25);
  });
});
