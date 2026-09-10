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
import { applyFloorMirror } from "./components/detail/floorMirror";

/** Taille de sortie (§5.6). 16:9 parce que c'est le rapport des `preview.png`
 * d'Assetto Corsa : la grille restant mixte pour toujours (§7), les deux
 * sources doivent occuper le même cadre sans bande noire ni recadrage. */
const WIDTH = 1024;
const HEIGHT = 576;

/**
 * Facteur de suréchantillonnage : on rend deux fois plus grand, puis on réduit.
 *
 * **Le MSAA ne suffit pas ici**, et c'est le même constat que sur l'aperçu de
 * la fiche : il échantillonne la *couverture* des triangles mais n'ombre qu'une
 * fois par texel, donc il ne peut rien contre un reflet spéculaire plus fin
 * qu'un pixel — exactement ce qui crénèle une arête de toit ou une jante. Rendre
 * en 2048×1152 puis réduire ombre quatre fois plus de points et règle les deux.
 *
 * Le coût est payé une fois par vignette, en arrière-plan, sur une image qui
 * sera servie des milliers de fois : c'est le bon endroit pour dépenser.
 */
const SUPERSAMPLE = 2;

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

/**
 * Pourquoi la file est suspendue, et par qui.
 *
 * **Un ensemble de raisons plutôt qu'un booléen**, parce qu'elles se
 * superposent et ne se lèvent pas ensemble : l'aperçu de l'écran de réglages
 * suspend le temps qu'on manipule ses curseurs, une session lancée suspend
 * jusqu'à la fermeture du jeu. Avec un booléen, fermer l'écran de réglages
 * relancerait la génération pendant que le jeu tourne.
 */
const pauses = new Set<string>();

/** L'aperçu de réglages : ses six conversions et le rendu qu'on manipule ne
 * doivent pas se disputer le brouillon ni le processeur (§6.3). */
export const PAUSE_STUDIO = "studio";
/**
 * Une session lancée. **La demande la plus forte de l'utilisateur** : le jeu
 * démarre, il veut toute la machine, et trois cents conversions en arrière-plan
 * sont exactement ce qu'il ne faut pas.
 *
 * Posée **dès le clic** sur « Démarrer » (`launchSession`) et levée à la
 * **fermeture du jeu**, que l'app sait voir : le fil qui coupe et reprend la
 * musique de Big Picture surveille déjà le process d'Assetto Corsa
 * (`music/watch.rs`), et `AppShell` s'abonne à ce qu'il annonce.
 *
 * Les deux bouts ne suivent pas le même signal, et c'est voulu : la musique
 * continue pendant tout l'écran de chargement et ne se coupe qu'une fois la
 * voiture pilotable, alors que les conversions doivent cesser tout de suite —
 * elles rallongeraient précisément ce chargement.
 */
export const PAUSE_SESSION = "session";

export function pauseGridThumbs(reason: string): void {
  pauses.add(reason);
}

export function resumeGridThumbs(reason: string): void {
  if (!pauses.delete(reason) || pauses.size > 0) return;
  void drain();
}

export function gridThumbsPaused(): boolean {
  return pauses.size > 0;
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
  // Rien pendant une pause : le temps écoulé continue de courir alors que rien
  // n'avance, donc l'estimation gonflerait à chaque seconde de course.
  if (!progress.running || pauses.size > 0 || progress.done < 10) return null;
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
  if (running || pauses.size > 0) return;
  running = true;
  progress.running = true;
  // Les réglages enregistrés avant la première vignette : produire trois cents
  // images sur les valeurs par défaut pour découvrir ensuite que l'utilisateur
  // en avait d'autres serait cinq minutes de travail à refaire.
  await gridThumbsReady();
  try {
    let job = queue.shift();
    while (job && pauses.size === 0) {
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
      // **Une respiration entre deux voitures.** Le rendu et l'écriture du PNG
      // se font sur le fil principal ; enchaîner sans rendre la main laisse
      // l'interface hachée même quand la conversion, elle, est bornée. Un
      // soixantième de seconde suffit à laisser passer une image.
      await new Promise((resolve) => setTimeout(resolve, 16));
      job = queue.shift();
    }
  } finally {
    running = false;
    progress.current = null;
    // **Suspendue n'est pas terminée**, et la nuance se voyait à l'écran :
    // sortir de la boucle sur une pause laisse la file pleine, si bien que le
    // rapport de fin annonçait « 148 vignettes générées » pendant que cent
    // soixante attendaient encore. Le lot reste donc « en cours » tant qu'il
    // reste du travail — la tâche de fond dit alors pourquoi il n'avance pas —
    // et le rapport n'arrive que la file vide.
    //
    // Le rapport, lui, reste affiché y compris après une annulation : le §8.3
    // veut « 148 vignettes générées, reprendre plus tard » plutôt qu'une
    // disparition silencieuse.
    const remaining = queue.length > 0;
    progress.running = remaining;
    progress.finished = !remaining && (progress.done > 0 || progress.failed > 0);
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
  /** La flaque peinte, montée une fois : sa texture est un canevas de 512 px
   * qu'on ne repeint pas à chaque image. Invisible quand le preset n'a pas de
   * sol. */
  pool: ThreeModule.Mesh<ThreeModule.PlaneGeometry, ThreeModule.MeshBasicMaterial>;
  /** Le receveur d'ombre, toujours là : l'ombre de contact existe même sans
   * sol visible, et c'est elle qui empêche la voiture de flotter. */
  shadow: ThreeModule.Mesh<ThreeModule.PlaneGeometry, ThreeModule.ShadowMaterial>;
  /** Le miroir, monté à la première image qui en demande un. `null` tant
   * qu'aucun preset ne reflète — c'est une seconde passe de rendu complète,
   * inutile de la payer pour un catalogue. */
  mirror: MirrorHandle | null;
  /** Le fond cuit dans l'image, et les couleurs actuellement peintes dessus —
   * pour ne repeindre son canevas que quand elles changent, plutôt qu'une fois
   * par vignette. */
  backdrop: ThreeModule.Texture | null;
  painted: string;
  /** Taille du tampon, pour le calcul de flou du miroir. */
  width: number;
  load: (url: string) => Promise<ThreeModule.Group>;
}

interface MirrorHandle {
  mesh: ThreeModule.Mesh;
  material: ThreeModule.ShaderMaterial;
  targetWidth: number;
}

let engine: Promise<Rig> | null = null;

/** Le banc de la file, monté une fois pour la session. */
function ensureEngine(): Promise<Rig> {
  engine ??= createRig(WIDTH * SUPERSAMPLE, HEIGHT * SUPERSAMPLE);
  return engine;
}

/**
 * Réduit le rendu à la taille de sortie.
 *
 * `drawImage` avec le lissage de qualité : c'est le filtre du navigateur, et il
 * est bien meilleur qu'une réduction naïve — c'est tout l'intérêt d'avoir rendu
 * plus grand.
 */
function downsample(source: HTMLCanvasElement): HTMLCanvasElement {
  const out = document.createElement("canvas");
  out.width = WIDTH;
  out.height = HEIGHT;
  const ctx = out.getContext("2d");
  if (!ctx) return source;
  ctx.imageSmoothingEnabled = true;
  ctx.imageSmoothingQuality = "high";
  ctx.drawImage(source, 0, 0, WIDTH, HEIGHT);
  return out;
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

    // Les deux plans du sol, montés une fois et repositionnés à chaque voiture.
    // Leur géométrie est un carré unitaire mis à l'échelle : recréer un
    // `PlaneGeometry` par vignette allouerait trois cents tampons pour rien.
    const { poolTexture } = await import("./components/detail/studioFloor");
    const plane = new THREE.PlaneGeometry(1, 1);
    const pool = new THREE.Mesh(
      plane,
      new THREE.MeshBasicMaterial({
        map: poolTexture(THREE),
        transparent: true,
        depthWrite: false,
        // Le dégradé est déjà la valeur voulue à l'écran : le faire passer par
        // le tone mapping l'assombrirait d'un tiers.
        toneMapped: false,
      }),
    );
    pool.rotation.x = -Math.PI / 2;
    pool.renderOrder = -2;
    pool.visible = false;
    scene.add(pool);

    const shadow = new THREE.Mesh(plane, new THREE.ShadowMaterial({ opacity: 0.35 }));
    shadow.rotation.x = -Math.PI / 2;
    shadow.receiveShadow = true;
    shadow.renderOrder = -1;
    scene.add(shadow);

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
      pool,
      shadow,
      mirror: null,
      backdrop: null,
      painted: "",
      width,
      load: async (url: string) => (await loader.loadAsync(url)).scene,
    };
  }
}

/** Portée du reflet : une constante, pas un curseur.
 *
 * C'est la valeur arrêtée sur l'aperçu de la fiche (75 %), et la rouvrir ici
 * ferait un curseur de plus pour un réglage qui décide de peu. Le **flou**, en
 * revanche, est passé dans le gabarit : c'est lui qui sépare un sol laqué d'un
 * sol mouillé, et c'est ce qu'on vient régler sur un preset de vitrine. */
const MIRROR_REACH = 75;

/**
 * Monte le miroir du sol, à la première image qui en demande un.
 *
 * Une **seconde passe de rendu complète** de la scène depuis une caméra
 * symétrique : c'est la seule façon d'obtenir un reflet, une carte
 * d'environnement ne reflétant que le studio figé et jamais la voiture (mesuré
 * au banc, `floorMirror.ts`). Elle double le temps GPU d'une vignette, ce qui
 * reste négligeable devant la seconde de conversion.
 */
async function ensureMirror(rig: Rig): Promise<MirrorHandle> {
  if (rig.mirror) return rig.mirror;
  const { Reflector } = await import("three/addons/objects/Reflector.js");
  const { floorMirrorShader } = await import("./components/detail/floorMirror");
  const THREE = rig.THREE;
  const mesh = new Reflector(new THREE.PlaneGeometry(1, 1), {
    textureWidth: rig.width,
    textureHeight: Math.round((rig.width * 9) / 16),
    shader: floorMirrorShader,
  });
  const target = mesh.getRenderTarget().texture;
  // Les mipmaps servent le flou : il lit le niveau que `applyFloorMirror`
  // choisit, pour que ses 25 prises restent jointives.
  target.minFilter = THREE.LinearMipmapLinearFilter;
  target.generateMipmaps = true;
  mesh.rotation.x = -Math.PI / 2;
  const material = mesh.material as ThreeModule.ShaderMaterial;
  material.transparent = true;
  material.depthWrite = false;
  // Sous la flaque (-2) et sous l'ombre (-1) : le reflet est le sol, tout le
  // reste se pose dessus.
  mesh.renderOrder = -3;

  // Le reflet ne doit montrer **que** la voiture : sans ça, la flaque et
  // l'ombre se retrouvent dans leur propre reflet et le sol se dédouble.
  const reflect = mesh.onBeforeRender;
  mesh.onBeforeRender = function (...args: Parameters<typeof reflect>) {
    rig.pool.visible = false;
    rig.shadow.visible = false;
    reflect.apply(this, args);
    rig.pool.visible = true;
    rig.shadow.visible = true;
  };
  rig.scene.add(mesh);
  rig.mirror = { mesh, material, targetWidth: mesh.getRenderTarget().width };
  return rig.mirror;
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

  // **Le sol se compose en trois couches**, dans cet ordre de bas en haut : le
  // reflet (le sol est laqué), la flaque (il y a un sol), l'ombre portée (la
  // voiture le touche). Un preset de catalogue n'en garde que la troisième et
  // reste entièrement détouré ; un preset de vitrine les allume toutes, et cuit
  // alors une part de son fond dans l'image — c'est le prix d'un reflet, qui
  // n'a rien à moduler sur du transparent.
  const floorY = box.min.y;
  const span = radius * 6;

  rig.shadow.position.set(center.x, floorY + radius / 300, center.z);
  rig.shadow.scale.set(span, span, 1);
  rig.shadow.material.opacity = template.shadow / 100;

  rig.pool.visible = template.floor > 0;
  rig.pool.position.set(center.x, floorY + radius / 400, center.z);
  rig.pool.scale.set(span, span, 1);
  rig.pool.material.opacity = template.floor / 100;

  if (rig.mirror) {
    // Le miroir suit le preset : allumé, il se pose ; éteint, il disparaît sans
    // être démonté — le remonter coûterait une cible de rendu à chaque bascule.
    rig.mirror.mesh.visible = template.reflection > 0 && template.floor > 0;
    rig.mirror.mesh.position.set(center.x, floorY + radius / 500, center.z);
    rig.mirror.mesh.scale.set(span, span, 1);
    applyFloorMirror(
      rig.mirror.material.uniforms,
      { reflection: template.reflection, reflectionBlur: template.reflectionBlur, reflectionReach: MIRROR_REACH },
      rig.mirror.targetWidth,
    );
  }

  placeCamera(THREE, camera, box, center, radius, template);
  placeLights(rig, camera, center, radius, template);
  applyBackdrop(rig, template);

  renderer.render(scene, camera);
}

/**
 * Pose le fond cuit (preset Officiel), ou le retire.
 *
 * **`scene.background` et non un plan dans la scène**, et c'est un correctif :
 * un plan avait été essayé, accroché à la caméra, transparent, avec
 * `renderOrder: -10` pour passer en premier. Il passait en dernier et
 * recouvrait la voiture — toutes les vignettes sortaient noires. La raison ne
 * se devine pas : three.js dessine **toute** la liste des objets opaques avant
 * la liste des transparents, et `renderOrder` ne trie qu'à l'intérieur d'une
 * liste. Un fond transparent est donc structurellement condamné à passer après
 * une voiture opaque, quel que soit son ordre de rendu.
 *
 * `scene.background` échappe entièrement à ce classement : three le dessine
 * avant tout, comme un fond, ce qu'il est. Et il rend l'image **opaque**, ce
 * que ce preset veut — c'est la seule façon qu'une vignette soit indiscernable
 * d'une `preview.png` d'origine posée à côté d'elle.
 */
function applyBackdrop(rig: Rig, template: GridTemplate): void {
  if (template.background <= 0) {
    rig.scene.background = null;
    return;
  }
  const key = `${template.matHi}|${template.matLo}`;
  if (rig.painted !== key || !rig.backdrop) {
    rig.backdrop?.dispose();
    rig.backdrop = backdropTexture(rig.THREE, template.matHi, template.matLo);
    rig.painted = key;
  }
  rig.scene.background = rig.backdrop;
}

/**
 * Le dégradé du fond : **exactement celui du mat de la carte**, peint dans
 * l'image.
 *
 * Les mêmes deux couleurs des deux côtés, et ce n'est pas une commodité : une
 * vignette qui porte son fond et une carte qui en porte un autre se voient au
 * premier coup d'œil, et c'est précisément le défaut que ce preset existe pour
 * effacer. C'est aussi pourquoi le backend fait entrer le mat dans l'empreinte
 * dès que le fond est cuit.
 *
 * Une ellipse et non un cercle : le cadre est en 16:9, un dégradé circulaire y
 * laisserait deux coins plus clairs que les deux autres.
 */
function backdropTexture(THREE: typeof ThreeModule, hi: string, lo: string): ThreeModule.Texture {
  const width = 256;
  const height = 144;
  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  const ctx = canvas.getContext("2d");
  if (ctx) {
    ctx.fillStyle = lo;
    ctx.fillRect(0, 0, width, height);
    // Le centre du dégradé est un peu au-dessus du milieu, là où se pose une
    // voiture — même géométrie que le mat CSS de la carte (`ellipse at 50% 44%`).
    ctx.save();
    ctx.translate(width / 2, height * 0.44);
    ctx.scale(width / height, 1);
    const glow = ctx.createRadialGradient(0, 0, 0, 0, 0, height * 0.76);
    glow.addColorStop(0, hi);
    glow.addColorStop(1, lo);
    ctx.fillStyle = glow;
    ctx.fillRect(-width, -height, width * 2, height * 2);
    ctx.restore();
  }
  const texture = new THREE.CanvasTexture(canvas);
  texture.colorSpace = THREE.SRGBColorSpace;
  return texture;
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
  if (template.reflection > 0 && template.floor > 0) await ensureMirror(rig);
  const model = await rig.load(url);
  castShadows(model);
  rig.scene.add(model);

  let png: Blob | null = null;
  try {
    drawModel(rig, model, template);
    checkPlausible(rig.renderer.domElement);
    png = await toPng(downsample(rig.renderer.domElement));
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
  radius: number,
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

  camera.position.copy(target).addScaledVector(dir, distance);
  camera.near = Math.max(distance / 100, 0.01);
  camera.far = distance * 4;
  camera.lookAt(target);
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
  /** Monte ce que ce gabarit demande et que `draw` ne peut pas charger lui-même
   * — le miroir, dont l'import est asynchrone. À appeler avant de dessiner un
   * gabarit qu'on n'a pas encore dessiné. */
  prepare(template: GridTemplate): Promise<void>;
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
    async prepare(template) {
      // Le miroir se monte hors du dessin : `draw` est synchrone — c'est ce qui
      // lui permet de suivre un curseur — et un `import()` ne l'est pas.
      if (template.reflection > 0 && template.floor > 0) await ensureMirror(rig);
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
  let min = 255;
  let max = 0;
  for (let i = 0; i < data.length; i += 4) {
    if (data[i + 3] < 24) continue;
    opaque += 1;
    if (data[i] > 244 && data[i + 1] > 244 && data[i + 2] > 244) white += 1;
    const luma = (data[i] * 299 + data[i + 1] * 587 + data[i + 2] * 114) / 1000;
    if (luma < min) min = luma;
    if (luma > max) max = luma;
  }
  const pixels = probe.width * probe.height;
  // Le seuil est bas exprès : une monoplace vue de trois-quarts couvre peu de
  // cadre, et l'ombre de contact compte à peine. Sous 2 %, il n'y a rien.
  if (opaque < pixels * 0.02) throw new Error("rendu vide (contexte WebGL perdu ?)");
  if (white > opaque * 0.9) throw new Error("rendu saturé (textures manquantes ?)");
  // **Un fond cuit rend le contrôle du vide inopérant** : l'image est opaque
  // partout, donc « rien n'a été dessiné » ressemble à « tout va bien ». Ce qui
  // le trahit, c'est l'écart : un fond seul est un dégradé très doux, une
  // voiture y ajoute forcément des clairs et des sombres. Mesuré sur le seul
  // dégradé, l'écart reste sous une dizaine de niveaux.
  if (max - min < 12) throw new Error("rendu sans voiture (fond seul ?)");
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
