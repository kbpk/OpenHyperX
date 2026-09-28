import { useEffect, useState } from "react";
import type { Edit, Snapshot } from "./types";

/** File-provenance chooser. No target or library macro is inferred from source IDs. */
export function UnresolvedAssignments({
  snapshot,
  disabled,
  edit,
}: {
  snapshot: Snapshot;
  disabled: boolean;
  edit: (edit: Edit) => Promise<boolean>;
}) {
  const entries = snapshot.profile.unresolved_button_assignments ?? [];
  const [sourceIndex, setSourceIndex] = useState<number | null>(null);
  const [control, setControl] = useState("");
  const [macroId, setMacroId] = useState("");
  const [confirmation, setConfirmation] = useState<"resolve" | "omit" | null>(
    null,
  );
  useEffect(() => {
    setSourceIndex(null);
    setControl("");
    setMacroId("");
    setConfirmation(null);
  }, [snapshot.revision]);

  if (entries.length === 0) return null;
  const entry = sourceIndex === null ? null : entries[sourceIndex];
  const source =
    sourceIndex === null ? null : snapshot.resolution_sources?.[sourceIndex];
  const target = source?.targets.find((candidate) => candidate.id === control);
  const controlInfo = snapshot.controls.find(
    (candidate) => candidate.id === control,
  );
  const macros = (controlInfo?.bindings ?? [])
    .map((index) => snapshot.binding_choices[index])
    .filter((choice) => choice?.binding.type === "macro");
  const selectedMacro = macros.find(
    (choice) =>
      choice.binding.type === "macro" && choice.binding.id === macroId,
  );
  const canResolve =
    !!entry &&
    !source?.error &&
    !!target &&
    !target.error &&
    !!selectedMacro &&
    !selectedMacro.error;

  async function confirm() {
    if (!entry || !confirmation) return;
    const action: Edit =
      confirmation === "omit"
        ? { kind: "omit-unresolved", source_id: entry.source_id, confirm: true }
        : {
            kind: "resolve-unresolved",
            source_id: entry.source_id,
            control,
            macro_id: macroId,
            confirm: true,
          };
    if (await edit(action)) {
      setConfirmation(null);
      setSourceIndex(null);
      setControl("");
      setMacroId("");
    }
  }

  return (
    <section
      className="panel resolution-panel"
      aria-label="Unresolved imported assignments"
    >
      <div className="section-heading">
        <h2>Unresolved imported assignments</h2>
        <span className="badge warning-badge">{entries.length} unresolved</span>
      </div>
      <p className="subtle">
        These are file provenance, not detected mouse bindings. Select one
        source, a physical control and an existing macro explicitly. A source
        macro hint never selects the library entry for you.
      </p>
      <label className="field-label">
        Imported source
        <select
          aria-label="Imported source"
          value={sourceIndex ?? ""}
          disabled={disabled}
          onChange={(event) => {
            setSourceIndex(
              event.target.value === "" ? null : Number(event.target.value),
            );
            setControl("");
            setMacroId("");
            setConfirmation(null);
          }}
        >
          <option value="">Choose a source…</option>
          {entries.map((item, index) => (
            <option key={`${item.source_id}-${index}`} value={index}>
              {item.source_id}
            </option>
          ))}
        </select>
      </label>
      {entry && (
        <>
          <p className="resolution-hint">
            Source hint: {entry.macro_source_id ?? "none"}. No control or
            timeline can be inferred from this hint.
          </p>
          {source?.error && <div className="notice error">{source.error}</div>}
          <label className="field-label">
            Physical target
            <select
              aria-label="Physical target"
              value={control}
              disabled={disabled || !source || !!source.error}
              onChange={(event) => {
                setControl(event.target.value);
                setMacroId("");
                setConfirmation(null);
              }}
            >
              <option value="">Choose a control…</option>
              {source?.targets.map((candidate) => (
                <option
                  key={candidate.id}
                  value={candidate.id}
                  disabled={!!candidate.error}
                >
                  {candidate.name}
                  {candidate.error ? " — unavailable" : ""}
                </option>
              ))}
            </select>
          </label>
          {!!source?.targets.some((candidate) => candidate.error) && (
            <details className="resolution-reasons">
              <summary>Why are some targets unavailable?</summary>
              {source.targets
                .filter((candidate) => candidate.error)
                .map((candidate) => (
                  <p key={candidate.id}>
                    {candidate.name}: {candidate.error}
                  </p>
                ))}
            </details>
          )}
          {target && !target.error && (
            <label className="field-label">
              Existing library macro
              <select
                aria-label="Existing library macro"
                value={macroId}
                disabled={disabled}
                onChange={(event) => {
                  setMacroId(event.target.value);
                  setConfirmation(null);
                }}
              >
                <option value="">Choose a macro…</option>
                {macros.map((choice, index) =>
                  choice.binding.type === "macro" ? (
                    <option
                      key={`${choice.binding.id}-${index}`}
                      value={choice.binding.id}
                      disabled={!!choice.error}
                    >
                      {choice.label}
                      {choice.error ? " — unavailable" : ""}
                    </option>
                  ) : null,
                )}
              </select>
            </label>
          )}
          {!!macros.some((choice) => choice.error) && (
            <details className="resolution-reasons">
              <summary>Why are some macros unavailable?</summary>
              {macros
                .filter((choice) => choice.error)
                .map((choice, index) => (
                  <p key={`${choice.label}-${index}`}>
                    {choice.label}: {choice.error}
                  </p>
                ))}
            </details>
          )}
          <div className="inline-actions resolution-actions">
            <button
              disabled={disabled || !canResolve}
              onClick={() => setConfirmation("resolve")}
            >
              Review resolution
            </button>
            <button
              disabled={disabled || !source || !!source.error}
              onClick={() => setConfirmation("omit")}
            >
              Review omission
            </button>
          </div>
          {confirmation && (
            <div
              className="notice resolution-confirm"
              role="group"
              aria-label="Confirm imported assignment edit"
            >
              <strong>
                {confirmation === "omit"
                  ? "Omit source provenance?"
                  : "Resolve source assignment?"}
              </strong>
              <p>
                {confirmation === "omit"
                  ? `Remove only ${entry.source_id} from this file. No button is disabled or reset.`
                  : `Assign existing macro ${macroId} to ${target?.name ?? control} and remove only ${entry.source_id} from unresolved provenance.`}
              </p>
              <p>
                No mouse, onboard profile or other file entry will be changed.
              </p>
              <div className="inline-actions">
                <button
                  disabled={
                    disabled || (confirmation === "resolve" && !canResolve)
                  }
                  onClick={() => void confirm()}
                >
                  {confirmation === "omit"
                    ? "Confirm omission"
                    : "Confirm resolution"}
                </button>
                <button
                  disabled={disabled}
                  onClick={() => setConfirmation(null)}
                >
                  Cancel
                </button>
              </div>
            </div>
          )}
        </>
      )}
    </section>
  );
}
