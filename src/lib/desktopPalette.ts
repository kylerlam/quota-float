import type { CSSProperties } from "react";
import type { WidgetTheme } from "../types";

export type DesktopPaletteName = "healthy" | "caution" | "critical" | "unavailable" | "stale" | "signed_out";
export type DesktopPaletteStyle = CSSProperties & {
  "--cool": string;
  "--glow": string;
  "--warm": string;
  "--progress-start": string;
  "--progress-end": string;
  "--glow-fade"?: string;
  "--warm-position"?: string;
  "--warm-fade"?: string;
  "--linear-warm"?: string;
  "--linear-end"?: string;
  "--gradient-angle"?: string;
  "--aurora-opacity"?: string;
};

export type DesktopPalettes = Record<DesktopPaletteName, DesktopPaletteStyle>;

// Runtime source of truth for every production desktop palette. The design
// workbench reads this object and never reads, writes, or persists its own
// palette values. Keep light and dark as separate records: changing one theme
// must never alter the other theme's desktop rendering.
//
// Light values intentionally match the v0.1.5 release visual baseline.
export const DESKTOP_PALETTES: Record<WidgetTheme, DesktopPalettes> = {
  light: {
    healthy: { "--cool": "#B9D5EE", "--glow": "#DFF4E5", "--warm": "#C7DDF2", "--progress-start": "#397AE0", "--progress-end": "#91BAF0", "--aurora-opacity": ".42" },
    caution: { "--cool": "#B7D0EC", "--glow": "#FFF0BA", "--warm": "#F4C979", "--progress-start": "#4D88D8", "--progress-end": "#9FC2EE", "--glow-fade": "58%", "--warm-position": "12% 96%", "--warm-fade": "66%", "--linear-warm": "#E4E7ED", "--linear-end": "#F1F5F8", "--gradient-angle": "213deg", "--aurora-opacity": ".5" },
    critical: { "--cool": "#C4CEE0", "--glow": "#FFD8A8", "--warm": "#F07260", "--progress-start": "#FF7848", "--progress-end": "#FFD064", "--glow-fade": "60%", "--warm-position": "11% 98%", "--warm-fade": "68%", "--linear-warm": "#E3E4E9", "--linear-end": "#F3F5F8", "--gradient-angle": "213deg", "--aurora-opacity": ".56" },
    unavailable: { "--cool": "#849CD6", "--glow": "#FFF4C3", "--warm": "#FF9A4E", "--progress-start": "#397AE0", "--progress-end": "#89B7FF" },
    stale: { "--cool": "#849CD6", "--glow": "#FFF4C3", "--warm": "#FF9A4E", "--progress-start": "#397AE0", "--progress-end": "#89B7FF" },
    signed_out: { "--cool": "#688CD4", "--glow": "#D7EEF3", "--warm": "#D89CA5", "--progress-start": "#397AE0", "--progress-end": "#89B7FF", "--linear-warm": "#BECBE2", "--linear-end": "#E3EAF4", "--gradient-angle": "145deg", "--aurora-opacity": ".58" },
  },
  dark: {
    healthy: { "--cool": "#3B4048", "--glow": "#252A31", "--warm": "#111419", "--progress-start": "#8D99A8", "--progress-end": "#D3D9E0", "--linear-warm": "#171A20", "--linear-end": "#0D1014", "--aurora-opacity": "1" },
    caution: { "--cool": "#40434A", "--glow": "#302E2B", "--warm": "#15171A", "--progress-start": "#B59B73", "--progress-end": "#E2D2B8", "--linear-warm": "#1B1A18", "--linear-end": "#101114", "--aurora-opacity": "1" },
    critical: { "--cool": "#3C3E43", "--glow": "#3A302E", "--warm": "#151619", "--progress-start": "#B8796C", "--progress-end": "#E2ADA2", "--warm-position": "11% 98%", "--warm-fade": "68%", "--linear-warm": "#1D1919", "--linear-end": "#101114", "--gradient-angle": "213deg", "--aurora-opacity": "1" },
    unavailable: { "--cool": "#41464E", "--glow": "#30343A", "--warm": "#191C21", "--progress-start": "#77808C", "--progress-end": "#B8C0CA" },
    stale: { "--cool": "#383D43", "--glow": "#2D3238", "--warm": "#181B1F", "--progress-start": "#747D87", "--progress-end": "#ADB5BE" },
    signed_out: { "--cool": "#42464D", "--glow": "#30343A", "--warm": "#202226", "--progress-start": "#858E9A", "--progress-end": "#C2C9D1", "--linear-warm": "#202328", "--linear-end": "#111419", "--aurora-opacity": "1" },
  },
};
