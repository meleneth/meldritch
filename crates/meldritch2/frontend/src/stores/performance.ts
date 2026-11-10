import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { PerformanceSocket } from "../transport/websocket";
import { credentialFromLocation } from "../transport/credential";
import type {
  AdsrControl,
  AccessLink,
  AudioTelemetry,
  Command,
  NoteEvent,
  Plane,
  Snapshot,
} from "../types/protocol";
export const usePerformanceStore = defineStore("performance", () => {
  const linkedToken = credentialFromLocation(window.location);
  const status = ref("connecting"),
    revision = ref(-1),
    schemaRevision = ref(-1),
    epoch = ref(""),
    name = ref("Meldritch"),
    role = ref<string | null>(null),
    planes = ref<Plane[]>([]),
    audio = ref<AudioTelemetry | null>(null),
    pending = ref(new Map<string, unknown>()),
    errors = ref<string[]>([]),
    accessLinks = ref<AccessLink[]>([]),
    token = ref(linkedToken);
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
            "pattern" | "step_count" | "notes" | "position"
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
    // Updates are complete authoritative value projections, not patches. A busy
    // composition may publish again while an authenticated schema snapshot is
    // in flight, so skipped revisions are safe and the newest projection wins.
    epoch.value = incomingEpoch;
    revision.value = message.revision;
    schemaRevision.value = message.schema_revision ?? schemaRevision.value;
    name.value = message.state.name || name.value;
    if (message.state.audio !== undefined) audio.value = message.state.audio;
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
      (links) => (accessLinks.value = links),
    );
    socket.connect(token.value);
  }
  function authenticate(value: string) {
    token.value = value;
    connect();
  }
  function send(command: Command, key?: string, value?: unknown) {
    if (key) pending.value.set(key, value);
    socket.send(command);
  }
  function patchControl(
    planeId: string,
    controlId: string,
    patch: Record<string, unknown>,
  ) {
    planes.value = planes.value.map((plane) =>
      plane.id !== planeId
        ? plane
        : {
            ...plane,
            controls: plane.controls.map((control) =>
              control.id === controlId
                ? ({ ...control, ...patch } as typeof control)
                : control,
            ),
          },
    );
  }
  const patchRange = (planeId: string, controlId: string, value: number) =>
    patchControl(planeId, controlId, { value });
  const patchNotes = (planeId: string, controlId: string, notes: NoteEvent[]) =>
    patchControl(planeId, controlId, { notes });
  const patchAdsr = (
    planeId: string,
    controlId: string,
    values: AdsrControl["values"],
  ) => patchControl(planeId, controlId, { values });
  return {
    status,
    revision,
    schemaRevision,
    name,
    role,
    planes,
    audio,
    visiblePlanes,
    pending,
    errors,
    accessLinks,
    connect,
    authenticate,
    send,
    patchRange,
    patchNotes,
    patchAdsr,
    connected: computed(() => status.value === "connected"),
    applySnapshot,
  };
});
