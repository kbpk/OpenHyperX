import { useEffect, useId, useRef, useState } from "react";
import type { Edit, MacroEvent, NamedMacro, Playback, Snapshot } from "./types";

const humanize = (value: string) => value.replaceAll("-", " ");
type EventDraft = {
  rowId: number;
  type: MacroEvent["type"];
  value: string;
  delay: string;
};
type Draft = {
  id: string;
  name: string;
  playback: Playback;
  events: EventDraft[];
  original: NamedMacro | null;
  sourcePath: string | null;
  sourceName: string;
};
type StoredDraft = { version: 1; baseline: string; draft: Draft };
type Recovery = {
  stored: StoredDraft | null;
  warning: string | null;
  blocked: boolean;
};
const recoveryKey = "openhyperx.gui.local-macro-draft.v1";
const maxRecoveryBytes = 256 * 1024;
const blankRecovery: Recovery = { stored: null, warning: null, blocked: false };

function record(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
function playback(value: unknown): value is Playback {
  return (
    value === "once" ||
    value === "toggle-repeat" ||
    value === "repeat-while-held"
  );
}
function macroEvent(value: unknown): value is MacroEvent {
  if (
    !record(value) ||
    typeof value.delay_ms !== "number" ||
    !Number.isInteger(value.delay_ms)
  )
    return false;
  if (value.delay_ms < 0 || value.delay_ms > 65535) return false;
  if (value.type === "key-down" || value.type === "key-up")
    return typeof value.key === "string";
  if (value.type === "mouse-button-down" || value.type === "mouse-button-up")
    return typeof value.button === "string";
  return false;
}
function namedMacro(value: unknown): value is NamedMacro {
  return (
    record(value) &&
    typeof value.source_id === "string" &&
    typeof value.name === "string" &&
    playback(value.playback) &&
    Array.isArray(value.events) &&
    value.events.length <= 4096 &&
    value.events.every(macroEvent)
  );
}
function storedDraft(value: unknown): value is StoredDraft {
  if (
    !record(value) ||
    value.version !== 1 ||
    typeof value.baseline !== "string"
  )
    return false;
  const draft = value.draft;
  if (!record(draft) || !Array.isArray(draft.events)) return false;
  return (
    typeof draft.id === "string" &&
    typeof draft.name === "string" &&
    playback(draft.playback) &&
    (draft.original === null || namedMacro(draft.original)) &&
    (draft.sourcePath === null || typeof draft.sourcePath === "string") &&
    typeof draft.sourceName === "string" &&
    draft.events.length <= 4096 &&
    draft.events.every(
      (event: unknown) =>
        record(event) &&
        typeof event.rowId === "number" &&
        Number.isSafeInteger(event.rowId) &&
        event.rowId >= 0 &&
        (event.type === "key-down" ||
          event.type === "key-up" ||
          event.type === "mouse-button-down" ||
          event.type === "mouse-button-up") &&
        typeof event.value === "string" &&
        typeof event.delay === "string",
    ) &&
    new Set(draft.events.map((event: EventDraft) => event.rowId)).size ===
      draft.events.length
  );
}
function readRecovery(): Recovery {
  let raw: string | null;
  try {
    raw = localStorage.getItem(recoveryKey);
  } catch {
    return {
      stored: null,
      warning:
        "Cannot read the local macro recovery snapshot. Editing is possible, but recovery is unavailable.",
      blocked: false,
    };
  }
  if (raw === null) return blankRecovery;
  try {
    if (raw.length > maxRecoveryBytes)
      throw new Error("Saved timeline exceeds the recovery limit");
    const parsed: unknown = JSON.parse(raw);
    if (storedDraft(parsed))
      return { stored: parsed, warning: null, blocked: true };
  } catch {
    // A malformed snapshot must not be silently overwritten.
  }
  return {
    stored: null,
    warning:
      "The saved macro timeline is invalid. Discard it explicitly before editing another timeline.",
    blocked: true,
  };
}

function references(snapshot: Snapshot, id: string): string[] {
  return [
    ...Object.entries(snapshot.profile.buttons ?? {})
      .filter(([, binding]) => binding.type === "macro" && binding.id === id)
      .map(([control]) => control),
    ...(snapshot.profile.unresolved_button_assignments ?? [])
      .filter((assignment) => assignment.macro_source_id === id)
      .map((assignment) => `unresolved source ${assignment.source_id}`),
  ];
}
function draftValue(draft: Draft): string {
  return JSON.stringify({
    name: draft.name,
    playback: draft.playback,
    events: draft.events.map(({ type, value, delay }) => ({
      type,
      value,
      delay,
    })),
  });
}
function makeDraft(
  macro: NamedMacro,
  nextRow: () => number,
  snapshot: Snapshot,
): Draft {
  return {
    id: macro.source_id,
    name: macro.name,
    playback: macro.playback,
    events: macro.events.map((event) => ({
      rowId: nextRow(),
      type: event.type,
      value: "key" in event ? event.key : event.button,
      delay: String(event.delay_ms),
    })),
    original: macro,
    sourcePath: snapshot.path,
    sourceName: snapshot.profile.name,
  };
}

/** Local timelines are not document edits until explicit submission. Kept mounted
 * while navigating, and never refreshed away by an unrelated IPC revision. */
export function MacroLibrary({
  snapshot,
  disabled,
  edit,
  onDraftChange,
  persistLocalDraft = false,
}: {
  snapshot: Snapshot;
  disabled: boolean;
  edit: (value: Edit) => Promise<boolean>;
  onDraftChange: (dirty: boolean) => void;
  persistLocalDraft?: boolean;
}) {
  const [draft, setDraft] = useState<Draft | null>(null);
  const [baseline, setBaseline] = useState("");
  const [search, setSearch] = useState("");
  const [confirmed, setConfirmed] = useState(false);
  const [removeId, setRemoveId] = useState<string | null>(null);
  const [recovery, setRecovery] = useState<Recovery>(() =>
    persistLocalDraft ? readRecovery() : blankRecovery,
  );
  const [recoveryWriteError, setRecoveryWriteError] = useState<string | null>(
    null,
  );
  const row = useRef(0);
  const keysId = useId();
  const library = snapshot.profile.macros ?? [];
  const localFingerprint = draft === null ? "" : draftValue(draft);
  const dirty = draft !== null && localFingerprint !== baseline;
  const current =
    draft && library.filter((macro) => macro.source_id === draft.id);
  const stale =
    !!draft?.original &&
    (current?.length !== 1 ||
      JSON.stringify(current[0]) !== JSON.stringify(draft.original));
  const wrongSource = !!draft && draft.sourcePath !== snapshot.path;
  const saved = recovery.stored?.draft;
  const recoveryMatchesDocument =
    !!saved &&
    saved.sourcePath === snapshot.path &&
    (!saved.original ||
      (library.filter((macro) => macro.source_id === saved.id).length === 1 &&
        JSON.stringify(
          library.find((macro) => macro.source_id === saved.id),
        ) === JSON.stringify(saved.original)));
  const refs = draft ? references(snapshot, draft.id) : [];
  const keys = snapshot.macro_keys ?? [];
  const buttons = snapshot.macro_mouse_buttons ?? [];
  const filteredKeys = keys.filter((key) =>
    key.toLowerCase().includes(search.toLowerCase()),
  );
  const invalidEvent = draft?.events.some(
    (event) => !/^\d+$/.test(event.delay) || Number(event.delay) > 65535,
  );
  useEffect(() => {
    onDraftChange(dirty);
  }, [dirty, localFingerprint, onDraftChange]);
  useEffect(() => {
    if (!dirty) return;
    const prevent = (event: BeforeUnloadEvent) => event.preventDefault();
    window.addEventListener("beforeunload", prevent);
    return () => window.removeEventListener("beforeunload", prevent);
  }, [dirty]);
  useEffect(() => {
    if (!persistLocalDraft || recovery.blocked || !draft) return;
    try {
      if (dirty) {
        const raw = JSON.stringify({ version: 1, baseline, draft });
        if (raw.length > maxRecoveryBytes)
          throw new Error("Macro timeline exceeds the local recovery limit.");
        localStorage.setItem(recoveryKey, raw);
      } else {
        localStorage.removeItem(recoveryKey);
      }
      setRecoveryWriteError(null);
    } catch {
      setRecoveryWriteError(
        "Could not save the local macro timeline for crash recovery. Keep this window open or add the macro to the file.",
      );
    }
  }, [baseline, dirty, draft, persistLocalDraft, recovery.blocked]);
  const nextRow = () => row.current++;
  function begin(macro: NamedMacro | null) {
    if (recovery.blocked) return;
    const next = macro
      ? makeDraft(macro, nextRow, snapshot)
      : {
          id: `macro-${crypto.randomUUID()}`,
          name: "New macro",
          playback: "once" as Playback,
          events: [],
          original: null,
          sourcePath: snapshot.path,
          sourceName: snapshot.profile.name,
        };
    setDraft(next);
    // A newly created empty timeline is already an uncommitted draft.
    setBaseline(macro ? draftValue(next) : "");
    setConfirmed(false);
    setSearch("");
    setRemoveId(null);
  }
  function closeDraft() {
    if (persistLocalDraft) {
      try {
        localStorage.removeItem(recoveryKey);
        setRecoveryWriteError(null);
      } catch {
        setRecoveryWriteError(
          "Could not clear the local recovery snapshot. It may appear again after restart.",
        );
      }
    }
    setDraft(null);
    setConfirmed(false);
    setSearch("");
  }
  function changeEvent(index: number, patch: Partial<EventDraft>) {
    setDraft(
      (value) =>
        value && {
          ...value,
          events: value.events.map((event, i) =>
            i === index ? { ...event, ...patch } : event,
          ),
        },
    );
    setConfirmed(false);
  }
  function moveEvent(index: number, direction: number) {
    setDraft((value) => {
      if (!value) return value;
      const events = [...value.events];
      [events[index], events[index + direction]] = [
        events[index + direction],
        events[index],
      ];
      return { ...value, events };
    });
    setConfirmed(false);
  }
  async function commit() {
    if (
      !draft ||
      disabled ||
      !dirty ||
      stale ||
      wrongSource ||
      invalidEvent ||
      !draft.name.trim() ||
      (draft.original && refs.length > 0 && !confirmed)
    )
      return;
    const macro: NamedMacro = {
      source_id: draft.id,
      name: draft.name,
      playback: draft.playback,
      events: draft.events.map((event): MacroEvent =>
        event.type === "key-down" || event.type === "key-up"
          ? {
              type: event.type,
              key: event.value,
              delay_ms: Number(event.delay),
            }
          : {
              type: event.type,
              button: event.value,
              delay_ms: Number(event.delay),
            },
      ),
    };
    const success = await edit(
      draft.original
        ? {
            kind: "macro-replace",
            source_id: draft.id,
            macro,
            confirm_references: confirmed,
          }
        : { kind: "macro-create", macro },
    );
    if (success) closeDraft();
  }
  return (
    <div className="macro-library">
      {recovery.blocked && (
        <div className="notice" role="status">
          <strong>Uncommitted macro timeline found</strong>
          {recovery.stored ? (
            <>
              <p>
                “{recovery.stored.draft.name}” from{" "}
                {recovery.stored.draft.sourcePath ??
                  `unsaved profile “${recovery.stored.draft.sourceName}”`}
                . Restore it explicitly; nothing has been added to the file or
                sent to the mouse.
              </p>
              {!recoveryMatchesDocument && (
                <p>
                  Open the original file or restore its FILE draft first. The
                  current document does not match this timeline's source.
                </p>
              )}
              <button
                type="button"
                disabled={!recoveryMatchesDocument}
                onClick={() => {
                  const saved = recovery.stored!;
                  row.current =
                    Math.max(
                      -1,
                      ...saved.draft.events.map((event) => event.rowId),
                    ) + 1;
                  setDraft(saved.draft);
                  setBaseline(saved.baseline);
                  setConfirmed(false);
                  setRecovery(blankRecovery);
                }}
              >
                Restore macro timeline
              </button>{" "}
            </>
          ) : (
            <p>{recovery.warning}</p>
          )}
          <button
            type="button"
            onClick={() => {
              try {
                localStorage.removeItem(recoveryKey);
                setRecovery(blankRecovery);
              } catch {
                setRecovery({
                  ...recovery,
                  warning:
                    "Could not discard the local macro recovery snapshot.",
                });
              }
            }}
          >
            Discard saved timeline
          </button>
        </div>
      )}
      {!recovery.blocked && recovery.warning && (
        <div className="notice error" role="alert">
          {recovery.warning}
        </div>
      )}
      {recoveryWriteError && (
        <div className="notice error" role="alert">
          {recoveryWriteError}
        </div>
      )}
      <div className="section-heading">
        <div>
          <h2>Named macro library</h2>
          <p>File definitions, not recordings or live mouse state.</p>
        </div>
        <button
          disabled={disabled || dirty || recovery.blocked}
          onClick={() => begin(null)}
        >
          New macro
        </button>
      </div>
      {draft && (
        <form
          className="panel macro-editor"
          aria-label="Edit macro timeline"
          onSubmit={(event) => {
            event.preventDefault();
            void commit();
          }}
        >
          <div className="section-heading">
            <h2>{draft.original ? "Edit macro" : "Create macro"}</h2>
            <span className="badge">LOCAL DRAFT</span>
          </div>
          <p className="subtle macro-id">
            ID: {draft.id} · Identifiers stay stable when renaming.
          </p>
          <div className="macro-fields">
            <label className="field-label">
              Macro name
              <input
                aria-label="Macro name"
                value={draft.name}
                disabled={disabled}
                onChange={(event) => {
                  setDraft({ ...draft, name: event.target.value });
                  setConfirmed(false);
                }}
              />
            </label>
            <label className="field-label">
              Playback
              <select
                aria-label="Macro playback"
                value={draft.playback}
                disabled={disabled}
                onChange={(event) => {
                  setDraft({
                    ...draft,
                    playback: event.target.value as Playback,
                  });
                  setConfirmed(false);
                }}
              >
                <option value="once">Play once</option>
                <option value="toggle-repeat">Toggle repeat</option>
                <option value="repeat-while-held">Hold repeat</option>
              </select>
            </label>
          </div>
          <label className="field-label macro-key-search">
            Search keys
            <input
              aria-label="Search macro keys"
              value={search}
              disabled={disabled}
              placeholder="left-shift, a, enter…"
              onChange={(event) => setSearch(event.target.value)}
            />
          </label>
          <ol className="macro-events" aria-label="Macro event sequence">
            {draft.events.map((event, index) => {
              const keyboard =
                event.type === "key-down" || event.type === "key-up";
              const options = keyboard ? filteredKeys : buttons;
              const visible = options.includes(event.value)
                ? options
                : event.value
                  ? [event.value, ...options]
                  : options;
              const validDelay =
                /^\d+$/.test(event.delay) && Number(event.delay) <= 65535;
              return (
                <li className="macro-event" key={event.rowId}>
                  <span className="event-number">{index + 1}</span>
                  <label className="field-label">
                    Event
                    <select
                      aria-label={`Event ${index + 1} type`}
                      disabled={disabled}
                      value={event.type}
                      onChange={(change) => {
                        const type = change.target.value as MacroEvent["type"];
                        const newKeyboard =
                          type === "key-down" || type === "key-up";
                        changeEvent(index, {
                          type,
                          value: newKeyboard === keyboard ? event.value : "",
                        });
                      }}
                    >
                      <option value="key-down">Key down</option>
                      <option value="key-up">Key up</option>
                      <option value="mouse-button-down">
                        Mouse button down
                      </option>
                      <option value="mouse-button-up">Mouse button up</option>
                    </select>
                  </label>
                  <label className="field-label">
                    {keyboard ? "Key" : "Mouse button"}
                    <select
                      id={`${keysId}-${event.rowId}`}
                      aria-label={`Event ${index + 1} ${keyboard ? "key" : "mouse button"}`}
                      disabled={disabled}
                      value={event.value}
                      onChange={(change) =>
                        changeEvent(index, { value: change.target.value })
                      }
                    >
                      <option value="">Choose explicitly…</option>
                      {visible.map((value) => (
                        <option key={value} value={value}>
                          {value}
                          {!(keyboard ? keys : buttons).includes(value)
                            ? " · imported value"
                            : ""}
                        </option>
                      ))}
                    </select>
                  </label>
                  <label className="field-label">
                    Delay after · ms
                    <input
                      type="number"
                      aria-label={`Event ${index + 1} delay`}
                      min={0}
                      max={65535}
                      step={1}
                      value={event.delay}
                      disabled={disabled}
                      aria-invalid={!validDelay}
                      onChange={(change) =>
                        changeEvent(index, { delay: change.target.value })
                      }
                    />
                  </label>
                  <div className="event-actions">
                    <button
                      type="button"
                      className="small-button"
                      aria-label={`Move event ${index + 1} up`}
                      disabled={disabled || index === 0}
                      onClick={() => moveEvent(index, -1)}
                    >
                      ↑
                    </button>
                    <button
                      type="button"
                      className="small-button"
                      aria-label={`Move event ${index + 1} down`}
                      disabled={disabled || index === draft.events.length - 1}
                      onClick={() => moveEvent(index, 1)}
                    >
                      ↓
                    </button>
                    <button
                      type="button"
                      className="small-button"
                      aria-label={`Remove event ${index + 1}`}
                      disabled={disabled}
                      onClick={() => {
                        setDraft({
                          ...draft,
                          events: draft.events.filter((_, i) => i !== index),
                        });
                        setConfirmed(false);
                      }}
                    >
                      Remove
                    </button>
                  </div>
                </li>
              );
            })}
          </ol>
          <button
            type="button"
            disabled={disabled}
            onClick={() => {
              setDraft({
                ...draft,
                events: [
                  ...draft.events,
                  { rowId: nextRow(), type: "key-down", value: "", delay: "0" },
                ],
              });
              setConfirmed(false);
            }}
          >
            Add event
          </button>
          <p className="subtle">
            Chords use separate downs and ups, e.g. left-shift down → a down → a
            up → left-shift up. Delay belongs after each event. Empty or
            unbalanced timelines can be saved as file drafts; that does not make
            them executable.
          </p>
          {invalidEvent && (
            <p className="field-error">
              Use an integer delay from 0 to 65535 ms. Device limits are checked
              separately; unspecified keys/buttons remain non-executable draft
              values.
            </p>
          )}
          {stale && (
            <div className="notice error" role="alert">
              This definition changed while you were editing. Your local
              timeline is preserved. Discard it and reopen the current
              definition before replacing it.
            </div>
          )}
          {wrongSource && (
            <div className="notice error" role="alert">
              This timeline belongs to{" "}
              {draft.sourcePath ?? "an unsaved profile"}, not the current file.
              Open the original profile before updating it.
            </div>
          )}
          {draft.original && refs.length > 0 && (
            <div className="notice">
              <p>
                Replacing this definition also changes these file references:{" "}
                {refs.join(", ")}. No binding is reassigned.
              </p>
              <label className="macro-confirm">
                <input
                  type="checkbox"
                  checked={confirmed}
                  disabled={disabled}
                  onChange={(event) => setConfirmed(event.target.checked)}
                />
                I confirm replacing the referenced definition.
              </label>
            </div>
          )}
          <div className="macro-editor-actions">
            <button type="button" disabled={disabled} onClick={closeDraft}>
              {dirty ? "Discard timeline edits" : "Close editor"}
            </button>
            <button
              type="submit"
              className="primary"
              disabled={
                disabled ||
                !dirty ||
                stale ||
                wrongSource ||
                invalidEvent ||
                !draft.name.trim() ||
                (!!draft.original && refs.length > 0 && !confirmed)
              }
            >
              {draft.original ? "Update file macro" : "Add macro to file"}
            </button>
          </div>
        </form>
      )}
      {!library.length && (
        <section className="panel empty-state">
          <h3>No macros in this file</h3>
          <p>
            Create a named timeline and assign it separately in Buttons. No
            global keyboard hook runs.
          </p>
        </section>
      )}
      {library.map((macro, macroIndex) => {
        const refs = references(snapshot, macro.source_id);
        const unique =
          library.filter((item) => item.source_id === macro.source_id)
            .length === 1;
        const destinations = snapshot.controls.flatMap((control) => {
          const choice = control.bindings
            .map((index) => snapshot.binding_choices[index])
            .find(
              (choice) =>
                choice?.binding.type === "macro" &&
                choice.binding.id === macro.source_id,
            );
          return choice ? [{ name: control.name, error: choice.error }] : [];
        });
        return (
          <section
            className="panel table-panel macro-panel"
            key={`${macroIndex}:${macro.source_id}`}
          >
            <div className="section-heading">
              <div>
                <h2>{macro.name}</h2>
                <p className="macro-id">ID: {macro.source_id}</p>
              </div>
              <span className="badge">
                {humanize(macro.playback).toUpperCase()}
              </span>
            </div>
            <p className="subtle">
              References: {refs.length ? refs.join(", ") : "None"}
            </p>
            {!!destinations.length && (
              <details className="macro-preflight">
                <summary>Destination preflight · offline encoding only</summary>
                {destinations.map((destination) => (
                  <p key={destination.name}>
                    <strong>{destination.name}:</strong>{" "}
                    {destination.error ??
                      "Accepted by the offline encoder. Not a confirmation of hardware or onboard persistence."}
                  </p>
                ))}
              </details>
            )}
            {!unique && (
              <p className="field-error">
                Duplicate identifier: definition preserved, but replacement and
                removal are unavailable.
              </p>
            )}
            <div className="macro-card-actions">
              <button
                disabled={disabled || dirty || recovery.blocked || !unique}
                aria-label={`Edit macro ${macro.name}`}
                onClick={() => begin(macro)}
              >
                Edit timeline
              </button>
              <button
                disabled={
                  disabled ||
                  dirty ||
                  recovery.blocked ||
                  !unique ||
                  refs.length > 0
                }
                title={
                  refs.length
                    ? "Remove file references before deleting a definition"
                    : undefined
                }
                aria-label={`Remove macro ${macro.name}`}
                onClick={() => setRemoveId(macro.source_id)}
              >
                Remove definition
              </button>
            </div>
            {removeId === macro.source_id && (
              <div className="notice">
                <p>
                  Remove “{macro.name}” from this file? This does not change the
                  mouse.
                </p>
                <button disabled={disabled} onClick={() => setRemoveId(null)}>
                  Cancel removal
                </button>{" "}
                <button
                  disabled={disabled || refs.length > 0}
                  onClick={() => {
                    void edit({
                      kind: "macro-remove",
                      source_id: macro.source_id,
                    }).then((success) => {
                      if (success) {
                        if (draft?.id === macro.source_id) closeDraft();
                        setRemoveId(null);
                      }
                    });
                  }}
                >
                  Confirm remove from file
                </button>
              </div>
            )}
            <div className="macro-table-scroll">
              <table>
                <thead>
                  <tr>
                    <th>#</th>
                    <th>Event</th>
                    <th>Key / button</th>
                    <th>Delay after event</th>
                  </tr>
                </thead>
                <tbody>
                  {macro.events.map((event, index) => (
                    <tr key={index}>
                      <td>{index + 1}</td>
                      <td>{humanize(event.type)}</td>
                      <td>
                        <code>{"key" in event ? event.key : event.button}</code>
                      </td>
                      <td>
                        <code>{event.delay_ms} ms</code>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </section>
        );
      })}
      <div className="notice">
        <p>
          Library editing is independent of a destination. Saving an offline
          draft is not a hardware support claim. Runtime and onboard playback
          support are separate:
        </p>
        {snapshot.controls
          .filter((control) => control.macros)
          .map((control) => (
            <p key={control.id}>
              <strong>{control.name}</strong> · runtime:{" "}
              {control.macros!.runtime.map(humanize).join(", ") || "none"};
              onboard:{" "}
              {control.macros!.onboard.map(humanize).join(", ") || "none"};
              capture-backed maximum {control.macros!.max_events} events /{" "}
              {control.macros!.max_delay_ms} ms per delay.
            </p>
          ))}
      </div>
    </div>
  );
}
