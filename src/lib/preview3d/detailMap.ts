// The repeated detail map of a material — the weave of a carbon body, the
// grain of a leather — laid back over its colour (docs/kn5-format.md, écart
// n°31).
//
// glTF has a single colour slot, and a map repeated 2000 times over the body
// cannot be baked into it. The converter therefore paints the diffuse with the
// map's **average** (`kn5-gltf`, `paint.rs`) and carries the map itself in the
// material's `extras` (`pitboxDetail`, see `DetailLayer`). Here the shader
// swaps that average for the texel under the pixel, with the same
// `MODULATE2X` the game uses:
//
//     colour *= lerp(1, detail * 2, useDetail)
//
// Seen from afar, the mipmaps bring the map back to its average and the
// surface to what the diffuse already says; close up, the weave comes out.
// Without it the carbon of `ks_lamborghini_sesto_elemento` was a flat grey
// (reported on screen, the game showing the weave).
import type * as ThreeModule from "three";
import type { GLTF } from "three/examples/jsm/loaders/GLTFLoader.js";

/** What the converter writes in a material's `extras`. */
interface DetailExtras {
  /** Index of the glTF texture. */
  texture: number;
  /** How many times it repeats over the colour map's UVs. */
  scale: number;
  useDetail: number;
  /** The average the diffuse was painted with, in [0,1] as stored. */
  average: [number, number, number];
}

/** A loaded detail map, kept on the material's `userData`. */
export interface DetailMap {
  map: ThreeModule.Texture;
  scale: number;
  useDetail: number;
  average: [number, number, number];
}

const KEY = "pitboxDetailMap";

function extrasOf(material: ThreeModule.Material): DetailExtras | null {
  const raw = (material.userData as { pitboxDetail?: Partial<DetailExtras> }).pitboxDetail;
  if (!raw || typeof raw.texture !== "number" || typeof raw.scale !== "number") return null;
  if (!Array.isArray(raw.average) || raw.average.length !== 3) return null;
  return {
    texture: raw.texture,
    scale: raw.scale,
    useDetail: typeof raw.useDetail === "number" ? raw.useDetail : 1,
    average: raw.average as [number, number, number],
  };
}

/** The detail map attached to a material, if any. */
export function detailMapOf(material: ThreeModule.Material): DetailMap | null {
  return (material.userData[KEY] as DetailMap | undefined) ?? null;
}

/**
 * Loads the detail maps a converted model announces, and attaches each to its
 * material. Before `prepareMaterials`, which builds the shader from them.
 *
 * A map that does not load leaves its material as it was — the average the
 * diffuse carries — and says so in the console: a missing weave is no reason
 * to lose the preview.
 */
export async function attachDetailMaps(gltf: GLTF, THREE: typeof ThreeModule): Promise<void> {
  const jobs: Promise<void>[] = [];
  gltf.scene.traverse((object) => {
    const mesh = object as ThreeModule.Mesh;
    if (!mesh.isMesh) return;
    for (const material of Array.isArray(mesh.material) ? mesh.material : [mesh.material]) {
      const extras = extrasOf(material);
      // Shared materials are met once per mesh: load once.
      if (!extras || material.userData[KEY] !== undefined) continue;
      material.userData[KEY] = null;
      jobs.push(
        gltf.parser
          .getDependency("texture", extras.texture)
          .then((loaded: ThreeModule.Texture) => {
            // A copy: the loader caches the texture by index, and its colour
            // space must stay what the file says — raw bytes, the space the
            // average was taken in.
            const map = loaded.clone();
            map.colorSpace = THREE.NoColorSpace;
            map.wrapS = THREE.RepeatWrapping;
            map.wrapT = THREE.RepeatWrapping;
            map.needsUpdate = true;
            const detail: DetailMap = { map, scale: extras.scale, useDetail: extras.useDetail, average: extras.average };
            material.userData[KEY] = detail;
          })
          .catch((e: unknown) => console.warn("detail map not loaded", material.name, e)),
      );
    }
  });
  await Promise.all(jobs);
}

/**
 * Lays the detail map over the colour, right after `<map_fragment>` has read
 * the diffuse.
 *
 * The factor is computed **in the space of the file** — the space the
 * converter painted the diffuse in — then raised to 2.2 to apply to the linear
 * colour the shader works with: the baked average is divided back out, the
 * texel put in its place.
 *
 * `vMapUv` carries the colour map's own repetition (`KHR_texture_transform`);
 * a material that repeats both its diffuse and its detail would repeat the
 * second by the product — met on no material so far.
 */
export function patchDetailShader(shader: ThreeModule.WebGLProgramParametersWithUniforms, detail: DetailMap): void {
  shader.uniforms.pbDetailMap = { value: detail.map };
  shader.uniforms.pbDetailScale = { value: detail.scale };
  shader.uniforms.pbUseDetail = { value: detail.useDetail };
  shader.uniforms.pbDetailAverage = { value: detail.average };
  shader.fragmentShader = shader.fragmentShader
    .replace(
      "#include <common>",
      `#include <common>
      uniform sampler2D pbDetailMap;
      uniform float pbDetailScale;
      uniform float pbUseDetail;
      uniform vec3 pbDetailAverage;`,
    )
    .replace(
      "#include <map_fragment>",
      `#include <map_fragment>
      #ifdef USE_MAP
      {
        vec3 pbTexel = texture2D( pbDetailMap, vMapUv * pbDetailScale ).rgb;
        vec3 pbFull = 1.0 + ( pbTexel * 2.0 - 1.0 ) * pbUseDetail;
        vec3 pbBaked = 1.0 + ( pbDetailAverage * 2.0 - 1.0 ) * pbUseDetail;
        diffuseColor.rgb *= pow( max( pbFull, vec3( 0.0 ) ) / max( pbBaked, vec3( 0.02 ) ), vec3( 2.2 ) );
      }
      #endif`,
    );
}
