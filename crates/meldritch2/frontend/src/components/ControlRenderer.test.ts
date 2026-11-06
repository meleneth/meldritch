import { describe, expect, it } from "vitest";
import { createApp, h, nextTick, type Component } from "vue";
import ControlRenderer from "./ControlRenderer.vue";
import FaderControl from "./controls/FaderControl.vue";

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
});
