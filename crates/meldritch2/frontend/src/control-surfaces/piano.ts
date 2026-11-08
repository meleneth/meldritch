import type Phaser from "phaser";
import type { SurfaceBinding } from "./binding";
import { SurfaceGestureController } from "./gesture";
import {
  createNote,
  snapPitch,
  updateNotes,
  type PatternModel,
  type PatternNote,
  type ScaleLock,
} from "./pattern";
import type { SurfaceDefinition, SurfaceRuntime } from "./surface";
import type { Point, SurfaceViewport } from "./viewport";

export interface PianoSurfaceOptions {
  id: string;
  label: string;
  pitchMin: number;
  pitchMax: number;
  scale: () => ScaleLock;
  nextId: () => number;
  onSelection?: (ids: ReadonlySet<number>) => void;
}

export function createPianoSurface(
  options: PianoSurfaceOptions,
): SurfaceDefinition<PatternModel> {
  return {
    key: `piano-${options.id}`,
    ariaLabel: `${options.label} piano roll`,
    create: (scene, canvas, binding, viewport) =>
      new PianoRuntime(scene, canvas, binding, viewport, options),
  };
}

type Edit =
  | { kind: "move"; note: PatternNote; origin: Point }
  | { kind: "resize"; note: PatternNote; origin: Point }
  | { kind: "create"; note: PatternNote }
  | undefined;

class PianoRuntime implements SurfaceRuntime<PatternModel> {
  private graphics: Phaser.GameObjects.Graphics;
  private gesture: SurfaceGestureController;
  private stop: () => void;
  private model: PatternModel;
  private selected = new Set<number>();
  private edit: Edit;
  private gestureStart?: PatternModel;
  private width = 1;
  private height = 1;
  private readonly handleSize = 18;

  constructor(
    scene: Phaser.Scene,
    canvas: HTMLCanvasElement,
    private binding: SurfaceBinding<PatternModel>,
    private viewport: SurfaceViewport,
    private options: PianoSurfaceOptions,
  ) {
    this.graphics = scene.add.graphics();
    this.model = binding.read();
    this.stop = binding.subscribe(({ value }) => {
      const lengthChanged = this.model.length !== value.length;
      this.model = value;
      if (lengthChanged) this.fitModel();
      this.render();
    });
    this.gesture = new SurfaceGestureController(canvas, {
      hitTest: () => (binding.readOnly ? "background" : "edit"),
      editStart: (point) => this.startEdit(point),
      editMove: (point) => this.moveEdit(point),
      editEnd: (point, _id, cancelled) => this.endEdit(point, cancelled),
      navigate: (delta, scale, anchor) => {
        this.viewport.panScreen(delta.x, delta.y);
        this.viewport.zoomAt(anchor, scale, scale);
        this.render();
      },
    });
  }

  private fitModel() {
    this.viewport.fit(
      {
        x: 0,
        y: 0,
        width: this.model.length,
        height: this.options.pitchMax - this.options.pitchMin + 1,
      },
      4,
    );
  }

  private noteRect(note: PatternNote) {
    const top = this.viewport.domainToScreen({
      x: note.start,
      y: this.options.pitchMax - note.pitch,
    });
    return {
      x: top.x,
      y: top.y,
      width: note.duration * this.viewport.zoomX,
      height: this.viewport.zoomY,
    };
  }

  private noteAt(point: Point) {
    return [...this.model.notes].reverse().find((note) => {
      const rect = this.noteRect(note);
      return (
        point.x >= rect.x - 5 &&
        point.x <= rect.x + rect.width + 5 &&
        point.y >= rect.y - 5 &&
        point.y <= rect.y + rect.height + 5
      );
    });
  }

  private startEdit(point: Point) {
    this.gestureStart = this.model;
    this.binding.beginGesture({ id: this.options.id, label: "Edit notes" });
    const note = this.noteAt(point);
    if (note) {
      if (!this.selected.has(note.id)) this.selected.add(note.id);
      this.options.onSelection?.(this.selected);
      const rect = this.noteRect(note);
      this.edit = {
        kind:
          point.x >= rect.x + rect.width - this.handleSize ? "resize" : "move",
        note: { ...note },
        origin: point,
      };
    } else {
      const domain = this.viewport.screenToDomain(point);
      const noteValue: PatternNote = {
        id: this.options.nextId(),
        start:
          Math.floor(domain.x / this.model.resolution) * this.model.resolution,
        duration: this.model.resolution,
        pitch: snapPitch(
          this.options.pitchMax - Math.floor(domain.y),
          this.options.scale(),
        ),
        velocity: 100,
      };
      this.model = createNote(this.model, noteValue, this.options.scale());
      this.selected = new Set([noteValue.id]);
      this.options.onSelection?.(this.selected);
      this.edit = { kind: "create", note: noteValue };
    }
    this.render();
  }

  private moveEdit(point: Point) {
    if (!this.edit || this.edit.kind === "create") return;
    const deltaX = (point.x - this.edit.origin.x) / this.viewport.zoomX;
    const deltaY = (point.y - this.edit.origin.y) / this.viewport.zoomY;
    const grid = this.model.resolution;
    if (this.edit.kind === "resize") {
      const duration = Math.max(
        grid,
        Math.round((this.edit.note.duration + deltaX) / grid) * grid,
      );
      this.model = updateNotes(
        this.model,
        new Set([this.edit.note.id]),
        (note) => ({ ...note, duration }),
      );
    } else {
      const start = Math.round((this.edit.note.start + deltaX) / grid) * grid;
      const pitch = snapPitch(
        this.edit.note.pitch - Math.round(deltaY),
        this.options.scale(),
      );
      const startNote = this.edit.note;
      this.model = updateNotes(
        this.model,
        this.selected,
        (note) => ({
          ...note,
          start: note.start + (start - startNote.start),
          pitch: note.pitch + (pitch - startNote.pitch),
        }),
        this.options.scale(),
      );
    }
    this.binding.write(this.model);
    this.render();
  }

  private endEdit(point: Point, cancelled: boolean) {
    if (!cancelled) {
      this.moveEdit(point);
      if (this.edit?.kind === "create") this.binding.write(this.model);
    } else if (this.gestureStart) {
      const changed = this.model !== this.gestureStart;
      this.model = this.gestureStart;
      if (changed && this.edit?.kind !== "create")
        this.binding.write(this.model);
      this.render();
    }
    this.edit = undefined;
    this.gestureStart = undefined;
    this.binding.endGesture(
      { id: this.options.id, label: "Edit notes" },
      cancelled,
    );
  }

  private render() {
    const g = this.graphics;
    if (!g) return;
    g.clear().fillStyle(0x151821, 1).fillRect(0, 0, this.width, this.height);
    const start = this.viewport.screenToDomain({ x: 0, y: 0 });
    const end = this.viewport.screenToDomain({ x: this.width, y: this.height });
    g.lineStyle(1, 0x303541, 1);
    for (
      let step = Math.max(0, Math.floor(start.x));
      step <= Math.min(this.model.length, Math.ceil(end.x));
      step++
    ) {
      const x = this.viewport.domainToScreen({ x: step, y: 0 }).x;
      g.lineStyle(
        step % 4 === 0 ? 2 : 1,
        step % 4 === 0 ? 0x555d6d : 0x303541,
        1,
      );
      g.lineBetween(x, 0, x, this.height);
    }
    for (let row = Math.floor(start.y); row <= Math.ceil(end.y); row++) {
      const y = this.viewport.domainToScreen({ x: 0, y: row }).y;
      g.lineStyle(1, 0x303541, 1).lineBetween(0, y, this.width, y);
    }
    for (const note of this.model.notes) {
      const rect = this.noteRect(note);
      if (
        rect.x + rect.width < 0 ||
        rect.x > this.width ||
        rect.y + rect.height < 0 ||
        rect.y > this.height
      )
        continue;
      g.fillStyle(this.selected.has(note.id) ? 0xf6d365 : 0xee6b3b, 1)
        .fillRoundedRect(
          rect.x + 1,
          rect.y + 2,
          Math.max(3, rect.width - 2),
          Math.max(3, rect.height - 4),
          4,
        )
        .fillStyle(0xffffff, 0.35)
        .fillRect(
          rect.x + rect.width - 8,
          rect.y + 3,
          5,
          Math.max(2, rect.height - 6),
        );
    }
  }

  resize(width: number, height: number) {
    this.width = Math.max(1, width);
    this.height = Math.max(1, height);
    this.viewport.resize(width, height);
    if (this.viewport.zoomX === 1 && this.viewport.zoomY === 1) this.fitModel();
    this.render();
  }
  setBinding(binding: SurfaceBinding<PatternModel>) {
    this.stop();
    this.binding = binding;
    this.model = binding.read();
    this.stop = binding.subscribe(({ value }) => {
      const lengthChanged = this.model.length !== value.length;
      this.model = value;
      if (lengthChanged) this.fitModel();
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
