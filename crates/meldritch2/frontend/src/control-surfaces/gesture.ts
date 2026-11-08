import type { Point } from "./viewport";

export type GestureMode = "idle" | "edit" | "navigate";

export interface GestureCallbacks {
  hitTest(point: Point): "edit" | "background";
  editStart(point: Point, pointerId: number): void;
  editMove(point: Point, pointerId: number): void;
  editEnd(point: Point, pointerId: number, cancelled: boolean): void;
  navigate(delta: Point, scale: number, anchor: Point): void;
}

interface TrackedPointer {
  point: Point;
  previous: Point;
}

/** Shared touch-first arbitration. One pointer edits; two pointers navigate. */
export class SurfaceGestureController {
  private pointers = new Map<number, TrackedPointer>();
  private mode: GestureMode = "idle";
  private editingPointer: number | null = null;

  constructor(
    private element: HTMLElement,
    private callbacks: GestureCallbacks,
  ) {
    element.addEventListener("pointerdown", this.down);
    element.addEventListener("pointermove", this.move);
    element.addEventListener("pointerup", this.up);
    element.addEventListener("pointercancel", this.cancel);
  }

  private local(event: PointerEvent): Point {
    const rect = this.element.getBoundingClientRect();
    return { x: event.clientX - rect.left, y: event.clientY - rect.top };
  }

  private down = (event: PointerEvent) => {
    const point = this.local(event);
    this.element.setPointerCapture?.(event.pointerId);
    this.pointers.set(event.pointerId, { point, previous: point });
    if (this.pointers.size === 1) {
      this.mode =
        this.callbacks.hitTest(point) === "edit" ? "edit" : "navigate";
      if (this.mode === "edit") {
        this.editingPointer = event.pointerId;
        this.callbacks.editStart(point, event.pointerId);
      }
    } else if (this.pointers.size === 2) {
      if (this.mode === "edit" && this.editingPointer !== null) {
        const editing = this.pointers.get(this.editingPointer)?.point ?? point;
        this.callbacks.editEnd(editing, this.editingPointer, true);
      }
      this.editingPointer = null;
      this.mode = "navigate";
    }
    event.preventDefault();
  };

  private move = (event: PointerEvent) => {
    const tracked = this.pointers.get(event.pointerId);
    if (!tracked) return;
    tracked.previous = tracked.point;
    tracked.point = this.local(event);
    if (this.mode === "edit" && event.pointerId === this.editingPointer) {
      this.callbacks.editMove(tracked.point, event.pointerId);
    } else if (this.mode === "navigate") {
      const values = [...this.pointers.values()];
      if (values.length === 1) {
        this.callbacks.navigate(
          {
            x: values[0].point.x - values[0].previous.x,
            y: values[0].point.y - values[0].previous.y,
          },
          1,
          values[0].point,
        );
      } else {
        const [a, b] = values;
        const center = {
          x: (a.point.x + b.point.x) / 2,
          y: (a.point.y + b.point.y) / 2,
        };
        const oldCenter = {
          x: (a.previous.x + b.previous.x) / 2,
          y: (a.previous.y + b.previous.y) / 2,
        };
        const distance = Math.hypot(
          a.point.x - b.point.x,
          a.point.y - b.point.y,
        );
        const oldDistance = Math.hypot(
          a.previous.x - b.previous.x,
          a.previous.y - b.previous.y,
        );
        this.callbacks.navigate(
          { x: center.x - oldCenter.x, y: center.y - oldCenter.y },
          oldDistance > 0 ? distance / oldDistance : 1,
          center,
        );
      }
    }
    event.preventDefault();
  };

  private finish(event: PointerEvent, cancelled: boolean) {
    const tracked = this.pointers.get(event.pointerId);
    if (!tracked) return;
    if (this.mode === "edit" && event.pointerId === this.editingPointer)
      this.callbacks.editEnd(tracked.point, event.pointerId, cancelled);
    this.pointers.delete(event.pointerId);
    this.element.releasePointerCapture?.(event.pointerId);
    if (this.pointers.size === 0) {
      this.mode = "idle";
      this.editingPointer = null;
    }
    event.preventDefault();
  }

  private up = (event: PointerEvent) => this.finish(event, false);
  private cancel = (event: PointerEvent) => this.finish(event, true);

  destroy() {
    this.element.removeEventListener("pointerdown", this.down);
    this.element.removeEventListener("pointermove", this.move);
    this.element.removeEventListener("pointerup", this.up);
    this.element.removeEventListener("pointercancel", this.cancel);
    this.pointers.clear();
  }
}
