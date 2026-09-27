import type { MouseView } from "./mouse-art";

export interface ButtonHotspot {
  id: string;
  view: MouseView;
  location: string;
  path: string;
  pin: [number, number];
  label: string;
}

// UI geometry in cropped RAID_VIEWS pixels, never HID slots/packet offsets.
// Physical identities: HyperX's user guide, English Overview (PDF page 4):
// https://media.kingston.com/support/downloads/HyperX-Pulsefire-Raid-User-guide.pdf
// With the front pointing LEFT in the side render: upper front = 7, upper
// rear = 6, lower front = 5, lower rear = 4, tall front = 8. Factory bindings
// in that guide identify positions only; current assignments come from files.
// Wheel arrows select the tilt motions, not additional physical keys.
const raid: ButtonHotspot[] = [
  {
    id: "left-click",
    view: "top",
    location: "Main left",
    path: "M50 60L144 23L154 65H138V155H156V206L145 232L80 285L54 263Z",
    pin: [94, 185],
    label: "L",
  },
  {
    id: "right-click",
    view: "top",
    location: "Main right",
    path: "M239 23L331 60L305 263L289 285L235 232L224 206V155H240V65H226Z",
    pin: [278, 185],
    label: "R",
  },
  {
    id: "wheel-click",
    view: "top",
    location: "Wheel press",
    path: "M168 68H213V163H168Z",
    pin: [190, 112],
    label: "3",
  },
  {
    id: "dpi",
    view: "top",
    location: "Behind the wheel",
    path: "M177 199H207V253H177Z",
    pin: [192, 226],
    label: "D",
  },
  {
    id: "wheel-tilt-left",
    view: "top",
    location: "Wheel tilt left",
    path: "M139 92H163V132H139Z",
    pin: [151, 112],
    label: "←",
  },
  {
    id: "wheel-tilt-right",
    view: "top",
    location: "Wheel tilt right",
    path: "M217 92H241V132H217Z",
    pin: [229, 112],
    label: "→",
  },
  {
    id: "button8",
    view: "left",
    location: "Tall front side button",
    path: "M201 93Q202 80 219 78L231 96L265 170Q263 185 246 184H224Z",
    pin: [229, 139],
    label: "8",
  },
  {
    id: "button7",
    view: "left",
    location: "Upper front side button",
    path: "M248 64L319 49L330 66L250 83Z",
    pin: [286, 67],
    label: "7",
  },
  {
    id: "button6",
    view: "left",
    location: "Upper rear side button",
    path: "M335 44L397 35Q416 37 445 59L346 66Z",
    pin: [383, 51],
    label: "6",
  },
  {
    id: "button5",
    view: "left",
    location: "Lower front side button",
    path: "M252 87L332 81L342 96L258 108Z",
    pin: [298, 95],
    label: "5",
  },
  {
    id: "button4",
    view: "left",
    location: "Lower rear side button",
    path: "M346 78L455 65Q465 82 456 91L351 99Z",
    pin: [401, 83],
    label: "4",
  },
];

export function buttonLayout(device: string): readonly ButtonHotspot[] {
  return device === "pulsefire-raid" ? raid : [];
}
