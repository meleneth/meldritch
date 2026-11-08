export type MappingScale = "linear" | "log" | "bipolar";

export interface AxisMapping {
  target: string;
  min: number;
  max: number;
  scale?: MappingScale;
  quantize?: number;
  center?: number;
  invert?: boolean;
}

const clamp01 = (value: number) => Math.min(1, Math.max(0, value));

export function mapNormalized(value: number, mapping: AxisMapping): number {
  let normalized = clamp01(mapping.invert ? 1 - value : value);
  let mapped: number;
  if (mapping.scale === "log") {
    if (mapping.min <= 0 || mapping.max <= 0)
      throw new RangeError("logarithmic mappings require positive endpoints");
    mapped = Math.exp(
      Math.log(mapping.min) +
        (Math.log(mapping.max) - Math.log(mapping.min)) * normalized,
    );
  } else if (mapping.scale === "bipolar") {
    const center = mapping.center ?? 0;
    mapped =
      normalized <= 0.5
        ? mapping.min + (center - mapping.min) * normalized * 2
        : center + (mapping.max - center) * (normalized - 0.5) * 2;
  } else {
    mapped = mapping.min + (mapping.max - mapping.min) * normalized;
  }
  if (mapping.quantize && mapping.quantize > 0)
    mapped = Math.round(mapped / mapping.quantize) * mapping.quantize;
  const low = Math.min(mapping.min, mapping.max);
  const high = Math.max(mapping.min, mapping.max);
  return Math.min(high, Math.max(low, mapped));
}

export function unmapValue(value: number, mapping: AxisMapping): number {
  const low = Math.min(mapping.min, mapping.max);
  const high = Math.max(mapping.min, mapping.max);
  const clamped = Math.min(high, Math.max(low, value));
  let normalized: number;
  if (mapping.scale === "log") {
    if (mapping.min <= 0 || mapping.max <= 0)
      throw new RangeError("logarithmic mappings require positive endpoints");
    normalized =
      (Math.log(clamped) - Math.log(mapping.min)) /
      (Math.log(mapping.max) - Math.log(mapping.min));
  } else if (mapping.scale === "bipolar") {
    const center = mapping.center ?? 0;
    normalized =
      clamped <= center
        ? ((clamped - mapping.min) / (center - mapping.min || 1)) * 0.5
        : 0.5 + ((clamped - center) / (mapping.max - center || 1)) * 0.5;
  } else {
    normalized = (clamped - mapping.min) / (mapping.max - mapping.min || 1);
  }
  return clamp01(mapping.invert ? 1 - normalized : normalized);
}

export function mapAxis(value: number, mappings: AxisMapping[]) {
  return Object.fromEntries(
    mappings.map((mapping) => [mapping.target, mapNormalized(value, mapping)]),
  );
}
