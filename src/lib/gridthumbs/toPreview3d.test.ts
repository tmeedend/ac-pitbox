import { describe, expect, it } from "vitest";
import { preview3dFromPreset } from "./toPreview3d";
import type { GridTemplate } from "./gridThumbs";

const template: GridTemplate = {
  azimuth: 306,
  elevation: 10,
  fov: 40,
  margin: 5,
  shadow: 30,
  height: -7,
  floor: 60,
  reflection: 90,
  reflectionBlur: 35,
  background: 100,
  trueScale: 100,
  ambient: 160,
  autoExposure: 100,
  gameFraming: 100,
  saturation: 75,
  coolTint: 100,
  glass: 200,
  chrome: 250,
  poolSize: 140,
  poolEdge: 50,
  matHi: "#05080a",
  matLo: "#020305",
  renderer: 7,
};

describe("preview3dFromPreset", () => {
  // Rule: what both screens mean the same way is copied, under the 3D name.
  it("copies the shared settings, renaming floor and ambient", () => {
    const values = preview3dFromPreset(template);
    expect(values.azimuth).toBe(306);
    expect(values.height).toBe(-7);
    expect(values.pool).toBe(60);
    expect(values.light).toBe(160);
    expect(values.chrome).toBe(250);
    expect(values.coolTint).toBe(100);
  });

  // Rule: what belongs to a still image does not reach the 3D preview.
  it("leaves out what only a still image means", () => {
    const keys = Object.keys(preview3dFromPreset(template));
    for (const still of ["margin", "trueScale", "gameFraming", "background", "autoExposure", "poolSize", "matHi"]) {
      expect(keys).not.toContain(still);
    }
  });
});
