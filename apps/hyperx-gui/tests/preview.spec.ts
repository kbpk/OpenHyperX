import { expect, test } from "@playwright/test";
import process from "node:process";
import fixture from "../src/demo.json" with { type: "json" };

test("preview navigation stays offline and read-only", async ({ page }) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto("/");
  await expect(
    page.getByRole("heading", { name: "Performance", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("slider", { name: "Stage 1 DPI slider" }),
  ).toBeDisabled();
  await expect(
    page.getByRole("button", { name: "Apply", exact: true }),
  ).toBeDisabled();
  await expect(
    page.getByRole("button", { name: "Save to mouse" }),
  ).toBeDisabled();
  if (process.env.GUI_SCREENSHOTS) {
    await page.screenshot({
      path: `${process.env.GUI_SCREENSHOTS}/openhyperx-performance.png`,
      fullPage: true,
    });
  }
  for (const name of ["Device", "Buttons", "Macros", "Lighting", "Profiles"]) {
    await page.getByRole("button", { name, exact: true }).click();
    await expect(
      page.getByRole("heading", { name, exact: true }),
    ).toBeVisible();
    if (name === "Lighting" && process.env.GUI_SCREENSHOTS) {
      await page.screenshot({
        path: `${process.env.GUI_SCREENSHOTS}/openhyperx-lighting.png`,
        fullPage: true,
      });
    }
  }
  await page.getByRole("button", { name: "Review", exact: true }).click();
  await expect(page.getByRole("dialog")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toHaveCount(0);
  expect(errors).toEqual([]);
});

test("real graphical hit targets select all 11 controls, without edits", async ({
  page,
}) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Buttons", exact: true }).click();
  const details = page.getByRole("region", { name: "Selected control" });
  for (const control of fixture.controls) {
    const pin = page.getByRole("button", {
      name: `Select ${control.name} on mouse`,
    });
    // Click visible SVG pin pixels, not a programmatically dispatched event.
    await pin.locator("circle").click();
    await expect(pin).toHaveAttribute("aria-pressed", "true");
    await expect(
      page.getByRole("button", { name: `Select ${control.name} in table` }),
    ).toHaveAttribute("aria-pressed", "true");
    await expect(
      details.getByRole("heading", { name: control.name }),
    ).toBeVisible();
  }
  const side = page.getByRole("button", { name: "Select Button 4 on mouse" });
  await page.getByRole("button", { name: "Select Button 4 in table" }).click();
  await expect(side).toHaveAttribute("aria-pressed", "true");
  await side.focus();
  await page.keyboard.press("Space");
  await expect(side).toBeFocused();
  const dpi = page.getByRole("button", { name: "Select DPI button on mouse" });
  await dpi.focus();
  await page.keyboard.press("Enter");
  await expect(dpi).toHaveAttribute("aria-pressed", "true");
  await expect(side).toHaveAttribute("aria-pressed", "false");
  await side.hover();
  await expect(side).toHaveClass(/hovered/);
  await expect(dpi).toHaveAttribute("aria-pressed", "true");
  await side.locator("circle").click();
  await expect(
    page.getByText("No unsaved file changes", { exact: true }),
  ).toBeVisible();
  if (process.env.GUI_SCREENSHOTS) {
    await page.screenshot({
      path: `${process.env.GUI_SCREENSHOTS}/openhyperx-buttons.png`,
      fullPage: true,
    });
  }
});

test("layout fits the minimum supported window", async ({ page }) => {
  await page.setViewportSize({ width: 900, height: 640 });
  await page.goto("/");
  for (const name of ["Performance", "Lighting", "Buttons", "Profiles"]) {
    await page.getByRole("button", { name, exact: true }).click();
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= window.innerWidth,
      ),
    ).toBe(true);
    await expect(
      page.getByRole("button", { name: "Save new file" }),
    ).toBeVisible();
  }
});

test("bundled atlas loads and model views do not dirty the profile", async ({
  page,
}) => {
  await page.goto("/");
  const dimensions = await page.evaluate(async () => {
    const image = new Image();
    image.src = "/devices/pulsefire-raid/ngenuity-legacy-atlas.png";
    await image.decode();
    return [image.naturalWidth, image.naturalHeight];
  });
  expect(dimensions).toEqual([1914, 640]);
  await page.getByRole("button", { name: "Device", exact: true }).click();
  for (const angle of ["Left side", "Right side", "Top"]) {
    await page.getByRole("button", { name: angle, exact: true }).click();
    await expect(
      page.getByRole("img", {
        name: `Pulsefire Raid ${angle.toLowerCase()} manufacturer render`,
      }),
    ).toBeVisible();
    await expect(
      page.getByText("No unsaved file changes", { exact: true }),
    ).toBeVisible();
    if (process.env.GUI_SCREENSHOTS) {
      await page.screenshot({
        path: `${process.env.GUI_SCREENSHOTS}/openhyperx-device-${angle.toLowerCase().replaceAll(" ", "-")}.png`,
        fullPage: true,
      });
    }
  }
});
