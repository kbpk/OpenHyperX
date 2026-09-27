import { useState } from "react";
import type { Binding, Snapshot } from "./types";

const categoryNames: Record<Binding["type"] | "preserve", string> = {
  preserve: "Not specified · preserve device assignment",
  mouse: "Mouse function",
  multimedia: "Multimedia",
  keyboard: "Keyboard key",
  "windows-shortcut": "Windows shortcut",
  macro: "Library macro",
  disabled: "Disabled",
};
const same = (left: Binding | null, right: Binding | null) =>
  left === null || right === null
    ? left === right
    : left.type === right.type && bindingName(left) === bindingName(right);
function bindingName(binding: Binding) {
  if (binding.type === "keyboard") return binding.key;
  if (binding.type === "macro") return binding.id;
  if (binding.type === "disabled") return "Disabled";
  return binding.action;
}

export function BindingEditor({
  snapshot,
  control,
  disabled,
  commit,
}: {
  snapshot: Snapshot;
  control: Snapshot["controls"][number];
  disabled: boolean;
  commit: (binding: Binding | null) => Promise<boolean>;
}) {
  const original = snapshot.profile.buttons?.[control.id] ?? null;
  const options = control.bindings
    .map((index) => snapshot.binding_choices[index])
    .filter((entry) => entry !== undefined);
  const initial = options.findIndex((entry) => same(entry.binding, original));
  const [category, setCategory] = useState<Binding["type"] | "preserve">(
    original?.type ?? "preserve",
  );
  const [choice, setChoice] = useState(
    initial >= 0 ? String(initial) : original ? "current" : "",
  );
  const [search, setSearch] = useState("");
  const selected =
    choice === "current" || choice === "" ? undefined : options[Number(choice)];
  const pending =
    category === "preserve"
      ? null
      : category === "disabled"
        ? { type: "disabled" as const }
        : choice === "current"
          ? original
          : choice !== ""
            ? selected?.binding
            : undefined;
  const valid = pending !== undefined && !selected?.error;
  const changed = valid && !same(pending, original);
  const available = new Set(options.map((entry) => entry.binding.type));
  const matching = options
    .map((entry, index) => ({ entry, index }))
    .filter(({ entry }) => entry.binding.type === category);
  const query = search.trim().toLowerCase();
  const filtered = matching.filter(
    ({ entry, index }) =>
      category !== "keyboard" ||
      entry.label.toLowerCase().includes(query) ||
      bindingName(entry.binding).toLowerCase().includes(query) ||
      String(index) === choice,
  );

  return (
    <form
      className="binding-editor"
      aria-label={`Edit ${control.name} binding`}
      onSubmit={(event) => {
        event.preventDefault();
        if (!disabled && changed && pending !== undefined) void commit(pending);
      }}
    >
      <div className="section-heading">
        <div>
          <span className="eyebrow">EDIT FILE ONLY</span>
          <h3>Assign {control.name}</h3>
        </div>
        <span className="badge">OFFLINE</span>
      </div>
      <label className="field-label">
        Assignment category
        <select
          aria-label="Assignment category"
          value={category}
          disabled={disabled}
          onChange={(event) => {
            setCategory(event.target.value as typeof category);
            setChoice("");
            setSearch("");
          }}
        >
          {Object.entries(categoryNames).map(([value, label]) => (
            <option
              key={value}
              value={value}
              disabled={
                value !== "preserve" &&
                !available.has(value as Binding["type"]) &&
                value !== original?.type
              }
            >
              {label}
            </option>
          ))}
        </select>
      </label>
      {category === "keyboard" && (
        <label className="field-label">
          Search named keys
          <input
            type="search"
            aria-label="Search keyboard keys"
            placeholder="A, shift, F12, keypad…"
            value={search}
            disabled={disabled}
            onChange={(event) => setSearch(event.target.value)}
          />
        </label>
      )}
      {category !== "preserve" && category !== "disabled" && (
        <label className="field-label">
          {categoryNames[category]}
          <select
            aria-label={`${categoryNames[category]} choice`}
            size={category === "keyboard" ? 6 : undefined}
            value={choice}
            disabled={disabled}
            onChange={(event) => setChoice(event.target.value)}
          >
            <option value="" disabled>
              Choose an assignment
            </option>
            {choice === "current" && original && (
              <option value="current">
                Current imported value: {bindingName(original)}
              </option>
            )}
            {filtered.map(({ entry, index }) => (
              <option
                key={index}
                value={String(index)}
                disabled={entry.error !== null}
              >
                {entry.label}
                {entry.error ? " · unavailable for this control" : ""}
              </option>
            ))}
          </select>
        </label>
      )}
      {category === "macro" && (
        <div className="binding-macro-notes">
          <p>
            Only existing definitions in this file. Editing/recording timelines
            is a separate step.
          </p>
          {matching
            .filter(({ entry }) => entry.error)
            .map(({ entry, index }) => (
              <p className="field-error" key={index}>
                {entry.label}: {entry.error}
              </p>
            ))}
        </div>
      )}
      {category === "preserve" && (
        <p className="binding-editor-help">
          Omit this control from the file. Applying a profile later preserves
          the device assignment — this is not Disabled or a factory reset.
        </p>
      )}
      {category === "disabled" && (
        <p className="binding-editor-help">
          Explicitly request Disabled in the file. No device action is performed
          here.
        </p>
      )}
      <div className="binding-editor-actions">
        <button
          type="submit"
          className="accent-button"
          disabled={disabled || !changed}
        >
          Update file binding
        </button>
        <span>
          {changed
            ? "Pending change · not yet saved to the draft"
            : "Choose a new assignment to edit this file."}
        </span>
      </div>
    </form>
  );
}
