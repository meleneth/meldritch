import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { PerformanceSocket } from "../transport/websocket";
import type { Command, Plane, Snapshot } from "../types/protocol";
export const usePerformanceStore = defineStore("performance", () => {
  const status = ref("connecting"),
    revision = ref(-1),
    schemaRevision = ref(-1),
    epoch = ref(""),
    name = ref("Meldritch"),
    role = ref<string | null>(null),
    planes = ref<Plane[]>([]),
    pending = ref(new Map<string, unknown>()),
    errors = ref<string[]>([]),
    token = ref(sessionStorage.getItem("meldritch-token") || "");
  let socket: PerformanceSocket;
  function applyValues(values: Record<string, unknown> = {}) {
    planes.value = planes.value.map((plane) => ({
      ...plane,
      controls: plane.controls.map((control) => {
        const value = values[`${plane.id}/${control.id}`];
        if (value === undefined) return control;
        if (control.kind === "fader" || control.kind === "knob")
          return { ...control, value: Number(value) };
        if (control.kind === "toggle")
          return { ...control, value: Boolean(value) };
        if (control.kind === "step_sequencer") {
          const state = value as { steps: boolean[]; position: number | null };
          return { ...control, steps: state.steps, position: state.position };
        }
        if (control.kind === "piano_roll") {
          const state = value as Pick<
            typeof control,
            "pattern" | "notes" | "position"
          >;
          return { ...control, ...state };
        }
        if (control.kind === "drum_grid") {
          const state = value as {
            current_kit: string;
            pattern: number;
            rows: typeof control.state_rows;
            position: number | null;
          };
          return {
            ...control,
            current_kit: state.current_kit,
            pattern: state.pattern,
            state_rows: state.rows,
            position: state.position,
          };
        }
        if (control.kind === "adsr")
          return { ...control, values: value as Record<string, number> };
        if (control.kind === "pattern_selector")
          return { ...control, ...(value as Partial<typeof control>) };
        if (control.kind === "preset_selector")
          return { ...control, ...(value as Partial<typeof control>) };
        return { ...control, serial: (value as { serial: number }).serial };
      }),
    }));
  }
  function applySnapshot(message: Snapshot) {
    const incomingEpoch = message.epoch || "legacy";
    if (epoch.value && incomingEpoch !== epoch.value) {
      revision.value = -1;
      schemaRevision.value = -1;
      planes.value = [];
      pending.value.clear();
    }
    if (message.revision <= revision.value) return;
    if (
      revision.value >= 0 &&
      message.type === "state.updated" &&
      message.revision !== revision.value + 1
    ) {
      errors.value.push("Revision gap; resynchronizing");
      socket?.close();
      connect();
      return;
    }
    epoch.value = incomingEpoch;
    revision.value = message.revision;
    schemaRevision.value = message.schema_revision ?? schemaRevision.value;
    name.value = message.state.name || name.value;
    if (message.state.planes) planes.value = message.state.planes;
    applyValues(message.state.values);
    pending.value.clear();
  }
  // Rust has already projected the schema for the authenticated role. Keeping
  // that projection intact also preserves read-only observer planes.
  const visiblePlanes = computed(() => planes.value);
  function connect() {
    socket?.close();
    socket = new PerformanceSocket(
      applySnapshot,
      (s) => (status.value = s),
      (e) => errors.value.push(e),
      (r) => {
        role.value = r;
        revision.value = -1;
        planes.value = [];
      },
    );
    socket.connect(token.value);
  }
  function authenticate(value: string) {
    token.value = value;
    sessionStorage.setItem("meldritch-token", value);
    connect();
  }
  function send(command: Command, key?: string, value?: unknown) {
    if (key) pending.value.set(key, value);
    socket.send(command);
  }
  return {
    status,
    revision,
    schemaRevision,
    name,
    role,
    planes,
    visiblePlanes,
    pending,
    errors,
    connect,
    authenticate,
    send,
    connected: computed(() => status.value === "connected"),
    applySnapshot,
  };
});
