// What moves on the car preview without a reconversion
// (docs/SPEC-preview-3d-kn5.md PREVIEW§15): the steered wheels and steering
// wheel, the driver's arms that hold it, and the driver's fade in and out.
// The conversion only *describes* these (`steer.rs`, `kn5-gltf/src/rig.rs`);
// everything here is a node rotation, a frame picked in a clip, or an
// opacity — free, instant, and invisible to the cache key. Out of
// `CarPreview3D.svelte` with the rest of the scene.
import type * as ThreeModule from "three";
import type { GLTF } from "three/examples/jsm/loaders/GLTFLoader.js";
import { DRIVER_MESH_PREFIX } from "./preview";
import { preview3dPrefs } from "./preview3dPrefs.svelte";
import { DRIVER_FADE_SECONDS } from "./tuning";

/** Un nœud braqué et ce que la conversion en dit (voir `steer.rs`). */
interface SteeredNode {
  node: ThreeModule.Object3D;
  axis: ThreeModule.Vector3;
  /** Facteur sur l'angle du volant : 1 pour le volant, 1/démultiplication
   * pour une roue. */
  gain: number;
  /** Butée, en degrés aux roues. `null` quand rien n'arrête ce nœud — ce qui
   * est le cas des roues elles-mêmes, voir `SteerNode::limit`. */
  limit: number | null;
}

/** De quoi poser les bras du pilote à un angle donné, **sans reconvertir**.
 *
 * La conversion exporte le mannequin en squelette vivant : ses os, sa peau et
 * l'animation de braquage de la voiture (`kn5-gltf/src/rig.rs`). Poser les
 * bras revient donc à choisir un instant dans ce clip. */
interface DriverPose {
  mixer: ThreeModule.AnimationMixer;
  action: ThreeModule.AnimationAction;
  frames: number;
  /** Les trois images qui comptent, **mesurées** sur l'animation : à fond
   * d'un côté, volant droit, à fond de l'autre. Ses bouts ne sont pas des
   * butées — voir `Rig::extremes`. */
  low: number;
  centre: number;
  high: number;
  /** Angle de roue au-delà duquel le volant est en butée : les bras le
   * tiennent, donc ils s'arrêtent avec lui. */
  wheelLimit: number;
}

/** The moving parts of a scene, found once when its model is loaded. */
export interface Rig {
  /** L'animation de braquage du pilote, et de quoi y choisir une image.
   * `null` quand la conversion n'a pas exporté de squelette — mannequin sans
   * peau, sans animation, ou pas de pilote du tout. */
  driverPose: DriverPose | null;
  /** Les nœuds que le braquage fait tourner — roues avant et volant —,
   * repérés à l'`extras` que la conversion leur pose. Chacun porte son axe,
   * son pivot (déjà en translation du nœud) et le facteur à appliquer à
   * l'angle du volant. */
  steered: SteeredNode[];
  /** Les maillages du pilote greffé, repérés à leur préfixe de nom
   * (`DRIVER_MESH_PREFIX`), et les matériaux qu'ils portent. Vides quand la
   * conversion n'a pas greffé de mannequin. */
  driver: ThreeModule.Mesh[];
  driverMaterials: ThreeModule.Material[];
  /** Opacité actuelle du pilote, et celle vers laquelle il va. Le fondu est
   * mené par la boucle de rendu, comme la rotation du plateau : c'est le
   * seul endroit qui connaisse le temps écoulé. */
  driverOpacity: number;
  driverTarget: number;
}

/** Finds the moving parts of a freshly loaded model. The driver starts where
 * it must end (`driverShown`): a driver already wanted must not fade in on
 * every change of skin. */
export function findRig(THREE: typeof ThreeModule, gltf: GLTF, driverShown: boolean): Rig {
  // Les nœuds braqués, repérés à l'`extras` que la conversion leur pose. Le
  // pivot est déjà leur translation et leurs sommets lui sont relatifs, donc
  // il n'y a qu'une rotation à écrire — c'est ce qui sort l'angle de la clé
  // de cache (voir `steer.rs`).
  const steered: SteeredNode[] = [];
  gltf.scene.traverse((object) => {
    const described = (object.userData as { pitboxSteer?: { axis: number[]; gain: number; limit?: number } })
      .pitboxSteer;
    if (!described || described.axis.length !== 3) return;
    steered.push({
      node: object,
      axis: new THREE.Vector3(described.axis[0], described.axis[1], described.axis[2]).normalize(),
      gain: described.gain,
      limit: described.limit ?? null,
    });
  });

  // L'animation de braquage du mannequin, quand la conversion l'a exportée.
  // Elle n'est jamais jouée : on y **saute** à l'image que l'angle désigne.
  let driverPose: DriverPose | null = null;
  for (const clip of gltf.animations) {
    const described = (
      clip.userData as {
        pitboxDriver?: { frames: number; low: number; centre: number; high: number; wheelLimit: number };
      }
    ).pitboxDriver;
    if (!described) continue;
    const mixer = new THREE.AnimationMixer(gltf.scene);
    const action = mixer.clipAction(clip);
    // `LoopOnce` et non la boucle par défaut : sans elle la dernière image se
    // replie sur la première, et l'angle maximal rendrait le volant droit.
    action.loop = THREE.LoopOnce;
    action.clampWhenFinished = true;
    action.play();
    driverPose = { mixer, action, ...described };
    break;
  }

  // Le pilote greffé, repéré à son préfixe de nom : c'est ce qui permet de le
  // montrer et de le retirer sans reconvertir (voir `DRIVER_MESH_PREFIX`
  // côté Rust). Les matériaux sont collectés à part et **dédoublonnés** :
  // un même matériau sert plusieurs maillages, et lui écrire son opacité
  // plusieurs fois par image ne coûterait que du temps.
  const driverMeshes: ThreeModule.Mesh[] = [];
  const driverMaterials = new Set<ThreeModule.Material>();
  gltf.scene.traverse((object) => {
    const mesh = object as ThreeModule.Mesh;
    if (!mesh.isMesh || !mesh.name.startsWith(DRIVER_MESH_PREFIX)) return;
    driverMeshes.push(mesh);
    for (const material of Array.isArray(mesh.material) ? mesh.material : [mesh.material]) {
      driverMaterials.add(material);
    }
  });

  const opacity = driverShown ? 1 : 0;
  return {
    driverPose,
    steered,
    driver: driverMeshes,
    driverMaterials: [...driverMaterials],
    driverOpacity: opacity,
    driverTarget: opacity,
  };
}

/**
 * Braque les roues et le volant du modèle en place.
 *
 * **Rien n'est reconverti ici**, et c'est tout l'objet : l'angle a été cuit
 * dans le `.glb` pendant un temps, si bien que chaque valeur essayée laissait
 * une entrée de cache complète — onze entrées de 42 Mo pour une seule
 * voiture, mesuré sur le poste, parce qu'un curseur se balaye. La conversion
 * ne fait plus que **décrire** ce qui tourne (pivot en translation du nœud,
 * axe et facteur en `extras`), et le braquage est ici une rotation de nœud :
 * gratuit, instantané, et invisible à la clé de cache.
 *
 * L'angle est celui des **roues**, et elles le prennent tel quel : rien ne
 * les arrête. Le volant, lui, le multiplie par la démultiplication de la
 * voiture et s'arrête à la course qu'elle déclare — voir `SteerNode::limit`
 * pour ce que cette dissymétrie coûte et pourquoi elle est voulue.
 */
export function applySteer(current: Rig) {
  if (current.steered.length === 0) return;
  const steer = preview3dPrefs().steer;
  for (const { node, axis, gain, limit } of current.steered) {
    const stopped = limit === null ? steer : Math.max(-limit, Math.min(limit, steer));
    node.quaternion.setFromAxisAngle(axis, (stopped * gain * Math.PI) / 180);
  }
  poseDriver(current, steer);
}

/**
 * Pose les bras du pilote au même angle, **sans reconvertir**.
 *
 * Le mannequin est exporté en squelette vivant, avec l'animation de braquage
 * de la voiture (`kn5-gltf/src/rig.rs`) : poser les bras revient à choisir un
 * instant dans ce clip, ce que `AnimationMixer` sait faire pour rien. C'est
 * le dernier endroit où bouger le curseur coûtait une conversion.
 *
 * **Les bras suivent le volant, pas les roues** : ils le tiennent, donc ils
 * s'arrêtent avec lui à la butée que la voiture déclare, pendant que les
 * roues, elles, continuent jusqu'où le réglage demande.
 *
 * Et l'image se cherche **entre les extrêmes mesurés**, pas entre les bouts
 * du clip : une animation de braquage est une oscillation complète, dont les
 * bouts sont le volant droit (voir `Rig::extremes`).
 */
function poseDriver(current: Rig, wheelDegrees: number) {
  const pose = current.driverPose;
  if (!pose || pose.frames < 2 || pose.wheelLimit <= 0) return;
  const held = Math.max(-pose.wheelLimit, Math.min(pose.wheelLimit, wheelDegrees)) / pose.wheelLimit;
  const frame =
    held >= 0 ? pose.centre + held * (pose.high - pose.centre) : pose.centre + held * (pose.centre - pose.low);
  // Le temps du clip **est** l'indice d'image : la conversion écrit une
  // entrée par image, aux temps 0, 1, 2… (voir `push_accessor_scalar`).
  pose.action.time = frame;
  // Un pas nul : on ne fait pas avancer l'animation, on s'y positionne.
  pose.mixer.update(0);
}

/** Écrit une opacité sur le pilote.
 *
 * **Le passage en fondu ne se fait qu'aux extrémités**, et c'est ce qui rend
 * l'effet gratuit : changer `transparent` recompile le programme du
 * matériau, alors que changer `opacity` ne fait qu'écrire un uniforme. On
 * bascule donc en fondu au départ, on rend l'état d'origine à l'arrivée, et
 * entre les deux on n'écrit qu'un nombre. Sans profondeur pendant le fondu,
 * sinon le pilote se découperait dans son propre corps. */
export function applyDriverOpacity(current: Rig, opacity: number) {
  const fading = opacity > 0 && opacity < 1;
  for (const material of current.driverMaterials) {
    const state = material as ThreeModule.Material & {
      __wasTransparent?: boolean;
      __wroteDepth?: boolean;
    };
    state.__wasTransparent ??= material.transparent;
    state.__wroteDepth ??= material.depthWrite;
    const wanted = fading || state.__wasTransparent;
    if (material.transparent !== wanted) {
      material.transparent = wanted;
      material.needsUpdate = true;
    }
    material.depthWrite = fading ? false : (state.__wroteDepth ?? true);
    material.opacity = fading ? opacity : 1;
  }
  // Retiré de la scène quand il n'est plus là du tout : un mannequin à
  // opacité nulle coûterait encore ses appels de dessin et son ombre.
  for (const mesh of current.driver) mesh.visible = opacity > 0.002;
}

/** Avance le fondu d'une image. Rend `true` tant qu'il reste à faire, ce qui
 * suffit à garder la boucle de rendu en vie. */
export function stepDriverFade(current: Rig, elapsed: number): boolean {
  if (current.driver.length === 0) return false;
  const target = current.driverTarget;
  if (current.driverOpacity === target) return false;
  const step = elapsed / DRIVER_FADE_SECONDS;
  current.driverOpacity =
    target > current.driverOpacity
      ? Math.min(target, current.driverOpacity + step)
      : Math.max(target, current.driverOpacity - step);
  applyDriverOpacity(current, current.driverOpacity);
  return current.driverOpacity !== target;
}
