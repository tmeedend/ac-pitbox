// A thumbnail preset as a starting point for the 3D preview (Réglages › Aperçu
// 3D, « Partir d'un preset de vignettes »). A copy, not a link: the two keep
// their own settings — touching the 3D preview costs nothing, touching a
// preset regenerates the library — and only what both screens mean the same
// way is carried over.
import type { GridTemplate } from "./gridThumbs";

/** The 3D preview settings a preset fills in. Each name is the 3D one; the
 * preset's own name follows when it differs. */
export type Preview3dFromPreset = {
  azimuth: number;
  elevation: number;
  fov: number;
  height: number;
  shadow: number;
  /** The preset's `floor`: the pool of light under the car. */
  pool: number;
  reflection: number;
  reflectionBlur: number;
  /** The preset's `ambient`: both are the studio's light, in percent. */
  light: number;
  glass: number;
  chrome: number;
  saturation: number;
  coolTint: number;
};

/**
 * What a preset gives the 3D preview. Left out, because they mean nothing for
 * a view one turns by hand or belong to a still image: the margin, true scale
 * and game framing, the baked background and its mat, auto exposure, the disc
 * of light. The 3D preview's zoom, spin, exposure and reflection reach are
 * its own and stay as they are.
 */
export function preview3dFromPreset(template: GridTemplate): Preview3dFromPreset {
  return {
    azimuth: template.azimuth,
    elevation: template.elevation,
    fov: template.fov,
    height: template.height,
    shadow: template.shadow,
    pool: template.floor,
    reflection: template.reflection,
    reflectionBlur: template.reflectionBlur,
    light: template.ambient,
    glass: template.glass,
    chrome: template.chrome,
    saturation: template.saturation,
    coolTint: template.coolTint,
  };
}
