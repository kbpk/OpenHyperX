import { useState } from "react";
import { buttonLayout } from "./button-layout";
import { MouseArt } from "./mouse-art";
import type { Profile, Snapshot } from "./types";

type Control = Snapshot["controls"][number];
const humanize = (text: string) => text.replaceAll("-", " ");

function assignment(control: Control, profile: Profile) {
  if (control.primary) {
    if (!profile.primary_buttons) return "Not specified";
    const left = control.id === "left-click";
    const standard = profile.primary_buttons === "standard";
    return `${left === standard ? "Left" : "Right"} click · ${profile.primary_buttons} primary pair`;
  }
  const binding = profile.buttons?.[control.id];
  if (!binding) return "Not specified";
  const value = binding.action ?? binding.key ?? binding.id;
  return [humanize(binding.type), value && humanize(value)]
    .filter(Boolean)
    .join(" · ");
}

export function ButtonAssignments({ snapshot }: { snapshot: Snapshot }) {
  const { profile, controls } = snapshot;
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [hoverId, setHoverId] = useState<string | null>(null);
  const selected =
    controls.find((control) => control.id === selectedId) ?? controls[0];
  const layout = buttonLayout(profile.device).filter((spot) =>
    controls.some((control) => control.id === spot.id),
  );
  const location = layout.find((spot) => spot.id === selected?.id)?.location;
  const binding = selected && profile.buttons?.[selected.id];
  const macro =
    binding?.type === "macro"
      ? profile.macros?.find((macro) => macro.source_id === binding.id)
      : undefined;

  return (
    <>
      <section
        className="panel button-picker"
        aria-label="Physical button selection"
      >
        <div className="section-heading">
          <div>
            <span className="eyebrow">POINT. SELECT. INSPECT.</span>
            <h2>Choose a control</h2>
          </div>
          <span className="badge">FILE BINDINGS</span>
        </div>
        {layout.length > 0 ? (
          <div className="button-map-views">
            {(["top", "left"] as const).map((view) => (
              <div className={`button-map-view button-map-${view}`} key={view}>
                <span className="eyebrow">
                  {view === "top"
                    ? "TOP · WHEEL ARROWS = TILT"
                    : "LEFT SIDE · FRONT AT LEFT"}
                </span>
                <MouseArt device={profile.device} view={view}>
                  {layout
                    .filter((spot) => spot.view === view)
                    .map((spot) => {
                      const control = controls.find(
                        (control) => control.id === spot.id,
                      )!;
                      return (
                        <g
                          key={spot.id}
                          role="button"
                          tabIndex={0}
                          aria-label={`Select ${control.name} on mouse`}
                          aria-pressed={selected?.id === spot.id}
                          data-control={spot.id}
                          className={`button-hotspot${hoverId === spot.id ? " hovered" : ""}`}
                          onClick={() => setSelectedId(spot.id)}
                          onMouseEnter={() => setHoverId(spot.id)}
                          onMouseLeave={() => setHoverId(null)}
                          onKeyDown={(event) => {
                            if (event.key === "Enter" || event.key === " ") {
                              event.preventDefault();
                              setSelectedId(spot.id);
                            }
                          }}
                        >
                          <title>
                            {control.name} — {spot.location}
                          </title>
                          <path className="button-hit-area" d={spot.path} />
                          <circle
                            className="button-pin"
                            cx={spot.pin[0]}
                            cy={spot.pin[1]}
                            r={view === "top" ? 18 : 12}
                          />
                          <text
                            x={spot.pin[0]}
                            y={spot.pin[1]}
                            aria-hidden="true"
                          >
                            {spot.label}
                          </text>
                        </g>
                      );
                    })}
                </MouseArt>
              </div>
            ))}
          </div>
        ) : (
          <p>
            No physical layout is available for this model. Select a control in
            the table.
          </p>
        )}
        <p className="button-map-help">
          Click the mouse or a table row. Tab + Enter / Space works too.
          Selection does not change the profile.
        </p>
        <div
          className="selected-control"
          role="region"
          aria-label="Selected control"
          aria-live="polite"
          aria-atomic="true"
        >
          {selected ? (
            <>
              <div>
                <span className="eyebrow">SELECTED CONTROL</span>
                <h3>{selected.name}</h3>
                <p>{location ?? selected.id}</p>
              </div>
              <div>
                <span className="eyebrow">BINDING IN FILE</span>
                <p className="selected-binding">
                  {assignment(selected, profile)}
                </p>
                {selected.primary && (
                  <p>Edited together with the other primary button.</p>
                )}
                {binding?.type === "macro" && (
                  <p>
                    {macro
                      ? `${macro.name} · ${humanize(macro.playback)} · ${macro.events.length} events`
                      : "Macro definition not present in this file."}
                  </p>
                )}
                {selected.macros && (
                  <p>
                    Macro runtime:{" "}
                    {selected.macros.runtime.map(humanize).join(" / ")}
                    <br />
                    Onboard:{" "}
                    {selected.macros.onboard.map(humanize).join(" / ") ||
                      "Not exposed"}
                  </p>
                )}
              </div>
            </>
          ) : (
            <p>No controls declared for this model.</p>
          )}
        </div>
      </section>
      <section className="panel table-panel">
        <div className="section-heading">
          <h2>Control assignments</h2>
          <span className="badge">INSPECT</span>
        </div>
        <table>
          <thead>
            <tr>
              <th>Control</th>
              <th>Binding in file</th>
              <th>Macro runtime support</th>
            </tr>
          </thead>
          <tbody>
            {controls.map((control) => (
              <tr
                key={control.id}
                className={`control-row${selected?.id === control.id ? " selected" : ""}${hoverId === control.id ? " hovered" : ""}`}
                onClick={() => setSelectedId(control.id)}
                onMouseEnter={() => setHoverId(control.id)}
                onMouseLeave={() => setHoverId(null)}
              >
                <td>
                  <button
                    className="control-select"
                    aria-label={`Select ${control.name} in table`}
                    aria-pressed={selected?.id === control.id}
                    onClick={() => setSelectedId(control.id)}
                  >
                    {control.name}
                    <small>{control.id}</small>
                  </button>
                </td>
                <td>{assignment(control, profile)}</td>
                <td>
                  {control.macros
                    ? control.macros.runtime.map(humanize).join(" / ")
                    : "Not exposed"}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </section>
    </>
  );
}
