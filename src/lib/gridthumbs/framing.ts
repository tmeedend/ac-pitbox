// Where the camera stands for a grid thumbnail (docs/SPEC-grille.md GRILLE§5.6):
// the orbit fitted to each car, its true-scale variant, the lens shift that
// centres the silhouette, and the game's own preview camera.
//
// Out of `gridThumbs.svelte.ts`, which keeps the queue and the rig: framing
// changes for its own reasons — a measurement against the game's images, a
// new option — and every one of them had grown the renderer file.
import type * as ThreeModule from "three";
import type { GridTemplate } from "./gridThumbs";

/**
 * Cadrage **ajusté**, pas à l'échelle : chaque voiture remplit le cadre quelle
 * que soit sa taille réelle (GRILLE§5.6).
 *
 * La distance ne se déduit pas d'un rayon : une voiture vue de trois-quarts est
 * large et basse, et la caler sur sa sphère englobante la laisserait minuscule
 * dans un cadre 16:9. Chacun des huit coins de la boîte est donc projeté dans
 * le repère de la caméra, et on retient la distance qui les fait tous entrer —
 * horizontalement **et** verticalement.
 */
export function placeCamera(
  THREE: typeof ThreeModule,
  camera: ThreeModule.PerspectiveCamera,
  box: ThreeModule.Box3,
  center: ThreeModule.Vector3,
  radius: number,
  template: GridTemplate,
): void {
  camera.fov = template.fov;
  camera.updateProjectionMatrix();
  if (template.gameFraming > 0 && gameFramingFits(radius)) {
    placeGameCamera(camera);
    return;
  }

  const azimuth = (template.azimuth * Math.PI) / 180;
  const elevation = (template.elevation * Math.PI) / 180;
  // Direction de la cible **vers** la caméra.
  const dir = new THREE.Vector3(
    Math.cos(elevation) * Math.sin(azimuth),
    Math.sin(elevation),
    Math.cos(elevation) * Math.cos(azimuth),
  ).normalize();
  const right = new THREE.Vector3().crossVectors(new THREE.Vector3(0, 1, 0), dir).normalize();
  const up = new THREE.Vector3().crossVectors(dir, right).normalize();

  const margin = 1 + template.margin / 100;
  const tanV = Math.tan((camera.fov * Math.PI) / 360) / margin;
  const tanH = tanV * camera.aspect;

  // Le point visé monte ou descend avec la hauteur de cadrage : c'est lui qui
  // décide de la place de la voiture dans le cadre, là où la plongée décide de
  // ce qu'on voit de son toit. Viser **sous** la voiture la remonte dans
  // l'image, ce que demande un preset à reflet — le reflet prend la place
  // libérée en dessous.
  const target = center.clone();
  target.y += (radius * template.height) / 100;

  let distance = 0;
  const corner = new THREE.Vector3();
  for (const x of [box.min.x, box.max.x]) {
    for (const y of [box.min.y, box.max.y]) {
      for (const z of [box.min.z, box.max.z]) {
        corner.set(x, y, z).sub(target);
        const depth = corner.dot(dir);
        distance = Math.max(
          distance,
          depth + Math.abs(corner.dot(right)) / tanH,
          depth + Math.abs(corner.dot(up)) / tanV,
        );
      }
    }
  }

  if (template.trueScale > 0) distance = trueScaleDistance(distance, tanH);

  camera.position.copy(target).addScaledVector(dir, distance);
  camera.near = Math.max(distance / 100, 0.01);
  camera.far = distance * 4;
  camera.lookAt(target);
  camera.clearViewOffset();
  camera.updateProjectionMatrix();
  centreHorizontally(THREE, camera, box);
}

/**
 * The camera Kunos shot its own previews with, from the official SDK:
 * `sdk/dev/ksPreviewBuilder/dev/showroom_start.ini`, section `[PREVIEW_MODE]`
 * (`CUSTOM_CAMERA_POSITION`, `LOOK_AT`) and `CAMERA_FOV`. Same frame of
 * reference as our models — the converter keeps the KN5 axes — so the numbers
 * go in as they are; mirrored, they matched the game's images half as well.
 *
 * In our terms: azimuth 317.4°, elevation 7.2°, 5.76 m from a point 0.7 m up
 * and 0.5 m ahead of the car's **own origin**, not the middle of its body.
 * That is what makes the angle differ from car to car in the game's images:
 * a short car whose origin sits near its tail (an Abarth) shows its front, a
 * long one its flank, and an orbit around each car's centre cannot do that.
 * Measured against the game's previews of sixteen stock cars, silhouette
 * overlap: 61 % with this camera, 46 % with the best orbit, 32 % with Content
 * Manager's own preview camera (`DarkPreviewsOptions`).
 */
const GAME_CAMERA_POSITION = [-3.867643, 1.42359, 4.70381] as const;
const GAME_CAMERA_TARGET = [0, 0.7, 0.5] as const;
const GAME_CAMERA_FOV = 30;

/** How far a car may stray from a real car's size, either way, before the game
 * camera stops being trusted for it. The camera does not adapt to the car, so
 * a mod built in the wrong unit would come out as a speck or as a close-up of
 * a wheel; it gets the orbit instead. Stock cars run from 0.6 (Abarth 595) to
 * 0.9 (Formula 1) of the reference radius. */
const GAME_FRAMING_RANGE = 3;

export function gameFramingFits(radius: number): boolean {
  const ratio = radius / TRUE_SCALE_RADIUS;
  return ratio > 1 / GAME_FRAMING_RANGE && ratio < GAME_FRAMING_RANGE;
}

/**
 * Places the game's preview camera (`gameFraming`): the same for every car,
 * nothing fitted, nothing recentred — where the car lands in the frame is the
 * point. The template's own framing values do not apply.
 */
function placeGameCamera(camera: ThreeModule.PerspectiveCamera): void {
  camera.fov = GAME_CAMERA_FOV;
  camera.clearViewOffset();
  camera.position.set(...GAME_CAMERA_POSITION);
  camera.near = 0.05;
  camera.far = 100;
  camera.lookAt(...GAME_CAMERA_TARGET);
  camera.updateProjectionMatrix();
}

/**
 * Radius, in metres, of the car that the true-scale framing fits exactly.
 *
 * Measured, not chosen: on the game's own previews, the share of the frame a
 * car takes is proportional to its size — 25 to 28 % of the width per metre of
 * bounding-box radius, from an Abarth 595 (1.8 m, 45 %) to a Formula 1 (2.9 m,
 * 77 %). The game shoots every car from the same distance. This is the radius
 * that, at the Officiel framing, reproduces that ratio.
 */
export const TRUE_SCALE_RADIUS = 3.1;

/** The furthest true scale may push a car back, as a multiple of its fitted
 * distance. Real cars stay well inside it (an Abarth needs 1.5); a mod built in
 * the wrong unit would otherwise end up a speck in the middle of the frame. */
const TRUE_SCALE_MAX_SHRINK = 2.5;

/**
 * Camera distance for the true-scale framing: the same for every car, as in
 * the game, so a small car looks small next to a large one.
 *
 * Bounded on both sides by the fitted distance: never closer than it, so a car
 * larger than the reference is fitted rather than cut; never more than
 * `TRUE_SCALE_MAX_SHRINK` times further.
 */
function trueScaleDistance(fitted: number, tanH: number): number {
  // `tanH` already carries the margin, so the margin keeps scaling the whole
  // set. The reference is a radius, not a box: the distance must not depend on
  // the shape of the car being drawn, or it would not be the same for all.
  const fixed = TRUE_SCALE_RADIUS / tanH;
  return Math.min(Math.max(fixed, fitted), fitted * TRUE_SCALE_MAX_SHRINK);
}

/**
 * Shifts the lens so that the box, **as projected**, sits in the middle of the
 * frame.
 *
 * Aiming at the centre of the 3D box does not centre the car in the image:
 * seen three-quarters on, its near end is enlarged by perspective and pulls the
 * silhouette towards it. Measured against the game's own previews, ours sat
 * ~6 % right of theirs on every car, a gap no preset value could close. A view
 * offset is a lens shift, not a camera move: perspective and the angle the car
 * is seen from stay exactly what the template says.
 *
 * Horizontal only: the vertical placement is a template value (`height`), and
 * recentring it here would silently override it.
 */
function centreHorizontally(
  THREE: typeof ThreeModule,
  camera: ThreeModule.PerspectiveCamera,
  box: ThreeModule.Box3,
): void {
  camera.updateMatrixWorld();
  let min = Infinity;
  let max = -Infinity;
  const corner = new THREE.Vector3();
  for (const x of [box.min.x, box.max.x]) {
    for (const y of [box.min.y, box.max.y]) {
      for (const z of [box.min.z, box.max.z]) {
        const ndc = corner.set(x, y, z).project(camera).x;
        min = Math.min(min, ndc);
        max = Math.max(max, ndc);
      }
    }
  }
  const centre = (min + max) / 2;
  if (!Number.isFinite(centre) || Math.abs(centre) < 1e-4) return;
  // Only the ratio of the two sizes matters to `setViewOffset`: a 1000-unit
  // wide virtual frame keeps the shift readable in "thousandths of a frame".
  const width = 1000 * camera.aspect;
  camera.setViewOffset(width, 1000, (centre * width) / 2, 0, width, 1000);
}
