import { afterEach, describe, expect, it } from "vitest";
import fixture from "./demo.json";
import {
  fieldMatches,
  readFieldRecovery,
  removeFieldDraft,
  writeFieldDraft,
  type FieldDraft,
} from "./field-recovery";
import type { Snapshot } from "./types";

afterEach(() => localStorage.clear());

const snapshot = structuredClone(fixture) as Snapshot;
const draft: FieldDraft = {
  field: "stage:0:dpi",
  value: "900",
  baseline: JSON.stringify(snapshot.profile.dpi?.stages[0]),
  path: snapshot.path,
  device: snapshot.profile.device,
  name: snapshot.profile.name,
};

describe("unfinished GUI field recovery", () => {
  it("stores bounded drafts separately and removes only the committed field", () => {
    writeFieldDraft(draft);
    writeFieldDraft({
      ...draft,
      field: "stage:0:color",
      value: "#112233",
    });
    expect(readFieldRecovery().drafts).toHaveLength(2);
    removeFieldDraft("stage:0:dpi");
    expect(readFieldRecovery().drafts.map((entry) => entry.field)).toEqual([
      "stage:0:color",
    ]);
  });

  it("never silently overwrites an invalid snapshot", () => {
    localStorage.setItem("openhyperx.gui.local-field-drafts.v1", "broken");
    expect(readFieldRecovery().error).toMatch(/unreadable/);
    expect(() => writeFieldDraft(draft)).toThrow();
    expect(localStorage.getItem("openhyperx.gui.local-field-drafts.v1")).toBe(
      "broken",
    );
  });

  it("restores only against the same source and original field value", () => {
    expect(fieldMatches(snapshot, draft)).toBe(true);
    expect(fieldMatches({ ...snapshot, path: "another.toml" }, draft)).toBe(
      false,
    );
    const changed = structuredClone(snapshot);
    changed.profile.dpi!.stages[0].x += 100;
    expect(fieldMatches(changed, draft)).toBe(false);
    expect(fieldMatches(snapshot, { ...draft, field: "raw:send" })).toBe(false);
  });
});
