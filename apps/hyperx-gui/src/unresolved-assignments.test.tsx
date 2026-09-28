import { afterEach, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import fixture from "./demo.json";
import { UnresolvedAssignments } from "./unresolved-assignments";
import type { Snapshot } from "./types";

afterEach(cleanup);

function imported(): Snapshot {
  const snapshot = structuredClone(fixture) as Snapshot;
  delete snapshot.profile.buttons?.button4;
  snapshot.profile.unresolved_button_assignments = [
    { source_id: "runtime:button4", macro_source_id: "ab" },
  ];
  snapshot.resolution_sources = [
    {
      source_id: "runtime:button4",
      error: null,
      targets: [
        { id: "button4", name: "Button 4", error: null },
        {
          id: "button5",
          name: "Button 5",
          error: "source belongs to Button 4",
        },
      ],
    },
  ];
  return snapshot;
}

it("requires explicit source, target, library macro and final confirmation before editing", async () => {
  const edit = vi.fn(async () => true);
  render(
    <UnresolvedAssignments
      snapshot={imported()}
      disabled={false}
      edit={edit}
    />,
  );
  const source = screen.getByRole("combobox", { name: "Imported source" });
  expect((source as HTMLSelectElement).value).toBe("");
  expect(edit).not.toHaveBeenCalled();
  await userEvent.selectOptions(source, "0");
  expect(screen.getByText(/Source hint: ab/)).toBeTruthy();
  const target = screen.getByRole("combobox", { name: "Physical target" });
  expect((target as HTMLSelectElement).value).toBe("");
  expect(
    (
      screen.getByRole("button", {
        name: "Review resolution",
      }) as HTMLButtonElement
    ).disabled,
  ).toBe(true);
  expect(
    (
      screen.getByRole("option", {
        name: /Button 5 — unavailable/,
      }) as HTMLOptionElement
    ).disabled,
  ).toBe(true);
  await userEvent.selectOptions(target, "button4");
  const macro = screen.getByRole("combobox", {
    name: "Existing library macro",
  });
  expect((macro as HTMLSelectElement).value).toBe("");
  await userEvent.selectOptions(macro, "ab");
  await userEvent.click(
    screen.getByRole("button", { name: "Review resolution" }),
  );
  expect(edit).not.toHaveBeenCalled();
  expect(screen.getByText(/Assign existing macro ab to Button 4/)).toBeTruthy();
  await userEvent.click(
    screen.getByRole("button", { name: "Confirm resolution" }),
  );
  expect(edit).toHaveBeenCalledExactlyOnceWith({
    kind: "resolve-unresolved",
    source_id: "runtime:button4",
    control: "button4",
    macro_id: "ab",
    confirm: true,
  });
});

it("keeps omission separate from Disabled and does nothing when cancelled", async () => {
  const edit = vi.fn(async () => true);
  render(
    <UnresolvedAssignments
      snapshot={imported()}
      disabled={false}
      edit={edit}
    />,
  );
  await userEvent.selectOptions(
    screen.getByRole("combobox", { name: "Imported source" }),
    "0",
  );
  await userEvent.click(
    screen.getByRole("button", { name: "Review omission" }),
  );
  expect(screen.getByText(/No button is disabled or reset/)).toBeTruthy();
  await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
  expect(edit).not.toHaveBeenCalled();
  await userEvent.click(
    screen.getByRole("button", { name: "Review omission" }),
  );
  await userEvent.click(
    screen.getByRole("button", { name: "Confirm omission" }),
  );
  expect(edit).toHaveBeenCalledExactlyOnceWith({
    kind: "omit-unresolved",
    source_id: "runtime:button4",
    confirm: true,
  });
});

it("shows duplicate or ambiguous source errors without offering file edits", async () => {
  const snapshot = imported();
  snapshot.resolution_sources![0] = {
    source_id: "runtime:button4",
    error: "source ID is ambiguous",
    targets: [],
  };
  const edit = vi.fn(async () => true);
  render(
    <UnresolvedAssignments snapshot={snapshot} disabled={false} edit={edit} />,
  );
  await userEvent.selectOptions(
    screen.getByRole("combobox", { name: "Imported source" }),
    "0",
  );
  expect(screen.getByText("source ID is ambiguous")).toBeTruthy();
  expect(
    (
      screen.getByRole("button", {
        name: "Review omission",
      }) as HTMLButtonElement
    ).disabled,
  ).toBe(true);
  expect(
    (
      screen.getByRole("button", {
        name: "Review resolution",
      }) as HTMLButtonElement
    ).disabled,
  ).toBe(true);
  expect(edit).not.toHaveBeenCalled();
});
