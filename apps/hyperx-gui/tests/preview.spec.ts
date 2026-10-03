import { expect, test } from "@playwright/test";
import process from "node:process";
import fixture from "../src/demo.json" with { type: "json" };
import type { Edit, Snapshot } from "../src/types";

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

test("keyboard navigation and review dialog keep focus in the active surface", async ({
  page,
}) => {
  await page.goto("/");
  for (const name of [
    "Device",
    "Performance",
    "Buttons",
    "Macros",
    "Lighting",
    "Profiles",
  ]) {
    const section = page.getByRole("button", { name, exact: true });
    await section.focus();
    await page.keyboard.press("Enter");
    await expect(section).toHaveAttribute("aria-current", "page");
    await expect(
      page.getByRole("heading", { name, exact: true }),
    ).toBeVisible();
  }

  const review = page.getByRole("button", { name: "Review", exact: true });
  await review.focus();
  await page.keyboard.press("Space");
  const dialog = page.getByRole("dialog", { name: "Review file changes" });
  await expect(dialog).toBeVisible();
  const close = dialog.getByRole("button", { name: "Close dialog" });
  const done = dialog.getByRole("button", { name: "Done" });
  await close.focus();
  await page.keyboard.press("Shift+Tab");
  await expect(done).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(close).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(done).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(close).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
  await expect(review).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(dialog).toBeVisible();
  await dialog.getByRole("button", { name: "Done" }).click();
  await expect(dialog).toHaveCount(0);
  await expect(review).toBeFocused();
  await expect(
    page.getByText("No unsaved file changes", { exact: true }),
  ).toBeVisible();
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
  for (const name of [
    "Performance",
    "Lighting",
    "Buttons",
    "Macros",
    "Profiles",
  ]) {
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

test("macro library exposes exact timelines and safe native-only editing", async ({
  page,
}) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Macros", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Named macro library" }),
  ).toBeVisible();
  await expect(page.getByRole("button", { name: "New macro" })).toBeDisabled();
  for (const macro of fixture.profile.macros) {
    await expect(
      page.getByRole("button", {
        name: `Edit macro ${macro.name}`,
        exact: true,
      }),
    ).toBeDisabled();
    await expect(
      page.getByRole("button", {
        name: `Remove macro ${macro.name}`,
        exact: true,
      }),
    ).toBeDisabled();
  }
  await expect(
    page.getByText(/Runtime and onboard playback support are separate/),
  ).toBeVisible();
  await expect(
    page.getByRole("cell", { name: "key down", exact: true }),
  ).toHaveCount(2);
  await expect(
    page.getByRole("cell", { name: "key up", exact: true }),
  ).toHaveCount(2);
  await expect(
    page.getByRole("cell", { name: "20 ms", exact: true }),
  ).toHaveCount(4);
  await expect(
    page.getByText("No unsaved file changes", { exact: true }),
  ).toBeVisible();
  await page.setViewportSize({ width: 900, height: 640 });
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
  ).toBe(true);
  if (process.env.GUI_SCREENSHOTS)
    await page.screenshot({
      path: `${process.env.GUI_SCREENSHOTS}/openhyperx-macro-library.png`,
      fullPage: true,
    });
});

test("editable timeline is responsive and emits one exact typed macro, using mocked native IPC", async ({
  page,
}) => {
  await page.addInitScript((fixture) => {
    let current = structuredClone(fixture) as Snapshot;
    const calls: { command: string; args: Record<string, unknown> }[] = [];
    const target = window as unknown as {
      isTauri: boolean;
      __testCalls: typeof calls;
      __TAURI_INTERNALS__: {
        invoke(
          command: string,
          args: Record<string, unknown>,
        ): Promise<unknown>;
      };
    };
    target.isTauri = true;
    target.__testCalls = calls;
    target.__TAURI_INTERNALS__ = {
      async invoke(command, args) {
        calls.push({ command, args });
        if (command === "gui_set_local_draft") return;
        if (command === "gui_snapshot") return structuredClone(current);
        if (command === "gui_edit") {
          if (args.expectedRevision !== current.revision)
            throw new Error("stale revision");
          const edit = args.edit as Edit;
          if (edit.kind !== "macro-create") throw new Error("unexpected edit");
          current = {
            ...current,
            revision: current.revision + 1,
            dirty: true,
            profile: {
              ...current.profile,
              macros: [...(current.profile.macros ?? []), edit.macro],
            },
          };
          return structuredClone(current);
        }
        throw new Error(`unexpected IPC ${command}`);
      },
    };
  }, fixture);
  await page.setViewportSize({ width: 900, height: 640 });
  await page.goto("/");
  await page.getByRole("button", { name: "Macros", exact: true }).click();
  await page.getByRole("button", { name: "New macro", exact: true }).click();
  await page.getByLabel("Macro name").fill("Shift A");
  await page.getByLabel("Macro playback").selectOption("repeat-while-held");
  const events = [
    { type: "key-down", key: "left-shift", delay_ms: 0 },
    { type: "key-down", key: "a", delay_ms: 7 },
    { type: "key-up", key: "a", delay_ms: 31 },
    { type: "key-up", key: "left-shift", delay_ms: 0 },
  ];
  for (const [index, event] of events.entries()) {
    await page.getByRole("button", { name: "Add event", exact: true }).click();
    await page
      .getByLabel(`Event ${index + 1} type`, { exact: true })
      .selectOption(event.type);
    await page
      .getByLabel(`Event ${index + 1} key`, { exact: true })
      .selectOption(event.key);
    await page
      .getByLabel(`Event ${index + 1} delay`, { exact: true })
      .fill(String(event.delay_ms));
  }
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
  ).toBe(true);
  await expect(
    page.getByRole("button", { name: "Save new file" }),
  ).toBeDisabled();
  await expect(
    page.getByText("Uncommitted macro timeline", { exact: true }),
  ).toBeVisible();
  if (process.env.GUI_SCREENSHOTS)
    await page.screenshot({
      path: `${process.env.GUI_SCREENSHOTS}/openhyperx-macro-editor.png`,
      fullPage: true,
    });
  await page
    .getByRole("button", { name: "Add macro to file", exact: true })
    .click();
  await expect(
    page.getByRole("heading", { name: "Shift A", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Save new file" }),
  ).toBeEnabled();
  const edits = await page.evaluate(() =>
    (
      window as unknown as {
        __testCalls: { command: string; args: Record<string, unknown> }[];
      }
    ).__testCalls.filter((call) => call.command === "gui_edit"),
  );
  expect(edits).toEqual([
    {
      command: "gui_edit",
      args: {
        expectedRevision: 0,
        edit: {
          kind: "macro-create",
          macro: {
            source_id: expect.stringMatching(/^macro-/),
            name: "Shift A",
            playback: "repeat-while-held",
            events,
          },
        },
      },
    },
  ]);
});

test("binding inspection exposes backend choices but preview cannot edit", async ({
  page,
}) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Buttons", exact: true }).click();
  await expect(
    page.getByRole("form", { name: "Edit Left click binding" }),
  ).toHaveCount(0);
  await page
    .getByRole("button", { name: "Select Button 4 on mouse" })
    .locator("circle")
    .click();
  await expect(page.getByLabel("Assignment category")).toBeDisabled();
  await expect(page.getByLabel("Mouse function choice")).toBeDisabled();
  await expect(
    page.getByRole("button", { name: "Update file binding" }),
  ).toBeDisabled();
  await expect(
    page.getByText("No unsaved file changes", { exact: true }),
  ).toBeVisible();
  await page.setViewportSize({ width: 900, height: 640 });
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
  ).toBe(true);
  if (process.env.GUI_SCREENSHOTS) {
    await page.screenshot({
      path: `${process.env.GUI_SCREENSHOTS}/openhyperx-binding-editor.png`,
      fullPage: true,
    });
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
