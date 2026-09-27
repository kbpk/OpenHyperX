import { describe, expect, it, vi } from "vitest";
import fixture from "./demo.json";
import { desktopBackend, GuiClient } from "./bridge";
import type { Snapshot } from "./types";

const snapshot = () => structuredClone(fixture) as Snapshot;
describe("serialized offline IPC", () => {
  it("uses the latest successful revision for queued edits", async () => {
    let current = snapshot();
    const calls: Record<string, unknown>[] = [];
    const client = new GuiClient({
      desktop: true,
      async request(command, args) {
        if (command !== "gui_snapshot") {
          calls.push(args!);
          expect(args?.expectedRevision).toBe(current.revision);
          current = { ...current, revision: current.revision + 1, dirty: true };
        }
        return current;
      },
    });
    await client.request("gui_snapshot");
    const results = await Promise.all([
      client.edit({ kind: "polling", hz: 500 }),
      client.edit({ kind: "polling", hz: 250 }),
    ]);
    expect(results.map((item) => item.revision)).toEqual([1, 2]);
    expect(calls.map((item) => item.expectedRevision)).toEqual([0, 1]);
    expect(client.state.busy).toBe(false);
  });
  it("preserves the last good document on errors and never retries", async () => {
    const request = vi.fn(async (command: string) => {
      if (command === "gui_snapshot") return snapshot();
      throw new Error("invalid DPI");
    });
    const client = new GuiClient({ desktop: true, request });
    const initial = await client.request("gui_snapshot");
    await expect(
      client.edit({ kind: "stage-dpi", index: 0, dpi: 1 }),
    ).rejects.toThrow("invalid DPI");
    expect(client.state.snapshot).toBe(initial);
    expect(client.state.error).toBe("invalid DPI");
    expect(client.state.busy).toBe(false);
    expect(request).toHaveBeenCalledTimes(2);
  });
  it("clears busy after a request without an initial document", async () => {
    const request = vi.fn(async () => snapshot());
    const client = new GuiClient({ desktop: true, request });
    await expect(client.edit({ kind: "polling", hz: 500 })).rejects.toThrow(
      "Load an offline document",
    );
    expect(client.state.busy).toBe(false);
    expect(request).not.toHaveBeenCalled();
    await client.request("gui_snapshot");
    expect(client.state.error).toBeNull();
  });
  it("browser preview is a fixture only, with all mutations rejected", async () => {
    expect(desktopBackend.desktop).toBe(false);
    const first = await desktopBackend.request("gui_snapshot");
    first.profile.name = "tampered";
    expect((await desktopBackend.request("gui_snapshot")).profile.name).toBe(
      fixture.profile.name,
    );
    await expect(desktopBackend.request("gui_edit")).rejects.toThrow(
      "read-only",
    );
    await expect(desktopBackend.request("gui_save_profile")).rejects.toThrow(
      "read-only",
    );
  });
});
