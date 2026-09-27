export type Origin = "empty" | "demo" | "file";
export type PrimaryLayout = "standard" | "swapped";
export type Playback = "once" | "toggle-repeat" | "repeat-while-held";
export interface Stage {
  x: number;
  y: number;
  color: string;
}
export type Binding = {
  type: string;
  action?: string;
  key?: string;
  id?: string;
};
export type MacroEvent = {
  type: string;
  key?: string;
  button?: string;
  delay_ms: number;
};
export interface Profile {
  name: string;
  device: string;
  partial: boolean;
  source: { format: string; format_version: number } | null;
  dpi: {
    stages: Stage[];
    active_stage: number | null;
    source_active_stage: number | null;
  } | null;
  polling: { hz: number } | null;
  primary_buttons: PrimaryLayout | null;
  buttons?: Record<string, Binding>;
  lighting: { mode: "solid"; zones: Record<string, string> } | null;
  macros?: {
    source_id: string;
    name: string;
    playback: Playback;
    events: MacroEvent[];
  }[];
  unresolved_button_assignments?: {
    source_id: string;
    macro_source_id: string | null;
  }[];
}
export interface Change {
  field: string;
  before: string | null;
  after: string | null;
}
export interface Snapshot {
  revision: number;
  origin: Origin;
  path: string | null;
  dirty: boolean;
  profile: Profile;
  readiness: { error: string | null; warnings: string[] };
  changes: { settings: Change[]; metadata: Change[]; error: string | null };
  capabilities: {
    name: string;
    button_count: number;
    dpi: {
      minimum: number;
      maximum: number;
      step: number;
      max_stages: number;
    } | null;
    polling_rates: number[];
    zones: { id: string; name: string }[];
  } | null;
  controls: {
    id: string;
    name: string;
    primary: boolean;
    macros: {
      runtime: Playback[];
      onboard: Playback[];
      max_events: number;
      max_delay_ms: number;
    } | null;
  }[];
}
export type Edit =
  | { kind: "stage-dpi"; index: number; dpi: number }
  | { kind: "stage-color"; index: number; color: string }
  | { kind: "add-stage"; dpi: number; color: string }
  | { kind: "remove-last-stage" }
  | { kind: "active-stage"; index: number | null }
  | { kind: "polling"; hz: number | null }
  | { kind: "primary-buttons"; layout: PrimaryLayout | null }
  | { kind: "solid-zone"; zone: string; color: string }
  | { kind: "name"; name: string };
export type Page =
  "Device" | "Performance" | "Buttons" | "Macros" | "Lighting" | "Profiles";
