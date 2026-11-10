// The finish of a grid thumbnail (docs/SPEC-grille.md GRILLE§5.6): what the
// glass reflects, the exposure, and the colour grade — the settings that move
// the render towards the game's look without touching its framing or floor.
import type * as ThreeModule from "three";
import type { GridTemplate } from "./gridThumbs";

/**
 * Window reflections (`glass`): how much of the studio the glass mirrors,
 * in percent of what the rest of the car does.
 *
 * Glass is blended, and blending attenuates its reflection along with the
 * rest of its colour (see `materials.ts`): against our dark studio, the
 * windows read as plain dark holes where the game's previews show a pale
 * sheen. Raising the environment would brighten the paint too; this raises
 * it on the glass only, by giving those materials the scene's environment
 * as their own map — the one way three lets a material weigh it apart.
 * At 100 % nothing is touched, so presets from before it render as they did.
 */
export function reflectGlass(model: ThreeModule.Object3D, scene: ThreeModule.Scene, glass: number): void {
  model.traverse((object) => {
    const mesh = object as ThreeModule.Mesh;
    if (!mesh.isMesh) return;
    for (const raw of Array.isArray(mesh.material) ? mesh.material : [mesh.material]) {
      const material = raw as ThreeModule.MeshPhysicalMaterial;
      if (!material.isMeshStandardMaterial || !(material.transparent || (material.transmission ?? 0) > 0)) continue;
      const own = glass !== 100;
      if ((material.envMap !== null) !== own) material.needsUpdate = true;
      material.envMap = own ? scene.environment : null;
      material.envMapIntensity = own ? (scene.environmentIntensity * glass) / 100 : 1;
    }
  });
}

/** Auto exposure (`autoExposure`). **The game's preview tool does not use
 * one** — its `ppeffects.ini` turns `AUTO_EXPOSURE` off — but our studio does
 * not light like theirs, and metering each image closes part of that gap.
 * The bounds are borrowed from the game's in-race filter
 * (`system/cfg/ppfilters/default.ini`: 0.2 to 0.5 around 0.28), as a ratio
 * only. The target is measured: the mean brightness of an Officiel
 * image runs from 0.006 (a black E30) to 0.052 (a white AE86), mostly
 * background; 0.015, just under the median, matched the game's previews best
 * over twelve cars. A first guess at 0.1 pinned every car to the ceiling.
 *
 * What it buys, measured: white paint stops burning out (AE86 median 141
 * against 144 in the game, 161 at fixed exposure). What it does not: black
 * paint stays black — near zero times 1.8 is still near zero. */
const EXPOSURE_TARGET = 0.015;
const EXPOSURE_MIN = 0.7;
const EXPOSURE_MAX = 1.8;

/**
 * Exposure that brings this image to the target brightness (`autoExposure`).
 *
 * The whole frame is metered, background included. Mean taken in linear light, on a 64×36
 * reduction: two thousand pixels say how bright an image is as well as two
 * million.
 */
export function meteredExposure(canvas: HTMLCanvasElement): number {
  const probe = document.createElement("canvas");
  probe.width = 64;
  probe.height = 36;
  const ctx = probe.getContext("2d", { willReadFrequently: true });
  if (!ctx) return 1;
  ctx.drawImage(canvas, 0, 0, probe.width, probe.height);
  const { data } = ctx.getImageData(0, 0, probe.width, probe.height);
  let sum = 0;
  for (let i = 0; i < data.length; i += 4) {
    const luma = (data[i] * 0.2126 + data[i + 1] * 0.7152 + data[i + 2] * 0.0722) / 255;
    sum += Math.pow(luma, 2.2) * (data[i + 3] / 255);
  }
  const mean = sum / (probe.width * probe.height);
  if (!(mean > 0)) return EXPOSURE_MAX;
  return Math.min(EXPOSURE_MAX, Math.max(EXPOSURE_MIN, EXPOSURE_TARGET / mean));
}

/**
 * The colour grade of the game's preview tool, as far as it carries over to
 * our render (`saturation`, `coolTint`): **saturation down and a cool tint**,
 * the same for every car, so nothing about it breaks comparability.
 *
 * The SDK's `ppeffects.ini` (`sdk/dev/ksPreviewBuilder/dev/previews/`) grades
 * every preview with gamma 1.2, saturation 0.9, brightness 1.08, contrast
 * 1.05 and a colour temperature of 6000 K against a 6400 K white balance.
 * Copied as is onto our render, it made every car too bright (our lights are
 * not theirs): measured on seventeen stock cars, only the saturation and the
 * tint carry over, at values of their own. Our paint came out too saturated
 * (+12 on a 0–255 chroma scale), and the game's whites and greys are bluish
 * where ours were neutral — an Abarth 500 is (151, 157, 163) in the game.
 * With saturation 75 % and cool tint 100 %, colour error 15.0 → 10.9,
 * saturation error 22 → 9.5, silhouette and brightness unchanged. The tint at
 * 100 % is that measured cast: red × 0.97, green × 1.02, blue × 1.055 —
 * other strengths scale it from neutral.
 *
 * Applied on a 2D copy: the WebGL canvas cannot be written back, and the
 * queue reads its PNG from the copy anyway.
 */
const GRADE_TINT = [-0.03, 0.02, 0.055] as const;

/** Whether this template changes any colour at all — the copy and the walk
 * over every pixel are skipped otherwise. */
export function gradesColours(template: GridTemplate): boolean {
  return template.saturation !== 100 || template.coolTint !== 0;
}

export function gradeColours(source: HTMLCanvasElement, template: GridTemplate): HTMLCanvasElement {
  const out = document.createElement("canvas");
  out.width = source.width;
  out.height = source.height;
  const ctx = out.getContext("2d", { willReadFrequently: true });
  if (!ctx) return source;
  ctx.drawImage(source, 0, 0);
  const image = ctx.getImageData(0, 0, out.width, out.height);
  const data = image.data;
  const tint = template.coolTint / 100;
  const tr = 1 + GRADE_TINT[0] * tint;
  const tg = 1 + GRADE_TINT[1] * tint;
  const tb = 1 + GRADE_TINT[2] * tint;
  const saturation = template.saturation / 100;
  for (let i = 0; i < data.length; i += 4) {
    const r = data[i] * tr;
    const g = data[i + 1] * tg;
    const b = data[i + 2] * tb;
    const luma = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    // Uint8ClampedArray clamps and rounds on write.
    data[i] = luma + (r - luma) * saturation;
    data[i + 1] = luma + (g - luma) * saturation;
    data[i + 2] = luma + (b - luma) * saturation;
  }
  ctx.putImageData(image, 0, 0);
  return out;
}
