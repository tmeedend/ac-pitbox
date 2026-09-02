// Vignettes des corps de pilote (docs/SPEC-ecran-pilote.md §9.1).
//
// **Pourquoi un rendu 3D et pas une image plate**, contrairement aux trois
// autres galeries : un corps n'a pas d'échantillon plat qui veuille dire
// quelque chose. Sa texture de casque est partagée par tous les corps de la
// même époque, sa combinaison par tous les corps tout court — deux mannequins
// très différents donneraient exactement la même case. Ce qui les distingue
// est leur géométrie : forme de casque, HANS ou pas, carrure, visage.
//
// **Pourquoi c'est paresseux et sérialisé.** Produire une vignette la première
// fois, c'est convertir un mannequin (~1 s) puis le rendre. Il y en a 45 sur
// l'installation de référence : les demander toutes au chargement de l'écran
// bloquerait une minute de travail pour des cases que personne ne regarde. On
// n'en demande donc que ce qui devient visible, une à la fois, et l'écran se
// remplit au fil du défilement.
//
// **Un seul contexte WebGL pour toutes.** Un `WebGLRenderer` par case
// épuiserait la limite du navigateur (16 contextes en pratique) dès la
// première rangée. Celui-ci est créé à la première demande, gardé pour la
// session, et il ne rend jamais à l'écran : il produit un PNG.
//
// **Le PNG est gardé sur disque**, et ce n'était pas le premier choix. Le
// raisonnement initial — « la conversion est déjà en cache, refaire l'image
// depuis le `.glb` ne coûte rien » — est faux deux fois, et les deux se
// mesurent :
//
//  1. le `.glb` d'un mannequin pèse ~4 Mo, qu'il faut retélécharger et
//     reparser dans three.js à chaque fois : une seconde pour quarante-cinq
//     cases, pas des millisecondes ;
//  2. surtout, les 45 conversions écrivent ~180 Mo dans un pool d'aperçus qui
//     est déjà à son plafond de 2 Gio, donc chacune **évince** une entrée plus
//     ancienne — y compris les mannequins des vignettes précédentes, qu'il
//     faut alors reconvertir. Le cache se mangeait lui-même.
//
// Une vignette rangée à part, hors du plafond, coupe court aux deux : le
// `.glb` ne sert qu'une fois dans la vie d'un corps, et peut être évincé sans
// conséquence. Son identité est celle de l'entrée de cache du mannequin, donc
// elle se périme quand le mod change, sans invalidation à écrire.
import { convertFileSrc } from "@tauri-apps/api/core";
import { bodyThumbnail, prepareBodyPreview, saveBodyThumbnail, type DriverRig } from "./preview";
import type * as ThreeModule from "three";

/** Côté de la vignette, en pixels de rendu. Deux fois la taille d'affichage
 * (104 px) pour rester net sur un écran à haute densité. */
const SIZE = 208;

type Entry = { url: string } | { pending: true } | { failed: true };

const cache = $state<Record<string, Entry>>({});

/** Ce qu'on a pour ce corps, ou `null` s'il n'a pas encore été demandé.
 * Lecture réactive : la case se peint dès que le rendu tombe. */
export function bodyThumb(key: string): string | null {
  const entry = cache[key];
  return entry && "url" in entry ? entry.url : null;
}

/** La clé mêle la voiture : c'est elle qui pose le mannequin, donc deux
 * voitures ne donnent pas la même vignette du même corps. */
function keyOf(carId: string, bodyId: string): string {
  return carId + "|" + bodyId;
}

interface Job {
  carId: string;
  skinId: string | null;
  bodyId: string;
  key: string;
}

const queue: Job[] = [];
let running = false;

/**
 * Demande la vignette d'un corps, si elle n'est pas déjà faite ou en cours.
 *
 * Appelée quand une case entre dans le champ de vision, jamais au chargement
 * de la liste.
 */
export function requestBodyThumb(carId: string, skinId: string | null, bodyId: string): void {
  const key = keyOf(carId, bodyId);
  if (cache[key]) return;
  cache[key] = { pending: true };
  queue.push({ carId, skinId, bodyId, key });
  void drain();
}

async function drain(): Promise<void> {
  if (running) return;
  running = true;
  try {
    let job = queue.shift();
    while (job) {
      try {
        // Le disque d'abord, toujours : c'est un aller-retour de quelques
        // millisecondes contre une conversion.
        const stored = await bodyThumbnail(job.carId, job.skinId, job.bodyId);
        const url = stored ? convertFileSrc(stored) : await render(job);
        cache[job.key] = url ? { url } : { failed: true };
      } catch (e) {
        // Une vignette manquante n'est pas une panne : la case garde son nom,
        // et le corps reste parfaitement choisissable.
        console.error("driver: vignette de corps", job.bodyId, e);
        cache[job.key] = { failed: true };
      }
      job = queue.shift();
    }
  } finally {
    running = false;
  }
}

// --- Le moteur de rendu, monté une fois ------------------------------------

interface Engine {
  THREE: typeof ThreeModule;
  renderer: ThreeModule.WebGLRenderer;
  scene: ThreeModule.Scene;
  camera: ThreeModule.PerspectiveCamera;
  load: (url: string) => Promise<ThreeModule.Group>;
}

let engine: Promise<Engine> | null = null;

function ensureEngine(): Promise<Engine> {
  engine ??= (async () => {
    const THREE = await import("three");
    const { GLTFLoader } = await import("three/examples/jsm/loaders/GLTFLoader.js");
    const { showroomEnvironment } = await import("./components/detail/showroomEnvironment");

    // `preserveDrawingBuffer` : sans lui, `toDataURL` rend une image vide dès
    // que le navigateur a eu le temps de vider le tampon entre le rendu et la
    // lecture. C'est le piège classique du rendu hors écran.
    const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true, preserveDrawingBuffer: true });
    renderer.setPixelRatio(1);
    renderer.setSize(SIZE, SIZE, false);
    renderer.toneMapping = THREE.ACESFilmicToneMapping;
    renderer.outputColorSpace = THREE.SRGBColorSpace;

    const scene = new THREE.Scene();
    const pmrem = new THREE.PMREMGenerator(renderer);
    scene.environment = pmrem.fromScene(showroomEnvironment(THREE), 0.04).texture;
    // Mêmes lampes que le plateau : une vignette qui n'éclaire pas comme
    // l'essayage ne sert pas de repère vers lui.
    const key = new THREE.DirectionalLight(0xffffff, 2.4);
    key.position.set(-1.2, 1.4, 2.2);
    scene.add(key);
    const rim = new THREE.DirectionalLight(0xffffff, 0.9);
    rim.position.set(1.4, 0.8, -1.8);
    scene.add(rim);
    scene.add(new THREE.AmbientLight(0xffffff, 0.35));

    const camera = new THREE.PerspectiveCamera(30, 1, 0.02, 40);
    const loader = new GLTFLoader();
    return {
      THREE,
      renderer,
      scene,
      camera,
      load: async (url: string) => (await loader.loadAsync(url)).scene,
    };
  })();
  return engine;
}

async function render(job: Job): Promise<string | null> {
  const preview = await prepareBodyPreview(job.carId, job.skinId, job.bodyId);
  if (!preview) return null;

  const { THREE, renderer, scene, camera, load } = await ensureEngine();
  const model = await load(preview.url);
  scene.add(model);
  let png: Blob | null = null;
  try {
    frame(THREE, camera, preview.rig, model);
    renderer.render(scene, camera);
    png = await toPng(renderer.domElement);
  } finally {
    scene.remove(model);
    dispose(THREE, model);
  }
  if (!png) return null;

  // Rangée pour les fois suivantes, mais affichée tout de suite depuis la
  // mémoire : attendre l'écriture disque pour peindre une case déjà rendue
  // serait un aller-retour pour rien.
  const bytes = new Uint8Array(await png.arrayBuffer());
  void saveBodyThumbnail(job.carId, job.skinId, job.bodyId, bytes).catch((e) =>
    console.error("driver: vignette non rangée", job.bodyId, e),
  );
  return URL.createObjectURL(png);
}

function toPng(canvas: HTMLCanvasElement): Promise<Blob | null> {
  return new Promise((resolve) => canvas.toBlob(resolve, "image/png"));
}

/**
 * Cadrage de la vignette : buste, du haut du casque à la poitrine.
 *
 * C'est là que se joue l'identité d'un mannequin — forme du casque, présence
 * d'un HANS, carrure. Descendre à la taille ne montrerait qu'une combinaison
 * que tous partagent, et monter au seul casque perdrait le reste.
 */
function frame(
  THREE: typeof ThreeModule,
  camera: ThreeModule.PerspectiveCamera,
  rig: DriverRig,
  model: ThreeModule.Group,
): void {
  const head = rig.head ? new THREE.Vector3(...rig.head) : null;
  const hips = rig.hips ? new THREE.Vector3(...rig.hips) : null;
  const box = new THREE.Box3().setFromObject(model);
  const target =
    head && hips ? head.clone().lerp(hips, 0.32) : box.getCenter(new THREE.Vector3());
  const radius = head && hips ? 0.34 : box.getSize(new THREE.Vector3()).length() / 2;

  // Trois-quarts avant gauche, comme le plateau et comme les photos du jeu.
  const azimuth = -0.42;
  const elevation = 0.1;
  const distance = (radius * 1.2) / Math.tan((camera.fov * Math.PI) / 360);
  camera.position.set(
    target.x + distance * Math.sin(azimuth) * Math.cos(elevation),
    target.y + distance * Math.sin(elevation),
    target.z + distance * Math.cos(azimuth) * Math.cos(elevation),
  );
  camera.lookAt(target);
  camera.updateProjectionMatrix();
}

function dispose(THREE: typeof ThreeModule, model: ThreeModule.Group): void {
  model.traverse((object) => {
    const mesh = object as ThreeModule.Mesh;
    if (!mesh.isMesh) return;
    mesh.geometry.dispose();
    for (const raw of Array.isArray(mesh.material) ? mesh.material : [mesh.material]) {
      const material = raw as ThreeModule.MeshStandardMaterial;
      material.map?.dispose();
      material.dispose();
    }
  });
}
