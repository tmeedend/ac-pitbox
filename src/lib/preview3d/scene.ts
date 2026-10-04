// The three.js scene of the car preview (docs/SPEC-preview-3d-kn5.md
// PREVIEW§8, PREVIEW§15): building it around a converted model, the render
// loop and its turntable, the camera placed from the settings, and freeing
// every byte of GPU memory when it goes. `CarPreview3D.svelte` keeps what is
// the component's — which model to load and when, the swap of a skin, the
// badge — and drives this module from its effects.
//
// three.js itself only ever arrives through `import()` in `build`: the
// biggest dependency of the front must weigh neither on the app's start nor
// on the screens that show no preview. The imports below are types only, or
// modules that import three as types only.
import type * as ThreeModule from "three";
import type { Reflector } from "three/addons/objects/Reflector.js";
import { reportListenerAngle } from "$lib/detail/enginePlayer.svelte";
import { applyFloorMirror } from "./floorMirror";
import { addFloor, attachMirror, sizeMirror } from "./floor";
import { prepareMaterials } from "./materials";
import { preview3dPrefs } from "./preview3dPrefs.svelte";
import { applyDriverOpacity, applySteer, findRig, stepDriverFade, type Rig } from "./rig";
import {
  FRAMING_DISTANCE,
  FRAMING_FOV,
  INTRO_LAUNCH_BOOST,
  INTRO_LAUNCH_MS,
  INTRO_LAUNCH_TAU_MS,
  INTRO_RAMP_MS,
  QUALITY,
  SPIN_SPEED,
  TUNING,
} from "./tuning";

/** What a scene asks of the component that shows it. **One per component,
 * shared by every scene it builds**: two coexist while a skin changes, and a
 * stand the user stopped by taking hold of the car stays stopped on the next
 * livery. */
export interface StageHost {
  /** The stand turns until the user takes hold of the car. */
  spinning: boolean;
  /** The panel is in view: scrolled out, there is nothing to draw. */
  onScreen: boolean;
  /** Should the driver show **now** (a mode, and the ignition key). */
  driverWanted(): boolean;
}

export interface ThreeScene extends Rig {
  THREE: typeof ThreeModule;
  renderer: ThreeModule.WebGLRenderer;
  scene: ThreeModule.Scene;
  camera: ThreeModule.PerspectiveCamera;
  controls: {
    update(): boolean;
    dispose(): void;
    addEventListener(type: string, listener: () => void): void;
    /** Point visé, déplacé verticalement par le réglage de hauteur. */
    target: ThreeModule.Vector3;
    /** Angle d'orbite, en radians depuis +Z vers +X — la même convention que
     * le cadrage par réglages plus bas, et que l'oreille côté son. */
    getAzimuthalAngle(): number;
    /** Angle depuis +Y : 90° = horizon. */
    getPolarAngle(): number;
  };
  pmrem: ThreeModule.PMREMGenerator;
  /** Le plateau : la voiture y est posée, l'ombre de contact non — un socle
   * de salon tourne sous la voiture, il n'emporte pas son ombre. */
  turntable: ThreeModule.Group;
  /** Centre et rayon du modèle, gardés pour recalculer le cadrage quand un
   * réglage change, sans reconstruire la scène. */
  center: ThreeModule.Vector3;
  radius: number;
  /** Boucle de rendu et observateurs **par scène** : deux aperçus coexistent
   * le temps d'un changement de skin (l'ancien tourne pendant que le nouveau
   * se construit), et une image en vol ou un ResizeObserver partagés
   * laisseraient l'un des deux figé ou abandonné derrière l'autre. */
  frame: number;
  lastFrameAt: number;
  observer: ResizeObserver | null;
  visibility: IntersectionObserver | null;
  /** Recale renderer, chaîne de post-traitement et caméra sur la taille du
   * conteneur. Portée par la scène pour qu'un changement de qualité puisse
   * la rejouer depuis l'extérieur de `build`. */
  resize: () => void;
  /** Le conteneur : sa taille décide du budget de pixels, qui se recalcule à
   * chaque changement de niveau. */
  host: HTMLElement;
  /** Horodatage du début de l'effet d'entrée, 0 quand il n'y en a pas ou
   * qu'il est terminé (PREVIEW§15 — effet d'intro). */
  introAt: number;
  /** Miroir du sol. `null` quand le reflet est à 0 : c'est un second rendu
   * de la scène, autant ne pas le construire du tout. */
  mirror: Reflector | null;
  /** Les deux plans réglables du sol, gardés pour leur appliquer les
   * préférences sans reconstruire la scène. */
  ground: ThreeModule.Mesh;
  shadowCatcher: ThreeModule.Mesh;
  /** Altitude du sol, pour poser le miroir quand il arrive après coup. */
  floorY: number;
  /** Ce qui fait avancer ou arrêter le plateau, partagé avec le composant
   * (`StageHost`). */
  stage: StageHost;
  /** Libérée : plus rien ne doit lui demander de rendu (une reprise de
   * rotation en attente, par exemple, survit à la scène qui l'a armée). */
  disposed: boolean;
}

/** Ce qu'un aperçu à l'écran transmet à son remplaçant quand seul le skin
 * change : sa scène est identique au triangle près, donc la reprendre en
 * l'état est exactement ce qui rend le changement de skin fluide. */
interface Carry {
  rotationY: number;
  position: ThreeModule.Vector3;
  target: ThreeModule.Vector3;
}

/** Le niveau de qualité courant. Passe par une fonction : lu à chaque
 * construction et à chaque changement de réglage, jamais capturé. */
function quality() {
  return QUALITY[preview3dPrefs().quality];
}

/**
 * Applies the current level's oversampling.
 *
 * The factor is **always the screen density times a whole number**, for the
 * reason spelled out where `QUALITY` is declared: the compositor reduces the
 * canvas with a single bilinear tap, and only a reduction of exactly two
 * lands where that tap averages four texels instead of skipping most of them.
 *
 * Which is also why the budget steps the *level* down rather than clamping
 * the factor. Clamping would hand back a fractional reduction — the very
 * defect this function exists to avoid — so the area is measured in physical
 * pixels, before oversampling, and only whole steps are ever taken.
 */
function applyPixelRatio(renderer: ThreeModule.WebGLRenderer, host: HTMLElement) {
  const density = window.devicePixelRatio || 1;
  const area = host.clientWidth * host.clientHeight * density * density;
  let oversampling: number = quality().oversampling;
  while (oversampling > 1 && area * oversampling * oversampling > TUNING.drawingPixels) {
    oversampling -= 1;
  }
  renderer.setPixelRatio(density * oversampling);
}

/**
 * Reporte sur la scène tout ce qui se règle sans la reconstruire : le décor
 * (exposition, éclairage) et le sol (reflet, flaque, ombre).
 *
 * Une seule fonction pour la construction et pour les changements de
 * réglage — deux chemins auraient divergé au premier ajout, et c'est
 * exactement ce qui rend un réglage « qui ne marche que si on rouvre la
 * fiche ».
 */
export function applyScene(current: ThreeScene) {
  const prefs = preview3dPrefs();
  current.renderer.toneMappingExposure = prefs.exposure / 100;
  // `scene.environmentIntensity`, et **pas** `material.envMapIntensity` :
  // mesuré au banc, ce dernier n'a aucun effet quand l'environnement vient
  // de la scène (voir `docs/SPEC-preview-3d-kn5.md` PREVIEW§15).
  current.scene.environmentIntensity = prefs.light / 100;
  const ground = current.ground.material as ThreeModule.MeshBasicMaterial;
  ground.opacity = prefs.pool / 100;
  const shadow = current.shadowCatcher.material as ThreeModule.ShadowMaterial;
  shadow.opacity = prefs.shadow / 100;
  if (current.mirror) {
    const material = current.mirror.material as ThreeModule.ShaderMaterial;
    applyFloorMirror(material.uniforms, prefs, current.mirror.getRenderTarget().width);
  }
}

/**
 * Facteur appliqué à la vitesse du plateau pendant l'effet d'entrée (PREVIEW§15).
 *
 * Se désarme lui-même en écrivant `introAt = 0` : une fois l'effet fini, il
 * ne reste aucun calcul par image, et la boucle de rendu retrouve exactement
 * le code qu'elle avait avant ce réglage.
 */
function introFactor(current: ThreeScene, now: number): number {
  if (!current.introAt) return 1;
  const elapsed = now - current.introAt;
  const mode = preview3dPrefs().intro;
  if (mode === "ramp" && elapsed < INTRO_RAMP_MS) {
    // Lissage en S : démarrer linéairement se voit — la voiture part d'un
    // coup à vitesse faible au lieu de s'ébranler.
    const x = elapsed / INTRO_RAMP_MS;
    return x * x * (3 - 2 * x);
  }
  if (mode === "launch" && elapsed < INTRO_LAUNCH_MS) {
    return 1 + INTRO_LAUNCH_BOOST * Math.exp(-elapsed / INTRO_LAUNCH_TAU_MS);
  }
  current.introAt = 0;
  return 1;
}

/** Arme l'effet d'entrée sur la scène donnée, si les réglages en veulent un.
 * Un plateau à l'arrêt n'en reçoit pas : il n'y a rien à lancer. */
export function armIntro(current: ThreeScene) {
  const prefs = preview3dPrefs();
  current.introAt =
    REDUCED_MOTION || prefs.intro === "none" || prefs.spin === 0 ? 0 : performance.now();
}

/** Une préférence système « moins d'animations » désactive le plateau : une
 * rotation permanente est exactement ce qu'elle demande d'éviter. */
export const REDUCED_MOTION =
  typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches;

/**
 * WebGL indisponible = repli silencieux sur la photo (PREVIEW§8.5) : ce n'est pas
 * une erreur à signaler, c'est une machine qui ne peut pas afficher de 3D.
 */
export function webglAvailable(): boolean {
  try {
    const probe = document.createElement("canvas");
    return !!(probe.getContext("webgl2") ?? probe.getContext("webgl"));
  } catch {
    return false;
  }
}

/**
 * Libère tout ce qui occupe la mémoire GPU (PREVIEW§8.3).
 *
 * Première cause de plantage de ce genre de composant : l'utilisateur
 * parcourt deux cents voitures, chacune laissant ses géométries et ses
 * textures derrière elle. Appelée au démontage **et** à chaque changement de
 * voiture, sans exception.
 */
export function disposeScene(current: ThreeScene | null) {
  if (!current || current.disposed) return;
  current.disposed = true;
  if (current.frame) cancelAnimationFrame(current.frame);
  current.frame = 0;
  current.lastFrameAt = 0;
  current.observer?.disconnect();
  current.observer = null;
  current.visibility?.disconnect();
  current.visibility = null;

  current.scene.traverse((object) => {
    const mesh = object as ThreeModule.Mesh;
    mesh.geometry?.dispose?.();
    const material = mesh.material as ThreeModule.Material | ThreeModule.Material[] | undefined;
    for (const m of Array.isArray(material) ? material : material ? [material] : []) {
      // Les textures ne sont pas libérées par `material.dispose()` : il faut
      // parcourir ses propriétés pour les attraper une par une.
      for (const value of Object.values(m)) {
        if (value && typeof value === "object" && "isTexture" in value) {
          (value as ThreeModule.Texture).dispose();
        }
      }
      m.dispose();
    }
  });
  // Avant le parcours : le miroir possède une cible de rendu que ni
  // `geometry.dispose()` ni `material.dispose()` ne libèrent.
  current.mirror?.dispose();
  current.mirror = null;
  current.scene.clear();
  current.controls.dispose();
  current.pmrem.dispose();
  current.renderer.dispose();
  // Rend explicitement le contexte WebGL : les navigateurs en limitent le
  // nombre simultané, et attendre le ramasse-miettes suffit à l'épuiser.
  current.renderer.forceContextLoss();
  current.renderer.domElement.remove();
}

/** Le plateau tourne-t-il en ce moment ? Quatre conditions, toutes
 * nécessaires — vitesse nulle comprise, sinon la boucle de rendu
 * continuerait à tourner pour ne rien déplacer. */
export function turning(current: ThreeScene): boolean {
  return current.stage.spinning && current.stage.onScreen && !document.hidden && preview3dPrefs().spin > 0;
}

/**
 * Dit à l'écoute moteur où se trouve l'oreille, si elle joue.
 *
 * **L'angle qui compte est celui de la caméra dans le repère de la voiture,
 * pas dans celui de la scène.** C'est le plateau qui tourne ici, pas la
 * caméra (voir la boucle de rendu) : sans retrancher sa rotation, le son ne
 * changerait pas d'un pouce pendant que la voiture pivote sur son socle —
 * exactement le moment où il devrait.
 *
 * L'appel part à chaque image ; c'est `enginePlayer` qui décide de l'envoyer
 * ou non, parce que lui seul sait si quelque chose est spatialisable.
 */
function reportEar(current: ThreeScene) {
  const camera = (current.controls.getAzimuthalAngle() * 180) / Math.PI;
  const car = (current.turntable.rotation.y * 180) / Math.PI;
  const azimuth = (((camera - car) % 360) + 360) % 360;
  const elevation = 90 - (current.controls.getPolarAngle() * 180) / Math.PI;
  // Les modèles AC sont en mètres. Borné : la courbe d'atténuation du jeu est
  // faite pour des distances de piste, et un zoom arrière complet finirait
  // par ne plus rien laisser entendre.
  const distance = Math.min(Math.max(current.camera.position.distanceTo(current.controls.target), 1.5), 15);
  reportListenerAngle(azimuth, elevation, distance);
}

/**
 * Une image. La boucle ne se prolonge que si quelque chose bouge encore :
 * le plateau, ou l'inertie d'OrbitControls après un lâcher de souris.
 */
export function requestRender(current: ThreeScene) {
  if (current.disposed || current.frame) return;
  current.frame = requestAnimationFrame((now) => {
    current.frame = 0;
    if (current.disposed) return;
    // Avance en fonction du temps écoulé, pas du nombre d'images : la vitesse
    // ne doit pas dépendre du taux de rafraîchissement de l'écran, sinon un
    // moniteur 144 Hz fait tourner la voiture deux fois plus vite.
    const elapsed = current.lastFrameAt ? Math.min((now - current.lastFrameAt) / 1000, 0.1) : 0;
    current.lastFrameAt = now;
    if (turning(current) && elapsed > 0) {
      // C'est la voiture qui tourne, pas la caméra : le cadrage reste
      // parfaitement stable, les reflets glissent sur la carrosserie, et
      // rien ne vient contrarier l'état interne d'OrbitControls quand
      // l'utilisateur prend la main.
      const intro = introFactor(current, now);
      current.turntable.rotation.y += SPIN_SPEED * (preview3dPrefs().spin / 100) * intro * elapsed;
    }
    const fading = stepDriverFade(current, elapsed);
    const moving = current.controls.update();
    reportEar(current);
    current.renderer.render(current.scene, current.camera);
    // Rien à ajouter pour l'effet d'entrée : il ne fait qu'accélérer un
    // plateau qui tourne, donc `turning()` le couvre déjà. L'ajouter ici
    // ferait tourner la boucle dans le vide si la vitesse passait à 0 en
    // cours d'effet — l'effet resterait armé, plus rien ne bougerait, et le
    // panneau redemanderait une image soixante fois par seconde.
    if (moving || turning(current) || fading) requestRender(current);
    else current.lastFrameAt = 0;
  });
}

/**
 * Pose la caméra d'après les réglages (PREVIEW§15) : distance, angle autour de
 * l'axe vertical, hauteur. Par défaut un trois-quarts avant, l'angle le plus
 * flatteur pour une voiture.
 *
 * Séparée de `build` parce qu'elle sert deux fois : à la construction, et à
 * chaque changement de réglage — recadrer ne demande pas de reconstruire la
 * scène, et surtout pas de reconvertir le modèle.
 */
export function placeCamera(current: ThreeScene) {
  const prefs = preview3dPrefs();
  // La focale change la perspective **sans** changer la taille de la voiture
  // dans le cadre : la distance est recalculée pour compenser. Sans ça, les
  // curseurs de focale et de zoom se marcheraient dessus, et le premier
  // servirait surtout à recadrer — alors que c'est le second qui recadre.
  const compensation =
    Math.tan((FRAMING_FOV * Math.PI) / 360) / Math.tan((prefs.fov * Math.PI) / 360);
  const distance = (current.radius * FRAMING_DISTANCE * 100 * compensation) / prefs.zoom;
  if (current.camera.fov !== prefs.fov) {
    current.camera.fov = prefs.fov;
    current.camera.updateProjectionMatrix();
  }
  const azimuth = (prefs.azimuth * Math.PI) / 180;
  const elevation = (prefs.elevation * Math.PI) / 180;
  // Le point visé monte ou descend avec la hauteur : c'est lui qui décide de
  // la place de la voiture dans le cadre, alors que la plongée décide de ce
  // qu'on voit de son toit. Les deux se règlent séparément.
  const targetY = current.center.y + current.radius * (prefs.height / 100);
  current.controls.target.set(current.center.x, targetY, current.center.z);
  current.camera.position.set(
    current.center.x + distance * Math.cos(elevation) * Math.sin(azimuth),
    targetY + distance * Math.sin(elevation),
    current.center.z + distance * Math.cos(elevation) * Math.cos(azimuth),
  );
  current.controls.update();
  requestRender(current);
}

/**
 * Construit la scène et l'ajoute au conteneur.
 *
 * `carry` est évalué **juste avant la première image**, pas à l'appel : la
 * scène qu'on remplace tourne encore pendant toute la conversion, et lire sa
 * rotation trop tôt ferait sauter la voiture en arrière au moment du
 * remplacement.
 */
export async function build(
  url: string,
  host: HTMLDivElement,
  stage: StageHost,
  carry: (() => Carry | null) | null = null,
): Promise<ThreeScene> {
  const THREE = await import("three");
  const { GLTFLoader } = await import("three/examples/jsm/loaders/GLTFLoader.js");
  const { OrbitControls } = await import("three/examples/jsm/controls/OrbitControls.js");
  const { showroomEnvironment } = await import("./showroomEnvironment");

  const renderer = new THREE.WebGLRenderer({ antialias: true, powerPreference: "high-performance" });
  // Suréchantillonner puis réduire est le remède direct au scintillement des
  // reflets, et le panneau est assez petit pour qu'on puisse se le payer. Le
  // facteur vient du niveau de qualité (PREVIEW§15) — c'était 1,5 à 2 avant lui.
  applyPixelRatio(renderer, host);
  renderer.toneMapping = THREE.ACESFilmicToneMapping;
  renderer.outputColorSpace = THREE.SRGBColorSpace;
  renderer.shadowMap.enabled = true;
  renderer.shadowMap.type = THREE.PCFSoftShadowMap;

  const scene = new THREE.Scene();
  // Image-based lighting, and no asset to ship for it (PREVIEW§8.1). The showroom is
  // dark on purpose — see `showroomEnvironment` for what a white room did to
  // the paint.
  const pmrem = new THREE.PMREMGenerator(renderer);
  scene.environment = pmrem.fromScene(showroomEnvironment(THREE), TUNING.environmentBlur).texture;

  const gltf = await new GLTFLoader().loadAsync(url);
  prepareMaterials(gltf.scene, renderer);

  // Cadrage calculé, jamais codé en dur : les mods ont des échelles très
  // variables et un cadrage fixe en couperait la moitié (PREVIEW§8.1).
  const box = new THREE.Box3().setFromObject(gltf.scene);
  const center = box.getCenter(new THREE.Vector3());
  const radius = box.getSize(new THREE.Vector3()).length() / 2;

  // Plateau centré sous la voiture : le modèle est décalé pour que l'axe de
  // rotation passe par son centre, sinon elle décrirait un cercle au lieu de
  // pivoter sur elle-même. Les positions dans le monde ne changent pas.
  const turntable = new THREE.Group();
  turntable.position.set(center.x, 0, center.z);
  gltf.scene.position.set(-center.x, 0, -center.z);
  turntable.add(gltf.scene);
  scene.add(turntable);

  const camera = new THREE.PerspectiveCamera(FRAMING_FOV, 16 / 9, radius / 100, radius * 90);

  const controls = new OrbitControls(camera, renderer.domElement);
  controls.target.copy(center);
  controls.enableDamping = true;
  controls.dampingFactor = 0.08;
  // **Clic droit : déplacer.** Le bouton gauche fait tourner, la molette
  // rapproche, et il manquait de quoi décentrer — sur une voiture longue,
  // aucun réglage ne permettait de venir regarder une roue de près. C'est le
  // partage par défaut d'`OrbitControls` (gauche = orbite, droit =
  // déplacement), qui supprime aussi le menu contextuel du bouton droit
  // pendant le geste.
  controls.enablePan = true;
  // Dans le plan de l'écran, et non dans celui du sol : l'utilisateur demande
  // X et Y, pas « avance et recule ». C'est déjà le défaut de three, écrit
  // ici parce que la nuance ne se devine pas au nom du réglage.
  controls.screenSpacePanning = true;
  controls.minDistance = radius * 1.1;
  // Assez large pour toute la plage des réglages, sinon la borne annulerait
  // le réglage en silence dès la première image. Le pire cas cumule le zoom
  // le plus faible (50 %) et la focale la plus longue (10°, soit deux fois
  // plus loin qu'à 20°) : environ vingt rayons.
  controls.maxDistance = radius * 26;
  // Borne l'angle polaire pour qu'on ne puisse pas passer sous le sol.
  controls.maxPolarAngle = Math.PI * 0.495;

  const { ground, shadowCatcher } = addFloor(THREE, scene, box, center, radius);
  const rig = findRig(THREE, gltf, stage.driverWanted());

  host.appendChild(renderer.domElement);
  const built: ThreeScene = {
    THREE,
    renderer,
    scene,
    camera,
    controls,
    pmrem,
    turntable,
    center,
    radius,
    frame: 0,
    lastFrameAt: 0,
    observer: null,
    visibility: null,
    resize: () => {},
    host,
    mirror: null,
    ground,
    shadowCatcher,
    floorY: box.min.y,
    introAt: 0,
    ...rig,
    stage,
    disposed: false,
  };
  applyDriverOpacity(built, built.driverOpacity);
  // Avant le cadrage : la boîte englobante se calcule ensuite, et une roue
  // braquée déborde un peu.
  applySteer(built);
  // Reprise de la scène précédente, ou cadrage réglé si on part de zéro.
  const carried = carry?.();
  if (carried) {
    turntable.rotation.y = carried.rotationY;
    camera.position.copy(carried.position);
    controls.target.copy(carried.target);
    controls.update();
  } else {
    placeCamera(built);
  }

  const resize = () => {
    const width = host.clientWidth;
    const height = host.clientHeight;
    if (width === 0 || height === 0) return;
    // Le budget de pixels dépend de la taille du panneau : il se recalcule
    // ici, sinon agrandir la fenêtre garderait le facteur d'avant et ferait
    // sauter le plafond mémoire au lieu de le respecter.
    applyPixelRatio(renderer, host);
    renderer.setSize(width, height, false);
    camera.aspect = width / height;
    camera.updateProjectionMatrix();
    // Après `setSize` : le miroir se cale sur le tampon, qui vient seulement
    // d'être redimensionné. C'est aussi ce qui le fait suivre quand le niveau
    // de qualité change — `applyQuality` rejoue `resize`.
    sizeMirror(built);
    requestRender(built);
  };
  await attachMirror(built);
  applyScene(built);

  built.resize = resize;
  built.observer = new ResizeObserver(resize);
  built.observer.observe(host);
  resize();

  controls.addEventListener("change", () => requestRender(built));
  // Première image, puis la boucle s'entretient tant que le plateau tourne.
  requestRender(built);
  // **La main de l'utilisateur prime, et définitivement** : le plateau
  // s'arrête à la prise et ne repart pas tout seul. Il repartait après
  // quelques secondes d'inactivité, et c'était une gêne plutôt qu'un
  // service — on règle un cadrage en regardant la voiture, et elle se
  // remettait à tourner sous les doigts, deux écrans concernés. Le bouton de
  // remise en place, lui, la relance quand on le veut.
  controls.addEventListener("start", () => {
    stage.spinning = false;
  });

  // Fiche sortie de l'écran par le scroll : plus rien à rendre.
  built.visibility = new IntersectionObserver((entries) => {
    stage.onScreen = entries.some((e) => e.isIntersecting);
    if (turning(built)) requestRender(built);
  });
  built.visibility.observe(host);

  return built;
}

/**
 * Applique un niveau de qualité à la scène en place, sans la reconstruire :
 * rien de ce que le niveau change ne dépend du modèle, et recharger coûterait
 * le retour à la photo pour finir sur la même voiture.
 */
export function applyQuality(current: ThreeScene | null) {
  if (!current || current.disposed) return;
  applyPixelRatio(current.renderer, current.host);
  current.resize();
  requestRender(current);
}
