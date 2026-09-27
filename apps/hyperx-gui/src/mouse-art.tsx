import { useId, useState, type ReactNode } from "react";

export type MouseView = "top" | "left" | "right";
export const RAID_ATLAS = "/devices/pulsefire-raid/ngenuity-legacy-atlas.png";

// Presentation geometry only: source-image pixels, not HID IDs or button slots.
// Keep the manufacturer atlas byte-identical; SVG selects one view at runtime.
export const RAID_VIEWS = {
  top: { x: 765, y: 0, width: 351, height: 640, label: "Top" },
  left: { x: 18, y: 353, width: 674, height: 215, label: "Left side" },
  right: { x: 1220, y: 357, width: 674, height: 211, label: "Right side" },
} as const;

export function MouseArt({
  device,
  view = "top",
  wheel,
  logo,
  children,
}: {
  device: string;
  view?: MouseView;
  wheel?: string;
  logo?: string;
  children?: ReactNode;
}) {
  const [failed, setFailed] = useState(false);
  const clipId = useId();
  if (device !== "pulsefire-raid" || failed) {
    return (
      <div
        className="mouse-art-empty"
        role="img"
        aria-label="Mouse render unavailable"
      >
        <span className="eyebrow">NO MODEL RENDER</span>
        <p>
          {device !== "pulsefire-raid"
            ? "No image for this model."
            : "Mouse image could not be loaded."}
        </p>
      </div>
    );
  }
  const crop = RAID_VIEWS[view];
  return (
    <svg
      className="mouse-art mouse-photo"
      viewBox={`0 0 ${crop.width} ${crop.height}`}
      role={children ? "group" : "img"}
      aria-label={`Pulsefire Raid ${crop.label.toLowerCase()} ${children ? "button map" : "manufacturer render"}`}
    >
      {/* SVG's letterboxing enlarges its visible user-space viewport. Clip to
          the crop itself so neighboring atlas views never leak into that space. */}
      <defs>
        <clipPath id={clipId}>
          <rect width={crop.width} height={crop.height} />
        </clipPath>
      </defs>
      <image
        clipPath={`url(#${clipId})`}
        href={RAID_ATLAS}
        x={-crop.x}
        y={-crop.y}
        width="1914"
        height="640"
        preserveAspectRatio="none"
        onError={() => setFailed(true)}
      />
      {children}
      {view === "top" && (
        <>
          {/* Markers indicate supplied FILE colors, not simulated LED pixels.
            Missing zones have no marker; black remains a visible off marker. */}
          {wheel !== undefined && (
            <g data-zone="wheel" aria-hidden="true">
              <path d="M215 110h22" stroke="#87928b" strokeWidth="2" />
              <circle
                cx="244"
                cy="110"
                r="8"
                fill={wheel}
                stroke="#87928b"
                strokeWidth="2"
              />
            </g>
          )}
          {logo !== undefined && (
            <g data-zone="logo" aria-hidden="true">
              <path d="M227 512h35" stroke="#87928b" strokeWidth="2" />
              <circle
                cx="270"
                cy="512"
                r="8"
                fill={logo}
                stroke="#87928b"
                strokeWidth="2"
              />
            </g>
          )}
        </>
      )}
    </svg>
  );
}

export function DeviceRender({ device }: { device: string }) {
  const [view, setView] = useState<MouseView>("top");
  return (
    <>
      <div className="device-render-frame">
        <MouseArt device={device} view={view} />
      </div>
      {device === "pulsefire-raid" && (
        <div className="angle-selector" aria-label="Mouse render view">
          {(Object.keys(RAID_VIEWS) as MouseView[]).map((angle) => (
            <button
              key={angle}
              className="small-button"
              aria-pressed={angle === view}
              onClick={() => setView(angle)}
            >
              {RAID_VIEWS[angle].label}
            </button>
          ))}
        </div>
      )}
      <span className="badge">MODEL REFERENCE</span>
    </>
  );
}
