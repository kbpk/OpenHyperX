import {
  useCallback,
  useEffect,
  useId,
  useRef,
  useState,
  type ReactNode,
} from "react";
import {
  desktopBackend,
  GuiClient,
  type Backend,
  type SessionState,
} from "./bridge";
import { Icon } from "./icons";
import { DeviceRender, MouseArt } from "./mouse-art";
import { ButtonAssignments } from "./button-assignments";
import { MacroLibrary } from "./macro-library";
import { UnresolvedAssignments } from "./unresolved-assignments";
import {
  discardFieldRecovery,
  emptyFieldRecovery,
  fieldMatches,
  fieldPage,
  readFieldRecovery,
  removeFieldDraft,
  writeFieldDraft,
  type FieldDraft,
} from "./field-recovery";
import type { Edit, Page, Snapshot, Stage } from "./types";

const pages: Page[] = [
  "Device",
  "Performance",
  "Buttons",
  "Macros",
  "Lighting",
  "Profiles",
];
const descriptions: Record<Page, string> = {
  Device: "One mouse. Your settings. No background service.",
  Performance: "Fine-tune your sensitivity and response.",
  Buttons: "Every control, with its own purpose.",
  Macros: "Build the exact sequence, including chords and individual delays.",
  Lighting: "Two zones. Independent colors.",
  Profiles: "Keep your configuration in an open, portable format.",
};

function Dialog({
  title,
  children,
  close,
}: {
  title: string;
  children: ReactNode;
  close: () => void;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  const titleId = useId();
  useEffect(() => {
    ref.current?.showModal();
  }, []);
  return (
    <dialog
      ref={ref}
      aria-labelledby={titleId}
      onCancel={(event) => {
        event.preventDefault();
        close();
      }}
    >
      <div className="dialog-title">
        <h2 id={titleId}>{title}</h2>
        <button
          className="icon-button"
          aria-label="Close dialog"
          onClick={close}
        >
          <Icon name="close" />
        </button>
      </div>
      {children}
    </dialog>
  );
}

function ColorField({
  label,
  color,
  disabled,
  commit,
  restored,
  resetVersion,
  field,
  onDraft,
  clearDraft,
}: {
  label: string;
  color?: string;
  disabled: boolean;
  commit: (color: string) => Promise<boolean>;
  restored?: string;
  resetVersion: number;
  field: string;
  onDraft: (value: string) => void;
  clearDraft: () => void;
}) {
  const [text, setText] = useState(color ?? "");
  const [invalid, setInvalid] = useState(false);
  const lastReset = useRef(resetVersion);
  useEffect(() => {
    setText(color ?? "");
    setInvalid(false);
  }, [color]);
  useEffect(() => {
    if (restored !== undefined) setText(restored);
  }, [restored]);
  useEffect(() => {
    if (lastReset.current !== resetVersion) {
      setText(color ?? "");
      setInvalid(false);
      lastReset.current = resetVersion;
    }
  }, [resetVersion]);
  async function submit() {
    if (!/^#[0-9a-f]{6}$/i.test(text)) {
      setInvalid(true);
      return;
    }
    setInvalid(false);
    if (text.toUpperCase() !== color?.toUpperCase()) {
      if (await commit(text.toUpperCase())) clearDraft();
    } else clearDraft();
  }
  return (
    <div className="color-field">
      <label
        className="color-chip"
        style={{ backgroundColor: color ?? "#252b2d" }}
      >
        <input
          type="color"
          aria-label={`${label} picker`}
          value={color ?? "#000000"}
          disabled={disabled}
          onChange={(event) => {
            setText(event.target.value.toUpperCase());
            onDraft(event.target.value.toUpperCase());
            void commit(event.target.value.toUpperCase()).then((saved) => {
              if (saved) clearDraft();
            });
          }}
        />
      </label>
      <input
        aria-label={label}
        className="hex"
        value={text}
        placeholder="#RRGGBB"
        disabled={disabled}
        aria-invalid={invalid}
        onChange={(event) => {
          setText(event.target.value);
          onDraft(event.target.value);
        }}
        onBlur={(event) => {
          if (event.relatedTarget?.getAttribute("data-discard-field") !== field)
            void submit();
        }}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            event.preventDefault();
            void submit();
          }
        }}
      />
      {invalid && <span className="field-error">Use #RRGGBB</span>}
    </div>
  );
}

function StageCard({
  stage,
  index,
  active,
  limits,
  disabled,
  edit,
  restoredDpi,
  restoredColor,
  resetVersions,
  onDraft,
  clearDraft,
}: {
  stage: Stage;
  index: number;
  active: boolean;
  limits: NonNullable<NonNullable<Snapshot["capabilities"]>["dpi"]>;
  disabled: boolean;
  edit: (edit: Edit) => Promise<boolean>;
  restoredDpi?: string;
  restoredColor?: string;
  resetVersions: Record<string, number>;
  onDraft: (field: string, value: string, baseline: string) => void;
  clearDraft: (field: string) => void;
}) {
  const dpiField = `stage:${index}:dpi`;
  const colorField = `stage:${index}:color`;
  const baseline = JSON.stringify(stage);
  const [value, setValue] = useState(String(stage.x));
  const [error, setError] = useState(false);
  const lastReset = useRef(resetVersions[dpiField]);
  useEffect(() => {
    setValue(String(stage.x));
    setError(false);
  }, [stage.x, stage.y]);
  useEffect(() => {
    if (restoredDpi !== undefined) setValue(restoredDpi);
  }, [restoredDpi]);
  useEffect(() => {
    if (lastReset.current !== resetVersions[dpiField]) {
      setValue(String(stage.x));
      setError(false);
      lastReset.current = resetVersions[dpiField];
    }
  }, [resetVersions[dpiField]]);
  const commit = async () => {
    const dpi = Number(value);
    if (
      !value.trim() ||
      !Number.isInteger(dpi) ||
      dpi < limits.minimum ||
      dpi > limits.maximum ||
      dpi % limits.step !== 0
    ) {
      setError(true);
      return;
    }
    setError(false);
    if (dpi !== stage.x || dpi !== stage.y) {
      if (await edit({ kind: "stage-dpi", index, dpi })) clearDraft(dpiField);
    } else clearDraft(dpiField);
  };
  return (
    <article className={`stage-card ${active ? "active" : ""}`}>
      <div className="stage-top">
        <span>
          <i style={{ backgroundColor: stage.color }} />
          Stage {index + 1}
        </span>
        <button
          className="small-button"
          disabled={disabled}
          aria-pressed={active}
          onClick={() => void edit({ kind: "active-stage", index })}
        >
          {active ? "Active" : "Set active"}
        </button>
      </div>
      <label className="dpi-value">
        <input
          type="number"
          aria-label={`Stage ${index + 1} DPI`}
          value={value}
          min={limits.minimum}
          max={limits.maximum}
          step={limits.step}
          disabled={disabled}
          aria-invalid={error}
          onChange={(event) => {
            setValue(event.target.value);
            onDraft(dpiField, event.target.value, baseline);
          }}
          onBlur={(event) => {
            if (
              event.relatedTarget?.getAttribute("data-discard-field") !==
              dpiField
            )
              void commit();
          }}
          onKeyDown={(event) => {
            if (event.key === "Enter") event.currentTarget.blur();
          }}
        />
        <span>DPI</span>
      </label>
      <input
        className="slider"
        type="range"
        aria-label={`Stage ${index + 1} DPI slider`}
        min={limits.minimum}
        max={limits.maximum}
        step={limits.step}
        value={Number(value) || limits.minimum}
        disabled={disabled}
        onChange={(event) => {
          setValue(event.target.value);
          onDraft(dpiField, event.target.value, baseline);
        }}
        onPointerUp={() => void commit()}
        onKeyUp={() => void commit()}
        onBlur={(event) => {
          if (
            event.relatedTarget?.getAttribute("data-discard-field") !== dpiField
          )
            void commit();
        }}
      />
      <div className="range-labels">
        <span>{limits.minimum}</span>
        <span>{limits.maximum.toLocaleString("en-US")}</span>
      </div>
      {stage.x !== stage.y && (
        <p className="field-error">
          X {stage.x} / Y {stage.y}. Editing links both axes.
        </p>
      )}
      {error && (
        <p className="field-error">
          {limits.minimum}–{limits.maximum}, in steps of {limits.step}.
        </p>
      )}
      <div className="stage-color">
        <span>Stage color</span>
        <ColorField
          label={`Stage ${index + 1} color`}
          color={stage.color}
          disabled={disabled}
          restored={restoredColor}
          resetVersion={resetVersions[colorField] ?? 0}
          field={colorField}
          onDraft={(color) => onDraft(colorField, color, baseline)}
          clearDraft={() => clearDraft(colorField)}
          commit={(color) => edit({ kind: "stage-color", index, color })}
        />
      </div>
    </article>
  );
}

export default function App({
  backend = desktopBackend,
}: {
  backend?: Backend;
}) {
  const [client] = useState(() => new GuiClient(backend));
  const [state, setState] = useState<SessionState>(client.state);
  const [page, setPage] = useState<Page>("Performance");
  const [modal, setModal] = useState<
    "review" | "add" | "discard" | "overwrite" | "discard-recovery" | null
  >(null);
  const [recoveryToken, setRecoveryToken] = useState<string | null>(null);
  const [replacement, setReplacement] = useState<"open" | "empty" | "demo">(
    "empty",
  );
  const [addedDpi, setAddedDpi] = useState(800);
  const [addedColor, setAddedColor] = useState("#FFFFFF");
  const [name, setName] = useState("");
  const [fieldRecovery, setFieldRecovery] = useState(() =>
    backend.desktop ? readFieldRecovery() : emptyFieldRecovery,
  );
  const [restoredFields, setRestoredFields] = useState<Record<string, string>>(
    {},
  );
  const [resetVersions, setResetVersions] = useState<Record<string, number>>(
    {},
  );
  const [fieldWriteError, setFieldWriteError] = useState<string | null>(null);
  const [volatileFields, setVolatileFields] = useState<string[]>([]);
  const [macroDraft, setMacroDraft] = useState(false);
  const [macroGeneration, setMacroGeneration] = useState(0);
  const reportMacroDraft = useCallback((pending: boolean) => {
    setMacroDraft(pending);
    setMacroGeneration((generation) => generation + 1);
  }, []);
  const [draftGuardError, setDraftGuardError] = useState<string | null>(null);
  const draftGuardQueue = useRef<Promise<unknown>>(Promise.resolve());
  const fieldDraft =
    fieldRecovery.drafts.length > 0 ||
    fieldRecovery.error !== null ||
    volatileFields.length > 0;
  const unresolvedFieldRecovery =
    fieldRecovery.error !== null ||
    fieldRecovery.drafts.some((draft) => !(draft.field in restoredFields));
  useEffect(() => {
    if (!backend.desktop || !backend.setLocalDraft) return;
    // Keep true/false notifications ordered, separately from document edits.
    draftGuardQueue.current = draftGuardQueue.current
      .catch(() => undefined)
      .then(async () => {
        try {
          await backend.setLocalDraft!(macroDraft || fieldDraft);
          setDraftGuardError(null);
        } catch {
          setDraftGuardError(
            "Cannot notify the native close guard. Keep this window open until local input is updated or discarded.",
          );
        }
      });
  }, [backend, macroDraft, macroGeneration, fieldDraft]);
  useEffect(() => {
    const unsubscribe = client.subscribe(setState);
    void client.request("gui_snapshot").catch(() => undefined);
    return unsubscribe;
  }, [client]);
  useEffect(() => {
    setName(state.snapshot?.profile.name ?? "");
  }, [state.snapshot?.profile.name]);
  useEffect(() => {
    if (restoredFields["profile.name"] !== undefined)
      setName(restoredFields["profile.name"]);
  }, [restoredFields]);
  useEffect(() => {
    setName(state.snapshot?.profile.name ?? "");
  }, [resetVersions["profile.name"]]);
  useEffect(() => {
    if (!fieldDraft) return;
    const prevent = (event: BeforeUnloadEvent) => event.preventDefault();
    window.addEventListener("beforeunload", prevent);
    return () => window.removeEventListener("beforeunload", prevent);
  }, [fieldDraft]);
  function saveFieldInput(field: string, value: string, baseline: string) {
    if (!backend.desktop || !state.snapshot) return;
    const snapshot = state.snapshot;
    const draft: FieldDraft = {
      field,
      value,
      baseline,
      path: snapshot.path,
      device: snapshot.profile.device,
      name: snapshot.profile.name,
    };
    try {
      setFieldRecovery(writeFieldDraft(draft));
      setRestoredFields((fields) => ({ ...fields, [field]: value }));
      setVolatileFields((fields) => fields.filter((entry) => entry !== field));
      setFieldWriteError(null);
    } catch {
      setVolatileFields((fields) => [...new Set([...fields, field])]);
      setFieldWriteError(
        "Could not save this unfinished input for crash recovery. Keep the window open and commit or discard it.",
      );
    }
  }
  function clearFieldInput(field: string) {
    try {
      setFieldRecovery(removeFieldDraft(field));
      setVolatileFields((fields) => fields.filter((entry) => entry !== field));
      setRestoredFields((fields) => {
        const next = { ...fields };
        delete next[field];
        return next;
      });
      setFieldWriteError(null);
    } catch {
      setFieldWriteError(
        "Could not clear local input recovery. Inspect or discard the saved draft before closing.",
      );
    }
  }
  function discardFieldInput(field: string) {
    clearFieldInput(field);
    setResetVersions((versions) => ({
      ...versions,
      [field]: (versions[field] ?? 0) + 1,
    }));
  }
  async function request(command: string, args?: Record<string, unknown>) {
    try {
      if (
        [
          "gui_save_profile",
          "gui_overwrite_profile",
          "gui_undo_file_edit",
          "gui_redo_file_edit",
          "gui_open_profile",
          "gui_reset",
          "gui_restore_recovery",
        ].includes(command)
      ) {
        if (fieldDraft) return false;
        await draftGuardQueue.current;
      }
      await client.request(command, args);
      return true;
    } catch {
      return false;
    }
  }
  async function edit(value: Edit) {
    try {
      await client.edit(value);
      return true;
    } catch {
      return false;
    }
  }
  function replace(action: "open" | "empty" | "demo") {
    if (macroDraft || fieldDraft) return;
    setReplacement(action);
    if (state.snapshot?.dirty) setModal("discard");
    else void doReplace(action, false);
  }
  async function doReplace(
    action: "open" | "empty" | "demo",
    discard: boolean,
  ) {
    setModal(null);
    if (action === "open")
      await request("gui_open_profile", { discardChanges: discard });
    else
      await request("gui_reset", {
        discardChanges: discard,
        demo: action === "demo",
      });
  }
  const snapshot = state.snapshot;
  const profile = snapshot?.profile;
  const disabled = !backend.desktop || state.busy || unresolvedFieldRecovery;
  const changes =
    (snapshot?.changes.settings.length ?? 0) +
    (snapshot?.changes.metadata.length ?? 0);
  const dpi = profile?.dpi;
  const limits = snapshot?.capabilities?.dpi;
  const zones = profile?.lighting?.zones;

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <a
          className="brand"
          href="#"
          onClick={(event) => event.preventDefault()}
        >
          <img src="/mark.svg" alt="" />
          <span>
            OpenHyperX<small>DEVICE CONTROL</small>
          </span>
        </a>
        <div className="sidebar-label">WORKSPACE</div>
        <nav aria-label="Sections">
          {pages.map((item) => (
            <button
              key={item}
              className={`nav-item ${page === item ? "selected" : ""}`}
              aria-current={page === item ? "page" : undefined}
              onClick={() => setPage(item)}
            >
              <Icon name={item} />
              {item}
              {item === "Profiles" && snapshot?.dirty && (
                <i className="dirty-dot" />
              )}
            </button>
          ))}
        </nav>
        <div className="sidebar-bottom">
          <div className="file-badge">
            <Icon name="Profiles" />
            <div>
              <strong>{profile?.name ?? "Loading…"}</strong>
              <small>
                {snapshot?.origin === "file"
                  ? "LOCAL FILE"
                  : snapshot?.origin === "recovered"
                    ? "RECOVERED DRAFT"
                    : snapshot?.origin === "demo"
                      ? "DEMO PROFILE"
                      : "NEW DRAFT"}
              </small>
            </div>
          </div>
          <p>Open source. Yours to control.</p>
          <span className="version">EXPERIMENTAL · 0.1</span>
        </div>
      </aside>
      <div className="workspace">
        <header className="toolbar">
          <div className="device-label">
            <i />
            {snapshot?.capabilities?.name ?? "OpenHyperX"}
            <span className="badge">OFFLINE</span>
          </div>
          <div className="toolbar-actions">
            <button
              disabled={disabled || macroDraft || fieldDraft}
              onClick={() => replace("open")}
            >
              <Icon name="open" />
              Open profile
            </button>
            <button
              className="primary"
              disabled={disabled || !snapshot || macroDraft || fieldDraft}
              onClick={() => void request("gui_save_profile")}
            >
              <Icon name="save" />
              Save new file
            </button>
          </div>
        </header>
        <main>
          <div className="page-heading">
            <div className="eyebrow">
              {(
                snapshot?.capabilities?.name ??
                profile?.device ??
                "LOCAL PROFILE"
              ).toUpperCase()}{" "}
              / {page.toUpperCase()}
            </div>
            <h1>{page}</h1>
            <p>{descriptions[page]}</p>
          </div>
          {!backend.desktop && (
            <div className="notice preview">
              <span className="badge">PREVIEW</span>Read-only demo in your
              browser. Editing and file dialogs are available in the native
              Tauri app.
            </div>
          )}
          {snapshot?.origin === "demo" && backend.desktop && (
            <div className="notice">
              Demo profile — not a readout of your connected mouse.
            </div>
          )}
          {snapshot?.origin === "recovered" && (
            <div className="notice">
              Recovered offline FILE draft. It has not been saved to disk or to
              the mouse. Save NEW, or review and confirm a FILE overwrite.
            </div>
          )}
          {snapshot?.recovery_warning && (
            <div className="notice error" role="alert">
              {snapshot.recovery_warning}
            </div>
          )}
          {backend.desktop && (snapshot?.pending_recovery.length ?? 0) > 0 && (
            <section className="notice" aria-label="Offline draft recovery">
              <strong>Older unsaved FILE drafts are available</strong>
              <p>
                Restore only after saving or discarding the current draft. This
                reads an offline snapshot; it does not connect to the mouse.
              </p>
              {snapshot?.pending_recovery.map((candidate) => (
                <div key={candidate.token}>
                  <span>
                    {candidate.profile_name ?? candidate.token}
                    {candidate.original_file
                      ? ` · originally ${candidate.original_file}`
                      : ""}
                  </span>
                  {candidate.error ? (
                    <p role="alert">
                      Unreadable snapshot, retained on disk: {candidate.error}
                    </p>
                  ) : (
                    <button
                      disabled={disabled || !!snapshot?.dirty || macroDraft}
                      onClick={() =>
                        void request("gui_restore_recovery", {
                          token: candidate.token,
                        })
                      }
                    >
                      Restore offline draft
                    </button>
                  )}
                  <button
                    disabled={disabled}
                    onClick={() => {
                      setRecoveryToken(candidate.token);
                      setModal("discard-recovery");
                    }}
                  >
                    Discard recovery snapshot
                  </button>
                </div>
              ))}
            </section>
          )}
          {state.error && (
            <div className="notice error" role="alert">
              {state.error}
            </div>
          )}
          {draftGuardError && (
            <div className="notice error" role="alert">
              {draftGuardError}
            </div>
          )}
          {fieldWriteError && (
            <div className="notice error" role="alert">
              {fieldWriteError}
            </div>
          )}
          {fieldDraft && (
            <section className="notice" aria-label="Unfinished input recovery">
              <strong>Unfinished input fields</strong>
              <p>
                These values are not in the FILE draft. Restore a matching
                field, then confirm it in its editor; or discard the local
                input. Saving or replacing the file is paused meanwhile.
              </p>
              {fieldRecovery.error && <p role="alert">{fieldRecovery.error}</p>}
              {fieldRecovery.drafts.map((draft) => {
                const matches = !!snapshot && fieldMatches(snapshot, draft);
                const restored = draft.field in restoredFields;
                return (
                  <div key={draft.field}>
                    <span>
                      {draft.field}: {draft.value}
                      {!matches && " · different document or original value"}
                      {restored && " · restored in editor"}
                    </span>
                    {!restored && (
                      <button
                        disabled={!matches}
                        onClick={() => {
                          setRestoredFields((fields) => ({
                            ...fields,
                            [draft.field]: draft.value,
                          }));
                          setPage(fieldPage(draft.field));
                        }}
                      >
                        Restore {draft.field}
                      </button>
                    )}
                    <button
                      data-discard-field={draft.field}
                      onClick={() => discardFieldInput(draft.field)}
                    >
                      Discard {draft.field}
                    </button>
                  </div>
                );
              })}
              {volatileFields
                .filter(
                  (field) =>
                    !fieldRecovery.drafts.some(
                      (draft) => draft.field === field,
                    ),
                )
                .map((field) => (
                  <div key={field}>
                    <span>{field} · recovery unavailable</span>
                    <button
                      data-discard-field={field}
                      onClick={() => discardFieldInput(field)}
                    >
                      Discard {field}
                    </button>
                  </div>
                ))}
              {fieldRecovery.error && (
                <button
                  onClick={() => {
                    try {
                      discardFieldRecovery();
                      setFieldRecovery(emptyFieldRecovery);
                      setFieldWriteError(null);
                    } catch {
                      setFieldWriteError(
                        "Could not discard unreadable local input recovery.",
                      );
                    }
                  }}
                >
                  Discard unreadable input recovery
                </button>
              )}
            </section>
          )}
          {macroDraft && page !== "Macros" && (
            <div className="notice">
              Uncommitted macro timeline preserved. Update or discard it before
              saving or replacing the file.
              <button onClick={() => setPage("Macros")}>
                Return to macro editor
              </button>
            </div>
          )}
          {!snapshot && (
            <div className="panel empty-state">
              <h2>
                {state.error
                  ? "Cannot load the offline document"
                  : "Loading your workspace…"}
              </h2>
              <button
                onClick={() => void request("gui_snapshot")}
                disabled={state.busy}
              >
                Retry document load
              </button>
            </div>
          )}
          {snapshot && profile && (
            <>
              {page === "Performance" && (
                <>
                  <section className="performance-hero panel">
                    <div>
                      <span className="eyebrow">PROFILE SENSITIVITY</span>
                      <div className="hero-value">
                        {dpi?.active_stage != null
                          ? (dpi.stages[dpi.active_stage]?.x.toLocaleString(
                              "en-US",
                            ) ?? "—")
                          : "—"}
                        <span>DPI</span>
                      </div>
                      <p>
                        {dpi?.active_stage != null
                          ? `Stage ${dpi.active_stage + 1} selected in this file`
                          : "No active stage specified in this file"}
                      </p>
                      <span className="subtle">
                        File settings only. Nothing is sent to your mouse.
                      </span>
                    </div>
                    <div
                      className="mouse-illustration"
                      aria-label="Mouse illustration, not live device state"
                    >
                      <MouseArt
                        device={profile.device}
                        wheel={zones?.wheel}
                        logo={zones?.logo}
                      />
                    </div>
                  </section>
                  <div className="section-heading">
                    <div>
                      <h2>DPI stages</h2>
                      <p>
                        Switch between sensitivities. Stage colors are separate
                        from lighting.
                      </p>
                    </div>
                    <div className="inline-actions">
                      <span className="counter">
                        {dpi?.stages.length ?? 0} / {limits?.max_stages ?? "—"}
                      </span>
                      <button
                        disabled={
                          disabled ||
                          !limits ||
                          (dpi?.stages.length ?? 0) >= limits.max_stages
                        }
                        onClick={() => setModal("add")}
                      >
                        <Icon name="plus" />
                        Add stage
                      </button>
                    </div>
                  </div>
                  {limits && (
                    <div className="stage-grid">
                      {dpi?.stages.map((stage, index) => (
                        <StageCard
                          key={index}
                          stage={stage}
                          index={index}
                          active={dpi.active_stage === index}
                          limits={limits}
                          disabled={disabled}
                          edit={edit}
                          restoredDpi={restoredFields[`stage:${index}:dpi`]}
                          restoredColor={restoredFields[`stage:${index}:color`]}
                          resetVersions={resetVersions}
                          onDraft={saveFieldInput}
                          clearDraft={clearFieldInput}
                        />
                      ))}
                    </div>
                  )}
                  {!dpi?.stages.length && (
                    <div className="panel empty-state">
                      <h3>No DPI stages in this draft</h3>
                      <p>
                        Add a stage with an explicit DPI and color. Missing
                        settings are never guessed.
                      </p>
                    </div>
                  )}
                  {!!dpi?.stages.length && (
                    <div className="stage-tools">
                      <button
                        className="text-button"
                        disabled={disabled || dpi.active_stage == null}
                        onClick={() =>
                          void edit({ kind: "active-stage", index: null })
                        }
                      >
                        Clear active selection
                      </button>
                      <button
                        className="text-button"
                        disabled={
                          disabled ||
                          dpi.stages.length <= 1 ||
                          dpi.active_stage === dpi.stages.length - 1
                        }
                        title="Select a different active stage before removing the last one"
                        onClick={() => void edit({ kind: "remove-last-stage" })}
                      >
                        Remove last stage
                      </button>
                    </div>
                  )}
                  <section className="panel polling-panel">
                    <div>
                      <h2>Polling rate</h2>
                      <p>USB reporting frequency in the profile.</p>
                    </div>
                    <div className="segmented" aria-label="Polling rate">
                      {snapshot.capabilities?.polling_rates.map((hz) => (
                        <button
                          key={hz}
                          aria-label={`${hz} Hz`}
                          disabled={disabled}
                          aria-pressed={profile.polling?.hz === hz}
                          className={profile.polling?.hz === hz ? "chosen" : ""}
                          onClick={() => void edit({ kind: "polling", hz })}
                        >
                          {hz}
                          <span>Hz</span>
                        </button>
                      ))}
                    </div>
                    <span className="subtle">
                      {profile.polling
                        ? `${(1000 / profile.polling.hz).toFixed(0)} ms interval`
                        : "Not specified"}
                    </span>
                  </section>
                </>
              )}
              {page === "Lighting" && (
                <>
                  <div className="lighting-layout">
                    <section className="panel lighting-visual">
                      <span className="eyebrow">PROFILE COLOR PREVIEW</span>
                      <MouseArt
                        device={profile.device}
                        wheel={zones?.wheel}
                        logo={zones?.logo}
                      />
                      <p>Markers show file colors · Not live LEDs</p>
                    </section>
                    <div className="zone-list">
                      {snapshot.capabilities?.zones.map((zone) => (
                        <section key={zone.id} className="panel zone-card">
                          <div className="section-heading">
                            <h2>{zone.name}</h2>
                            <span className="badge">SOLID</span>
                          </div>
                          <p>
                            {zones?.[zone.id]
                              ? "Independent zone color in this file."
                              : "No color specified. The preview does not imply a device value."}
                          </p>
                          <ColorField
                            label={`${zone.name} color`}
                            color={zones?.[zone.id]}
                            disabled={disabled}
                            restored={restoredFields[`zone:${zone.id}:color`]}
                            resetVersion={
                              resetVersions[`zone:${zone.id}:color`] ?? 0
                            }
                            field={`zone:${zone.id}:color`}
                            onDraft={(value) =>
                              saveFieldInput(
                                `zone:${zone.id}:color`,
                                value,
                                zones?.[zone.id] ?? "",
                              )
                            }
                            clearDraft={() =>
                              clearFieldInput(`zone:${zone.id}:color`)
                            }
                            commit={(color) =>
                              edit({
                                kind: "solid-zone",
                                zone: zone.id,
                                color,
                              })
                            }
                          />
                          <div className="swatches">
                            {[
                              "#FF4B4B",
                              "#FFAB4B",
                              "#B9EB83",
                              "#44CEC2",
                              "#5489FF",
                              "#AF7AFF",
                              "#FFFFFF",
                            ].map((color) => (
                              <button
                                key={color}
                                aria-label={`${zone.name} ${color}`}
                                style={{ backgroundColor: color }}
                                disabled={disabled}
                                onClick={() =>
                                  void edit({
                                    kind: "solid-zone",
                                    zone: zone.id,
                                    color,
                                  })
                                }
                              />
                            ))}
                            <button
                              className="off-swatch"
                              disabled={disabled}
                              onClick={() =>
                                void edit({
                                  kind: "solid-zone",
                                  zone: zone.id,
                                  color: "#000000",
                                })
                              }
                            >
                              Off
                            </button>
                          </div>
                        </section>
                      ))}
                    </div>
                  </div>
                  <div className="notice">
                    This first GUI edits Solid zones only. Hardware effects and
                    software light sync are not exposed here yet. No lighting
                    command or keepalive runs.
                  </div>
                </>
              )}
              {page === "Device" && (
                <div className="device-grid">
                  <section className="panel device-art">
                    <DeviceRender device={profile.device} />
                  </section>
                  <section className="panel">
                    <h2>{snapshot.capabilities?.name ?? profile.device}</h2>
                    <p className="subtle">
                      Supported model metadata, not a connected-device scan.
                    </p>
                    <dl className="spec-list">
                      <div>
                        <dt>Controls</dt>
                        <dd>
                          {snapshot.capabilities?.button_count ?? "Unknown"}
                        </dd>
                      </div>
                      <div>
                        <dt>Supported DPI</dt>
                        <dd>
                          {limits
                            ? `${limits.minimum}–${limits.maximum.toLocaleString("en-US")}`
                            : "Unknown"}
                        </dd>
                      </div>
                      <div>
                        <dt>Lighting zones</dt>
                        <dd>
                          {snapshot.capabilities?.zones
                            .map((zone) => zone.name)
                            .join(" / ") ?? "Unknown"}
                        </dd>
                      </div>
                      <div>
                        <dt>Connection</dt>
                        <dd>Not opened</dd>
                      </div>
                      <div>
                        <dt>Firmware / serial</dt>
                        <dd>Not read</dd>
                      </div>
                    </dl>
                    <div className="notice">
                      Offline mode does not enumerate HID, open the mouse, or
                      send USB reports.
                    </div>
                  </section>
                </div>
              )}
              {page === "Buttons" && (
                <>
                  <ButtonAssignments
                    key={profile.device}
                    snapshot={snapshot}
                    disabled={disabled}
                    edit={edit}
                  />
                  <div className="panel primary-buttons">
                    <div>
                      <h2>Primary buttons</h2>
                      <p>Left and right form one coupled pair.</p>
                    </div>
                    <select
                      aria-label="Primary button layout"
                      value={profile.primary_buttons ?? ""}
                      disabled={disabled}
                      onChange={(event) =>
                        void edit({
                          kind: "primary-buttons",
                          layout:
                            event.target.value === ""
                              ? null
                              : (event.target.value as "standard" | "swapped"),
                        })
                      }
                    >
                      <option value="">Not specified</option>
                      <option value="standard">Standard · left / right</option>
                      <option value="swapped">Swapped · right / left</option>
                    </select>
                  </div>
                  <div className="notice">
                    Binding edits change this file only. Existing library
                    definitions and unresolved source assignments are preserved;
                    invalid values are not silently discarded. Runtime/offline
                    encoding support is not a confirmation of onboard
                    persistence.
                  </div>
                </>
              )}
              <section
                hidden={page !== "Macros"}
                aria-label="Macro library workspace"
              >
                <MacroLibrary
                  snapshot={snapshot}
                  disabled={disabled}
                  edit={edit}
                  onDraftChange={reportMacroDraft}
                  persistLocalDraft={backend.desktop}
                />
              </section>
              {page === "Profiles" && (
                <>
                  <section className="panel profile-details">
                    <div className="section-heading">
                      <h2>Software profile</h2>
                      <span className="badge">TOML</span>
                    </div>
                    <label className="field-label">
                      Profile name
                      <input
                        aria-label="Profile name"
                        value={name}
                        disabled={disabled}
                        maxLength={128}
                        onChange={(event) => {
                          setName(event.target.value);
                          saveFieldInput(
                            "profile.name",
                            event.target.value,
                            profile.name,
                          );
                        }}
                        onBlur={(event) => {
                          if (
                            event.relatedTarget?.getAttribute(
                              "data-discard-field",
                            ) === "profile.name"
                          )
                            return;
                          if (name !== profile.name) {
                            void edit({ kind: "name", name }).then((saved) => {
                              if (saved) clearFieldInput("profile.name");
                            });
                          } else clearFieldInput("profile.name");
                        }}
                        onKeyDown={(event) => {
                          if (event.key === "Enter") event.currentTarget.blur();
                        }}
                      />
                    </label>
                    <dl className="spec-list">
                      <div>
                        <dt>Source</dt>
                        <dd>
                          {snapshot.path ??
                            (snapshot.origin === "demo"
                              ? "Bundled example — demo only"
                              : "Unsaved new draft")}
                        </dd>
                      </div>
                      {snapshot.recovery_path && (
                        <div>
                          <dt>Latest FILE recovery copy</dt>
                          <dd>{snapshot.recovery_path}</dd>
                        </div>
                      )}
                      <div>
                        <dt>Target model</dt>
                        <dd>{profile.device}</dd>
                      </div>
                      <div>
                        <dt>Partial import</dt>
                        <dd>
                          {profile.partial
                            ? "Yes — values may be missing"
                            : "No"}
                        </dd>
                      </div>
                      <div>
                        <dt>Legacy provenance</dt>
                        <dd>
                          {profile.source
                            ? `${profile.source.format} · source format ${profile.source.format_version}`
                            : "None"}
                        </dd>
                      </div>
                      <div>
                        <dt>Unresolved assignments</dt>
                        <dd>
                          {profile.unresolved_button_assignments?.length ?? 0}
                        </dd>
                      </div>
                      {dpi?.source_active_stage != null && (
                        <div>
                          <dt>Unconfirmed source stage</dt>
                          <dd>
                            {dpi.source_active_stage} — preserved, not applied
                            as active
                          </dd>
                        </div>
                      )}
                    </dl>
                    <div className="inline-actions">
                      <button
                        disabled={
                          disabled ||
                          macroDraft ||
                          fieldDraft ||
                          !snapshot.can_undo
                        }
                        onClick={() => void request("gui_undo_file_edit")}
                      >
                        Undo file edit
                      </button>
                      <button
                        disabled={
                          disabled ||
                          macroDraft ||
                          fieldDraft ||
                          !snapshot.can_redo
                        }
                        onClick={() => void request("gui_redo_file_edit")}
                      >
                        Redo file edit
                      </button>
                      <button
                        disabled={disabled || macroDraft || fieldDraft}
                        onClick={() => replace("empty")}
                      >
                        New draft
                      </button>
                      <button
                        disabled={disabled || macroDraft || fieldDraft}
                        onClick={() => replace("demo")}
                      >
                        Load demo
                      </button>
                      <button
                        disabled={disabled || macroDraft || fieldDraft}
                        onClick={() => replace("open")}
                      >
                        <Icon name="open" />
                        Open TOML
                      </button>
                      <button
                        disabled={
                          disabled ||
                          macroDraft ||
                          fieldDraft ||
                          !snapshot.path ||
                          !snapshot.dirty
                        }
                        onClick={() => setModal("overwrite")}
                      >
                        Overwrite opened FILE…
                      </button>
                    </div>
                  </section>
                  <UnresolvedAssignments
                    snapshot={snapshot}
                    disabled={disabled || macroDraft}
                    edit={edit}
                  />
                  <section className="panel validation">
                    <div className="section-heading">
                      <h2>Validation</h2>
                      <span
                        className={`badge ${snapshot.readiness.error ? "warning-badge" : "success-badge"}`}
                      >
                        {snapshot.readiness.error
                          ? "NEEDS ATTENTION"
                          : "VALID PROFILE"}
                      </span>
                    </div>
                    <p>
                      {snapshot.readiness.error ??
                        "The shared model validates this profile. This does not verify a connected device or authorize a hardware write."}
                    </p>
                    {snapshot.readiness.warnings.map((warning) => (
                      <p className="subtle" key={warning}>
                        {warning}
                      </p>
                    ))}
                    <button
                      onClick={() => setModal("review")}
                      disabled={state.busy}
                    >
                      Review changes <Icon name="arrow" />
                    </button>
                  </section>
                  <div className="notice">
                    Save new file never overwrites. Overwrite opened FILE is a
                    separate confirmed action that keeps the exact old file in a
                    recovery directory and refuses external edits. Neither
                    action saves to mouse; hardware writes remain unavailable.
                  </div>
                </>
              )}
            </>
          )}
        </main>
        <footer className="statusbar">
          <div>
            <i
              className={
                snapshot?.dirty || macroDraft || fieldDraft
                  ? "status-dot unsaved"
                  : "status-dot"
              }
            />
            <span>
              {state.busy
                ? "Working on file…"
                : fieldDraft
                  ? "Unfinished input fields"
                  : macroDraft
                    ? "Uncommitted macro timeline"
                    : snapshot?.dirty
                      ? "Unsaved file changes"
                      : "No unsaved file changes"}
            </span>
            <button
              className="text-button"
              disabled={!snapshot || state.busy}
              onClick={() => setModal("review")}
            >
              Review{changes > 0 ? ` (${changes})` : ""}
            </button>
          </div>
          <div className="hardware-actions">
            <span>Hardware actions unavailable</span>
            <button disabled title="Offline editor: no device writes">
              Apply
            </button>
            <button disabled title="Offline editor: no onboard writes">
              Save to mouse
            </button>
          </div>
        </footer>
      </div>
      {modal === "review" && snapshot && (
        <Dialog title="Review file changes" close={() => setModal(null)}>
          <p className="subtle">
            Compared with the last successfully opened or saved file. Not
            compared with the mouse.
          </p>
          {macroDraft && (
            <div className="notice">
              The local macro timeline is not yet in this file diff. Update or
              discard it in Macros before saving the profile.
            </div>
          )}
          {snapshot.changes.error && (
            <div className="notice error">{snapshot.changes.error}</div>
          )}
          {changes === 0 && !snapshot.changes.error && (
            <p>No changes to review.</p>
          )}
          {[...snapshot.changes.settings, ...snapshot.changes.metadata].map(
            (change) => (
              <div className="change-row" key={change.field}>
                <strong>{change.field}</strong>
                <code>{change.before ?? "Not specified"}</code>
                <Icon name="arrow" />
                <code>{change.after ?? "Not specified"}</code>
              </div>
            ),
          )}
          <div className="dialog-actions">
            <button className="primary" onClick={() => setModal(null)}>
              Done
            </button>
          </div>
        </Dialog>
      )}
      {modal === "discard" && (
        <Dialog
          title="Discard unsaved file changes?"
          close={() => setModal(null)}
        >
          <p>
            Replacing this document will discard your edits. No mouse settings
            have been changed.
          </p>
          <div className="dialog-actions">
            <button onClick={() => setModal(null)}>Cancel</button>
            <button
              className="primary"
              disabled={state.busy}
              onClick={() => void doReplace(replacement, true)}
            >
              Discard and continue
            </button>
          </div>
        </Dialog>
      )}
      {modal === "overwrite" && snapshot && (
        <Dialog title="Overwrite opened FILE?" close={() => setModal(null)}>
          <p>
            This replaces only the file at <code>{snapshot.path}</code>. The
            exact old bytes, including comments, will remain in a numbered
            recovery directory. If another program changed this file since it
            was opened or saved, the operation will be refused.
          </p>
          <p className="subtle">
            A crash or I/O failure may require restoring from that directory.
            This does not save anything to the mouse.
          </p>
          {state.error && <div className="notice error">{state.error}</div>}
          <div className="dialog-actions">
            <button onClick={() => setModal(null)}>Cancel</button>
            <button
              className="primary"
              disabled={
                state.busy ||
                macroDraft ||
                fieldDraft ||
                !snapshot.path ||
                !snapshot.dirty
              }
              onClick={() =>
                void request("gui_overwrite_profile", { confirmed: true }).then(
                  (saved) => {
                    if (saved) setModal(null);
                  },
                )
              }
            >
              Overwrite FILE and keep recovery copy
            </button>
          </div>
        </Dialog>
      )}
      {modal === "discard-recovery" && recoveryToken && (
        <Dialog
          title="Discard offline recovery snapshot?"
          close={() => setModal(null)}
        >
          <p>
            Permanently remove only snapshot <code>{recoveryToken}</code>? This
            cannot be undone. The current FILE draft and mouse are unaffected.
          </p>
          {state.error && <div className="notice error">{state.error}</div>}
          <div className="dialog-actions">
            <button onClick={() => setModal(null)}>Keep snapshot</button>
            <button
              disabled={state.busy}
              onClick={() =>
                void request("gui_discard_recovery", {
                  token: recoveryToken,
                  confirmed: true,
                }).then((discarded) => {
                  if (discarded) {
                    setRecoveryToken(null);
                    setModal(null);
                  }
                })
              }
            >
              Discard snapshot permanently
            </button>
          </div>
        </Dialog>
      )}
      {modal === "add" && limits && (
        <Dialog title="Add DPI stage" close={() => setModal(null)}>
          <form
            onSubmit={async (event) => {
              event.preventDefault();
              if (
                await edit({
                  kind: "add-stage",
                  dpi: addedDpi,
                  color: addedColor,
                })
              )
                setModal(null);
            }}
          >
            <label className="field-label">
              DPI
              <input
                type="number"
                aria-label="New stage DPI"
                min={limits.minimum}
                max={limits.maximum}
                step={limits.step}
                required
                value={addedDpi}
                disabled={disabled}
                onChange={(event) => setAddedDpi(event.target.valueAsNumber)}
              />
            </label>
            <label className="field-label">
              Stage color
              <input
                type="color"
                aria-label="New stage color"
                value={addedColor}
                disabled={disabled}
                onChange={(event) =>
                  setAddedColor(event.target.value.toUpperCase())
                }
              />
            </label>
            <p className="subtle">
              Adding a stage does not select it as active.
            </p>
            {state.error && (
              <div className="notice error" role="alert">
                {state.error}
              </div>
            )}
            <div className="dialog-actions">
              <button type="button" onClick={() => setModal(null)}>
                Cancel
              </button>
              <button type="submit" className="primary" disabled={disabled}>
                Add stage
              </button>
            </div>
          </form>
        </Dialog>
      )}
    </div>
  );
}
