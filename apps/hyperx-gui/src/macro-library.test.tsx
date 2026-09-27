import { afterEach, describe, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import fixture from "./demo.json";
import { MacroLibrary } from "./macro-library";
import type { Snapshot } from "./types";

afterEach(cleanup);
function data(): Snapshot {
  const snapshot = structuredClone(fixture) as Snapshot;
  snapshot.macro_keys = ["a", "b", "left-shift", "right-shift", "enter"];
  snapshot.macro_mouse_buttons = ["left", "right", "middle"];
  snapshot.profile.buttons = {};
  return snapshot;
}
function mount(snapshot = data(), disabled = false, success = true) {
  const edit = vi.fn(async () => success);
  const onDraftChange = vi.fn();
  const view = render(
    <MacroLibrary
      snapshot={snapshot}
      disabled={disabled}
      edit={edit}
      onDraftChange={onDraftChange}
    />,
  );
  return { ...view, edit, onDraftChange, snapshot };
}
async function openExisting() {
  await userEvent.click(
    screen.getByRole("button", { name: "Edit macro AB, 20 ms" }),
  );
}

describe("offline named macro timeline editor", () => {
  it("creates a chord with separate downs/ups and individual delays through one typed edit", async () => {
    const { edit, onDraftChange } = mount();
    await userEvent.click(screen.getByRole("button", { name: "New macro" }));
    fireEvent.change(screen.getByLabelText("Macro name"), {
      target: { value: "Shift A" },
    });
    await userEvent.selectOptions(
      screen.getByLabelText("Macro playback"),
      "repeat-while-held",
    );
    const events = [
      { type: "key-down", key: "left-shift", delay_ms: 0 },
      { type: "key-down", key: "a", delay_ms: 7 },
      { type: "key-up", key: "a", delay_ms: 31 },
      { type: "key-up", key: "left-shift", delay_ms: 0 },
    ];
    for (const [index, event] of events.entries()) {
      await userEvent.click(screen.getByRole("button", { name: "Add event" }));
      await userEvent.selectOptions(
        screen.getByLabelText(`Event ${index + 1} type`),
        event.type,
      );
      await userEvent.selectOptions(
        screen.getByLabelText(`Event ${index + 1} key`),
        event.key,
      );
      fireEvent.change(screen.getByLabelText(`Event ${index + 1} delay`), {
        target: { value: event.delay_ms },
      });
    }
    expect(edit).not.toHaveBeenCalled();
    expect(onDraftChange).toHaveBeenLastCalledWith(true);
    // Filtering never loses the selected key and is sourced from backend metadata.
    fireEvent.change(screen.getByLabelText("Search macro keys"), {
      target: { value: "shift" },
    });
    expect(
      (screen.getByLabelText("Event 2 key") as HTMLSelectElement).value,
    ).toBe("a");
    expect(screen.getByLabelText("Event 1 key").textContent).toContain(
      "right-shift",
    );
    expect(screen.getByLabelText("Event 1 key").textContent).not.toContain(
      "enter",
    );
    await userEvent.click(
      screen.getByRole("button", { name: "Add macro to file" }),
    );
    expect(edit).toHaveBeenCalledExactlyOnceWith({
      kind: "macro-create",
      macro: {
        source_id: expect.stringMatching(/^macro-/),
        name: "Shift A",
        playback: "repeat-while-held",
        events,
      },
    });
    await waitFor(() => expect(onDraftChange).toHaveBeenLastCalledWith(false));
  });
  it("requires fresh explicit confirmation when replacing resolved or unresolved references", async () => {
    const snapshot = data();
    snapshot.profile.buttons = { button4: { type: "macro", id: "ab" } };
    snapshot.profile.unresolved_button_assignments = [
      { source_id: "unknown-target", macro_source_id: "ab" },
    ];
    const original = structuredClone(snapshot.profile);
    const { edit } = mount(snapshot);
    await openExisting();
    fireEvent.change(screen.getByLabelText("Macro name"), {
      target: { value: "Renamed AB" },
    });
    expect(
      (
        screen.getByRole("button", {
          name: "Update file macro",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(true);
    expect(
      screen.getByText(/Replacing this definition also changes/).textContent,
    ).toContain("unresolved source unknown-target");
    const confirm = screen.getByRole("checkbox", {
      name: /I confirm replacing/,
    });
    await userEvent.click(confirm);
    fireEvent.change(screen.getByLabelText("Event 1 delay"), {
      target: { value: "99" },
    });
    expect((confirm as HTMLInputElement).checked).toBe(false);
    await userEvent.click(confirm);
    await userEvent.click(
      screen.getByRole("button", { name: "Update file macro" }),
    );
    expect(edit).toHaveBeenCalledExactlyOnceWith({
      kind: "macro-replace",
      source_id: "ab",
      macro: {
        ...original.macros![0],
        name: "Renamed AB",
        events: original.macros![0].events.map((event, index) =>
          index === 0 ? { ...event, delay_ms: 99 } : event,
        ),
      },
      confirm_references: true,
    });
    expect(snapshot.profile).toEqual(original);
    expect(
      (
        screen.getByRole("button", {
          name: "Remove macro AB, 20 ms",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(true);
  });
  it("reorders and removes events without mutating the original definition", async () => {
    const { snapshot, edit } = mount();
    const original = structuredClone(snapshot.profile.macros![0]);
    await openExisting();
    await userEvent.click(
      screen.getByRole("button", { name: "Move event 3 up" }),
    );
    expect(
      (screen.getByLabelText("Event 2 key") as HTMLSelectElement).value,
    ).toBe("b");
    await userEvent.click(
      screen.getByRole("button", { name: "Remove event 4" }),
    );
    await userEvent.selectOptions(
      screen.getByLabelText("Macro playback"),
      "toggle-repeat",
    );
    await userEvent.click(
      screen.getByRole("button", { name: "Update file macro" }),
    );
    expect(edit).toHaveBeenCalledExactlyOnceWith({
      kind: "macro-replace",
      source_id: "ab",
      confirm_references: false,
      macro: {
        ...original,
        playback: "toggle-repeat",
        events: [original.events[0], original.events[2], original.events[1]],
      },
    });
    expect(snapshot.profile.macros![0]).toEqual(original);
  });
  it("supports mouse-button events and incomplete offline drafts without asserting hardware readiness", async () => {
    const { edit } = mount();
    await userEvent.click(screen.getByRole("button", { name: "New macro" }));
    await userEvent.click(screen.getByRole("button", { name: "Add event" }));
    await userEvent.selectOptions(
      screen.getByLabelText("Event 1 type"),
      "mouse-button-down",
    );
    await userEvent.selectOptions(
      screen.getByLabelText("Event 1 mouse button"),
      "middle",
    );
    fireEvent.change(screen.getByLabelText("Event 1 delay"), {
      target: { value: "65535" },
    });
    await userEvent.click(
      screen.getByRole("button", { name: "Add macro to file" }),
    );
    expect(edit).toHaveBeenCalledExactlyOnceWith({
      kind: "macro-create",
      macro: {
        source_id: expect.any(String),
        name: "New macro",
        playback: "once",
        events: [
          { type: "mouse-button-down", button: "middle", delay_ms: 65535 },
        ],
      },
    });
  });
  it("preserves imported aliases, rejects non-u16 delay text, and retains drafts after failed IPC", async () => {
    const snapshot = data();
    snapshot.profile.macros![0].events[0] = {
      type: "key-down",
      key: "RETURN",
      delay_ms: 10000,
    };
    const { edit } = mount(snapshot, false, false);
    await openExisting();
    expect(
      (screen.getByLabelText("Event 1 key") as HTMLSelectElement).value,
    ).toBe("RETURN");
    expect(screen.getByLabelText("Event 1 key").textContent).toContain(
      "RETURN · imported value",
    );
    fireEvent.change(screen.getByLabelText("Event 1 delay"), {
      target: { value: "65536" },
    });
    expect(
      (
        screen.getByRole("button", {
          name: "Update file macro",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(true);
    fireEvent.change(screen.getByLabelText("Event 1 delay"), {
      target: { value: "" },
    });
    expect(
      screen.getByLabelText("Event 1 delay").getAttribute("aria-invalid"),
    ).toBe("true");
    fireEvent.change(screen.getByLabelText("Event 1 delay"), {
      target: { value: "4" },
    });
    await userEvent.click(
      screen.getByRole("button", { name: "Update file macro" }),
    );
    expect(edit).toHaveBeenCalledOnce();
    expect(
      (screen.getByLabelText("Event 1 key") as HTMLSelectElement).value,
    ).toBe("RETURN");
    expect(
      (screen.getByLabelText("Event 1 delay") as HTMLInputElement).value,
    ).toBe("4");
    await userEvent.click(
      screen.getByRole("button", { name: "Discard timeline edits" }),
    );
    expect(
      screen.queryByRole("form", { name: "Edit macro timeline" }),
    ).toBeNull();
  });
  it("preserves local edits across unrelated revisions and blocks stale-definition replacement", async () => {
    const { snapshot, rerender, edit, onDraftChange } = mount();
    await openExisting();
    fireEvent.change(screen.getByLabelText("Macro name"), {
      target: { value: "My local draft" },
    });
    const next = structuredClone(snapshot);
    next.revision++;
    next.profile.polling = { hz: 250 };
    rerender(
      <MacroLibrary
        snapshot={next}
        disabled={false}
        edit={edit}
        onDraftChange={onDraftChange}
      />,
    );
    expect(
      (screen.getByLabelText("Macro name") as HTMLInputElement).value,
    ).toBe("My local draft");
    expect(screen.queryByRole("alert")).toBeNull();
    const conflict = structuredClone(next);
    conflict.profile.macros![0].name = "External change";
    rerender(
      <MacroLibrary
        snapshot={conflict}
        disabled={false}
        edit={edit}
        onDraftChange={onDraftChange}
      />,
    );
    expect(screen.getByRole("alert").textContent).toContain(
      "definition changed",
    );
    expect(
      (
        screen.getByRole("button", {
          name: "Update file macro",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(true);
    expect(
      (screen.getByLabelText("Macro name") as HTMLInputElement).value,
    ).toBe("My local draft");
    expect(edit).not.toHaveBeenCalled();
  });
  it("requires explicit deletion confirmation and refuses duplicate identifiers", async () => {
    const { edit, unmount } = mount();
    await userEvent.click(
      screen.getByRole("button", { name: "Remove macro AB, 20 ms" }),
    );
    expect(edit).not.toHaveBeenCalled();
    await userEvent.click(
      screen.getByRole("button", { name: "Cancel removal" }),
    );
    expect(edit).not.toHaveBeenCalled();
    await userEvent.click(
      screen.getByRole("button", { name: "Remove macro AB, 20 ms" }),
    );
    await userEvent.click(
      screen.getByRole("button", { name: "Confirm remove from file" }),
    );
    expect(edit).toHaveBeenCalledExactlyOnceWith({
      kind: "macro-remove",
      source_id: "ab",
    });
    unmount();
    const snapshot = data();
    snapshot.profile.macros!.push(structuredClone(snapshot.profile.macros![0]));
    mount(snapshot);
    expect(screen.getAllByText(/Duplicate identifier/)).toHaveLength(2);
    expect(
      screen
        .getAllByRole("button", { name: "Edit macro AB, 20 ms" })
        .every((button) => (button as HTMLButtonElement).disabled),
    ).toBe(true);
    expect(
      screen
        .getAllByRole("button", { name: "Remove macro AB, 20 ms" })
        .every((button) => (button as HTMLButtonElement).disabled),
    ).toBe(true);
  });
  it("keeps browser preview read-only and exposes target-specific runtime/onboard limits", () => {
    const { edit } = mount(data(), true);
    expect(
      (screen.getByRole("button", { name: "New macro" }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);
    expect(
      (
        screen.getByRole("button", {
          name: "Edit macro AB, 20 ms",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(true);
    expect(
      screen.getByText(/Runtime and onboard playback support are separate/),
    ).toBeTruthy();
    expect(
      screen.getAllByText(/capture-backed maximum/).length,
    ).toBeGreaterThan(0);
    expect(edit).not.toHaveBeenCalled();
  });
});
