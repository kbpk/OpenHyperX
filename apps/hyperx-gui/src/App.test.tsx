import { afterEach, describe, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import App from "./App";
import fixture from "./demo.json";
import type { Snapshot } from "./types";

afterEach(cleanup);
// jsdom has no native dialog implementation; browsers are covered by Playwright.
HTMLDialogElement.prototype.showModal = function () {
  this.setAttribute("open", "");
};
function fakeBackend(initial = structuredClone(fixture) as Snapshot) {
  let current = initial;
  const request = vi.fn(
    async (command: string, args?: Record<string, unknown>) => {
      if (command === "gui_edit") {
        current = { ...current, dirty: true, revision: current.revision + 1 };
      }
      if (command === "gui_save_profile")
        current = { ...current, dirty: false, revision: current.revision + 1 };
      if (command === "gui_reset") {
        current = { ...current, dirty: false, revision: current.revision + 1 };
      }
      void args;
      return current;
    },
  );
  return {
    desktop: true,
    request,
    setLocalDraft: vi.fn(async (_pending: boolean) => undefined),
  };
}
describe("offline profile UI", () => {
  it("exposes explicit imported-assignment resolution in Profiles through typed IPC", async () => {
    const imported = structuredClone(fixture) as Snapshot;
    delete imported.profile.buttons?.button4;
    imported.profile.unresolved_button_assignments = [
      { source_id: "runtime:button4", macro_source_id: "ab" },
    ];
    imported.resolution_sources = [
      {
        source_id: "runtime:button4",
        error: null,
        targets: [{ id: "button4", name: "Button 4", error: null }],
      },
    ];
    const backend = fakeBackend(imported);
    render(<App backend={backend} />);
    await screen.findByRole("slider", { name: "Stage 1 DPI slider" });
    await userEvent.click(screen.getByRole("button", { name: "Profiles" }));
    await userEvent.selectOptions(
      screen.getByRole("combobox", { name: "Imported source" }),
      "0",
    );
    await userEvent.selectOptions(
      screen.getByRole("combobox", { name: "Physical target" }),
      "button4",
    );
    await userEvent.selectOptions(
      screen.getByRole("combobox", { name: "Existing library macro" }),
      "ab",
    );
    await userEvent.click(
      screen.getByRole("button", { name: "Review resolution" }),
    );
    expect(backend.request).toHaveBeenCalledTimes(1);
    await userEvent.click(
      screen.getByRole("button", { name: "Confirm resolution" }),
    );
    await waitFor(() =>
      expect(backend.request).toHaveBeenCalledWith("gui_edit", {
        expectedRevision: 0,
        edit: {
          kind: "resolve-unresolved",
          source_id: "runtime:button4",
          control: "button4",
          macro_id: "ab",
          confirm: true,
        },
      }),
    );
  });
  it("retains uncommitted timelines across navigation/revisions and protects save/reset/native close", async () => {
    const backend = fakeBackend();
    render(<App backend={backend} />);
    await screen.findByRole("slider", { name: "Stage 1 DPI slider" });
    await userEvent.click(screen.getByRole("button", { name: /^Macros$/ }));
    await userEvent.click(screen.getByRole("button", { name: "New macro" }));
    fireEvent.change(screen.getByLabelText("Macro name"), {
      target: { value: "Untitled chord draft" },
    });
    await waitFor(() =>
      expect(backend.setLocalDraft).toHaveBeenLastCalledWith(true),
    );
    expect(screen.getByText("Uncommitted macro timeline")).toBeTruthy();
    expect(screen.queryByText("No unsaved file changes")).toBeNull();
    const trueCalls = backend.setLocalDraft.mock.calls.filter(
      ([pending]) => pending,
    ).length;
    fireEvent.change(screen.getByLabelText("Macro name"), {
      target: { value: "Named chord draft" },
    });
    await waitFor(() =>
      expect(
        backend.setLocalDraft.mock.calls.filter(([pending]) => pending).length,
      ).toBeGreaterThan(trueCalls),
    );
    await userEvent.click(screen.getByRole("button", { name: "Performance" }));
    expect(
      (
        screen.getByRole("button", {
          name: "Save new file",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(true);
    expect(
      (
        screen.getByRole("button", {
          name: "Open profile",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(true);
    await userEvent.click(screen.getByRole("button", { name: "250 Hz" }));
    await userEvent.click(screen.getByRole("button", { name: "Profiles" }));
    expect(
      (screen.getByRole("button", { name: "New draft" }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);
    expect(
      (screen.getByRole("button", { name: "Load demo" }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);
    await userEvent.click(
      screen.getByRole("button", { name: "Return to macro editor" }),
    );
    expect(
      (screen.getByLabelText("Macro name") as HTMLInputElement).value,
    ).toBe("Named chord draft");
    await userEvent.click(
      screen.getByRole("button", { name: "Add macro to file" }),
    );
    await waitFor(() =>
      expect(backend.request).toHaveBeenCalledWith("gui_edit", {
        expectedRevision: 1,
        edit: {
          kind: "macro-create",
          macro: {
            source_id: expect.stringMatching(/^macro-/),
            name: "Named chord draft",
            playback: "once",
            events: [],
          },
        },
      }),
    );
    await waitFor(() =>
      expect(backend.setLocalDraft).toHaveBeenLastCalledWith(false),
    );
    expect(
      (
        screen.getByRole("button", {
          name: "Save new file",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(false);
  });
  it("warns when the native local-draft close notification fails without losing the draft", async () => {
    const backend = fakeBackend();
    backend.setLocalDraft.mockRejectedValue(new Error("guard unavailable"));
    render(<App backend={backend} />);
    await screen.findByRole("slider", { name: "Stage 1 DPI slider" });
    await userEvent.click(screen.getByRole("button", { name: /^Macros$/ }));
    await userEvent.click(screen.getByRole("button", { name: "New macro" }));
    await screen.findByRole("alert");
    expect(screen.getByRole("alert").textContent).toContain(
      "native close guard",
    );
    expect(screen.getByLabelText("Macro name")).toBeTruthy();
    await userEvent.click(
      screen.getByRole("button", { name: "Discard timeline edits" }),
    );
    expect(
      screen.queryByRole("form", { name: "Edit macro timeline" }),
    ).toBeNull();
  });
  it("commits exactly the selected control's typed binding through existing IPC", async () => {
    const backend = fakeBackend();
    render(<App backend={backend} />);
    await screen.findByRole("slider", { name: "Stage 1 DPI slider" });
    await userEvent.click(screen.getByRole("button", { name: /^Buttons$/ }));
    await userEvent.click(
      screen.getByRole("button", { name: "Select Button 4 on mouse" }),
    );
    await userEvent.selectOptions(
      screen.getByLabelText("Assignment category"),
      "disabled",
    );
    expect(backend.request).toHaveBeenCalledTimes(1);
    await userEvent.click(
      screen.getByRole("button", { name: "Update file binding" }),
    );
    await waitFor(() =>
      expect(backend.request).toHaveBeenCalledWith("gui_edit", {
        expectedRevision: 0,
        edit: {
          kind: "button-binding",
          control: "button4",
          binding: { type: "disabled" },
        },
      }),
    );
    expect(backend.request).toHaveBeenCalledTimes(2);
  });
  it("selects graphic/table controls without edits or other IPC", async () => {
    const backend = fakeBackend();
    render(<App backend={backend} />);
    await screen.findByRole("slider", { name: "Stage 1 DPI slider" });
    await userEvent.click(screen.getByRole("button", { name: /^Buttons$/ }));
    await userEvent.click(
      screen.getByRole("button", { name: "Select Button 4 on mouse" }),
    );
    await userEvent.click(
      screen.getByRole("button", { name: "Select Wheel click in table" }),
    );
    expect(
      screen
        .getByRole("button", { name: "Select Wheel click on mouse" })
        .getAttribute("aria-pressed"),
    ).toBe("true");
    expect(backend.request).toHaveBeenCalledTimes(1);
    expect(
      screen.getByText("No unsaved file changes", { exact: true }),
    ).toBeTruthy();
  });
  it("shows stage metadata and forbids hardware actions", async () => {
    render(<App backend={fakeBackend()} />);
    await screen.findByRole("spinbutton", { name: "Stage 1 DPI" });
    expect(
      (screen.getByRole("button", { name: "Apply" }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);
    expect(
      (
        screen.getByRole("button", {
          name: "Save to mouse",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(true);
    expect(screen.getAllByRole("slider")).toHaveLength(3);
  });
  it("commits a slider once on release, via typed IPC", async () => {
    const backend = fakeBackend();
    render(<App backend={backend} />);
    const slider = await screen.findByRole("slider", {
      name: "Stage 1 DPI slider",
    });
    fireEvent.change(slider, { target: { value: "900" } });
    expect(backend.request).toHaveBeenCalledTimes(1);
    fireEvent.pointerUp(slider);
    await waitFor(() =>
      expect(backend.request).toHaveBeenCalledWith("gui_edit", {
        expectedRevision: 0,
        edit: { kind: "stage-dpi", index: 0, dpi: 900 },
      }),
    );
    expect(backend.request).toHaveBeenCalledTimes(2);
  });
  it("rejects invalid numeric text before IPC", async () => {
    const backend = fakeBackend();
    render(<App backend={backend} />);
    const input = await screen.findByRole("spinbutton", {
      name: "Stage 1 DPI",
    });
    await waitFor(() =>
      expect((input as HTMLInputElement).disabled).toBe(false),
    );
    fireEvent.change(input, { target: { value: "201" } });
    fireEvent.blur(input);
    await waitFor(() =>
      expect(input.getAttribute("aria-invalid")).toBe("true"),
    );
    expect(backend.request).toHaveBeenCalledTimes(1);
  });
  it("edits exactly one lighting zone, not an implicit all-zones command", async () => {
    const backend = fakeBackend();
    render(<App backend={backend} />);
    await screen.findByRole("slider", { name: "Stage 1 DPI slider" });
    await userEvent.click(screen.getByRole("button", { name: "Lighting" }));
    const picker = screen.getByLabelText("Logo color picker");
    fireEvent.change(picker, { target: { value: "#ff0000" } });
    await waitFor(() =>
      expect(backend.request).toHaveBeenCalledWith("gui_edit", {
        expectedRevision: 0,
        edit: { kind: "solid-zone", zone: "logo", color: "#FF0000" },
      }),
    );
  });
  it("requires an explicit discard when replacing a dirty draft", async () => {
    const backend = fakeBackend();
    render(<App backend={backend} />);
    await screen.findByRole("slider", { name: "Stage 1 DPI slider" });
    await userEvent.click(screen.getByRole("button", { name: "500 Hz" }));
    await screen.findByText("Unsaved file changes");
    await userEvent.click(screen.getByRole("button", { name: "Profiles" }));
    await userEvent.click(screen.getByRole("button", { name: "New draft" }));
    expect(screen.getByRole("dialog")).toBeTruthy();
    expect(
      backend.request.mock.calls.filter(([command]) => command === "gui_reset"),
    ).toHaveLength(0);
    await userEvent.click(
      screen.getByRole("button", { name: "Discard and continue" }),
    );
    await waitFor(() =>
      expect(backend.request).toHaveBeenCalledWith("gui_reset", {
        expectedRevision: 1,
        discardChanges: true,
        demo: false,
      }),
    );
  });
  it("uses a distinct save-new-file command, never hardware save", async () => {
    const backend = fakeBackend();
    render(<App backend={backend} />);
    await screen.findByRole("slider", { name: "Stage 1 DPI slider" });
    await userEvent.click(
      screen.getByRole("button", { name: "Save new file" }),
    );
    await waitFor(() =>
      expect(backend.request).toHaveBeenCalledWith("gui_save_profile", {
        expectedRevision: 0,
      }),
    );
    expect(
      backend.request.mock.calls.some(
        ([command]) => command.includes("mouse") || command.includes("apply"),
      ),
    ).toBe(false);
  });
  it("makes the browser preview read-only", async () => {
    render(<App />);
    const input = await screen.findByRole("spinbutton", {
      name: "Stage 1 DPI",
    });
    expect((input as HTMLInputElement).disabled).toBe(true);
    expect(
      (
        screen.getByRole("button", {
          name: "Open profile",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(true);
    expect(screen.getByText(/Read-only demo in your browser/)).toBeTruthy();
  });
});
