// Génération des vignettes de la grille (docs/SPEC-grille.md §5).
//
// **Le problème que ça règle.** Les `preview.png` d'origine sont hétérogènes —
// rendus sur noir, sur blanc, captures en jeu, photos — et l'œil passe son
// temps à se réadapter à un fond nouveau à chaque carte, si bien qu'il ne
// compare jamais les *formes*. Or identifier une voiture, c'est comparer des
// formes. Rendre les 312 dans le même cadrage, la même lumière et sur le même
// fond est un changement de nature, pas de degré.
//
// **Trois décisions structurent ce module, toutes prises contre une solution
// plus évidente :**
//
//  1. *Au fil de l'eau, jamais en masse.* Une carte demande sa vignette quand
//     elle entre dans le champ de vision, pas au chargement de la liste : la
//     bibliothèque se normalise pendant qu'on l'utilise, sans attente initiale.
//     Ce qui devient visible passe devant le reste de la file (§5.4).
//  2. *Une conversion à la fois, et hors du cache d'aperçus.* Le backend écrit
//     le modèle dans un brouillon qu'il vide avant chaque conversion : le pic
//     disque est celui d'une voiture, et le cache LRU — qui protège les
//     voitures qu'on consulte vraiment — n'est jamais touché (§5.3).
//  3. *Un seul contexte WebGL, gardé pour la session.* Un `WebGLRenderer` par
//     carte épuiserait la limite du navigateur (seize contextes en pratique)
//     dès la première rangée. Celui-ci ne rend jamais à l'écran : il produit un
//     PNG, que le backend range et resert ensuite tel quel.
import { convertFileSrc } from "@tauri-apps/api/core";
import type * as ThreeModule from "three";

import {
  forgetGridThumbnail,
  gridThumbnail,
  markGridThumbnailFailed,
  prepareGridModel,
  releaseGridModel,
  saveGridThumbnail,
  type GridTemplate,
} from "./gridThumbs";
import { appliedTemplate, gridThumbsOn, gridThumbsReady } from "./gridThumbPrefs.svelte";

/** Taille de rendu (§5.6). 16:9 parce que c'est le rapport des `preview.png`
 * d'Assetto Corsa : la grille restant mixte pour toujours (§7), les deux
 * sources doivent occuper le même cadre sans bande noire ni recadrage. */
const WIDTH = 1024;
const HEIGHT = 576;

/** Intensité de la lumière principale à 100 %, en unités three.js. Le gabarit
 * exprime tout en pourcentage d'elle. */
const KEY_BASE = 2.2;

/** Part de l'éclairage qui vient du showroom plutôt que des trois lampes.
 * L'environnement n'est pas décoratif : sans lui, une carrosserie ne reflète
 * rien et la peinture rend plate — c'est lui qui a été calibré sur les photos
 * Kunos (SPEC-preview-3d-kn5 §8.1). Les lampes viennent par-dessus, pour
 * détacher la silhouette. */
const ENVIRONMENT = 0.55;

type Entry = { url: string } | { pending: true } | { failed: string };

const cache = $state<Record<string, Entry>>({});

/** Empreinte du gabarit **appliqué**, dans la clé du cache mémoire : appliquer
 * un nouveau gabarit fait donc manquer toutes les lectures, et chaque carte
 * visible redemande la sienne d'elle-même. Rien à réinitialiser à la main, et
 * surtout pas d'appel de l'écran de réglages vers ce module. */
const fingerprint = $derived(JSON.stringify(appliedTemplate()));

function keyOf(carId: string, skinId: string | null): string {
  return `${fingerprint}|${carId}|${skinId ?? ""}`;
}

/**
 * La vignette régénérée de cette voiture, ou `null` s'il n'y en a pas (encore).
 *
 * Lecture réactive et **sans effet de bord** : appelée pour chaque carte à
 * chaque rendu de la grille, elle ne doit rien déclencher. C'est
 * `requestGridThumb` qui met en file, et seulement quand la carte devient
 * visible.
 */
export function gridThumb(carId: string, skinId: string | null): string | null {
  const entry = cache[keyOf(carId, skinId)];
  return entry && "url" in entry ? entry.url : null;
}

interface Job {
  carId: string;
  skinId: string | null;
  key: string;
}

const queue: Job[] = [];
let running = false;

/** Avancement de la génération, pour la tâche de fond du §8. */
const progress = $state({ done: 0, failed: 0, queued: 0, current: null as string | null });

export function gridThumbProgress() {
  return progress;
}

/**
 * Demande la vignette d'une voiture devenue visible.
 *
 * Déjà en file mais pas encore commencée, elle **remonte en tête** : changer de
 * filtre ou faire défiler réordonne la file, ce qui est nouvellement visible
 * passe devant (§8.4).
 */
export function requestGridThumb(carId: string, skinId: string | null): void {
  if (!gridThumbsOn()) return;
  const key = keyOf(carId, skinId);
  if (cache[key]) {
    // Déjà rendue, déjà ratée, ou déjà en file — dans ce dernier cas elle
    // remonte en tête, sans se dupliquer.
    const queued = queue.findIndex((job) => job.key === key);
    if (queued > 0) queue.unshift(...queue.splice(queued, 1));
    return;
  }
  cache[key] = { pending: true };
  queue.unshift({ carId, skinId, key });
  progress.queued = queue.length;
  void drain();
}

/**
 * Refait la vignette d'une seule voiture, quel que soit son état.
 *
 * **Le cas qui l'a rendue nécessaire n'est pas celui qu'on croit.** Ce n'est
 * pas un mod modifié — celui-là change son empreinte et se régénère tout seul —
 * mais un rendu **abîmé sans que le mod y soit pour rien** : contexte WebGL
 * perdu, textures qui n'arrivent pas jusqu'à la page. Rien ne distingue une
 * telle image d'une bonne une fois écrite, et la clé de cache, elle, est
 * parfaitement valide : sans cette porte, l'image reste à l'écran pour
 * toujours. Le contrôle de plausibilité ci-dessous en attrape une partie, pas
 * toutes — d'où un bouton, en plus.
 */
export async function regenerateGridThumb(carId: string, skinId: string | null): Promise<void> {
  const template = appliedTemplate();
  const known = await gridThumbnail(carId, skinId, template);
  await forgetGridThumbnail(known.stem);
  const key = keyOf(carId, skinId);
  delete cache[key];
  cache[key] = { pending: true };
  queue.unshift({ carId, skinId, key });
  progress.queued = queue.length;
  void drain();
}

async function drain(): Promise<void> {
  if (running) return;
  running = true;
  // Les réglages enregistrés avant la première vignette : produire trois cents
  // images sur les valeurs par défaut pour découvrir ensuite que l'utilisateur
  // en avait d'autres serait cinq minutes de travail à refaire.
  await gridThumbsReady();
  try {
    let job = queue.shift();
    while (job) {
      progress.queued = queue.length;
      progress.current = job.carId;
      try {
        cache[job.key] = await produce(job);
      } catch (e) {
        // Une vignette manquante n'est pas une panne : la carte garde la
        // `preview.png` du mod, qui est exactement le comportement d'avant.
        console.error("vignette de grille", job.carId, e);
        delete cache[job.key];
      }
      job = queue.shift();
    }
  } finally {
    running = false;
    progress.current = null;
    progress.queued = 0;
  }
}

async function produce(job: Job): Promise<Entry> {
  const template = appliedTemplate();
  // Le disque d'abord, toujours : quelques `stat` contre une conversion. C'est
  // aussi ce qui répond « déjà essayé, impossible » sans reparser un KN5 de
  // quatorze mégaoctets à chaque lancement (§7).
  const known = await gridThumbnail(job.carId, job.skinId, template);
  if (known.path) {
    progress.done += 1;
    return { url: convertFileSrc(known.path) };
  }
  if (known.failed) {
    progress.failed += 1;
    return { failed: known.failed };
  }

  let url: string;
  try {
    url = await prepareGridModel(job.carId, job.skinId);
  } catch (e) {
    const reason = typeof e === "string" ? e : String(e);
    // Une voiture chiffrée ne rendra **jamais** : on le note à côté de l'image
    // qu'on n'a pas pu produire, avec l'empreinte du mod dans le nom, donc la
    // tentative se refera d'elle-même le jour où le mod change (§7).
    if (reason.startsWith("errors.preview")) {
      await markGridThumbnailFailed(known.stem, reason).catch((err) =>
        console.error("échec de vignette non mémorisé", job.carId, err),
      );
      progress.failed += 1;
      return { failed: reason };
    }
    throw e;
  }

  try {
    const png = await render(url, template);
    if (!png) throw new Error("rendu vide");
    const bytes = new Uint8Array(await png.arrayBuffer());
    const path = await saveGridThumbnail(known.stem, bytes);
    progress.done += 1;
    // Affichée depuis le disque et non depuis le blob mémoire : c'est le même
    // fichier que toutes les visites suivantes serviront, autant qu'il soit à
    // l'écran tout de suite pour qu'un défaut se voie maintenant.
    return { url: convertFileSrc(path) };
  } finally {
    // Le modèle part **quoi qu'il arrive** : vingt mégaoctets laissés derrière
    // un rendu raté, et le disque grossit d'une voiture ratée à l'autre.
    await releaseGridModel().catch((e) => console.error("brouillon de vignette non jeté", e));
  }
}

// --- Le moteur de rendu, monté une fois -------------------------------------

interface Engine {
  THREE: typeof ThreeModule;
  renderer: ThreeModule.WebGLRenderer;
  scene: ThreeModule.Scene;
  camera: ThreeModule.PerspectiveCamera;
  key: ThreeModule.DirectionalLight;
  fill: ThreeModule.DirectionalLight;
  rim: ThreeModule.DirectionalLight;
  sun: ThreeModule.DirectionalLight;
  load: (url: string) => Promise<ThreeModule.Group>;
}

let engine: Promise<Engine> | null = null;

function ensureEngine(): Promise<Engine> {
  engine ??= (async () => {
    const THREE = await import("three");
    const { GLTFLoader } = await import("three/examples/jsm/loaders/GLTFLoader.js");
    const { showroomEnvironment } = await import("./components/detail/showroomEnvironment");

    // `preserveDrawingBuffer` : sans lui, la lecture du canevas rend une image
    // vide dès que le navigateur a eu le temps de vider le tampon entre le
    // rendu et la lecture. C'est le piège classique du rendu hors écran.
    // `alpha` : le fond reste transparent, c'est la carte qui fournit le sien
    // (§5.6) — un dégradé en CSS suit le thème et les états sans jamais
    // demander de régénérer une image.
    const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true, preserveDrawingBuffer: true });
    renderer.setPixelRatio(1);
    renderer.setSize(WIDTH, HEIGHT, false);
    // Neutre et **fixe pour les 312** : une auto-exposition ramènerait une
    // voiture noire et une voiture blanche au même gris moyen, c'est-à-dire
    // qu'elle effacerait exactement la différence qu'on cherche à montrer.
    renderer.toneMapping = THREE.NeutralToneMapping;
    renderer.toneMappingExposure = 1;
    renderer.outputColorSpace = THREE.SRGBColorSpace;
    renderer.shadowMap.enabled = true;
    renderer.shadowMap.type = THREE.PCFSoftShadowMap;

    const scene = new THREE.Scene();
    const pmrem = new THREE.PMREMGenerator(renderer);
    scene.environment = pmrem.fromScene(showroomEnvironment(THREE), 0.04).texture;
    scene.environmentIntensity = ENVIRONMENT;

    // Les trois lampes du §5.6. Leurs positions sont posées à chaque rendu,
    // relativement à la voiture : un mod fait deux mètres, un autre en fait
    // quinze si son auteur s'est trompé d'unité.
    const key = new THREE.DirectionalLight(0xffffff, 0);
    const fill = new THREE.DirectionalLight(0xffffff, 0);
    const rim = new THREE.DirectionalLight(0xffffff, 0);
    scene.add(key, fill, rim);

    // Le projecteur de l'ombre de contact, **à intensité nulle** : il n'éclaire
    // rien, tout ce que la voiture reçoit vient des trois autres. Il n'existe
    // que pour donner une direction de projection à three.js — `ShadowMaterial`
    // lit le masque d'ombre, pas la contribution de la lampe, donc les deux
    // sujets restent séparés.
    const sun = new THREE.DirectionalLight(0xffffff, 0);
    sun.castShadow = true;
    // La douceur se règle par la **résolution** et c'est contre-intuitif :
    // `PCFSoftShadowMap` a un noyau fixe exprimé en texels, donc moins de
    // texels = un flou plus large (mesuré sur l'aperçu de la fiche).
    sun.shadow.mapSize.set(512, 512);
    sun.shadow.bias = -0.0015;
    scene.add(sun, sun.target);

    const camera = new THREE.PerspectiveCamera(22, WIDTH / HEIGHT, 0.05, 500);
    const loader = new GLTFLoader();
    return {
      THREE,
      renderer,
      scene,
      camera,
      key,
      fill,
      rim,
      sun,
      load: async (url: string) => (await loader.loadAsync(url)).scene,
    };
  })();
  return engine;
}

async function render(url: string, template: GridTemplate): Promise<Blob | null> {
  const e = await ensureEngine();
  const { THREE, renderer, scene, camera } = e;
  const model = await e.load(url);
  model.traverse((object) => {
    const mesh = object as ThreeModule.Mesh;
    if (mesh.isMesh) mesh.castShadow = true;
  });
  scene.add(model);

  const box = new THREE.Box3().setFromObject(model);
  const center = box.getCenter(new THREE.Vector3());
  const radius = box.getSize(new THREE.Vector3()).length() / 2;

  // Le sol : rien d'autre que l'ombre. Pas de reflet miroir (§5.6) — il exige
  // un sol visible, donc un fond, et à une centaine de pixels de haut il
  // consommerait la moitié du cadre.
  const ground = new THREE.Mesh(
    new THREE.PlaneGeometry(radius * 6, radius * 6),
    new THREE.ShadowMaterial({ opacity: template.shadow / 100 }),
  );
  ground.rotation.x = -Math.PI / 2;
  ground.position.set(center.x, box.min.y, center.z);
  ground.receiveShadow = true;
  scene.add(ground);

  placeCamera(THREE, camera, box, center, template);
  placeLights(e, camera, center, radius, template);

  let png: Blob | null = null;
  try {
    renderer.render(scene, camera);
    checkPlausible(renderer.domElement);
    png = await toPng(renderer.domElement);
  } finally {
    scene.remove(model, ground);
    ground.geometry.dispose();
    ground.material.dispose();
    dispose(model);
  }
  return png;
}

/**
 * Cadrage **ajusté**, pas à l'échelle : chaque voiture remplit le cadre quelle
 * que soit sa taille réelle (§5.6).
 *
 * La distance ne se déduit pas d'un rayon : une voiture vue de trois-quarts est
 * large et basse, et la caler sur sa sphère englobante la laisserait minuscule
 * dans un cadre 16:9. Chacun des huit coins de la boîte est donc projeté dans
 * le repère de la caméra, et on retient la distance qui les fait tous entrer —
 * horizontalement **et** verticalement.
 */
function placeCamera(
  THREE: typeof ThreeModule,
  camera: ThreeModule.PerspectiveCamera,
  box: ThreeModule.Box3,
  center: ThreeModule.Vector3,
  template: GridTemplate,
): void {
  camera.fov = template.fov;
  camera.updateProjectionMatrix();

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

  let distance = 0;
  const corner = new THREE.Vector3();
  for (const x of [box.min.x, box.max.x]) {
    for (const y of [box.min.y, box.max.y]) {
      for (const z of [box.min.z, box.max.z]) {
        corner.set(x, y, z).sub(center);
        const depth = corner.dot(dir);
        distance = Math.max(
          distance,
          depth + Math.abs(corner.dot(right)) / tanH,
          depth + Math.abs(corner.dot(up)) / tanV,
        );
      }
    }
  }

  camera.position.copy(center).addScaledVector(dir, distance);
  camera.near = Math.max(distance / 100, 0.01);
  camera.far = distance * 4;
  camera.lookAt(center);
  camera.updateProjectionMatrix();
}

/** Les trois lampes, posées relativement à la caméra (§5.6). */
function placeLights(
  e: Engine,
  camera: ThreeModule.PerspectiveCamera,
  center: ThreeModule.Vector3,
  radius: number,
  template: GridTemplate,
): void {
  const { key, fill, rim, sun } = e;
  const intensity = (KEY_BASE * template.key) / 100;
  key.intensity = intensity;
  fill.intensity = (intensity * template.fill) / 100;
  rim.intensity = (intensity * template.rim) / 100;

  // L'azimut de la caméra, dont les lampes se décalent : le rig suit le
  // cadrage, sinon régler l'angle de vue déplacerait aussi la lumière.
  const base = Math.atan2(camera.position.x - center.x, camera.position.z - center.z);
  const place = (light: ThreeModule.DirectionalLight, offset: number, elevationDeg: number) => {
    const angle = base + (offset * Math.PI) / 180;
    const elevation = (elevationDeg * Math.PI) / 180;
    light.position.set(
      center.x + radius * 4 * Math.cos(elevation) * Math.sin(angle),
      center.y + radius * 4 * Math.sin(elevation),
      center.z + radius * 4 * Math.cos(elevation) * Math.cos(angle),
    );
    light.target.position.copy(center);
    light.target.updateMatrixWorld();
  };
  place(key, 45, 35);
  place(fill, -135, 20);
  // Le contre-jour : derrière le sujet, rasant sur la ligne de toit. **C'est
  // le paramètre le plus directement utile au problème de départ** — c'est lui
  // qui détache la silhouette d'une carrosserie noire, plus que n'importe quel
  // réglage de fond.
  place(rim, 175, 12);

  // L'ombre tombe presque à la verticale, avec juste assez de décalage pour se
  // lire comme une ombre.
  sun.position.set(center.x - radius * 0.3, center.y + radius * 4, center.z + radius * 0.5);
  sun.target.position.copy(center);
  sun.target.updateMatrixWorld();
  const shadow = sun.shadow.camera;
  shadow.left = -radius;
  shadow.right = radius;
  shadow.top = radius;
  shadow.bottom = -radius;
  shadow.near = radius * 0.5;
  shadow.far = radius * 8;
  shadow.updateProjectionMatrix();
}

/**
 * Refuse une image qui ne peut pas être ce qu'on voulait rendre.
 *
 * Deux avaries observées en développement, l'une et l'autre invisibles une fois
 * le PNG écrit — c'est bien le problème : le fichier existe, son nom est
 * valide, donc il est resservi pour toujours.
 *
 *  - **Rien du tout.** Un contexte WebGL perdu (recompilation, veille, pilote
 *    qui redémarre) rend un canevas vide, qui donne un PNG entièrement
 *    transparent : à l'écran, une carte au mat nu.
 *  - **Tout blanc.** Une voiture dont les textures ne sont pas arrivées jusqu'à
 *    la page rend en matériau par défaut, que l'éclairage du studio sature.
 *
 * Le contrôle est fait sur une réduction à 64×36 — deux mille pixels suffisent
 * à distinguer « une voiture » de « rien » ou de « un aplat », et coûtent
 * quelques dixièmes de milliseconde sur un rendu qui en a pris mille.
 *
 * Lève plutôt que de renvoyer un booléen : l'appelant ne doit **ni ranger
 * l'image, ni mémoriser un échec** — c'est un accident, pas un verdict sur le
 * mod, et la voiture doit repasser normalement à la prochaine demande.
 */
function checkPlausible(canvas: HTMLCanvasElement): void {
  const probe = document.createElement("canvas");
  probe.width = 64;
  probe.height = 36;
  const ctx = probe.getContext("2d", { willReadFrequently: true });
  // Pas de contexte 2D : on ne peut pas juger, donc on ne juge pas. Refuser par
  // précaution reviendrait à ne jamais rien produire.
  if (!ctx) return;
  ctx.drawImage(canvas, 0, 0, probe.width, probe.height);
  const { data } = ctx.getImageData(0, 0, probe.width, probe.height);
  let opaque = 0;
  let white = 0;
  for (let i = 0; i < data.length; i += 4) {
    if (data[i + 3] < 24) continue;
    opaque += 1;
    if (data[i] > 244 && data[i + 1] > 244 && data[i + 2] > 244) white += 1;
  }
  const pixels = probe.width * probe.height;
  // Le seuil est bas exprès : une monoplace vue de trois-quarts couvre peu de
  // cadre, et l'ombre de contact compte à peine. Sous 2 %, il n'y a rien.
  if (opaque < pixels * 0.02) throw new Error("rendu vide (contexte WebGL perdu ?)");
  if (white > opaque * 0.9) throw new Error("rendu saturé (textures manquantes ?)");
}

function toPng(canvas: HTMLCanvasElement): Promise<Blob | null> {
  return new Promise((resolve) => canvas.toBlob(resolve, "image/png"));
}

function dispose(model: ThreeModule.Group): void {
  model.traverse((object) => {
    const mesh = object as ThreeModule.Mesh;
    if (!mesh.isMesh) return;
    mesh.geometry.dispose();
    for (const raw of Array.isArray(mesh.material) ? mesh.material : [mesh.material]) {
      const material = raw as ThreeModule.MeshStandardMaterial;
      material.map?.dispose();
      material.normalMap?.dispose();
      material.roughnessMap?.dispose();
      material.metalnessMap?.dispose();
      material.emissiveMap?.dispose();
      material.dispose();
    }
  });
}
