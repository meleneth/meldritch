import type Phaser from "phaser";
import type { SurfaceBinding } from "./binding";
import { SurfaceViewport } from "./viewport";

export interface SurfaceRuntime<T> {
  resize(width: number, height: number): void;
  setBinding(binding: SurfaceBinding<T>): void;
  destroy(): void;
}

export interface SurfaceDefinition<T> {
  readonly key: string;
  readonly ariaLabel: string;
  create(
    scene: Phaser.Scene,
    canvas: HTMLCanvasElement,
    binding: SurfaceBinding<T>,
    viewport: SurfaceViewport,
  ): SurfaceRuntime<T>;
}
