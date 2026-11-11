// How much of the studio a family of materials reflects, apart from the rest of
// the car (docs/SPEC-preview-3d-kn5.md PREVIEW§8.1, GRILLE§5.6).
//
// The studio is dark on purpose — that is what keeps a paint deep — and a
// mirror only shows what is around it: windows read as dark holes, chrome trims
// and headlight reflectors as black. Raising the whole environment would wash
// the paint out; giving those materials the scene's environment as their own
// map, at a higher level, raises it on them alone. It is the one way three lets
// a material weigh its reflections apart: `material.envMapIntensity` does
// nothing while the environment comes from the scene (PREVIEW§15).
import type * as ThreeModule from "three";

type Standard = ThreeModule.MeshPhysicalMaterial;

function reflectApart(
  model: ThreeModule.Object3D,
  scene: ThreeModule.Scene,
  select: (material: Standard) => boolean,
  percent: number,
): void {
  model.traverse((object) => {
    const mesh = object as ThreeModule.Mesh;
    if (!mesh.isMesh) return;
    for (const raw of Array.isArray(mesh.material) ? mesh.material : [mesh.material]) {
      const material = raw as Standard;
      if (!material.isMeshStandardMaterial || !select(material)) continue;
      // At 100 nothing is touched, so a template from before the setting
      // renders as it did.
      const own = percent !== 100;
      if ((material.envMap !== null) !== own) material.needsUpdate = true;
      material.envMap = own ? scene.environment : null;
      material.envMapIntensity = own ? (scene.environmentIntensity * percent) / 100 : 1;
    }
  });
}

/** Glass: blended, or physical (`KHR_materials_transmission`). */
function isGlass(material: Standard): boolean {
  return material.transparent || (material.transmission ?? 0) > 0;
}

/** Chrome and bare metal: what the converter makes metallic (`fresnelC`,
 * kn5-format écart n°10) — headlight reflectors, trims, polished rims. */
function isChrome(material: Standard): boolean {
  return material.metalness >= 0.5 && !isGlass(material);
}

/** Window reflections, in percent of what the paint reflects. */
export function reflectGlass(model: ThreeModule.Object3D, scene: ThreeModule.Scene, percent: number): void {
  reflectApart(model, scene, isGlass, percent);
}

/** Chrome reflections, in percent of what the paint reflects. */
export function reflectChrome(model: ThreeModule.Object3D, scene: ThreeModule.Scene, percent: number): void {
  reflectApart(model, scene, isChrome, percent);
}
