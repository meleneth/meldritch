import { describe, expect, it, vi } from "vitest";
import { createSurfaceBinding } from "./binding";

function source(initial = 0) {
  let value = initial;
  const listeners = new Set<(value: number) => void>();
  return {
    read: () => value,
    set(next: number) {
      value = next;
      listeners.forEach((listener) => listener(next));
    },
    subscribe(listener: (value: number) => void) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    count: () => listeners.size,
  };
}

describe("surface binding", () => {
  it("writes immediately, suppresses its echo, and cleans up", () => {
    const state = source();
    const send = vi.fn();
    const binding = createSurfaceBinding({
      read: state.read,
      subscribe: state.subscribe,
      optimistic: state.set,
      write: send,
    });
    const changes = vi.fn();
    binding.subscribe(changes);
    binding.beginGesture({ id: "drag" });
    binding.write(0.25);
    binding.write(0.75);
    expect(send.mock.calls).toEqual([[0.25], [0.75]]);
    expect(state.read()).toBe(0.75);
    expect(changes).toHaveBeenCalledTimes(2);
    binding.endGesture({ id: "drag" });
    binding.dispose();
    expect(state.count()).toBe(0);
  });

  it("queues the latest conflicting external update until gesture end", () => {
    const state = source();
    const binding = createSurfaceBinding({
      read: state.read,
      subscribe: state.subscribe,
      write: vi.fn(),
    });
    const changes: Array<[number, string]> = [];
    binding.subscribe((change) => changes.push([change.value, change.origin]));
    binding.beginGesture({ id: "drag" });
    state.set(2);
    state.set(3);
    expect(changes).toEqual([]);
    binding.endGesture({ id: "drag" });
    expect(changes).toEqual([[3, "reconcile"]]);
  });

  it("keeps multiple surfaces synchronized through shared external state", () => {
    const state = source();
    const make = () =>
      createSurfaceBinding({
        read: state.read,
        subscribe: state.subscribe,
        optimistic: state.set,
        write: vi.fn(),
      });
    const first = make(),
      second = make(),
      observed: number[] = [];
    second.subscribe(({ value }) => observed.push(value));
    first.write(0.6);
    expect(observed).toEqual([0.6]);
    first.dispose();
    second.dispose();
  });
});
