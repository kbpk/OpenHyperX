import type { Snapshot } from "./types";

export type FieldDraft = {
  field: string;
  value: string;
  baseline: string;
  path: string | null;
  device: string;
  name: string;
};

type Stored = { version: 1; drafts: FieldDraft[] };
export type FieldRecovery = {
  drafts: FieldDraft[];
  error: string | null;
  unavailable: boolean;
};

const key = "openhyperx.gui.local-field-drafts.v1";
const maxBytes = 16 * 1024;
export const emptyFieldRecovery: FieldRecovery = {
  drafts: [],
  error: null,
  unavailable: false,
};

function isDraft(value: unknown): value is FieldDraft {
  if (typeof value !== "object" || value === null || Array.isArray(value))
    return false;
  const draft = value as Record<string, unknown>;
  return (
    typeof draft.field === "string" &&
    draft.field.length <= 80 &&
    typeof draft.value === "string" &&
    draft.value.length <= 256 &&
    typeof draft.baseline === "string" &&
    draft.baseline.length <= 1024 &&
    (draft.path === null || typeof draft.path === "string") &&
    typeof draft.device === "string" &&
    typeof draft.name === "string"
  );
}

export function readFieldRecovery(): FieldRecovery {
  let raw: string | null;
  try {
    raw = localStorage.getItem(key);
  } catch {
    return { drafts: [], error: null, unavailable: true };
  }
  if (raw === null) return emptyFieldRecovery;
  try {
    if (raw.length > maxBytes) throw new Error("Recovery exceeds size limit");
    const stored: unknown = JSON.parse(raw);
    if (
      typeof stored !== "object" ||
      stored === null ||
      (stored as Stored).version !== 1 ||
      !Array.isArray((stored as Stored).drafts) ||
      (stored as Stored).drafts.length > 16 ||
      !(stored as Stored).drafts.every(isDraft) ||
      new Set((stored as Stored).drafts.map((draft) => draft.field)).size !==
        (stored as Stored).drafts.length
    )
      throw new Error("Invalid recovery snapshot");
    return {
      drafts: (stored as Stored).drafts,
      error: null,
      unavailable: false,
    };
  } catch {
    return {
      drafts: [],
      error:
        "Local input recovery is unreadable. Discard it explicitly before editing; it was not overwritten.",
      unavailable: false,
    };
  }
}

export function writeFieldDraft(draft: FieldDraft): FieldRecovery {
  const recovery = readFieldRecovery();
  if (recovery.unavailable) throw new Error("Local storage is unavailable");
  if (recovery.error) throw new Error(recovery.error);
  const drafts = [
    ...recovery.drafts.filter((entry) => entry.field !== draft.field),
    draft,
  ];
  const raw = JSON.stringify({ version: 1, drafts });
  if (drafts.length > 16 || raw.length > maxBytes)
    throw new Error("Local input recovery exceeds its size limit");
  localStorage.setItem(key, raw);
  return { drafts, error: null, unavailable: false };
}

export function removeFieldDraft(field: string): FieldRecovery {
  const recovery = readFieldRecovery();
  if (recovery.unavailable) return recovery;
  if (recovery.error) throw new Error(recovery.error);
  const drafts = recovery.drafts.filter((draft) => draft.field !== field);
  if (drafts.length)
    localStorage.setItem(key, JSON.stringify({ version: 1, drafts }));
  else localStorage.removeItem(key);
  return { drafts, error: null, unavailable: false };
}

export function discardFieldRecovery(): void {
  localStorage.removeItem(key);
}

export function fieldMatches(snapshot: Snapshot, draft: FieldDraft): boolean {
  const profile = snapshot.profile;
  if (
    snapshot.path !== draft.path ||
    profile.device !== draft.device ||
    profile.name !== draft.name
  )
    return false;
  if (draft.field === "profile.name") return draft.baseline === profile.name;
  const stage = /^stage:(\d+):(dpi|color)$/.exec(draft.field);
  if (stage)
    return (
      draft.baseline ===
      JSON.stringify(profile.dpi?.stages[Number(stage[1])] ?? null)
    );
  const zone = /^zone:([a-z0-9_-]+):color$/.exec(draft.field);
  if (zone)
    return (
      !!snapshot.capabilities?.zones.some((entry) => entry.id === zone[1]) &&
      draft.baseline === (profile.lighting?.zones[zone[1]] ?? "")
    );
  return false;
}

export function fieldPage(
  field: string,
): "Profiles" | "Performance" | "Lighting" {
  if (field.startsWith("stage:")) return "Performance";
  if (field.startsWith("zone:")) return "Lighting";
  return "Profiles";
}
