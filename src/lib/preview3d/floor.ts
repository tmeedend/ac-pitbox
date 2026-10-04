// The studio floor of the car preview (docs/SPEC-preview-3d-kn5.md PREVIEW§8.1,
// PREVIEW§15): the pool of light, the projected shadow, and the mirror that
// lacquers it. Out of `CarPreview3D.svelte` with the rest of the scene; the
// pool and the mirror shader themselves are `studioFloor.ts` and
// `floorMirror.ts`, shared with the grid thumbnails.
import type * as ThreeModule from "three";
import { applyFloorMirror, floorMirrorShader } from "./floorMirror";
import { preview3dPrefs } from "./preview3dPrefs.svelte";
import { poolTexture } from "./studioFloor";
import type { ThreeScene } from "./scene";
import { TUNING } from "./tuning";

/**
 * Aligns the reflection target on the drawing buffer, and reports the
 * settings that depend on its size.
 *
 * **This is the whole point of the fix**, so it is worth stating why the
 * fixed 512×512 that stood here was wrong. The target is a *screen-projected*
 * texture: it covers the panel, one texel for one pixel when the two match.
 * At 512² on a 1268-pixel panel supersampled 2,5×, one texel covered about
 * six pixels horizontally and four vertically — the square target on a
 * rectangular panel adding an anisotropy on top of the plain lack of
 * resolution. A rasterised edge therefore did not slide across the
 * reflection, it *jumped* from texel to texel, five screen pixels at a time.
 * A still frame hides that behind bilinear magnification, which is exactly
 * why the defect only ever showed in motion (retour utilisateur : « ça choque
 * beaucoup moins quand je fais une capture, quand ça bouge ça scintille »).
 *
 * Every level of the quality setting missed the mirror for the same reason:
 * the supersampling factor lands on the drawing buffer, never on a target
 * whose size is a literal.
 */
export function sizeMirror(current: ThreeScene): void {
  if (!current.mirror) return;
  const buffer = current.renderer.getDrawingBufferSize(new current.THREE.Vector2());
  let width = Math.max(Math.round(buffer.x), 1);
  let height = Math.max(Math.round(buffer.y), 1);
  const area = width * height;
  if (area > TUNING.mirrorPixels) {
    // Le budget porte sur la surface, la forme reste celle du panneau : c'est
    // la cible carrée qui étirait le reflet.
    const factor = Math.sqrt(TUNING.mirrorPixels / area);
    width = Math.max(Math.round(width * factor), 1);
    height = Math.max(Math.round(height * factor), 1);
  }
  const target = current.mirror.getRenderTarget();
  if (target.width !== width || target.height !== height) target.setSize(width, height);
  const material = current.mirror.material as ThreeModule.ShaderMaterial;
  applyFloorMirror(material.uniforms, preview3dPrefs(), width);
}

/**
 * Pose le miroir du sol, ou ne fait rien si le reflet est réglé à 0 %.
 *
 * Séparée de `build` parce qu'elle sert deux fois : à la construction, et
 * quand l'utilisateur remonte le reflet depuis 0 — auquel cas il faut
 * l'ajouter à chaud plutôt que de recharger la fiche, un curseur n'ayant pas
 * à faire clignoter l'aperçu.
 *
 * À 0 % le miroir n'existe pas, plutôt que d'exister à l'opacité zéro :
 * c'est un **second rendu de la scène**, le seul poste de ce panneau qui
 * coûte vraiment.
 */
export async function attachMirror(current: ThreeScene): Promise<void> {
  if (current.mirror || preview3dPrefs().reflection <= 0) return;
  const { Reflector } = await import("three/addons/objects/Reflector.js");
  if (current.disposed) return;
  const THREE = current.THREE;
  const size = current.radius * 5;
  const mirror = new Reflector(new THREE.PlaneGeometry(size, size), {
    // Taille provisoire : `sizeMirror` l'aligne sur le tampon de rendu juste
    // en dessous, puis à chaque redimensionnement et à chaque changement de
    // qualité. Aucune valeur écrite ici n'est un réglage.
    textureWidth: 512,
    textureHeight: 512,
    color: 0xffffff,
    shader: floorMirrorShader,
    // No MSAA on the reflection pass, and the memory it would have taken goes
    // into resolution instead. Same argument as PREVIEW§15: MSAA samples triangle
    // *coverage* but still shades once per texel, so it can do nothing about
    // a sub-pixel specular highlight — while the target now follows a drawing
    // buffer already supersampled 1,5× to 4×, which is precisely what does
    // raise the shading rate. Four samples would cost four times the memory
    // for geometry edges alone.
    multisample: TUNING.mirrorSamples,
  });
  // Mipmaps on the target: the blur reads the level `applyFloorMirror` picks,
  // so that its 25 taps stay edge to edge whatever the resolution. three
  // regenerates them on its own every time it unbinds the target — they only
  // have to be asked for.
  const reflection = mirror.getRenderTarget().texture;
  reflection.minFilter = THREE.LinearMipmapLinearFilter;
  reflection.generateMipmaps = true;
  mirror.rotation.x = -Math.PI / 2;
  mirror.position.set(current.center.x, current.floorY + current.radius / 500, current.center.z);
  // `Reflector` hérite le type de matériau de `Mesh`, donc un `Material` tout
  // court : c'est bien un `ShaderMaterial`, construit à partir du shader
  // qu'on lui passe.
  const material = mirror.material as ThreeModule.ShaderMaterial;
  material.transparent = true;
  material.depthWrite = false;
  // Sous la flaque (-2) et sous l'ombre (-1) : le reflet est le sol, tout le
  // reste se pose dessus.
  mirror.renderOrder = -3;

  // Le reflet ne doit montrer **que** la voiture : sans ça, la flaque et
  // l'ombre se retrouvent dans leur propre reflet et le sol se dédouble.
  //
  // ⚠️ **Refreshed on every frame**, where it used to be one frame in two.
  // The saving was real and the reasoning behind it ("two tenths of a degree
  // per frame, invisible on a blurred surface") was about the reflection's
  // *position* — but the price was paid on its *cadence*. Skipping a frame
  // does not make the reflection lag by a tenth of a degree, it makes it
  // advance in double steps at 30 Hz under a car that turns at 60 Hz. That
  // judder is invisible on a still frame and reads as shimmer in motion,
  // which is exactly what the user reported.
  const reflect = mirror.onBeforeRender;
  mirror.onBeforeRender = function (...args: Parameters<typeof reflect>) {
    current.ground.visible = false;
    current.shadowCatcher.visible = false;
    reflect.apply(this, args);
    current.ground.visible = true;
    current.shadowCatcher.visible = true;
  };

  current.scene.add(mirror);
  current.mirror = mirror;
  sizeMirror(current);
}

/** Lays the floor under a model: the pool of light, and the plane that
 * catches the car's projected shadow, with the light that projects it. */
export function addFloor(
  THREE: typeof ThreeModule,
  scene: ThreeModule.Scene,
  box: ThreeModule.Box3,
  center: ThreeModule.Vector3,
  radius: number,
): { ground: ThreeModule.Mesh; shadowCatcher: ThreeModule.Mesh } {
  // The ground: a pool of light with the contact shadow in its middle, drawn
  // as one gradient rather than a shadow map (PREVIEW§8.1). Two things at once,
  // because they are two halves of the same thing — the car sits on a lit
  // floor and blocks part of that light.
  const ground = new THREE.Mesh(
    new THREE.PlaneGeometry(radius * 5, radius * 5),
    new THREE.MeshBasicMaterial({
      map: poolTexture(THREE),
      transparent: true,
      depthWrite: false,
      // Le dégradé est déjà la valeur voulue à l'écran : le faire passer par
      // le tone mapping l'assombrirait d'un tiers.
      toneMapped: false,
    }),
  );
  ground.rotation.x = -Math.PI / 2;
  ground.position.set(center.x, box.min.y + radius / 400, center.z);
  ground.renderOrder = -2;
  scene.add(ground);

  // The car's own shadow, projected on that floor.
  //
  // The light is at **intensity zero**: it lights nothing, and everything the
  // car receives still comes from the environment map, which is what was
  // calibrated against the Kunos photos. It exists only so three.js has a
  // direction to project from — `ShadowMaterial` reads the shadow mask, not
  // the light's contribution, so the two concerns stay apart.
  const sun = new THREE.DirectionalLight(0xffffff, 0);
  sun.castShadow = true;
  // Above and slightly to the front-left, the direction the ceiling strips
  // of the showroom come from: the shadow falls almost straight under the
  // car, as in the photos, with just enough offset to be read as a shadow.
  sun.position.set(center.x - radius * 0.35, center.y + radius * 4, center.z + radius * 0.6);
  sun.target.position.copy(center);
  scene.add(sun.target);
  scene.add(sun);
  const shadowCamera = sun.shadow.camera;
  shadowCamera.left = -radius;
  shadowCamera.right = radius;
  shadowCamera.top = radius;
  shadowCamera.bottom = -radius;
  shadowCamera.near = radius * 0.5;
  shadowCamera.far = radius * 8;
  shadowCamera.updateProjectionMatrix();
  // La douceur se règle par la **résolution**, et c'est contre-intuitif :
  // `PCFSoftShadowMap` a un noyau de filtrage fixe, exprimé en texels, donc
  // moins de texels = un flou plus large. `shadow.radius` n'y fait rien
  // (three.js le documente), et VSM, qui l'écouterait, a été essayé puis
  // écarté : il zébrait le sol de barres grises (retour utilisateur).
  // 512 sur une rampe de plafond large donne le bord mou d'une ombre de
  // studio ; monter cette valeur la redurcit.
  sun.shadow.mapSize.set(TUNING.shadowMapSize, TUNING.shadowMapSize);
  // Sans ce biais, la carte d'ombre s'auto-ombre en fines rayures sur les
  // surfaces presque parallèles à la lumière (le capot, le toit).
  sun.shadow.bias = TUNING.shadowBias;

  const shadowCatcher = new THREE.Mesh(
    new THREE.PlaneGeometry(radius * 5, radius * 5),
    new THREE.ShadowMaterial({ opacity: 0.5 }),
  );
  shadowCatcher.receiveShadow = true;
  shadowCatcher.rotation.x = -Math.PI / 2;
  shadowCatcher.position.set(center.x, box.min.y + radius / 300, center.z);
  shadowCatcher.renderOrder = -1;
  scene.add(shadowCatcher);
  return { ground, shadowCatcher };
}
