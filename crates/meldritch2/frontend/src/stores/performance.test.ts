import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { usePerformanceStore } from "./performance";
import type { Snapshot } from "../types/protocol";

const snapshot = (
  revision: number,
  type: Snapshot["type"] = "state.snapshot",
): Snapshot => ({
  type,
  epoch: "run-one",
  revision,
  schema_revision: 1,
  state: {
    name: "Test",
    planes:
      type === "state.snapshot"
        ? [
            {
              id: "synth",
              label: "Synth",
              roles: ["synth"],
              controls: [
                {
                  id: "cutoff",
                  label: "Cutoff",
                  kind: "fader",
                  min: 20,
                  max: 20000,
                  scale: "log",
                  value: 20,
                },
              ],
            },
          ]
        : undefined,
    values: { "synth/cutoff": 880 },
  },
});

describe("performance store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    sessionStorage.clear();
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => ({ json: async () => ({ websocket: null }) })),
    );
  });

  it("hydrates schema with authoritative values and revisions", () => {
    const store = usePerformanceStore();
    store.applySnapshot(snapshot(4));
    expect(store.revision).toBe(4);
    expect(store.planes[0].controls[0]).toMatchObject({
      id: "cutoff",
      value: 880,
    });
  });

  it("detects a revision gap instead of applying it", () => {
    const store = usePerformanceStore();
    store.applySnapshot(snapshot(4));
    store.applySnapshot(snapshot(6, "state.updated"));
    expect(store.revision).toBe(4);
    expect(store.errors.at(-1)).toContain("Revision gap");
  });

  it("resets stale state when the server epoch changes", () => {
    const store = usePerformanceStore();
    store.applySnapshot(snapshot(9));
    const restarted = snapshot(0);
    restarted.epoch = "run-two";
    restarted.state.values = { "synth/cutoff": 440 };
    store.applySnapshot(restarted);
    expect(store.revision).toBe(0);
    expect(store.planes[0].controls[0]).toMatchObject({ value: 440 });
  });

  it("reconciles piano-roll, ADSR, and pending-pattern authoritative values", () => {
    const store = usePerformanceStore();
    const message = snapshot(1);
    message.state.planes![0].controls = [
      {
        id: "notes",
        label: "Notes",
        kind: "piano_roll",
        step_count: 16,
        pitch_min: 48,
        pitch_max: 72,
        voices: 4,
        pattern: 0,
        notes: [],
        position: null,
      },
      {
        id: "env",
        label: "Envelope",
        kind: "adsr",
        parameters: [],
        values: {},
      },
      {
        id: "patterns",
        label: "Patterns",
        kind: "pattern_selector",
        patterns: ["A", "B"],
        boundaries: ["bar"],
        default_boundary: "bar",
        active: 0,
        pending: null,
        boundary: "bar",
      },
    ];
    message.state.values = {
      "synth/notes": {
        pattern: 1,
        notes: [
          {
            id: 7,
            step: 2,
            pitch: 60,
            velocity: 90,
            duration: 3,
            legato: false,
          },
        ],
        position: 4,
      },
      "synth/env": { attack: 0.02, sustain: 0.7 },
      "synth/patterns": {
        active: 0,
        active_name: "A",
        pending: 1,
        pending_name: "B",
        boundary: "bar",
      },
    };
    store.applySnapshot(message);
    expect(store.planes[0].controls[0]).toMatchObject({
      pattern: 1,
      position: 4,
      notes: [{ id: 7 }],
    });
    expect(store.planes[0].controls[1]).toMatchObject({
      values: { attack: 0.02, sustain: 0.7 },
    });
    expect(store.planes[0].controls[2]).toMatchObject({
      active: 0,
      pending: 1,
    });
  });
});
