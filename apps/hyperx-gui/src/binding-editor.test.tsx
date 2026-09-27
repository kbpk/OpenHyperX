import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { BindingEditor } from "./binding-editor";
import fixture from "./demo.json";
import type { Snapshot } from "./types";

afterEach(cleanup);
const snapshot = () => structuredClone(fixture) as Snapshot;
function editor(data = snapshot(), id = "button4", disabled = false) {
  const commit = vi.fn(async () => true);
  render(
    <BindingEditor
      snapshot={data}
      control={data.controls.find((control) => control.id === id)!}
      disabled={disabled}
      commit={commit}
    />,
  );
  return commit;
}
const submit = () =>
  screen.getByRole("button", { name: "Update file binding" });

describe("offline binding editor", () => {
  it("does not edit while changing categories/searching; requires an explicit option and submit", async () => {
    const commit = editor();
    await userEvent.selectOptions(
      screen.getByLabelText("Assignment category"),
      "keyboard",
    );
    expect((submit() as HTMLButtonElement).disabled).toBe(true);
    await userEvent.type(
      screen.getByLabelText("Search keyboard keys"),
      "shift",
    );
    const keys = screen.getByLabelText("Keyboard key choice");
    expect(
      within(keys)
        .getAllByRole("option")
        .map((option) => option.textContent),
    ).toEqual(["Choose an assignment", "LEFT SHIFT", "RIGHT SHIFT"]);
    expect(commit).not.toHaveBeenCalled();
    await userEvent.selectOptions(
      keys,
      within(keys).getByRole("option", { name: "LEFT SHIFT" }),
    );
    await userEvent.click(submit());
    expect(commit).toHaveBeenCalledExactlyOnceWith({
      type: "keyboard",
      key: "left-shift",
    });
  });

  it("distinguishes Disabled from explicit omission", async () => {
    const commit = editor();
    await userEvent.selectOptions(
      screen.getByLabelText("Assignment category"),
      "disabled",
    );
    await userEvent.click(submit());
    expect(commit).toHaveBeenLastCalledWith({ type: "disabled" });
    await userEvent.selectOptions(
      screen.getByLabelText("Assignment category"),
      "preserve",
    );
    await userEvent.click(submit());
    expect(commit).toHaveBeenLastCalledWith(null);
  });

  it("uses backend-provided mouse/media/shortcut choices, not client hardcoded capabilities", async () => {
    const data = snapshot();
    // A future driver's restricted catalog must result in restricted UI.
    const control = data.controls.find((control) => control.id === "button4")!;
    control.bindings = control.bindings.filter(
      (index) => data.binding_choices[index].binding.type === "multimedia",
    );
    const commit = editor(data);
    expect(
      (
        screen.getByRole("option", {
          name: "Keyboard key",
        }) as HTMLOptionElement
      ).disabled,
    ).toBe(true);
    await userEvent.selectOptions(
      screen.getByLabelText("Assignment category"),
      "multimedia",
    );
    const choices = screen.getByLabelText("Multimedia choice");
    await userEvent.selectOptions(
      choices,
      within(choices).getByRole("option", { name: "Volume up" }),
    );
    await userEvent.click(submit());
    expect(commit).toHaveBeenCalledExactlyOnceWith({
      type: "multimedia",
      action: "volume-up",
    });
  });

  it("assigns an existing legal library macro and never edits its definition", async () => {
    const data = snapshot();
    const before = structuredClone(data);
    const commit = editor(data);
    await userEvent.selectOptions(
      screen.getByLabelText("Assignment category"),
      "macro",
    );
    const choices = screen.getByLabelText("Library macro choice");
    await userEvent.selectOptions(
      choices,
      within(choices).getByRole("option", {
        name: "AB, 20 ms · ab",
      }),
    );
    await userEvent.click(submit());
    expect(commit).toHaveBeenCalledExactlyOnceWith({ type: "macro", id: "ab" });
    expect(data).toEqual(before);
  });

  it("shows rejected target macros without making them assignable", async () => {
    const commit = editor(snapshot(), "dpi");
    await userEvent.selectOptions(
      screen.getByLabelText("Assignment category"),
      "macro",
    );
    const choices = screen.getByLabelText("Library macro choice");
    expect(
      (
        within(choices).getByRole("option", {
          name: /AB, 20 ms/,
        }) as HTMLOptionElement
      ).disabled,
    ).toBe(true);
    expect((submit() as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByText(/AB, 20 ms.*binding rejected/)).toBeTruthy();
    expect(commit).not.toHaveBeenCalled();
  });

  it("preserves imported aliases and keeps a selected key while filtering", async () => {
    const data = snapshot();
    data.profile.buttons!.button4 = { type: "keyboard", key: "RETURN" };
    const commit = editor(data);
    expect(
      screen.getByRole("option", { name: "Current imported value: RETURN" }),
    ).toBeTruthy();
    await userEvent.type(
      screen.getByLabelText("Search keyboard keys"),
      "shift",
    );
    expect((submit() as HTMLButtonElement).disabled).toBe(true);
    expect(commit).not.toHaveBeenCalled();
    const keys = screen.getByLabelText("Keyboard key choice");
    await userEvent.selectOptions(
      keys,
      within(keys).getByRole("option", { name: "LEFT SHIFT" }),
    );
    await userEvent.clear(screen.getByLabelText("Search keyboard keys"));
    await userEvent.type(
      screen.getByLabelText("Search keyboard keys"),
      "keypad",
    );
    await userEvent.click(submit());
    expect(commit).toHaveBeenCalledExactlyOnceWith({
      type: "keyboard",
      key: "left-shift",
    });
  });

  it("keeps preview/busy controls disabled", () => {
    editor(snapshot(), "button5", true);
    expect(
      (screen.getByLabelText("Assignment category") as HTMLSelectElement)
        .disabled,
    ).toBe(true);
    expect(
      (screen.getByLabelText("Library macro choice") as HTMLSelectElement)
        .disabled,
    ).toBe(true);
    expect((submit() as HTMLButtonElement).disabled).toBe(true);
  });
});
