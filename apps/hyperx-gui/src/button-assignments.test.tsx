import { afterEach, describe, expect, it } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  within,
} from "@testing-library/react";
import { ButtonAssignments } from "./button-assignments";
import { buttonLayout } from "./button-layout";
import { RAID_VIEWS } from "./mouse-art";
import fixture from "./demo.json";
import type { Snapshot } from "./types";

afterEach(cleanup);
const snapshot = () => structuredClone(fixture) as Snapshot;
const details = () =>
  within(screen.getByRole("region", { name: "Selected control" }));

describe("physical button selection", () => {
  it("maps each declared Raid control once, with pins inside its crop", () => {
    const layout = buttonLayout("pulsefire-raid");
    expect(layout.map((spot) => spot.id).sort()).toEqual(
      fixture.controls.map((control) => control.id).sort(),
    );
    expect(new Set(layout.map((spot) => spot.id)).size).toBe(11);
    for (const spot of layout) {
      expect(spot.pin[0]).toBeGreaterThan(0);
      expect(spot.pin[0]).toBeLessThan(RAID_VIEWS[spot.view].width);
      expect(spot.pin[1]).toBeGreaterThan(0);
      expect(spot.pin[1]).toBeLessThan(RAID_VIEWS[spot.view].height);
    }
    const side = layout.filter((spot) => spot.view === "left");
    expect(
      side
        .filter((spot) => spot.id === "button7" || spot.id === "button5")
        .every((spot) => spot.pin[0] < 335),
    ).toBe(true);
    expect(
      side
        .filter((spot) => spot.id === "button6" || spot.id === "button4")
        .every((spot) => spot.pin[0] > 335),
    ).toBe(true);
  });

  it("synchronizes every graphical control with the table and details", () => {
    const data = snapshot();
    const before = structuredClone(data);
    render(<ButtonAssignments snapshot={data} />);
    for (const control of data.controls) {
      fireEvent.click(
        screen.getByRole("button", { name: `Select ${control.name} on mouse` }),
      );
      expect(
        screen
          .getByRole("button", { name: `Select ${control.name} in table` })
          .getAttribute("aria-pressed"),
      ).toBe("true");
      expect(
        details().getByRole("heading", { name: control.name }),
      ).toBeTruthy();
    }
    expect(data).toEqual(before);
  });

  it("selects from the table and keyboard; hover does not replace selection", () => {
    render(<ButtonAssignments snapshot={snapshot()} />);
    const pin = screen.getByRole("button", {
      name: "Select Button 4 on mouse",
    });
    fireEvent.click(
      screen.getByRole("button", { name: "Select Button 4 in table" }),
    );
    expect(pin.getAttribute("aria-pressed")).toBe("true");
    const other = screen.getByRole("button", {
      name: "Select Button 5 on mouse",
    });
    fireEvent.mouseEnter(other);
    expect(other.classList.contains("hovered")).toBe(true);
    expect(pin.getAttribute("aria-pressed")).toBe("true");
    fireEvent.mouseLeave(other);
    expect(other.classList.contains("hovered")).toBe(false);
    fireEvent.keyDown(other, { key: " " });
    expect(other.getAttribute("aria-pressed")).toBe("true");
    fireEvent.keyDown(pin, { key: "Enter" });
    expect(pin.getAttribute("aria-pressed")).toBe("true");
  });

  it("shows file bindings, including macro definitions and coupled primary swaps", () => {
    const data = snapshot();
    data.profile.primary_buttons = "swapped";
    render(<ButtonAssignments snapshot={data} />);
    expect(
      details().getByText("Right click · swapped primary pair"),
    ).toBeTruthy();
    fireEvent.click(
      screen.getByRole("button", { name: "Select Button 5 on mouse" }),
    );
    expect(details().getByText("AB, 20 ms · once · 4 events")).toBeTruthy();
  });

  it("does not invent factory assignments or missing macro definitions", () => {
    const data = snapshot();
    data.profile.buttons = { button4: { type: "macro", id: "missing" } };
    render(<ButtonAssignments snapshot={data} />);
    fireEvent.click(
      screen.getByRole("button", { name: "Select Button 4 on mouse" }),
    );
    expect(
      details().getByText("Macro definition not present in this file."),
    ).toBeTruthy();
    fireEvent.click(
      screen.getByRole("button", { name: "Select Button 7 on mouse" }),
    );
    expect(details().getByText("Not specified")).toBeTruthy();
  });

  it("filters geometry against declared controls and recovers removed selections", () => {
    const data = snapshot();
    const { rerender } = render(<ButtonAssignments snapshot={data} />);
    fireEvent.click(
      screen.getByRole("button", { name: "Select Button 4 on mouse" }),
    );
    data.controls = data.controls.filter((control) => control.id !== "button4");
    rerender(<ButtonAssignments snapshot={data} />);
    expect(
      screen.queryByRole("button", { name: "Select Button 4 on mouse" }),
    ).toBeNull();
    expect(details().getByRole("heading", { name: "Left click" })).toBeTruthy();
  });

  it("keeps the table usable when the asset fails or the model has no layout", () => {
    const data = snapshot();
    const { container, rerender } = render(
      <ButtonAssignments snapshot={data} />,
    );
    fireEvent.error(container.querySelector("image")!);
    expect(
      screen.queryByRole("button", { name: "Select Left click on mouse" }),
    ).toBeNull();
    fireEvent.click(
      screen.getByRole("button", { name: "Select Left click in table" }),
    );
    expect(details().getByRole("heading", { name: "Left click" })).toBeTruthy();
    data.profile.device = "unknown-mouse";
    rerender(<ButtonAssignments snapshot={data} />);
    expect(screen.queryAllByRole("button", { name: /on mouse$/ })).toHaveLength(
      0,
    );
    fireEvent.click(
      screen.getByRole("button", { name: "Select Button 4 in table" }),
    );
    expect(details().getByRole("heading", { name: "Button 4" })).toBeTruthy();
    data.controls = [];
    rerender(<ButtonAssignments snapshot={data} />);
    expect(
      details().getByText("No controls declared for this model."),
    ).toBeTruthy();
  });
});
