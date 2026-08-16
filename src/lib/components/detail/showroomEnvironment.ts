// Dark showroom used as the preview's image-based lighting
// (docs/SPEC-preview-3d-kn5.md §8.1, §15 point 7).
//
// three.js ships `RoomEnvironment`, a bright **white** room. On car paint —
// which the surface maps now report as nearly smooth — the whole body ends up
// mirroring white walls, and the colour washes out. Measured against Kunos'
// `preview.jpg` on the green Supra: the bodywork came out at (113, 205, 112)
// where the game renders (14, 81, 36).
//
// A showroom is the opposite: a dark box with lamps overhead. The paint then
// reflects black except where it catches a strip, which is what keeps a colour
// deep and draws the long highlight along a bonnet.
//
// Procedural, like `RoomEnvironment` itself: §8.1 rules out shipping any asset
// for the preview, and an HDRI would be one.
import type * as ThreeModule from "three";

/** Emissive strength of the two ceiling strips — the key light. */
const CEILING = 6;
/** Side panels. Low on purpose: they lift the flanks off the background
 * without printing a second reflection on them. */
const FILL = 0.05;
/** The enclosure itself. Not black: at zero, everything that faces no lamp
 * goes perfectly flat and the car loses its edges. */
const WALLS = 0.01;

/**
 * Builds the scene `PMREMGenerator` turns into an environment map.
 *
 * Takes the `THREE` module rather than importing it: three.js is loaded
 * dynamically (it is the heaviest dependency of the front end and must not
 * weigh on screens that show no preview), so there is only ever one instance
 * and it is the caller's.
 */
export function showroomEnvironment(THREE: typeof ThreeModule): ThreeModule.Scene {
  const scene = new THREE.Scene();
  const geometry = new THREE.BoxGeometry();
  geometry.deleteAttribute("uv");

  const areaLight = (intensity: number) =>
    new THREE.MeshLambertMaterial({ color: 0x000000, emissive: 0xffffff, emissiveIntensity: intensity });

  const panel = (intensity: number, position: [number, number, number], scale: [number, number, number]) => {
    const mesh = new THREE.Mesh(geometry, areaLight(intensity));
    mesh.position.set(...position);
    mesh.scale.set(...scale);
    scene.add(mesh);
  };

  const room = new THREE.Mesh(geometry, areaLight(WALLS));
  room.material.side = THREE.BackSide;
  room.position.set(0, 4, 0);
  room.scale.set(20, 12, 20);
  scene.add(room);

  // Two strips along the car's axis: that is what draws the stretched
  // reflection on the bonnet and roof of the Kunos photos.
  panel(CEILING, [0, 8.5, -2.5], [10, 0.4, 5]);
  panel(CEILING, [0, 8.5, 2.5], [10, 0.4, 5]);
  panel(FILL, [-7, 3, 0], [0.4, 5, 12]);
  panel(FILL, [7, 3, 0], [0.4, 5, 12]);

  return scene;
}
