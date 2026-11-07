import { describe, expect, it } from "vitest";
import { createApp, h, nextTick, type Component } from "vue";
import ControlRenderer from "./ControlRenderer.vue";
import FaderControl from "./controls/FaderControl.vue";
import PianoRoll from "./controls/PianoRoll.vue";
import AdsrControl from "./controls/AdsrControl.vue";
import PatternSelector from "./controls/PatternSelector.vue";
import PresetSelector from "./controls/PresetSelector.vue";

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
  app.mount(root);
  return {
    root,
    unmount() {
      app.unmount();
      root.remove();
    },
  };
}

describe("control registry", () => {
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

  it("creates, selects, edits, and deletes piano-roll notes including chords", async () => {
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
    const noteButtons = view.root.querySelectorAll("button.note");
    expect(noteButtons).toHaveLength(2);
    (noteButtons[1] as HTMLButtonElement).click();
    await nextTick();
    const velocity = Array.from(view.root.querySelectorAll("input")).find(
      (input) => (input as HTMLInputElement).value === "100",
    ) as HTMLInputElement;
    velocity.value = "72";
    velocity.dispatchEvent(new Event("change", { bubbles: true }));
    await nextTick();
    expect(updated[0]).toMatchObject({ id: 1, velocity: 72 });
    const empty = view.root.querySelector(
      "button[aria-label='C4 step 2']",
    ) as HTMLButtonElement;
    empty.click();
    expect(created[0]).toMatchObject({ step: 1, pitch: 60 });
    (view.root.querySelector("fieldset button") as HTMLButtonElement).click();
    expect(deleted).toEqual([1]);
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
          ],
          values: { attack: 0.001 },
        },
      },
      { onChange: (id: string, value: number) => changes.push([id, value]) },
    );
    const range = adsr.root.querySelector("input") as HTMLInputElement;
    range.value = "1";
    range.dispatchEvent(new Event("input", { bubbles: true }));
    expect(changes[0][0]).toBe("attack");
    expect(changes[0][1]).toBeCloseTo(1);
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
    const selections: number[] = [];
    const view = render(
      PresetSelector,
      {
        control: {
          id: "serum_preset",
          label: "Performance Patch",
          kind: "preset_selector",
          presets: ["Clean", "Glass", "Driven"],
          active: 1,
          active_name: "Glass",
          pending: 2,
          failed: null,
          read_only: true,
        },
      },
      { onSelect: (preset: number) => selections.push(preset) },
    );
    expect(view.root.querySelectorAll("button.active")).toHaveLength(1);
    expect(view.root.querySelectorAll("button.pending")).toHaveLength(1);
    expect(view.root.querySelector("fieldset")?.hasAttribute("disabled")).toBe(true);
    (view.root.querySelectorAll("button")[0] as HTMLButtonElement).click();
    expect(selections).toEqual([]);
    view.unmount();
  });
});
