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
