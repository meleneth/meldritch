import type { Scale } from "../types/protocol";

export interface EnvelopeValues {
  attack: number;
  decay: number;
  sustain: number;
  release: number;
}

export interface EnvelopeRange {
  min: number;
  max: number;
  scale: Scale;
}

export type EnvelopeRanges = Record<keyof EnvelopeValues, EnvelopeRange>;

const clamp = (value: number, min: number, max: number) =>
  Math.min(max, Math.max(min, value));

export function mapEnvelopeValue(normalized: number, range: EnvelopeRange) {
  const value = clamp(normalized, 0, 1);
  if (range.scale === "log") {
    if (range.min <= 0) throw new RangeError("log time range must be positive");
    return Math.exp(
      Math.log(range.min) + (Math.log(range.max) - Math.log(range.min)) * value,
    );
  }
  return range.min + (range.max - range.min) * value;
}

export function normalizeEnvelopeValue(value: number, range: EnvelopeRange) {
  const bounded = clamp(value, range.min, range.max);
  return range.scale === "log"
    ? (Math.log(bounded) - Math.log(range.min)) /
        (Math.log(range.max) - Math.log(range.min))
    : (bounded - range.min) / (range.max - range.min);
}

export function constrainEnvelope(
  value: EnvelopeValues,
  ranges: EnvelopeRanges,
): EnvelopeValues {
  return {
    attack: clamp(value.attack, ranges.attack.min, ranges.attack.max),
    decay: clamp(value.decay, ranges.decay.min, ranges.decay.max),
    sustain: clamp(value.sustain, ranges.sustain.min, ranges.sustain.max),
    release: clamp(value.release, ranges.release.min, ranges.release.max),
  };
}
