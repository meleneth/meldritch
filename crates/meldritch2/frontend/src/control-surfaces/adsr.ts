import type Phaser from "phaser";
import type { SurfaceBinding } from "./binding";
import {
  constrainEnvelope,
  mapEnvelopeValue,
  normalizeEnvelopeValue,
  type EnvelopeRanges,
  type EnvelopeValues,
} from "./envelope";
import { SurfaceGestureController } from "./gesture";
import type { SurfaceDefinition, SurfaceRuntime } from "./surface";
import type { Point, SurfaceViewport } from "./viewport";

export interface AdsrSurfaceOptions {
  id: string;
  label: string;
  ranges: EnvelopeRanges;
}

export function createAdsrSurface(
  options: AdsrSurfaceOptions,
): SurfaceDefinition<EnvelopeValues> {
  return {
    key: `adsr-${options.id}`,
    ariaLabel: `${options.label} envelope editor`,
    create: (scene, canvas, binding, viewport) =>
      new AdsrRuntime(scene, canvas, binding, viewport, options),
  };
}

type Handle = keyof EnvelopeValues;

class AdsrRuntime implements SurfaceRuntime<EnvelopeValues> {
  private graphics: Phaser.GameObjects.Graphics;
  private gesture: SurfaceGestureController;
  private stop: () => void;
  private value: EnvelopeValues;
  private active?: Handle;
  private gestureStart?: EnvelopeValues;
  private width = 1;
  private height = 1;
  private padding = 22;

  constructor(
    scene: Phaser.Scene,
    canvas: HTMLCanvasElement,
    private binding: SurfaceBinding<EnvelopeValues>,
    private viewport: SurfaceViewport,
    private options: AdsrSurfaceOptions,
  ) {
    this.graphics = scene.add.graphics();
    this.value = constrainEnvelope(binding.read(), options.ranges);
    this.stop = binding.subscribe(({ value }) => {
      this.value = constrainEnvelope(value, options.ranges);
      this.render();
    });
    this.gesture = new SurfaceGestureController(canvas, {
      hitTest: (point) =>
        !binding.readOnly && this.handleAt(point) ? "edit" : "background",
      editStart: (point) => {
        this.active = this.handleAt(point);
        this.gestureStart = { ...this.value };
        if (this.active)
          this.binding.beginGesture({
            id: options.id,
            label: `Edit ${this.active}`,
          });
      },
      editMove: (point) => this.move(point),
      editEnd: (point, _id, cancelled) => {
        if (!cancelled) this.move(point);
        else if (this.active && this.gestureStart) {
          this.value = this.gestureStart;
          this.binding.write(this.value);
          this.render();
        }
        if (this.active)
          this.binding.endGesture(
            { id: options.id, label: `Edit ${this.active}` },
            cancelled,
          );
        this.active = undefined;
        this.gestureStart = undefined;
      },
      navigate: (delta, scale, anchor) => {
        this.viewport.panScreen(delta.x, 0);
        this.viewport.zoomAt(anchor, scale, 1);
        this.render();
      },
    });
  }

  private normalized() {
    const attack = normalizeEnvelopeValue(
      this.value.attack,
      this.options.ranges.attack,
    );
    const decay = normalizeEnvelopeValue(
      this.value.decay,
      this.options.ranges.decay,
    );
    const release = normalizeEnvelopeValue(
      this.value.release,
      this.options.ranges.release,
    );
    const sustain = normalizeEnvelopeValue(
      this.value.sustain,
      this.options.ranges.sustain,
    );
    const total = attack + decay + 1 + release;
    return {
      attack: { x: attack / total, y: 0 },
      decay: { x: (attack + decay) / total, y: 1 - sustain },
      sustain: { x: (attack + decay + 1) / total, y: 1 - sustain },
      release: { x: 1, y: 1 },
    };
  }

  private screen(point: Point): Point {
    return {
      x: this.padding + point.x * (this.width - this.padding * 2),
      y: this.padding + point.y * (this.height - this.padding * 2),
    };
  }

  private handleAt(point: Point): Handle | undefined {
    const positions = this.normalized();
    return (Object.keys(positions) as Handle[]).find((key) => {
      const handle = this.screen(positions[key]);
      return Math.hypot(handle.x - point.x, handle.y - point.y) <= 28;
    });
  }

  private move(point: Point) {
    if (!this.active) return;
    const x = Math.min(
      1,
      Math.max(0, (point.x - this.padding) / (this.width - this.padding * 2)),
    );
    const y = Math.min(
      1,
      Math.max(0, (point.y - this.padding) / (this.height - this.padding * 2)),
    );
    if (this.active === "sustain" || this.active === "decay") {
      this.value = {
        ...this.value,
        sustain: mapEnvelopeValue(1 - y, this.options.ranges.sustain),
      };
    }
    if (this.active !== "sustain") {
      this.value = {
        ...this.value,
        [this.active]: mapEnvelopeValue(
          this.active === "release" ? 1 - x : x,
          this.options.ranges[this.active],
        ),
      };
    }
    this.value = constrainEnvelope(this.value, this.options.ranges);
    this.binding.write(this.value);
    this.render();
  }

  private render() {
    if (!this.graphics) return;
    const positions = this.normalized();
    const start = this.screen({ x: 0, y: 1 });
    const a = this.screen(positions.attack);
    const d = this.screen(positions.decay);
    const s = this.screen(positions.sustain);
    const r = this.screen(positions.release);
    const g = this.graphics;
    g.clear()
      .fillStyle(0x151821, 1)
      .fillRoundedRect(0, 0, this.width, this.height, 14);
    g.fillStyle(0x252a36, 1).fillRect(
      a.x,
      this.padding,
      Math.max(0, s.x - a.x),
      this.height - this.padding * 2,
    );
    g.lineStyle(3, 0x77e09a, 1);
    g.beginPath()
      .moveTo(start.x, start.y)
      .lineTo(a.x, a.y)
      .lineTo(d.x, d.y)
      .lineTo(s.x, s.y);
    g.strokePath();
    g.lineStyle(3, 0xf15b3d, 1).lineBetween(s.x, s.y, r.x, r.y);
    for (const [name, point] of Object.entries({
      attack: a,
      decay: d,
      sustain: s,
      release: r,
    })) {
      g.fillStyle(this.active === name ? 0xf6d365 : 0xffffff, 1).fillCircle(
        point.x,
        point.y,
        9,
      );
      g.lineStyle(4, 0xffffff, 0.16).strokeCircle(point.x, point.y, 18);
    }
  }

  resize(width: number, height: number) {
    this.width = Math.max(1, width);
    this.height = Math.max(1, height);
    this.viewport.resize(width, height);
    this.render();
  }
  setBinding(binding: SurfaceBinding<EnvelopeValues>) {
    this.stop();
    this.binding = binding;
    this.value = constrainEnvelope(binding.read(), this.options.ranges);
    this.stop = binding.subscribe(({ value }) => {
      this.value = constrainEnvelope(value, this.options.ranges);
      this.render();
    });
    this.render();
  }
  destroy() {
    this.stop();
    this.gesture.destroy();
    this.graphics.destroy();
  }
}
