import type Phaser from "phaser";
import type { CompositeBindingValue, SurfaceBinding } from "./binding";
import { SurfaceGestureController } from "./gesture";
import { mapAxis, unmapValue, type AxisMapping } from "./mapping";
import type { SurfaceDefinition, SurfaceRuntime } from "./surface";
import type { Point, SurfaceViewport } from "./viewport";

export interface XySurfaceOptions {
  id: string;
  label: string;
  x: AxisMapping[];
  y: AxisMapping[];
  mode?: "absolute" | "relative";
  returnToCenter?: boolean;
}

export function createXySurface(
  options: XySurfaceOptions,
): SurfaceDefinition<CompositeBindingValue> {
  return {
    key: `xy-${options.id}`,
    ariaLabel: options.label,
    create(scene, canvas, initialBinding, viewport) {
      return new XyRuntime(scene, canvas, initialBinding, viewport, options);
    },
  };
}

class XyRuntime implements SurfaceRuntime<CompositeBindingValue> {
  private graphics: Phaser.GameObjects.Graphics;
  private gesture: SurfaceGestureController;
  private stop: () => void;
  private width = 1;
  private height = 1;
  private normalized: Point = { x: 0.5, y: 0.5 };
  private start: Point = { x: 0.5, y: 0.5 };
  private lastPointer: Point = { x: 0, y: 0 };

  constructor(
    scene: Phaser.Scene,
    canvas: HTMLCanvasElement,
    private binding: SurfaceBinding<CompositeBindingValue>,
    private viewport: SurfaceViewport,
    private options: XySurfaceOptions,
  ) {
    this.graphics = scene.add.graphics();
    this.sync(binding.read());
    this.stop = binding.subscribe(({ value }) => this.sync(value));
    this.gesture = new SurfaceGestureController(canvas, {
      hitTest: () => (binding.readOnly ? "background" : "edit"),
      editStart: (point) => {
        this.binding.beginGesture({ id: options.id, label: options.label });
        this.start = { ...this.normalized };
        this.lastPointer = point;
        this.apply(point);
      },
      editMove: (point) => this.apply(point),
      editEnd: (point, _id, cancelled) => {
        if (!cancelled) this.apply(point);
        if (options.returnToCenter) {
          this.normalized = { x: 0.5, y: 0.5 };
          this.write();
        }
        this.binding.endGesture(
          { id: options.id, label: options.label },
          cancelled,
        );
      },
      navigate: () => undefined,
    });
  }

  private sync(value: CompositeBindingValue) {
    const x = this.options.x[0];
    const y = this.options.y[0];
    if (x && value[x.target] !== undefined)
      this.normalized.x = unmapValue(value[x.target], x);
    if (y && value[y.target] !== undefined)
      this.normalized.y = 1 - unmapValue(value[y.target], y);
    this.render();
  }

  private apply(point: Point) {
    if (this.options.mode === "relative") {
      this.normalized.x = Math.min(
        1,
        Math.max(0, this.start.x + (point.x - this.lastPointer.x) / this.width),
      );
      this.normalized.y = Math.min(
        1,
        Math.max(
          0,
          this.start.y + (point.y - this.lastPointer.y) / this.height,
        ),
      );
    } else {
      this.normalized.x = Math.min(1, Math.max(0, point.x / this.width));
      this.normalized.y = Math.min(1, Math.max(0, point.y / this.height));
    }
    this.write();
  }

  private write() {
    this.binding.write({
      ...mapAxis(this.normalized.x, this.options.x),
      ...mapAxis(1 - this.normalized.y, this.options.y),
    });
    this.render();
  }

  private render() {
    if (!this.graphics) return;
    const x = this.normalized.x * this.width;
    const y = this.normalized.y * this.height;
    this.graphics.clear();
    this.graphics
      .fillStyle(0x151821, 1)
      .fillRoundedRect(0, 0, this.width, this.height, 14);
    this.graphics.lineStyle(1, 0x414755, 1);
    for (let i = 1; i < 4; i++) {
      this.graphics.lineBetween(
        (this.width * i) / 4,
        0,
        (this.width * i) / 4,
        this.height,
      );
      this.graphics.lineBetween(
        0,
        (this.height * i) / 4,
        this.width,
        (this.height * i) / 4,
      );
    }
    this.graphics.lineStyle(2, 0xf6d365, 0.75);
    this.graphics.lineBetween(x, 0, x, this.height);
    this.graphics.lineBetween(0, y, this.width, y);
    this.graphics.fillStyle(0xee6b3b, 1).fillCircle(x, y, 14);
    this.graphics.lineStyle(5, 0xffffff, 0.22).strokeCircle(x, y, 23);
  }

  resize(width: number, height: number) {
    this.width = Math.max(1, width);
    this.height = Math.max(1, height);
    this.viewport.resize(width, height);
    this.render();
  }
  setBinding(binding: SurfaceBinding<CompositeBindingValue>) {
    this.stop();
    this.binding = binding;
    this.stop = binding.subscribe(({ value }) => this.sync(value));
    this.sync(binding.read());
  }
  destroy() {
    this.stop();
    this.gesture.destroy();
    this.graphics.destroy();
  }
}
