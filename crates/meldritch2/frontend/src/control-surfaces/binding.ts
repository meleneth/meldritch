import type { WatchStopHandle } from "vue";

export type ChangeOrigin = "surface" | "external" | "reconcile";

export interface BindingChange<T> {
  value: T;
  origin: ChangeOrigin;
  revision?: number;
}

export interface GestureInfo {
  id: string;
  label?: string;
}

export interface SurfaceBinding<T> {
  readonly readOnly: boolean;
  read(): T;
  subscribe(listener: (change: BindingChange<T>) => void): () => void;
  write(value: T, origin?: ChangeOrigin): void;
  beginGesture(info: GestureInfo): void;
  endGesture(info: GestureInfo, cancelled?: boolean): void;
  dispose(): void;
}

export interface BindingAdapterOptions<T> {
  read: () => T;
  write: (value: T) => void;
  subscribe: (listener: (value: T) => void) => WatchStopHandle | (() => void);
  readOnly?: boolean;
  equals?: (left: T, right: T) => boolean;
  optimistic?: (value: T) => void;
  beginTransaction?: (info: GestureInfo) => void;
  endTransaction?: (info: GestureInfo, cancelled: boolean) => void;
}

/**
 * Adapts authoritative application state to a surface without teaching the
 * surface about Pinia. Writes are synchronous. An authoritative update that
 * arrives during a gesture is delivered after the gesture finishes.
 */
export function createSurfaceBinding<T>(
  options: BindingAdapterOptions<T>,
): SurfaceBinding<T> {
  const listeners = new Set<(change: BindingChange<T>) => void>();
  const equals = options.equals ?? Object.is;
  let gestureDepth = 0;
  let disposed = false;
  let lastSurfaceValue: T | undefined;
  let queuedExternal: T | undefined;

  const stop = options.subscribe((value) => {
    if (disposed) return;
    if (lastSurfaceValue !== undefined && equals(value, lastSurfaceValue)) {
      lastSurfaceValue = undefined;
      return;
    }
    if (gestureDepth > 0) {
      queuedExternal = value;
      return;
    }
    for (const listener of listeners) listener({ value, origin: "external" });
  });

  return {
    readOnly: options.readOnly ?? false,
    read: options.read,
    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    write(value, origin = "surface") {
      if (disposed || options.readOnly) return;
      lastSurfaceValue = value;
      options.optimistic?.(value);
      options.write(value);
      for (const listener of listeners) listener({ value, origin });
    },
    beginGesture(info) {
      if (disposed || options.readOnly) return;
      if (gestureDepth++ === 0) options.beginTransaction?.(info);
    },
    endGesture(info, cancelled = false) {
      if (disposed || options.readOnly || gestureDepth === 0) return;
      if (--gestureDepth === 0) {
        options.endTransaction?.(info, cancelled);
        if (queuedExternal !== undefined) {
          const value = queuedExternal;
          queuedExternal = undefined;
          lastSurfaceValue = undefined;
          for (const listener of listeners)
            listener({ value, origin: "reconcile" });
        }
      }
    },
    dispose() {
      if (disposed) return;
      disposed = true;
      stop();
      listeners.clear();
      queuedExternal = undefined;
    },
  };
}

export interface CompositeBindingValue {
  [key: string]: number;
}

export function composeBindings(
  bindings: Record<string, SurfaceBinding<number>>,
): SurfaceBinding<CompositeBindingValue> {
  const listeners = new Set<
    (change: BindingChange<CompositeBindingValue>) => void
  >();
  const stops = Object.entries(bindings).map(([key, binding]) =>
    binding.subscribe((change) => {
      const value = Object.fromEntries(
        Object.entries(bindings).map(([name, item]) => [name, item.read()]),
      );
      value[key] = change.value;
      for (const listener of listeners)
        listener({ value, origin: change.origin });
    }),
  );
  return {
    readOnly: Object.values(bindings).every((binding) => binding.readOnly),
    read: () =>
      Object.fromEntries(
        Object.entries(bindings).map(([key, binding]) => [key, binding.read()]),
      ),
    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    write(values, origin) {
      for (const [key, value] of Object.entries(values))
        bindings[key]?.write(value, origin);
    },
    beginGesture(info) {
      for (const binding of Object.values(bindings)) binding.beginGesture(info);
    },
    endGesture(info, cancelled) {
      for (const binding of Object.values(bindings))
        binding.endGesture(info, cancelled);
    },
    dispose() {
      stops.forEach((stop) => stop());
      Object.values(bindings).forEach((binding) => binding.dispose());
      listeners.clear();
    },
  };
}
