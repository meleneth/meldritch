import { describe, expect, it, vi } from "vitest";
import { SurfaceGestureController, type GestureCallbacks } from "./gesture";

function pointer(type: string, id: number, x: number, y: number) {
  const event = new MouseEvent(type, { bubbles: true, clientX: x, clientY: y });
  Object.defineProperty(event, "pointerId", { value: id });
  return event;
}

function setup(hit: "edit" | "background" = "edit") {
  const element = document.createElement("div");
  document.body.append(element);
  const callbacks: GestureCallbacks = {
    hitTest: vi.fn(() => hit),
    editStart: vi.fn(),
    editMove: vi.fn(),
    editEnd: vi.fn(),
    navigate: vi.fn(),
  };
  const controller = new SurfaceGestureController(element, callbacks);
  return { element, callbacks, controller };
}

describe("surface gesture arbitration", () => {
  it("delivers every single-pointer edit move and its final position", () => {
    const { element, callbacks, controller } = setup();
    element.dispatchEvent(pointer("pointerdown", 1, 5, 6));
    element.dispatchEvent(pointer("pointermove", 1, 8, 9));
    element.dispatchEvent(pointer("pointermove", 1, 12, 14));
    element.dispatchEvent(pointer("pointerup", 1, 12, 14));
    expect(callbacks.editStart).toHaveBeenCalledWith({ x: 5, y: 6 }, 1);
    expect(callbacks.editMove).toHaveBeenCalledTimes(2);
    expect(callbacks.editEnd).toHaveBeenCalledWith({ x: 12, y: 14 }, 1, false);
    controller.destroy();
  });

  it("ends an owned edit as cancelled on pointer cancellation", () => {
    const { element, callbacks, controller } = setup();
    element.dispatchEvent(pointer("pointerdown", 4, 1, 2));
    element.dispatchEvent(pointer("pointercancel", 4, 3, 4));
    expect(callbacks.editEnd).toHaveBeenCalledWith({ x: 1, y: 2 }, 4, true);
    controller.destroy();
  });

  it("cancels editing and arbitrates two pointers to pan and pinch", () => {
    const { element, callbacks, controller } = setup();
    element.dispatchEvent(pointer("pointerdown", 1, 10, 10));
    element.dispatchEvent(pointer("pointerdown", 2, 30, 10));
    expect(callbacks.editEnd).toHaveBeenCalledWith({ x: 10, y: 10 }, 1, true);
    element.dispatchEvent(pointer("pointermove", 2, 40, 20));
    expect(callbacks.navigate).toHaveBeenCalledTimes(1);
    const [delta, scale, anchor] = vi.mocked(callbacks.navigate).mock.calls[0];
    expect(delta).toEqual({ x: 5, y: 5 });
    expect(scale).toBeCloseTo(Math.hypot(30, 10) / 20);
    expect(anchor).toEqual({ x: 25, y: 15 });
    controller.destroy();
  });

  it("removes scoped listeners on destroy", () => {
    const { element, callbacks, controller } = setup();
    controller.destroy();
    element.dispatchEvent(pointer("pointerdown", 1, 1, 1));
    expect(callbacks.editStart).not.toHaveBeenCalled();
  });
});
