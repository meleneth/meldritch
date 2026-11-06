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
});
