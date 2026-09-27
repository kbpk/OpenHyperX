import { afterEach, describe, expect, it } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { DeviceRender, MouseArt, RAID_ATLAS, RAID_VIEWS } from "./mouse-art";

afterEach(cleanup);
describe("manufacturer image presentation", () => {
  it("clips each atlas crop independently, including letterboxed SVG space", () => {
    const { container } = render(
      <>
        <MouseArt device="pulsefire-raid" />
        <MouseArt device="pulsefire-raid" view="left" />
      </>,
    );
    const images = container.querySelectorAll("image");
    const clips = container.querySelectorAll("clipPath");
    expect(clips).toHaveLength(2);
    expect(clips[0].id).not.toBe(clips[1].id);
    for (const [index, crop] of [RAID_VIEWS.top, RAID_VIEWS.left].entries()) {
      expect(images[index].getAttribute("clip-path")).toBe(
        `url(#${clips[index].id})`,
      );
      expect(clips[index].querySelector("rect")?.getAttribute("width")).toBe(
        String(crop.width),
      );
      expect(clips[index].querySelector("rect")?.getAttribute("height")).toBe(
        String(crop.height),
      );
    }
  });
  it("keeps the source atlas byte-identical, with all crops inside it", () => {
    const file = readFileSync(
      resolve(
        dirname(fileURLToPath(import.meta.url)),
        "../public",
        RAID_ATLAS.slice(1),
      ),
    );
    expect(createHash("sha256").update(file).digest("hex")).toBe(
      "e5011474275ba7a2518fdfe61dfc916790376ab76706279d092afb2b44481b06",
    );
    expect(file.readUInt32BE(16)).toBe(1914);
    expect(file.readUInt32BE(20)).toBe(640);
    for (const crop of Object.values(RAID_VIEWS)) {
      expect(crop.x).toBeGreaterThanOrEqual(0);
      expect(crop.y).toBeGreaterThanOrEqual(0);
      expect(crop.x + crop.width).toBeLessThanOrEqual(1914);
      expect(crop.y + crop.height).toBeLessThanOrEqual(640);
    }
  });
  it("adds only supplied file-color markers and never assumes missing zones", () => {
    const { container, rerender } = render(
      <MouseArt device="pulsefire-raid" logo="#0000FF" />,
    );
    expect(container.querySelector('[data-zone="wheel"]')).toBeNull();
    expect(
      container
        .querySelector('[data-zone="logo"] circle')
        ?.getAttribute("fill"),
    ).toBe("#0000FF");
    rerender(<MouseArt device="pulsefire-raid" wheel="#000000" />);
    expect(container.querySelector('[data-zone="logo"]')).toBeNull();
    expect(
      container
        .querySelector('[data-zone="wheel"] circle')
        ?.getAttribute("fill"),
    ).toBe("#000000");
  });
  it("switches side views locally without file edits or lighting markers", async () => {
    const { container } = render(<DeviceRender device="pulsefire-raid" />);
    await userEvent.click(screen.getByRole("button", { name: "Left side" }));
    expect(
      screen
        .getByRole("img", { name: /left side manufacturer render/ })
        .getAttribute("viewBox"),
    ).toBe("0 0 674 215");
    expect(container.querySelectorAll("[data-zone]")).toHaveLength(0);
    await userEvent.click(screen.getByRole("button", { name: "Right side" }));
    expect(
      screen
        .getByRole("img", { name: /right side manufacturer render/ })
        .getAttribute("viewBox"),
    ).toBe("0 0 674 211");
  });
  it("shows a text placeholder, not invented mouse art, when no render is available", () => {
    const { container, rerender } = render(<MouseArt device="unknown" />);
    expect(container.querySelector("image")).toBeNull();
    expect(screen.getByText("No image for this model.")).toBeTruthy();
    rerender(<MouseArt device="pulsefire-raid" />);
    fireEvent.error(container.querySelector("image")!);
    expect(container.querySelector("image")).toBeNull();
    expect(container.querySelector("svg")).toBeNull();
    expect(
      screen.getByRole("img", { name: "Mouse render unavailable" }),
    ).toBeTruthy();
    expect(screen.getByText("Mouse image could not be loaded.")).toBeTruthy();
  });
});
