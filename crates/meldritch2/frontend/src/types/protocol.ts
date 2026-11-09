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
export interface ToggleControl extends BaseControl {
  kind: "toggle";
  value: boolean;
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
export interface DrumHit {
  id: number;
  step: number;
  velocity: number;
  duration: number;
}
export interface DrumRowDefinition {
  id: string;
  label: string;
  note: number;
}
export interface DrumRowState {
  id: string;
  muted: boolean;
  hits: DrumHit[];
}
export interface DrumGridControl extends BaseControl {
  kind: "drum_grid";
  kit: string;
  current_kit: string;
  step_count: number;
  rows: DrumRowDefinition[];
  pattern: number;
  state_rows: DrumRowState[];
  position: number | null;
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
export interface PresetControl extends BaseControl {
  kind: "preset_selector";
  presets: string[];
  entries: Array<{ id: string; name: string; bank: string; category: string }>;
  active: number;
  active_name?: string;
  pending: number | null;
  failed: string | null;
}
export type ControlDefinition =
  | RangeControl
  | ToggleControl
  | StepControl
  | TriggerControl
  | DrumGridControl
  | PianoRollControl
  | AdsrControl
  | PatternControl
  | PresetControl;
export interface Plane {
  id: string;
  label: string;
  layout?: string;
  roles?: string[];
  read_only?: boolean;
  controls: ControlDefinition[];
}
export interface AudioTelemetry {
  buffer_underruns: number;
  stream_errors: number;
  timed_note_errors: number;
  first_stream_error: string | null;
  sample_rate: number;
  channels: number;
  buffer_frames: number;
  dsp_workers: number;
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
    audio?: AudioTelemetry | null;
  };
}
export type Command =
  | { type: "control.set"; name: string; value: number | boolean | string }
  | { type: "step.toggle"; plane: string; control: string; step: number }
  | { type: "trigger.fire"; plane: string; control: string }
  | {
      type: "drum.hit.toggle";
      plane: string;
      control: string;
      pattern: number;
      row: string;
      step: number;
    }
  | {
      type: "drum.hit.velocity";
      plane: string;
      control: string;
      pattern: number;
      row: string;
      step: number;
      velocity: number;
    }
  | {
      type: "drum.mute";
      plane: string;
      control: string;
      row: string;
      muted: boolean;
    }
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
    }
  | {
      type: "preset.select";
      plane: string;
      control: string;
      preset: number | string;
    };
