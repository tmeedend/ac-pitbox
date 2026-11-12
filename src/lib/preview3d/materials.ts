// What the car preview changes on the materials of a converted model, once,
// as it arrives (docs/SPEC-preview-3d-kn5.md PREVIEW§8.2, PREVIEW§15):
// anisotropic filtering, alpha cut-outs on sample coverage, the roughness
// floor against shimmer, the repeated detail maps, glass drawn after the opaque
// body, and who casts the shadow. Out of `CarPreview3D.svelte` with the rest of
// the scene.
import type * as ThreeModule from "three";
import { TUNING } from "./tuning";
import { detailMapOf, patchDetailShader } from "./detailMap";
import { patchGradeShader } from "./grade";

/**
 * Is this material physical glass, i.e. does it carry
 * `KHR_materials_transmission`?
 *
 * Glass declared by a mod's `ext_config.ini` is converted as transmissive
 * rather than blended (`kn5-gltf`, SPEC PREVIEW§4.5ter): blending attenuates the
 * whole surface response, specular reflection included, and it is that
 * reflection that makes a pane read as glass. The consequence here is that
 * such a material is **not** `transparent`, so every rule keyed on that flag
 * misses it.
 */
function isTransmissive(material: ThreeModule.Material): boolean {
  return ((material as ThreeModule.MeshPhysicalMaterial).transmission ?? 0) > 0;
}

/**
 * Applies `TUNING.roughnessFloor`, per pixel, inside the shader.
 *
 * Injected rather than configured, because three has no such setting — and a
 * plain `material.roughness` would not do: three *multiplies* it by the green
 * channel of the roughness map (`roughnessmap_fragment`), so it scales the
 * map instead of lifting its floor, and this project reads its roughness from
 * `txMaps` on most materials.
 *
 * The insertion point is right after `<roughnessmap_fragment>`, which is where
 * `roughnessFactor` is declared; `<lights_physical_fragment>` consumes it much
 * further down. Verified against three r185.
 */
function applyRoughnessFloor(shader: ThreeModule.WebGLProgramParametersWithUniforms, floor: number): void {
  shader.fragmentShader = shader.fragmentShader.replace(
    "#include <roughnessmap_fragment>",
    `#include <roughnessmap_fragment>
      roughnessFactor = max( roughnessFactor, ${floor.toFixed(4)} );`,
  );
}

/**
 * The preview's changes to a material's shader: the roughness floor (not on
 * physical glass), the repeated detail map (`detailMap.ts`) and the colour
 * grade (`grade.ts`). One `onBeforeCompile` for all — a material has only one.
 */
function patchShader(material: ThreeModule.MeshStandardMaterial): void {
  const floor = isTransmissive(material) ? 0 : TUNING.roughnessFloor;
  const detail = detailMapOf(material);
  material.onBeforeCompile = (shader) => {
    if (floor > 0) applyRoughnessFloor(shader, floor);
    if (detail) patchDetailShader(shader, detail);
    patchGradeShader(shader);
  };
  // Sans cette clé, three met en commun le programme compilé de deux
  // matériaux qu'il croit identiques : il ne regarde pas ce qu'`onBeforeCompile`
  // a changé.
  material.customProgramCacheKey = () => `pitbox-graded-roughfloor-${floor}${detail ? "-detail" : ""}`;
}

/** Prepares every mesh of a freshly loaded model for the preview's renderer. */
export function prepareMaterials(model: ThreeModule.Object3D, renderer: ThreeModule.WebGLRenderer): void {
  // Filtrage anisotrope, au maximum de ce que la carte accepte (16 en
  // pratique). Le MSAA du contexte ne lisse que les **bords de géométrie** ;
  // le fourmillement d'une texture vue en biais — les décalcomanies d'une
  // portière, les rainures d'un pneu, le sol — vient du filtrage, et c'est
  // l'anisotropie qui le règle. Une ligne pour le gain le plus visible.
  const maxAnisotropy = renderer.capabilities.getMaxAnisotropy();

  model.traverse((object) => {
    const mesh = object as ThreeModule.Mesh;
    if (!mesh.isMesh) return;
    const materials = Array.isArray(mesh.material) ? mesh.material : [mesh.material];
    for (const material of materials) {
      for (const value of Object.values(material)) {
        if (value && typeof value === "object" && "isTexture" in value) {
          const texture = value as ThreeModule.Texture;
          texture.anisotropy = maxAnisotropy;
          texture.needsUpdate = true;
        }
      }
      // The detail map lives on `userData`, out of the loop above's sight —
      // and a weave seen at a grazing angle is exactly what anisotropy is for.
      const detail = detailMapOf(material);
      if (detail) detail.map.anisotropy = maxAnisotropy;
      // Découpes en alpha (calandres, jantes ajourées, grillages) : leur bord
      // est décidé par un seuil, donc le MSAA ne le voit pas — il ne lisse
      // que la silhouette du triangle, pas le trou qu'on y perce. Reporté sur
      // la couverture des échantillons, ce bord retrouve le même adoucissement
      // que le reste.
      const standard = material as ThreeModule.MeshStandardMaterial;
      if (standard.alphaTest > 0) {
        standard.alphaToCoverage = true;
      }
      // L'un des deux leviers contre le scintillement en mouvement — voir le
      // tableau de mesures devant `TUNING.roughnessFloor`.
      // Physical glass is exempt: three blurs the transmitted image by the
      // same roughness, so a 0.15 floor would frost every windowpane.
      if (standard.isMeshStandardMaterial) patchShader(standard);
    }
  });
  applyGlassRules(model);
}

/**
 * Where glass goes in the drawing order, and who casts the shadow.
 *
 * Les vitres passent après l'opaque et n'écrivent pas dans le tampon de
 * profondeur, sinon l'intérieur disparaît derrière le pare-brise (PREVIEW§8.2).
 */
export function applyGlassRules(model: ThreeModule.Object3D): void {
  model.traverse((object) => {
    const mesh = object as ThreeModule.Mesh;
    if (!mesh.isMesh) return;
    const materials = Array.isArray(mesh.material) ? mesh.material : [mesh.material];
    if (materials.some((m) => (m as ThreeModule.Material).transparent)) {
      mesh.renderOrder = 1;
      for (const m of materials) (m as ThreeModule.Material).depthWrite = false;
    } else if (!materials.some(isTransmissive)) {
      // Only the opaque body casts: a windscreen that casts a shadow map
      // casts it solid black, and the car ends up sitting on a dark blob.
      //
      // Transmissive glass is opaque as far as the sorting goes — its
      // transparency lives in `KHR_materials_transmission`, not in an alpha —
      // so it lands in this branch and has to be excluded by hand, or the
      // dark blob comes straight back.
      mesh.castShadow = true;
    }
  });
}
