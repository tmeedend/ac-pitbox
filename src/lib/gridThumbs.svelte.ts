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
import { gridThumbsOn, gridThumbsReady } from "./gridThumbPrefs.svelte";

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

/**
 * La clé mêle le **gabarit**, pas le preset.
 *
 * Deux presets aux mêmes valeurs de rendu produisent la même image et doivent
 * la partager ; ce qui les distingue par ailleurs — leur nom, leur mat — ne
 * change pas un pixel. C'est la même règle que côté disque, où le nom de
 * fichier porte l'empreinte du gabarit et rien d'autre : changer la couleur de
 * fond d'un preset ne doit pas régénérer trois cents images.
 */
function keyOf(template: GridTemplate, carId: string, skinId: string | null): string {
  return `${JSON.stringify(template)}|${carId}|${skinId ?? ""}`;
}

/**
 * La vignette régénérée de cette voiture, ou `null` s'il n'y en a pas (encore).
 *
 * Lecture réactive et **sans effet de bord** : appelée pour chaque carte à
 * chaque rendu de la grille, elle ne doit rien déclencher. C'est
 * `requestGridThumb` qui met en file, et seulement quand la carte devient
 * visible.
 */
export function gridThumb(template: GridTemplate, carId: string, skinId: string | null): string | null {
  const entry = cache[keyOf(template, carId, skinId)];
  return entry && "url" in entry ? entry.url : null;
}

interface Job {
  /** Le gabarit sous lequel produire. Porté par le travail et non lu au moment
   * de le faire : la file peut mêler deux presets — la grille en lie un par
   * densité — et une voiture mise en file pour l'un ne doit pas se retrouver
   * rendue avec l'autre parce qu'on a changé de vue entre-temps. */
  template: GridTemplate;
  carId: string;
  skinId: string | null;
  /** Nom lisible, pour la tâche de fond : le §8.2 y montre « Nissan Skyline
   * GT-R R34 » et non un identifiant de dossier. */
  name: string;
  key: string;
}

const queue: Job[] = [];
let running = false;

/**
 * Verrou du **brouillon**, côté frontend.
 *
 * Le backend sérialise déjà les conversions, mais pas la fenêtre qui suit :
 * entre le moment où `prepareGridModel` rend son URL et celui où three.js a
 * fini d'aller chercher la géométrie et les textures, le dossier doit rester
 * en place. Or la conversion suivante le vide en commençant. Un seul client
 * (la file) ne se marche jamais dessus ; deux — la file et l'aperçu de
 * réglages — si, et le symptôme serait une voiture sans texture, c'est-à-dire
 * blanche.
 */
let scratchLock: Promise<unknown> = Promise.resolve();

function withScratch<T>(work: () => Promise<T>): Promise<T> {
  // `catch` sur la chaîne, pas sur le travail : un échec ne doit pas geler
  // toutes les prises de verrou suivantes (même piège que la file d'écriture
  // de `ui_prefs.json`).
  const next = scratchLock.then(work, work);
  scratchLock = next.catch(() => undefined);
  return next;
}

/** La file est-elle suspendue ? L'aperçu de l'écran de réglages la met en
 * pause : ses six conversions et le rendu qu'on manipule ne doivent pas se
 * disputer le brouillon ni le processeur (§6.3 — manipuler les réglages ne
 * régénère rien). */
let paused = false;

export function pauseGridThumbs(): void {
  paused = true;
}

export function resumeGridThumbs(): void {
  if (!paused) return;
  paused = false;
  void drain();
}

/**
 * Avancement de la génération, pour la tâche de fond du §8.
 *
 * `total` est **cumulatif sur le lot** et non la taille de la file : celle-ci
 * se vide au fur et à mesure, donc s'en servir donnerait une barre qui recule.
 * Un lot commence quand la file part de zéro et finit quand elle se vide.
 */
const progress = $state({
  total: 0,
  done: 0,
  failed: 0,
  current: null as string | null,
  running: false,
  cancelling: false,
  /** Le lot est fini et son rapport attend d'être fermé à la main (§8.2). */
  finished: false,
  startedAt: 0,
});

export function gridThumbProgress() {
  return progress;
}

/**
 * Temps restant estimé, en secondes, ou `null` tant qu'il serait fantaisiste.
 *
 * **Rien avant une dizaine de voitures** (§8.3) : une estimation tirée de deux
 * mesures est fausse d'un facteur trois, et elle détruit la confiance dans
 * toutes les suivantes. Afficher le décompte seul coûte moins cher.
 */
export function gridThumbEta(): number | null {
  if (!progress.running || progress.done < 10) return null;
  const elapsed = (Date.now() - progress.startedAt) / 1000;
  const remaining = progress.total - progress.done - progress.failed;
  if (remaining <= 0) return null;
  return (elapsed / progress.done) * remaining;
}

/**
 * Arrête le lot. **Ce qui est fait est gardé**, et le rapport le dit — sans
 * cette phrase, on se retrouve avec une grille mixte sans savoir qu'on peut
 * reprendre (§8.3).
 */
export function cancelGridThumbs(): void {
  queue.length = 0;
  progress.cancelling = true;
}

/** Ferme le rapport de fin. Il ne part jamais tout seul : cinq minutes de
 * travail méritent qu'on ait le temps de lire ce qu'elles ont donné. */
export function dismissGridThumbReport(): void {
  progress.finished = false;
}

/** Ouvre un lot si la file était vide. */
function beginBatch(): void {
  if (progress.running || queue.length > 0) return;
  progress.total = 0;
  progress.done = 0;
  progress.failed = 0;
  progress.cancelling = false;
  progress.finished = false;
  progress.startedAt = Date.now();
}

/** Met une voiture en file si elle n'y est pas déjà. `front` = elle passe
 * devant tout le reste. */
function enqueue(
  template: GridTemplate,
  carId: string,
  skinId: string | null,
  name: string,
  front: boolean,
): void {
  const key = keyOf(template, carId, skinId);
  if (cache[key]) {
    // Déjà rendue, déjà ratée, ou déjà en file — dans ce dernier cas elle
    // remonte en tête si on la redemande en priorité, sans se dupliquer.
    if (!front) return;
    const queued = queue.findIndex((job) => job.key === key);
    if (queued > 0) queue.unshift(...queue.splice(queued, 1));
    return;
  }
  beginBatch();
  cache[key] = { pending: true };
  const job = { template, carId, skinId, name, key };
  if (front) queue.unshift(job);
  else queue.push(job);
  progress.total += 1;
}

/**
 * Demande la vignette d'une voiture **devenue visible**.
 *
 * Elle passe devant tout le reste de la file : faire défiler ou changer de
 * filtre la réordonne donc de lui-même, ce qui est nouvellement visible
 * d'abord (§8.4).
 */
export function requestGridThumb(
  template: GridTemplate,
  carId: string,
  skinId: string | null,
  name = carId,
): void {
  if (!gridThumbsOn()) return;
  enqueue(template, carId, skinId, name, true);
  void drain();
}

/**
 * Met en file tout ce qui reste, derrière ce qui est visible (§5.4).
 *
 * Sans cette seconde moitié, la génération ne produirait que ce qu'on a
 * regardé, et le décompte de la tâche de fond n'aurait pas de dénominateur : la
 * file se viderait à chaque arrêt du défilement. C'est elle qui fait de la
 * génération un travail qui finit.
 */
export function enqueueGridThumbs(
  template: GridTemplate,
  cars: { id: string; skin: string | null; name: string }[],
): void {
  if (!gridThumbsOn()) return;
  for (const car of cars) enqueue(template, car.id, car.skin, car.name, false);
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
export async function regenerateGridThumb(
  template: GridTemplate,
  carId: string,
  skinId: string | null,
  name?: string,
): Promise<void> {
  const known = await gridThumbnail(carId, skinId, template);
  await forgetGridThumbnail(known.stem);
  delete cache[keyOf(template, carId, skinId)];
  enqueue(template, carId, skinId, name ?? carId, true);
  void drain();
}

async function drain(): Promise<void> {
  if (running || paused) return;
  running = true;
  progress.running = true;
  // Les réglages enregistrés avant la première vignette : produire trois cents
  // images sur les valeurs par défaut pour découvrir ensuite que l'utilisateur
  // en avait d'autres serait cinq minutes de travail à refaire.
  await gridThumbsReady();
  try {
    let job = queue.shift();
    while (job && !paused) {
      progress.current = job.name;
      try {
        cache[job.key] = await produce(job);
      } catch (e) {
        // Une vignette manquante n'est pas une panne : la carte garde la
        // `preview.png` du mod, qui est exactement le comportement d'avant.
        // Rien n'est mémorisé — un accident de rendu n'est pas un verdict sur
        // le mod, la voiture repassera normalement à la prochaine demande.
        console.error("vignette de grille", job.carId, e);
        delete cache[job.key];
        progress.total -= 1;
      }
      job = queue.shift();
    }
  } finally {
    running = false;
    progress.running = false;
    progress.current = null;
    // Le rapport reste, y compris après une annulation : le §8.3 veut
    // « 148 vignettes générées, reprendre plus tard » plutôt qu'une
    // disparition silencieuse.
    progress.finished = progress.done > 0 || progress.failed > 0;
  }
}

async function produce(job: Job): Promise<Entry> {
  const template = job.template;
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

  try {
    return await withScratch(() => convert(job, known.stem, template));
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
}

/** Convertir, rendre, ranger, jeter — la séquence du §5.3, sous le verrou du
 * brouillon. */
async function convert(job: Job, stem: string, template: GridTemplate): Promise<Entry> {
  const url = await prepareGridModel(job.carId, job.skinId);
  try {
    const png = await render(url, template);
    if (!png) throw new Error("rendu vide");
    const bytes = new Uint8Array(await png.arrayBuffer());
    const path = await saveGridThumbnail(stem, bytes);
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

// --- Le moteur de rendu ------------------------------------------------------
//
// **Un banc, deux clients.** La file en monte un seul, à la taille de sortie,
// gardé pour la session : un `WebGLRenderer` par carte épuiserait la limite du
// navigateur (seize contextes en pratique) dès la première rangée. L'aperçu de
// l'écran de réglages (§6.2) en monte un second, plus petit, avec ses six
// voitures gardées en mémoire — bouger un curseur doit redessiner, jamais
// reconvertir.
//
// Ils ne peuvent pas partager le même : le rendu de la file s'étend de
// `renderer.render` à `toBlob`, qui est asynchrone, et une image dessinée entre
// les deux repartirait dans le PNG de l'autre.

interface Rig {
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

let engine: Promise<Rig> | null = null;

/** Le banc de la file, monté une fois pour la session. */
function ensureEngine(): Promise<Rig> {
  engine ??= createRig(WIDTH, HEIGHT);
  return engine;
}

async function createRig(width: number, height: number): Promise<Rig> {
  {
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
    renderer.setSize(width, height, false);
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

    const camera = new THREE.PerspectiveCamera(22, width / height, 0.05, 500);
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
  }
}

/**
 * Pose la voiture, la caméra, les lampes et le sol, puis dessine.
 *
 * Le modèle est ajouté et retiré **par l'appelant** : la file le jette après
 * une image, l'aperçu de réglages le garde pour la suivante. Tout le reste —
 * le sol notamment, qui dépend de la taille de la voiture et de l'opacité
 * réglée — appartient à l'image et repart avec elle.
 */
function drawModel(rig: Rig, model: ThreeModule.Group, template: GridTemplate): void {
  const { THREE, renderer, scene, camera } = rig;
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
  placeLights(rig, camera, center, radius, template);

  try {
    renderer.render(scene, camera);
  } finally {
    scene.remove(ground);
    ground.geometry.dispose();
    ground.material.dispose();
  }
}

/** Prépare un modèle à être dessiné : tout ce qui est maillage projette une
 * ombre, sinon le sol reste vide et la voiture flotte. */
function castShadows(model: ThreeModule.Group): void {
  model.traverse((object) => {
    const mesh = object as ThreeModule.Mesh;
    if (mesh.isMesh) mesh.castShadow = true;
  });
}

async function render(url: string, template: GridTemplate): Promise<Blob | null> {
  const rig = await ensureEngine();
  const model = await rig.load(url);
  castShadows(model);
  rig.scene.add(model);

  let png: Blob | null = null;
  try {
    drawModel(rig, model, template);
    checkPlausible(rig.renderer.domElement);
    png = await toPng(rig.renderer.domElement);
  } finally {
    rig.scene.remove(model);
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
  rig: Rig,
  camera: ThreeModule.PerspectiveCamera,
  center: ThreeModule.Vector3,
  radius: number,
  template: GridTemplate,
): void {
  const { key, fill, rim, sun } = rig;
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

// --- L'aperçu de l'écran de réglages (§6.2) ---------------------------------

/** Une voiture chargée une fois et gardée, pour être redessinée à chaque
 * mouvement de curseur. */
export interface StudioCar {
  id: string;
  name: string;
  model: ThreeModule.Group;
}

export interface GridStudio {
  /** Convertit et garde une voiture. `null` si elle ne rend pas — une voiture
   * protégée dans l'échantillon ne doit pas vider l'aperçu. */
  load(carId: string, skinId: string | null, name: string): Promise<StudioCar | null>;
  /** Redessine une voiture au gabarit donné et rend une URL d'image. Synchrone
   * côté GPU : c'est ce qui permet de suivre un curseur. */
  draw(car: StudioCar, template: GridTemplate): string;
  dispose(): void;
}

/**
 * Monte le banc de l'aperçu de réglages : **six voitures gardées en mémoire**,
 * redessinées à chaque mouvement de curseur.
 *
 * C'est la décision structurante du §6.2. Régler l'angle sur une seule voiture
 * conduit à l'optimiser pour elle et à massacrer les autres : on édite un
 * catalogue, l'aperçu doit être un catalogue. Et garder les modèles chargés est
 * ce qui rend le geste possible — reconvertir six voitures à chaque pixel de
 * curseur prendrait six secondes par image.
 *
 * Son propre contexte WebGL, plus petit : voir l'en-tête du moteur.
 */
export async function createGridStudio(width: number, height: number): Promise<GridStudio> {
  const rig = await createRig(width, height);
  const held: ThreeModule.Group[] = [];
  return {
    async load(carId, skinId, name) {
      try {
        return await withScratch(async () => {
          const url = await prepareGridModel(carId, skinId);
          try {
            const model = await rig.load(url);
            castShadows(model);
            held.push(model);
            return { id: carId, name, model };
          } finally {
            await releaseGridModel().catch(() => undefined);
          }
        });
      } catch (e) {
        console.error("aperçu de gabarit", carId, e);
        return null;
      }
    },
    draw(car, template) {
      rig.scene.add(car.model);
      try {
        drawModel(rig, car.model, template);
        return rig.renderer.domElement.toDataURL("image/png");
      } finally {
        rig.scene.remove(car.model);
      }
    },
    dispose() {
      for (const model of held) dispose(model);
      held.length = 0;
      rig.renderer.dispose();
    },
  };
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
