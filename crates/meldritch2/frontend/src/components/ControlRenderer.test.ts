import { describe, expect, it } from "vitest";
import { createApp, h, nextTick, type Component } from "vue";
import { createPinia } from "pinia";
import ControlRenderer from "./ControlRenderer.vue";
import FaderControl from "./controls/FaderControl.vue";
import PianoRoll from "./controls/PianoRoll.vue";
import AdsrControl from "./controls/AdsrControl.vue";
import PatternSelector from "./controls/PatternSelector.vue";
import PresetSelector from "./controls/PresetSelector.vue";
import DrumGrid from "./controls/DrumGrid.vue";
import PerformancePlane from "./PerformancePlane.vue";

function render(
  component: Component,
  props: Record<string, unknown>,
  listeners: Record<string, unknown> = {},
) {
  const root = document.createElement("div");
  document.body.append(root);
  const app = createApp({
    render: () => h(component, { ...props, ...listeners }),
  });
  app.use(createPinia()).mount(root);
  return {
    root,
    unmount() {
      app.unmount();
      root.remove();
    },
  };
}

describe("control registry", () => {
  it("renders an authoritative mute toggle and emits its inverse state", async () => {
    const changes: Array<[string, boolean]> = [];
    const view = render(
      ControlRenderer,
      {
        control: {
          id: "bass_mute",
          label: "BASS MUTE",
          kind: "toggle",
          value: false,
        },
      },
      {
        onSetToggle: (id: string, value: boolean) => changes.push([id, value]),
      },
    );
    const button = view.root.querySelector("button") as HTMLButtonElement;
    expect(button.getAttribute("aria-pressed")).toBe("false");
    expect(button.textContent).toContain("LIVE");
    button.click();
    await nextTick();
    expect(changes).toEqual([["bass_mute", true]]);
    view.unmount();
  });

  it("collapses a performance station without discarding its schema", async () => {
    const view = render(PerformancePlane, {
      plane: {
        id: "bass",
        label: "SERUM // BASS",
        roles: ["bass"],
        controls: [],
      },
    });
    const station = view.root.querySelector("section") as HTMLElement;
    const grid = view.root.querySelector(".plane-grid") as HTMLElement;
    const button = view.root.querySelector(
      ".plane-collapse",
    ) as HTMLButtonElement;
    expect(station.dataset.station).toBe("bass");
    expect(button.getAttribute("aria-expanded")).toBe("true");
    button.click();
    await nextTick();
    expect(button.getAttribute("aria-expanded")).toBe("false");
    expect(grid.style.display).toBe("none");
    button.click();
    await nextTick();
    expect(button.getAttribute("aria-expanded")).toBe("true");
    view.unmount();
  });

  it("renders and dispatches sequencer edits", async () => {
    const edits: number[] = [];
    const view = render(
      ControlRenderer,
      {
        control: {
          id: "kick",
          label: "Kick",
          kind: "step_sequencer",
          step_count: 16,
          steps: Array.from({ length: 16 }, (_, index) => index === 0),
          position: 3,
        },
      },
      { onToggle: (step: number) => edits.push(step) },
    );
    const buttons = view.root.querySelectorAll("button");
    expect(buttons).toHaveLength(16);
    buttons[5].click();
    await nextTick();
    expect(edits).toEqual([5]);
    view.unmount();
  });

  it("keeps triggers as repeatable events", async () => {
    let fires = 0;
    const view = render(
      ControlRenderer,
      {
        control: {
          id: "fill",
          label: "FILL",
          kind: "trigger",
          serial: 2,
        },
      },
      { onFire: () => fires++ },
    );
    const button = view.root.querySelector("button")!;
    button.dispatchEvent(new Event("pointerdown", { bubbles: true }));
    button.dispatchEvent(new Event("pointerup", { bubbles: true }));
    button.dispatchEvent(new Event("pointerdown", { bubbles: true }));
    await nextTick();
    expect(fires).toBe(2);
    view.unmount();
  });

  it("maps a logarithmic fader and delivers its final value", async () => {
    const changes: number[] = [];
    const view = render(
      FaderControl,
      {
        control: {
          id: "cutoff",
          label: "Cutoff",
          kind: "fader",
          min: 20,
          max: 20000,
          scale: "log",
          value: 20,
        },
      },
      { onChange: (value: number) => changes.push(value) },
    );
    const input = view.root.querySelector("input") as HTMLInputElement;
    input.value = "0.5";
    input.dispatchEvent(new Event("change", { bubbles: true }));
    await nextTick();
    expect(changes[0]).toBeCloseTo(Math.sqrt(20 * 20000));
    view.unmount();
  });

  it("mounts the piano-roll surface and dispatches data transformations", async () => {
    const created: unknown[] = [],
      updated: unknown[] = [],
      deleted: number[] = [];
    const control = {
      id: "notes",
      label: "Notes",
      kind: "piano_roll" as const,
      step_count: 16,
      pitch_min: 60,
      pitch_max: 61,
      voices: 2,
      pattern: 0,
      position: 2,
      notes: [
        {
          id: 1,
          step: 0,
          pitch: 60,
          velocity: 100,
          duration: 4,
          legato: false,
        },
        { id: 2, step: 0, pitch: 61, velocity: 90, duration: 2, legato: false },
      ],
    };
    const view = render(
      PianoRoll,
      { control },
      {
        onCreate: (note: unknown) => created.push(note),
        onUpdate: (note: unknown) => updated.push(note),
        onDelete: (id: number) => deleted.push(id),
      },
    );
    expect(view.root.querySelector("[role='application']")).not.toBeNull();
    const transpose = Array.from(view.root.querySelectorAll("button")).find(
      (button) => button.textContent?.includes("semitone"),
    ) as HTMLButtonElement;
    transpose.click();
    await nextTick();
    expect(updated).toEqual([
      expect.objectContaining({ id: 1, pitch: 61 }),
      expect.objectContaining({ id: 2, pitch: 62 }),
    ]);
    expect(created).toEqual([]);
    expect(deleted).toEqual([]);
    view.unmount();
  });

  it("maps ADSR values and keeps pending patterns distinct from active", async () => {
    const changes: Array<[string, number]> = [];
    const adsr = render(
      AdsrControl,
      {
        control: {
          id: "env",
          label: "Envelope",
          kind: "adsr",
          parameters: [
            {
              name: "attack",
              control: "env.attack",
              min: 0.001,
              max: 1,
              scale: "log",
              unit: "s",
            },
            {
              name: "decay",
              control: "env.decay",
              min: 0.001,
              max: 1,
              scale: "log",
              unit: "s",
            },
            {
              name: "sustain",
              control: "env.sustain",
              min: 0,
              max: 1,
              scale: "linear",
              unit: "",
            },
            {
              name: "release",
              control: "env.release",
              min: 0.001,
              max: 2,
              scale: "log",
              unit: "s",
            },
          ],
          values: { attack: 0.001, decay: 0.1, sustain: 0.7, release: 0.2 },
        },
        planeId: "synth",
      },
      { onChange: (id: string, value: number) => changes.push([id, value]) },
    );
    const range = adsr.root.querySelector(
      "input[type='number']",
    ) as HTMLInputElement;
    range.value = "0.5";
    range.dispatchEvent(new Event("input", { bubbles: true }));
    expect(changes[0][0]).toBe("attack");
    expect(changes[0][1]).toBeCloseTo(0.5);
    adsr.unmount();

    const selections: unknown[] = [];
    const patterns = render(
      PatternSelector,
      {
        control: {
          id: "patterns",
          label: "Patterns",
          kind: "pattern_selector",
          patterns: ["A", "B"],
          boundaries: ["beat", "bar"],
          default_boundary: "bar",
          active: 0,
          active_name: "A",
          pending: 1,
          pending_name: "B",
          boundary: "bar",
        },
      },
      { onSelect: (...args: unknown[]) => selections.push(args) },
    );
    expect(patterns.root.querySelectorAll("button.active")).toHaveLength(1);
    expect(patterns.root.querySelectorAll("button.pending")).toHaveLength(1);
    (patterns.root.querySelectorAll("button")[1] as HTMLButtonElement).click();
    expect(selections).toEqual([[1, "bar"]]);
    patterns.unmount();
  });

  it("renders authoritative preset state and honors read-only projection", () => {
    const selections: string[] = [];
    const view = render(
      PresetSelector,
      {
        control: {
          id: "serum_preset",
          label: "Performance Patch",
          kind: "preset_selector",
          presets: ["Clean", "Glass", "Driven"],
          entries: ["Clean", "Glass", "Driven"].map((name) => ({
            id: name.toLowerCase(),
            name,
            bank: "Factory",
            category: "Lead",
          })),
          active: 1,
          active_name: "Glass",
          pending: 2,
          failed: null,
          read_only: true,
        },
      },
      { onSelect: (preset: string) => selections.push(preset) },
    );
    expect(view.root.querySelectorAll("button.active")).toHaveLength(1);
    expect(view.root.querySelectorAll(".pending")).toHaveLength(1);
    expect(view.root.querySelector("fieldset")?.hasAttribute("disabled")).toBe(
      true,
    );
    (view.root.querySelectorAll("button")[0] as HTMLButtonElement).click();
    expect(selections).toEqual([]);
    view.unmount();
  });

  it("searches a large preset catalog and emits stable identifiers", async () => {
    const selections: string[] = [];
    const entries = Array.from({ length: 250 }, (_, index) => ({
      id: `stable-${index}`,
      name: index === 173 ? "Needle Lead" : `Patch ${index}`,
      bank: index % 2 ? "Factory" : "User",
      category: index % 3 ? "Lead" : "Pad",
    }));
    const view = render(
      PresetSelector,
      {
        control: {
          id: "catalog",
          label: "Catalog",
          kind: "preset_selector",
          presets: entries.map((entry) => entry.name),
          entries,
          active: 173,
          active_name: entries[173].name,
          pending: null,
          failed: null,
        },
      },
      { onSelect: (preset: string) => selections.push(preset) },
    );
    expect(
      view.root.querySelectorAll("[role=option]").length,
    ).toBeLessThanOrEqual(40);
    const input = view.root.querySelector("input") as HTMLInputElement;
    input.value = "needle";
    input.dispatchEvent(new Event("input"));
    await nextTick();
    expect(view.root.querySelectorAll("[role=option]")).toHaveLength(1);
    expect(view.root.querySelectorAll("button.active")).toHaveLength(1);
    (view.root.querySelector("[role=option]") as HTMLButtonElement).click();
    expect(selections).toEqual(["stable-173"]);
    view.unmount();
  });

  it("renders drum lanes and emits hit, velocity, and mute edits", async () => {
    const toggles: unknown[] = [],
      velocities: unknown[] = [],
      mutes: unknown[] = [];
    const view = render(
      DrumGrid,
      {
        control: {
          id: "grid",
          label: "Drums",
          kind: "drum_grid",
          kit: "TimeKit",
          current_kit: "TimeKit",
          step_count: 16,
          pattern: 0,
          position: 3,
          rows: [{ id: "kick", label: "Kick", note: 36 }],
          state_rows: [
            {
              id: "kick",
              muted: false,
              hits: [{ id: 1, step: 0, velocity: 100, duration: 1 }],
            },
          ],
        },
      },
      {
        onToggle: (...args: unknown[]) => toggles.push(args),
        onVelocity: (...args: unknown[]) => velocities.push(args),
        onMute: (...args: unknown[]) => mutes.push(args),
      },
    );
    const buttons = view.root.querySelectorAll(".drum-row button");
    (buttons[2] as HTMLButtonElement).click();
    expect(toggles).toEqual([["kick", 1]]);
    (buttons[0] as HTMLButtonElement).click();
    expect(mutes).toEqual([["kick", true]]);
    (buttons[1] as HTMLButtonElement).click();
    await nextTick();
    const range = view.root.querySelector(
      "input[type=range]",
    ) as HTMLInputElement;
    range.value = "64";
    range.dispatchEvent(new Event("change", { bubbles: true }));
    expect(velocities).toEqual([["kick", 0, 64]]);
    (
      view.root.querySelector(".drum-hit-editor button") as HTMLButtonElement
    ).click();
    expect(toggles).toEqual([
      ["kick", 1],
      ["kick", 0],
    ]);
    view.unmount();
  });
});
