import { describe, expect, it } from "vitest";
import { DESKTOP_PALETTES } from "./desktopPalette";

describe("desktop palettes", () => {
  it("keeps the v0.1.5 light quota progress gradients", () => {
    expect(DESKTOP_PALETTES.light.healthy).toMatchObject({
      "--progress-start": "#397AE0",
      "--progress-end": "#91BAF0",
    });
    expect(DESKTOP_PALETTES.light.caution).toMatchObject({
      "--progress-start": "#4D88D8",
      "--progress-end": "#9FC2EE",
    });
    expect(DESKTOP_PALETTES.light.critical).toMatchObject({
      "--progress-start": "#FF7848",
      "--progress-end": "#FFD064",
    });
  });

  it("keeps light and dark palettes independent", () => {
    expect(DESKTOP_PALETTES.light.caution).not.toEqual(DESKTOP_PALETTES.dark.caution);
    expect(DESKTOP_PALETTES.light.critical).not.toEqual(DESKTOP_PALETTES.dark.critical);
  });

  it("uses the grey-space dark caution and critical palettes", () => {
    expect(DESKTOP_PALETTES.dark.caution).toMatchObject({
      "--cool": "#40434A",
      "--glow": "#302E2B",
      "--warm": "#15171A",
      "--progress-start": "#B59B73",
      "--progress-end": "#E2D2B8",
    });
    expect(DESKTOP_PALETTES.dark.critical).toMatchObject({
      "--cool": "#3C3E43",
      "--glow": "#3A302E",
      "--warm": "#151619",
      "--progress-start": "#B8796C",
      "--progress-end": "#E2ADA2",
    });
  });
});
