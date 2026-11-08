export interface PatternNote {
  id: number;
  start: number;
  duration: number;
  pitch: number;
  velocity: number;
  legato?: boolean;
}

export interface PatternModel {
  length: number;
  resolution: number;
  notes: PatternNote[];
}

export type ScaleMode =
  | "chromatic"
  | "major"
  | "minor"
  | "dorian"
  | "mixolydian"
  | "pentatonic";

const intervals: Record<ScaleMode, number[]> = {
  chromatic: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
  major: [0, 2, 4, 5, 7, 9, 11],
  minor: [0, 2, 3, 5, 7, 8, 10],
  dorian: [0, 2, 3, 5, 7, 9, 10],
  mixolydian: [0, 2, 4, 5, 7, 9, 10],
  pentatonic: [0, 2, 4, 7, 9],
};

export interface ScaleLock {
  tonic: number;
  mode: ScaleMode;
  chordRoot?: number;
  pitchMin: number;
  pitchMax: number;
}

const clamp = (value: number, min: number, max: number) =>
  Math.min(max, Math.max(min, value));

export function snapPitch(pitch: number, lock: ScaleLock): number {
  const rounded = clamp(Math.round(pitch), lock.pitchMin, lock.pitchMax);
  if (lock.mode === "chromatic") return rounded;
  const root = (((lock.tonic + (lock.chordRoot ?? 0)) % 12) + 12) % 12;
  const allowed = intervals[lock.mode];
  for (let distance = 0; distance <= 12; distance++) {
    for (const candidate of [rounded - distance, rounded + distance]) {
      if (
        candidate >= lock.pitchMin &&
        candidate <= lock.pitchMax &&
        allowed.includes((((candidate - root) % 12) + 12) % 12)
      )
        return candidate;
    }
  }
  return rounded;
}

export function normalizeNote(
  note: PatternNote,
  pattern: PatternModel,
): PatternNote {
  const start = clamp(
    note.start,
    0,
    Math.max(0, pattern.length - pattern.resolution),
  );
  return {
    ...note,
    start,
    duration: clamp(note.duration, pattern.resolution, pattern.length - start),
    pitch: clamp(Math.round(note.pitch), 0, 127),
    velocity: clamp(Math.round(note.velocity), 1, 127),
  };
}

export function createNote(
  pattern: PatternModel,
  note: PatternNote,
  lock?: ScaleLock,
): PatternModel {
  const value = normalizeNote(
    { ...note, pitch: lock ? snapPitch(note.pitch, lock) : note.pitch },
    pattern,
  );
  return { ...pattern, notes: [...pattern.notes, value] };
}

export function updateNotes(
  pattern: PatternModel,
  ids: ReadonlySet<number>,
  update: (note: PatternNote) => PatternNote,
  lock?: ScaleLock,
): PatternModel {
  return {
    ...pattern,
    notes: pattern.notes.map((note) =>
      ids.has(note.id)
        ? normalizeNote(
            lock
              ? { ...update(note), pitch: snapPitch(update(note).pitch, lock) }
              : update(note),
            pattern,
          )
        : note,
    ),
  };
}

export const deleteNotes = (
  pattern: PatternModel,
  ids: ReadonlySet<number>,
) => ({
  ...pattern,
  notes: pattern.notes.filter((note) => !ids.has(note.id)),
});

export const transpose = (pattern: PatternModel, semitones: number) => ({
  ...pattern,
  notes: pattern.notes.map((note) => ({
    ...note,
    pitch: clamp(note.pitch + semitones, 0, 127),
  })),
});

export const shiftTime = (pattern: PatternModel, amount: number) => ({
  ...pattern,
  notes: pattern.notes.map((note) =>
    normalizeNote({ ...note, start: note.start + amount }, pattern),
  ),
});

export const scaleTime = (pattern: PatternModel, factor: number) => ({
  ...pattern,
  notes: pattern.notes.map((note) =>
    normalizeNote(
      { ...note, start: note.start * factor, duration: note.duration * factor },
      pattern,
    ),
  ),
});

export const invertPitch = (pattern: PatternModel, pivot = 60) => ({
  ...pattern,
  notes: pattern.notes.map((note) => ({
    ...note,
    pitch: clamp(Math.round(pivot * 2 - note.pitch), 0, 127),
  })),
});

export const rotatePattern = (pattern: PatternModel, amount: number) => ({
  ...pattern,
  notes: pattern.notes.map((note) => ({
    ...note,
    start:
      (((note.start + amount) % pattern.length) + pattern.length) %
      pattern.length,
  })),
});

export function quantizePattern(
  pattern: PatternModel,
  grid = pattern.resolution,
) {
  return {
    ...pattern,
    notes: pattern.notes.map((note) =>
      normalizeNote(
        {
          ...note,
          start: Math.round(note.start / grid) * grid,
          duration: Math.max(grid, Math.round(note.duration / grid) * grid),
        },
        pattern,
      ),
    ),
  };
}

/**
 * IDs common to both patterns interpolate directly. Unmatched notes pair in
 * stable (start, pitch, id) order and retain the left ID; surplus notes remain
 * present, fading velocity toward/from 1. Notes are never silently discarded.
 */
export function interpolatePatterns(
  left: PatternModel,
  right: PatternModel,
  amount: number,
): PatternModel {
  const t = clamp(amount, 0, 1);
  const byId = new Map(right.notes.map((note) => [note.id, note]));
  const used = new Set<number>();
  const remainingRight = () =>
    right.notes
      .filter(
        (note) =>
          !used.has(note.id) && !left.notes.some((n) => n.id === note.id),
      )
      .sort((a, b) => a.start - b.start || a.pitch - b.pitch || a.id - b.id);
  const unmatchedLeft = left.notes
    .filter((note) => !byId.has(note.id))
    .sort((a, b) => a.start - b.start || a.pitch - b.pitch || a.id - b.id);
  const pair = new Map(
    unmatchedLeft.map((note, index) => [note.id, remainingRight()[index]]),
  );
  const mix = (a: number, b: number) => a + (b - a) * t;
  const notes = left.notes.map((note) => {
    const target = byId.get(note.id) ?? pair.get(note.id);
    if (!target)
      return {
        ...note,
        velocity: Math.max(1, Math.round(mix(note.velocity, 1))),
      };
    used.add(target.id);
    return {
      ...note,
      start: mix(note.start, target.start),
      duration: Math.max(left.resolution, mix(note.duration, target.duration)),
      pitch: Math.round(mix(note.pitch, target.pitch)),
      velocity: Math.max(1, Math.round(mix(note.velocity, target.velocity))),
    };
  });
  for (const note of right.notes.filter(
    (candidate) =>
      !used.has(candidate.id) &&
      !left.notes.some((leftNote) => leftNote.id === candidate.id),
  ))
    notes.push({
      ...note,
      velocity: Math.max(1, Math.round(mix(1, note.velocity))),
    });
  return { ...left, notes };
}
