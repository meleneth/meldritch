<script setup lang="ts" generic="T">
import type Phaser from "phaser";
import { onBeforeUnmount, onMounted, shallowRef, watch } from "vue";
import type { SurfaceBinding } from "../control-surfaces/binding";
import type {
  SurfaceDefinition,
  SurfaceRuntime,
} from "../control-surfaces/surface";
import {
  SurfaceViewport,
  type ViewportOptions,
} from "../control-surfaces/viewport";

const props = withDefaults(
  defineProps<{
    surface: SurfaceDefinition<T>;
    binding: SurfaceBinding<T>;
    viewport?: ViewportOptions;
    height?: number;
  }>(),
  { height: 320 },
);

const container = shallowRef<HTMLElement>();
let game: Phaser.Game | undefined;
let runtime: SurfaceRuntime<T> | undefined;
let observer: ResizeObserver | undefined;
let viewport: SurfaceViewport | undefined;
let generation = 0;

async function mountGame() {
  if (!container.value) return;
  // Domain and binding behavior are tested without booting a renderer in jsdom.
  if (navigator.userAgent.includes("jsdom")) return;
  const ticket = ++generation;
  const { default: Phaser } = await import("phaser");
  if (!container.value || ticket !== generation) return;
  viewport = new SurfaceViewport(props.viewport);
  const definition = props.surface;
  const binding = props.binding;
  class ControlScene extends Phaser.Scene {
    constructor() {
      super({ key: `surface-${definition.key}-${crypto.randomUUID()}` });
    }
    create() {
      runtime = definition.create(this, this.game.canvas, binding, viewport!);
      const rect = container.value!.getBoundingClientRect();
      runtime.resize(rect.width, rect.height);
    }
  }
  const rect = container.value.getBoundingClientRect();
  const config = {
    type: Phaser.AUTO,
    parent: container.value,
    width: Math.max(1, rect.width),
    height: Math.max(1, rect.height),
    transparent: true,
    resolution: Math.min(window.devicePixelRatio || 1, 3),
    render: { antialias: true, roundPixels: true },
    scale: { mode: Phaser.Scale.RESIZE },
    input: { activePointers: 3 },
    scene: ControlScene,
    audio: { noAudio: true },
  } as Phaser.Types.Core.GameConfig;
  game = new Phaser.Game(config);
  observer = new ResizeObserver((entries) => {
    const entry = entries.at(-1);
    if (!entry || !game || !runtime || !viewport) return;
    const { width, height } = entry.contentRect;
    viewport.resize(width, height);
    game.scale.resize(width, height);
    runtime.resize(width, height);
  });
  observer.observe(container.value);
}

function destroyGame() {
  generation++;
  observer?.disconnect();
  observer = undefined;
  runtime?.destroy();
  runtime = undefined;
  game?.destroy(true, false);
  game = undefined;
  viewport = undefined;
}

onMounted(() => void mountGame());
onBeforeUnmount(destroyGame);

watch(
  () => props.binding,
  (binding) => runtime?.setBinding(binding),
);
watch(
  () => props.surface.key,
  () => {
    destroyGame();
    void mountGame();
  },
);
</script>

<template>
  <div
    ref="container"
    class="phaser-control-surface"
    role="application"
    :aria-label="surface.ariaLabel"
    :style="{ height: `${height}px` }"
  />
</template>
