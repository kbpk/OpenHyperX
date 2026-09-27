import type { Page } from "./types";

const paths: Record<
  Page | "open" | "save" | "plus" | "check" | "arrow" | "close",
  string
> = {
  Device: "M8 3h8l3 4v10l-3 4H8l-3-4V7z M12 3v7 M10 10h4",
  Performance:
    "M3 17a9 9 0 1 1 18 0 M6 17h12 M12 14l4-6 M7 8l1 1 M16 5v1 M4 13h1",
  Buttons: "M5 4h14v16H5z M5 10h14 M12 4v6 M8 14h3 M14 14h2 M8 17h3",
  Macros: "M8 5H4v14h4 M16 5h4v14h-4 M13 7l-2 10",
  Lighting: "M9 18h6 M10 21h4 M8 13a6 6 0 1 1 8 0l-1 3H9z",
  Profiles: "M5 3h10l4 4v14H5z M14 3v5h5 M8 12h8 M8 16h6",
  open: "M3 7h7l2 2h9l-2 11H3z M3 7V4h7l2 3",
  save: "M4 3h14l3 3v15H3V3z M7 3v6h10V3 M7 21v-8h10v8",
  plus: "M12 5v14 M5 12h14",
  check: "M5 12l4 4L19 6",
  arrow: "M5 12h14 M13 6l6 6-6 6",
  close: "M6 6l12 12 M6 18 18 6",
};
export function Icon({
  name,
  size = 19,
}: {
  name: keyof typeof paths;
  size?: number;
}) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.6"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d={paths[name]} />
    </svg>
  );
}
