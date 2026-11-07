export type Scale = "linear" | "log";
export interface BaseControl {
  id: string;
  label: string;
  kind: string;
  unit?: string;
  read_only?: boolean;
}
export interface RangeControl extends BaseControl {
  kind: "fader" | "knob";
  min: number;
  max: number;
  scale: Scale;
  value: number;
}
export interface StepControl extends BaseControl {
  kind: "step_sequencer";
  step_count: number;
  steps: boolean[];
  position: number | null;
}
export interface TriggerControl extends BaseControl {
  kind: "trigger";
  serial: number;
}
export interface NoteEvent {
  id: number;
  step: number;
  pitch: number;
  velocity: number;
  duration: number;
  legato: boolean;
}
export interface PianoRollControl extends BaseControl {
  kind: "piano_roll";
  step_count: number;
  pitch_min: number;
  pitch_max: number;
  voices: number;
  pattern: number;
  notes: NoteEvent[];
  position: number | null;
}
export interface AdsrParameter {
  name: "attack" | "decay" | "sustain" | "release";
  control: string;
  min: number;
  max: number;
  scale: Scale;
  unit: string;
}
export interface AdsrControl extends BaseControl {
  kind: "adsr";
  parameters: AdsrParameter[];
  values: Record<string, number>;
}
export interface PatternControl extends BaseControl {
  kind: "pattern_selector";
  patterns: string[];
  boundaries: Array<"immediate" | "beat" | "bar" | "cycle">;
  default_boundary: "immediate" | "beat" | "bar" | "cycle";
  active: number;
  active_name?: string;
  pending: number | null;
  pending_name?: string | null;
  boundary: "immediate" | "beat" | "bar" | "cycle";
}
export type ControlDefinition =
  | RangeControl
  | StepControl
  | TriggerControl
  | PianoRollControl
  | AdsrControl
  | PatternControl;
export interface Plane {
  id: string;
  label: string;
  layout?: string;
  roles?: string[];
  read_only?: boolean;
  controls: ControlDefinition[];
}
export interface Snapshot {
  type: "state.snapshot" | "state.updated";
  epoch?: string;
  revision: number;
  schema_revision?: number;
  state: {
    name: string;
    role?: string | null;
    planes?: Plane[];
    values?: Record<string, unknown>;
    controls?: Record<string, unknown>;
  };
}
export type Command =
  | { type: "control.set"; name: string; value: number | boolean | string }
  | { type: "step.toggle"; plane: string; control: string; step: number }
  | { type: "trigger.fire"; plane: string; control: string }
  | ({
      type: "note.create" | "note.update";
      plane: string;
      control: string;
      pattern: number;
    } & Omit<NoteEvent, "id" | "legato"> & { id?: number; legato?: boolean })
  | {
      type: "note.delete";
      plane: string;
      control: string;
      pattern: number;
      id: number;
    }
  | {
      type: "pattern.select";
      plane: string;
      control: string;
      pattern: number;
      boundary: "immediate" | "beat" | "bar" | "cycle";
    }
  | {
      type: "adsr.set";
      plane: string;
      control: string;
      parameter: AdsrParameter["name"];
      value: number;
    };
