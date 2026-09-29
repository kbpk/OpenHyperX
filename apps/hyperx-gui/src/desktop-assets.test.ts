import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const desktop = resolve(
  dirname(fileURLToPath(import.meta.url)),
  "../src-tauri",
);

describe("native desktop icons", () => {
  it("ships all declared icons, including RGBA PNG for non-Windows codegen", () => {
    const config = JSON.parse(
      readFileSync(resolve(desktop, "tauri.conf.json"), "utf8"),
    );
    expect(config.bundle.active).toBe(false);
    expect(config.bundle.icon).toEqual([
      "icons/icon.png",
      "icons/icon.ico",
      "icons/icon.icns",
    ]);
    const png = readFileSync(resolve(desktop, "icons/icon.png"));
    expect(png.subarray(0, 8).toString("hex")).toBe("89504e470d0a1a0a");
    expect(png.readUInt32BE(16)).toBe(png.readUInt32BE(20));
    expect(png[24]).toBe(8); // 8-bit channels
    expect(png[25]).toBe(6); // RGBA, required by Tauri's icon loader
    const ico = readFileSync(resolve(desktop, "icons/icon.ico"));
    expect(ico.subarray(0, 4).toString("hex")).toBe("00000100");
    const icns = readFileSync(resolve(desktop, "icons/icon.icns"));
    expect(icns.subarray(0, 4).toString("ascii")).toBe("icns");
    expect(icns.readUInt32BE(4)).toBe(icns.length);
  });
});

describe("offline desktop IPC capability", () => {
  it("lists every registered command in the build manifest and local-window capability", () => {
    const manifest = readFileSync(resolve(desktop, "build.rs"), "utf8");
    const capability = JSON.parse(
      readFileSync(resolve(desktop, "capabilities/main.json"), "utf8"),
    );
    for (const command of [
      "gui_snapshot",
      "gui_edit",
      "gui_set_local_draft",
      "gui_reset",
      "gui_open_profile",
      "gui_save_profile",
      "gui_overwrite_profile",
      "gui_undo_file_edit",
      "gui_redo_file_edit",
      "gui_restore_recovery",
      "gui_discard_recovery",
    ]) {
      expect(manifest).toContain(`"${command}"`);
      expect(capability.permissions).toContain(
        `allow-${command.replaceAll("_", "-")}`,
      );
    }
    expect(capability.windows).toEqual(["main"]);
    expect(capability.permissions).not.toContain("fs:default");
    expect(capability.permissions).not.toContain("shell:default");
  });
});
