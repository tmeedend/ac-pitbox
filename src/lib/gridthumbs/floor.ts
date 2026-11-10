// The floor of a grid thumbnail (docs/SPEC-grille.md GRILLE§5.6): the wide glow
// of the detail page preview, or the game's disc of light on a darker floor.
// The mirror is the other half of the floor, and stays with the rig.
import type * as ThreeModule from "three";
import type { GridTemplate } from "./gridThumbs";
import { TRUE_SCALE_RADIUS, gameFramingFits } from "./framing";

/** The part of the rig the floor needs — the rig extends it. */
export interface StudioFloor {
  THREE: typeof ThreeModule;
  /** La flaque peinte, montée une fois : sa texture est un canevas de 512 px
   * qu'on ne repeint pas à chaque image. Invisible quand le preset n'a pas de
   * sol. */
  pool: ThreeModule.Mesh<ThreeModule.PlaneGeometry, ThreeModule.MeshBasicMaterial>;
  /** The pool's two looks: the wide glow of the detail page preview, and the
   * disc of light (`poolSize`), drawn once per edge softness and kept. */
  glow: ThreeModule.Texture;
  discs: Map<number, ThreeModule.Texture>;
  /** The studio floor around the disc, darker than the wall behind it — see
   * `placePool`. Only shown with a disc and a baked backdrop. */
  ground: ThreeModule.Mesh<ThreeModule.PlaneGeometry, ThreeModule.MeshBasicMaterial>;
}

/**
 * Lays the floor pool: the wide glow, or a **disc of light** (`poolSize`).
 *
 * The game's floor is a disc. With the contrast pushed up, every one of its
 * previews shows the same lit ellipse around the car, same size and same
 * place whatever the car — the studio's turntable — with an edge you can
 * point at. Our glow fades over six car radii instead, and seen from the
 * game's low camera that squashes into a blurred band across the frame, the
 * "horizon" the user noticed. The glow stays the default: it is the floor of
 * the detail page preview, and Showcase is built on it.
 *
 * Under the game's camera the disc is centred on the car's own origin and
 * sized from the reference radius, so it is the same for every car, as in the
 * game; otherwise it follows the car, sized from its radius.
 */
export function placePool(
  rig: StudioFloor,
  center: ThreeModule.Vector3,
  radius: number,
  floorY: number,
  span: number,
  template: GridTemplate,
): void {
  let map = rig.glow;
  if (template.poolSize > 0) {
    const studio = template.gameFraming > 0 && gameFramingFits(radius);
    const size = (2 * (studio ? TRUE_SCALE_RADIUS : radius) * template.poolSize) / 100;
    rig.pool.position.set(studio ? 0 : center.x, floorY + radius / 400, studio ? 0 : center.z);
    rig.pool.scale.set(size, size, 1);
    map = discTexture(rig, template.poolEdge);
    // **The floor beyond the disc is darker than the wall**, and that is
    // what draws the disc: measured on the game's previews, the frame runs
    // 3–4 on the wall, 1–2 on the floor just past the disc, 11–14 on the
    // disc. With one backdrop for both, ours ran 4–5 straight down to the
    // disc, which melted into it — the blurred horizon. Only with a baked
    // backdrop: on a cut-out preset it would paint a floor where the card
    // should show through. And only with a floor: without the pool it is a
    // dark patch with nothing in it.
    rig.ground.visible = template.background > 0 && template.floor > 0;
    rig.ground.material.color.set(template.matLo).multiplyScalar(GROUND_SHADE);
    rig.ground.position.set(rig.pool.position.x, floorY, rig.pool.position.z);
    rig.ground.scale.set(size * GROUND_EXTENT, size * GROUND_EXTENT, 1);
  } else {
    rig.ground.visible = false;
    rig.pool.position.set(center.x, floorY + radius / 400, center.z);
    rig.pool.scale.set(span, span, 1);
  }
  if (rig.pool.material.map !== map) {
    rig.pool.material.map = map;
    rig.pool.material.needsUpdate = true;
  }
}

/**
 * The studio floor's shape: solid in the middle, fading out towards its rim.
 * A plain square drew its far edge as a straight line across the frame, the
 * very horizon the disc was there to remove; the game's dark floor fades
 * into the wall instead.
 */
export function groundTexture(THREE: typeof ThreeModule): ThreeModule.Texture {
  const size = 256;
  const canvas = document.createElement("canvas");
  canvas.width = size;
  canvas.height = size;
  const ctx = canvas.getContext("2d");
  if (ctx) {
    const middle = size / 2;
    const gradient = ctx.createRadialGradient(middle, middle, 0, middle, middle, middle);
    gradient.addColorStop(0, "rgba(255,255,255,1)");
    gradient.addColorStop(0.5, "rgba(255,255,255,1)");
    gradient.addColorStop(1, "rgba(255,255,255,0)");
    ctx.fillStyle = gradient;
    ctx.fillRect(0, 0, size, size);
  }
  return new THREE.CanvasTexture(canvas);
}

/** How dark the studio floor is, against the wall (the backdrop's darker
 * colour). Measured on the game's previews: 1–2 on the floor past the disc
 * for 3–4 on the wall; this lands at 1. */
const GROUND_SHADE = 0.4;

/** How far the studio floor reaches before the wall, in disc diameters. The
 * game's floor stops short: its dark band sits at 44–62 % of the frame
 * height, between wall and disc. Measured column profiles at both edges of
 * the frame line up with the game's at 3 (at 20 the floor ran to the
 * horizon and the band started at 31 %). */
const GROUND_EXTENT = 3;

/**
 * The disc of light: an almost even plateau, then a fade over `edge` percent
 * of its radius. Its brightness at the centre is the glow's, so `floor` means
 * the same thing for both. Drawn once per softness and kept: a slider dragged
 * across the range draws at most twenty of them.
 */
function discTexture(rig: StudioFloor, edge: number): ThreeModule.Texture {
  const cached = rig.discs.get(edge);
  if (cached) return cached;
  const size = 512;
  const canvas = document.createElement("canvas");
  canvas.width = size;
  canvas.height = size;
  const ctx = canvas.getContext("2d");
  if (ctx) {
    const middle = size / 2;
    // A sharp edge still gets a couple of percent, or the rim aliases.
    const fade = Math.max(0.02, edge / 100);
    const gradient = ctx.createRadialGradient(middle, middle, 0, middle, middle, middle);
    gradient.addColorStop(0, "rgba(255,255,255,0.11)");
    gradient.addColorStop(1 - fade, "rgba(255,255,255,0.09)");
    gradient.addColorStop(1, "rgba(255,255,255,0)");
    ctx.fillStyle = gradient;
    ctx.fillRect(0, 0, size, size);
  }
  const texture = new rig.THREE.CanvasTexture(canvas);
  texture.colorSpace = rig.THREE.SRGBColorSpace;
  rig.discs.set(edge, texture);
  return texture;
}
