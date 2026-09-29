export type Origin = "empty" | "demo" | "file" | "recovered";
export type PrimaryLayout = "standard" | "swapped";
export type Playback = "once" | "toggle-repeat" | "repeat-while-held";
export interface Stage {
  x: number;
  y: number;
  color: string;
}
export type Binding =
  | { type: "mouse" | "multimedia" | "windows-shortcut"; action: string }
  | { type: "keyboard"; key: string }
  | { type: "macro"; id: string }
  | { type: "disabled" };
export interface BindingChoice {
  label: string;
  binding: Binding;
  error: string | null;
}
export type MacroEvent =
  | { type: "key-down" | "key-up"; key: string; delay_ms: number }
  | {
      type: "mouse-button-down" | "mouse-button-up";
      button: string;
      delay_ms: number;
    };
export interface NamedMacro {
  source_id: string;
  name: string;
  playback: Playback;
  events: MacroEvent[];
}
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
  macros?: NamedMacro[];
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
  recovery_path: string | null;
  recovery_warning: string | null;
  pending_recovery: {
    token: string;
    profile_name: string | null;
    original_file: string | null;
    error: string | null;
  }[];
  dirty: boolean;
  can_undo: boolean;
  can_redo: boolean;
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
    bindings: number[];
    macros: {
      runtime: Playback[];
      onboard: Playback[];
      max_events: number;
      max_delay_ms: number;
    } | null;
  }[];
  binding_choices: BindingChoice[];
  macro_keys: string[];
  macro_mouse_buttons: string[];
  resolution_sources?: {
    source_id: string;
    error: string | null;
    targets: { id: string; name: string; error: string | null }[];
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
  | { kind: "button-binding"; control: string; binding: Binding | null }
  | { kind: "macro-create"; macro: NamedMacro }
  | {
      kind: "macro-replace";
      source_id: string;
      macro: NamedMacro;
      confirm_references: boolean;
    }
  | { kind: "macro-remove"; source_id: string }
  | { kind: "solid-zone"; zone: string; color: string }
  | {
      kind: "resolve-unresolved";
      source_id: string;
      control: string;
      macro_id: string;
      confirm: true;
    }
  | { kind: "omit-unresolved"; source_id: string; confirm: true }
  | { kind: "name"; name: string };
export type Page =
  "Device" | "Performance" | "Buttons" | "Macros" | "Lighting" | "Profiles";
