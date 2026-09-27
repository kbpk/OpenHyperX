import { invoke, isTauri } from "@tauri-apps/api/core";
import fixture from "./demo.json";
import type { Edit, Snapshot } from "./types";

export interface Backend {
  desktop: boolean;
  request(command: string, args?: Record<string, unknown>): Promise<Snapshot>;
  setLocalDraft?(pending: boolean): Promise<void>;
}
export const desktopBackend: Backend = {
  desktop: isTauri(),
  async setLocalDraft(pending) {
    if (isTauri()) await invoke("gui_set_local_draft", { pending });
  },
  async request(command, args) {
    if (isTauri()) return invoke<Snapshot>(command, args);
    if (command === "gui_snapshot") return structuredClone(fixture) as Snapshot;
    throw new Error(
      "Browser preview is read-only. Run the native Tauri application to edit files.",
    );
  },
};

export interface SessionState {
  snapshot: Snapshot | null;
  busy: boolean;
  error: string | null;
}

/** Serialize IPC, choosing the revision at execution, never at stale render time.
 * Failed commands leave the last successful snapshot intact; no auto retry. */
export class GuiClient {
  state: SessionState = { snapshot: null, busy: false, error: null };
  private tail: Promise<unknown> = Promise.resolve();
  private pending = 0;
  private listeners = new Set<(state: SessionState) => void>();
  constructor(readonly backend: Backend) {}
  subscribe(listener: (state: SessionState) => void): () => void {
    this.listeners.add(listener);
    listener(this.state);
    return () => {
      this.listeners.delete(listener);
    };
  }
  private emit() {
    for (const listener of this.listeners) listener({ ...this.state });
  }
  request(
    command: string,
    args: Record<string, unknown> = {},
  ): Promise<Snapshot> {
    this.pending++;
    this.state = { ...this.state, busy: true, error: null };
    this.emit();
    const action = this.tail.then(async () => {
      const request =
        command === "gui_snapshot"
          ? args
          : { ...args, expectedRevision: this.state.snapshot?.revision };
      try {
        if (command !== "gui_snapshot" && !this.state.snapshot)
          throw new Error("Load an offline document first.");
        const snapshot = await this.backend.request(command, request);
        this.state = { ...this.state, snapshot, error: null };
        return snapshot;
      } catch (error) {
        this.state = {
          ...this.state,
          error: String(error instanceof Error ? error.message : error),
        };
        throw error;
      } finally {
        this.pending--;
        this.state = { ...this.state, busy: this.pending > 0 };
        this.emit();
      }
    });
    this.tail = action.catch(() => undefined);
    return action;
  }
  edit(edit: Edit) {
    return this.request("gui_edit", { edit });
  }
}
