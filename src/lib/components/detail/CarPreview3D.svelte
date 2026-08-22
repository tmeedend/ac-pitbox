<script lang="ts">
  // Aperçu 3D interactif d'une voiture (docs/SPEC-preview-3d-kn5.md §8).
  //
  // Le modèle est converti côté Rust en glTF binaire, mis en cache et servi
  // par le protocole `carpreview` : ici on ne reçoit qu'une URL, jamais les
  // octets (§7.2).
  //
  // three.js est chargé en `import()` dynamique : c'est de loin la plus grosse
  // dépendance du front, et elle ne doit peser ni au démarrage de l'app ni sur
  // les écrans qui n'affichent aucun aperçu.
  import { onDestroy, untrack } from "svelte";
  import { prepareCarPreview, onPreviewProgress, type PreviewStage } from "$lib/preview";
  import {
    preview3dPrefs,
    preview3dReady,
    preview3dResets,
    type PreviewQuality,
  } from "$lib/preview3dPrefs.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { errorText } from "$lib/errors";
  import type * as ThreeModule from "three";
  import type { EffectComposer } from "three/examples/jsm/postprocessing/EffectComposer.js";

  let {
    carId,
    skinId = null,
    fallbackSrc = null,
  }: { carId: string; skinId?: string | null; fallbackSrc?: string | null } = $props();

  type Phase = "loading" | "ready" | "unavailable";

  let phase = $state<Phase>("loading");
  let stage = $state<PreviewStage | null>(null);
  /** Clé i18n ou message technique, affiché en infobulle du badge (§8.5). */
  let reason = $state<string | null>(null);
  /** Un nouveau skin se prépare pendant que le précédent reste à l'écran :
   * le badge doit le dire, mais l'aperçu ne bascule pas pour autant. */
  let swapping = $state(false);
  let canvasHost = $state<HTMLDivElement | null>(null);

  /** Tout ce qui doit être libéré : vit hors des runes, ce n'est pas de l'état
   * d'affichage et le rendre réactif ne ferait que déclencher des effets. */
  let scene: ThreeScene | null = null;
  /** Couple voiture/skin réellement en place, pour ne pas reconstruire une
   * scène identique. Vide tant que rien n'est chargé. */
  let loaded = "";
  /** Voiture du modèle en place — la moitié de `loaded` qui décide si un
   * changement de skin peut se faire à chaud (même géométrie) ou non. */
  let loadedCar = "";

  interface ThreeScene {
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
    /** Chaîne de post-traitement, quand la qualité demande SMAA. `null` sinon,
     * et le rendu passe alors directement par le renderer. */
    composer: EffectComposer | null;
    /** Horodatage du début de l'effet d'entrée, 0 quand il n'y en a pas ou
     * qu'il est terminé (§15 — effet d'intro). */
    introAt: number;
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

  // Plateau tournant, comme un socle de salon : c'est la raison d'être de tout
  // ce chantier — voir la voiture tourner, pas seulement pouvoir la tourner.
  //
  // Le §8.4 de la spec demande l'inverse (« ne pas rendre en continu à 60 fps
  // sur un panneau statique ») et les deux sont inconciliables. La contrepartie
  // est donc payée là où elle se voit : la rotation s'arrête dès que la fiche
  // quitte l'écran, que la fenêtre passe en arrière-plan, ou que l'utilisateur
  // attrape le modèle — un panneau qu'on ne regarde pas ne consomme rien.

  /** Un tour en ~28 s à 100 % : assez lent pour être calme, assez vif pour
   * qu'on voie les reflets glisser sur la carrosserie. */
  const SPIN_SPEED = 0.22;

  // Lens and framing, measured against Kunos' `preview.jpg` (§15 point 7).
  //
  // A 20° field of view rather than the 35° first used: at 35° the nose of a
  // car looms and its tail falls away, a distortion the game's own previews do
  // not have. The distance that follows is what puts the car at the size Kunos
  // frames it — the two go together, since a longer lens has to step back.
  const FRAMING_FOV = 20;
  /** Camera distance at zoom 100 %, in multiples of the model's radius. */
  const FRAMING_DISTANCE = 4.9;
  /** Reprise après un lâcher de souris. Assez long pour examiner un détail
   * sans que le plateau ne redémarre sous les doigts. */
  const SPIN_RESUME_MS = 4000;

  // Qualité de rendu (§15). Ne touche **que** l'affichage : aucun de ces
  // réglages n'entre dans la conversion, donc en changer n'invalide aucune
  // entrée de cache et s'applique à l'image suivante.
  //
  // Les deux leviers ne traitent pas le même défaut, et c'est pour ça qu'il
  // en faut deux. Le suréchantillonnage augmente le **taux d'ombrage** : c'est
  // le seul qui attaque le scintillement d'un reflet plus fin qu'un pixel sur
  // une carrosserie, que le MSAA ne voit pas (il échantillonne la couverture
  // des triangles, mais n'ombre qu'une fois par pixel). SMAA, lui, travaille
  // sur l'image finie et rattrape les marches d'escalier qui restent, y
  // compris sur une géométrie sous-pixel — les lames d'une calandre.
  const QUALITY = {
    /** Ce que faisait l'app avant ce réglage. */
    standard: { pixelRatio: 2, smaa: false },
    high: { pixelRatio: 2.5, smaa: true },
    ultra: { pixelRatio: 3, smaa: true },
  } as const;

  /** Plancher de suréchantillonnage, même sur un écran à 1 dpi : le MSAA
   * échantillonne les bords, pas l'intérieur des surfaces, et sur une
   * carrosserie lisse ce sont les reflets qui scintillent. */
  const MIN_PIXEL_RATIO = 1.5;

  // Effet d'entrée du plateau (§15). Deux gestes, et rien d'autre qu'un
  // facteur appliqué à la vitesse déjà calculée : aucune image de plus, aucun
  // coût GPU.
  /** Montée en douceur jusqu'à la vitesse réglée. */
  const INTRO_RAMP_MS = 1200;
  /** Départ lancé : la voiture part à `1 + BOOST` fois la vitesse réglée et
   * décroît vers elle. Durée choisie pour que le dernier dixième de l'écart
   * soit déjà imperceptible quand on coupe. */
  const INTRO_LAUNCH_MS = 2600;
  const INTRO_LAUNCH_BOOST = 4;
  const INTRO_LAUNCH_TAU_MS = 900;

  /** Le niveau de qualité courant. Passe par une fonction : lu à chaque
   * construction et à chaque changement de réglage, jamais capturé. */
  function quality() {
    return QUALITY[preview3dPrefs().quality];
  }

  /** Applique le suréchantillonnage du niveau courant. Le plancher reste en
   * place quel que soit le niveau : c'est lui qui traite le scintillement des
   * reflets, pas le confort d'affichage. */
  function applyPixelRatio(renderer: ThreeModule.WebGLRenderer) {
    renderer.setPixelRatio(Math.min(Math.max(window.devicePixelRatio, MIN_PIXEL_RATIO), quality().pixelRatio));
  }

  /**
   * Chaîne SMAA.
   *
   * La cible est **multi-échantillonnée à la main** (`samples: 4`), et c'est
   * le piège de tout le montage : dès qu'on passe par un `EffectComposer`, le
   * rendu ne va plus dans le tampon d'écran, donc l'`antialias: true` du
   * contexte ne s'applique plus à rien. Sans cette option, activer SMAA
   * *retirerait* le MSAA — un antialiasing échangé contre un autre au lieu des
   * deux cumulés.
   *
   * Ordre des passes : rendu → SMAA → sortie. SMAA **avant** `OutputPass`,
   * et non après comme on l'écrirait spontanément pour un filtre
   * morphologique : trois.js documente cette implémentation comme travaillant
   * en `linear-srgb` (en-tête de `SMAAPass.js`, r185). Placée en dernier, la
   * passe chercherait ses contours dans des valeurs déjà converties, ce pour
   * quoi ses seuils ne sont pas réglés.
   */
  async function buildComposer(
    THREE: typeof ThreeModule,
    renderer: ThreeModule.WebGLRenderer,
    scene: ThreeModule.Scene,
    camera: ThreeModule.PerspectiveCamera,
  ): Promise<EffectComposer> {
    const [{ EffectComposer }, { RenderPass }, { OutputPass }, { SMAAPass }] = await Promise.all([
      import("three/examples/jsm/postprocessing/EffectComposer.js"),
      import("three/examples/jsm/postprocessing/RenderPass.js"),
      import("three/examples/jsm/postprocessing/OutputPass.js"),
      import("three/examples/jsm/postprocessing/SMAAPass.js"),
    ]);
    const size = renderer.getSize(new THREE.Vector2());
    const ratio = renderer.getPixelRatio();
    const target = new THREE.WebGLRenderTarget(
      Math.max(Math.round(size.x * ratio), 1),
      Math.max(Math.round(size.y * ratio), 1),
      { type: THREE.HalfFloatType, samples: 4 },
    );
    const composer = new EffectComposer(renderer, target);
    composer.setPixelRatio(ratio);
    composer.addPass(new RenderPass(scene, camera));
    composer.addPass(new SMAAPass());
    composer.addPass(new OutputPass());
    return composer;
  }

  /** Libère une chaîne de post-traitement. `EffectComposer.dispose()` ne
   * s'occupe que de ses deux tampons : les passes gardent les leurs (SMAA en
   * a deux, plus ses textures de recherche), et personne ne les libérerait. */
  function disposeComposer(composer: EffectComposer | null) {
    if (!composer) return;
    for (const pass of composer.passes) pass.dispose?.();
    composer.dispose();
  }

  /**
   * Facteur appliqué à la vitesse du plateau pendant l'effet d'entrée (§15).
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
  function armIntro(current: ThreeScene) {
    const prefs = preview3dPrefs();
    current.introAt =
      reducedMotion || prefs.intro === "none" || prefs.spin === 0 ? 0 : performance.now();
  }

  /** Une préférence système « moins d'animations » désactive le plateau : une
   * rotation permanente est exactement ce qu'elle demande d'éviter. */
  const reducedMotion =
    typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches;

  let spinning = !reducedMotion;
  let onScreen = true;
  let resumeTimer: ReturnType<typeof setTimeout> | null = null;

  /** La scène en place à cet instant. Passe par une fonction parce que `build`
   * a sa propre `scene` — celle de three.js — qui masque celle-ci. */
  function liveScene(): ThreeScene | null {
    return scene;
  }

  /** Annule une reprise de rotation en attente. Séparée de `disposeScene` : un
   * changement de skin remplace la scène sans rien changer à ce que
   * l'utilisateur était en train de faire avec la souris. */
  function stopResume() {
    if (resumeTimer) clearTimeout(resumeTimer);
    resumeTimer = null;
  }

  /**
   * WebGL indisponible = repli silencieux sur la photo (§8.5) : ce n'est pas
   * une erreur à signaler, c'est une machine qui ne peut pas afficher de 3D.
   */
  function webglAvailable(): boolean {
    try {
      const probe = document.createElement("canvas");
      return !!(probe.getContext("webgl2") ?? probe.getContext("webgl"));
    } catch {
      return false;
    }
  }

  /**
   * Libère tout ce qui occupe la mémoire GPU (§8.3).
   *
   * Première cause de plantage de ce genre de composant : l'utilisateur
   * parcourt deux cents voitures, chacune laissant ses géométries et ses
   * textures derrière elle. Appelée au démontage **et** à chaque changement de
   * voiture, sans exception.
   */
  function disposeScene(current: ThreeScene | null) {
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
    current.scene.clear();
    disposeComposer(current.composer);
    current.composer = null;
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
  function turning(): boolean {
    return spinning && onScreen && !document.hidden && preview3dPrefs().spin > 0;
  }

  /**
   * Une image. La boucle ne se prolonge que si quelque chose bouge encore :
   * le plateau, ou l'inertie d'OrbitControls après un lâcher de souris.
   */
  function requestRender(current: ThreeScene) {
    if (current.disposed || current.frame) return;
    current.frame = requestAnimationFrame((now) => {
      current.frame = 0;
      if (current.disposed) return;
      // Avance en fonction du temps écoulé, pas du nombre d'images : la vitesse
      // ne doit pas dépendre du taux de rafraîchissement de l'écran, sinon un
      // moniteur 144 Hz fait tourner la voiture deux fois plus vite.
      const elapsed = current.lastFrameAt ? Math.min((now - current.lastFrameAt) / 1000, 0.1) : 0;
      current.lastFrameAt = now;
      if (turning() && elapsed > 0) {
        // C'est la voiture qui tourne, pas la caméra : le cadrage reste
        // parfaitement stable, les reflets glissent sur la carrosserie, et
        // rien ne vient contrarier l'état interne d'OrbitControls quand
        // l'utilisateur prend la main.
        const intro = introFactor(current, now);
        current.turntable.rotation.y += SPIN_SPEED * (preview3dPrefs().spin / 100) * intro * elapsed;
      }
      const moving = current.controls.update();
      if (current.composer) current.composer.render();
      else current.renderer.render(current.scene, current.camera);
      // Rien à ajouter pour l'effet d'entrée : il ne fait qu'accélérer un
      // plateau qui tourne, donc `turning()` le couvre déjà. L'ajouter ici
      // ferait tourner la boucle dans le vide si la vitesse passait à 0 en
      // cours d'effet — l'effet resterait armé, plus rien ne bougerait, et le
      // panneau redemanderait une image soixante fois par seconde.
      if (moving || turning()) requestRender(current);
      else current.lastFrameAt = 0;
    });
  }

  /**
   * Pose la caméra d'après les réglages (§15) : distance, angle autour de
   * l'axe vertical, hauteur. Par défaut un trois-quarts avant, l'angle le plus
   * flatteur pour une voiture.
   *
   * Séparée de `build` parce qu'elle sert deux fois : à la construction, et à
   * chaque changement de réglage — recadrer ne demande pas de reconstruire la
   * scène, et surtout pas de reconvertir le modèle.
   */
  function placeCamera(current: ThreeScene) {
    const prefs = preview3dPrefs();
    const distance = (current.radius * FRAMING_DISTANCE * 100) / prefs.zoom;
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
  async function build(
    url: string,
    host: HTMLDivElement,
    carry: (() => Carry | null) | null = null,
  ): Promise<ThreeScene> {
    const THREE = await import("three");
    const { GLTFLoader } = await import("three/examples/jsm/loaders/GLTFLoader.js");
    const { OrbitControls } = await import("three/examples/jsm/controls/OrbitControls.js");
    const { showroomEnvironment } = await import("./showroomEnvironment");

    const renderer = new THREE.WebGLRenderer({ antialias: true, powerPreference: "high-performance" });
    // Suréchantillonner puis réduire est le remède direct au scintillement des
    // reflets, et le panneau est assez petit pour qu'on puisse se le payer. Le
    // plafond vient du niveau de qualité (§15) — c'était 2 en dur avant lui.
    applyPixelRatio(renderer);
    renderer.toneMapping = THREE.ACESFilmicToneMapping;
    renderer.outputColorSpace = THREE.SRGBColorSpace;
    renderer.shadowMap.enabled = true;
    renderer.shadowMap.type = THREE.PCFSoftShadowMap;

    const scene = new THREE.Scene();
    // Image-based lighting, and no asset to ship for it (§8.1). The showroom is
    // dark on purpose — see `showroomEnvironment` for what a white room did to
    // the paint.
    const pmrem = new THREE.PMREMGenerator(renderer);
    scene.environment = pmrem.fromScene(showroomEnvironment(THREE), 0.04).texture;

    const gltf = await new GLTFLoader().loadAsync(url);

    // Filtrage anisotrope, au maximum de ce que la carte accepte (16 en
    // pratique). Le MSAA du contexte ne lisse que les **bords de géométrie** ;
    // le fourmillement d'une texture vue en biais — les décalcomanies d'une
    // portière, les rainures d'un pneu, le sol — vient du filtrage, et c'est
    // l'anisotropie qui le règle. Une ligne pour le gain le plus visible.
    const maxAnisotropy = renderer.capabilities.getMaxAnisotropy();

    // Les vitres passent après l'opaque et n'écrivent pas dans le tampon de
    // profondeur, sinon l'intérieur disparaît derrière le pare-brise (§8.2).
    gltf.scene.traverse((object) => {
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
        // Découpes en alpha (calandres, jantes ajourées, grillages) : leur bord
        // est décidé par un seuil, donc le MSAA ne le voit pas — il ne lisse
        // que la silhouette du triangle, pas le trou qu'on y perce. Reporté sur
        // la couverture des échantillons, ce bord retrouve le même adoucissement
        // que le reste.
        const standard = material as ThreeModule.MeshStandardMaterial;
        if (standard.alphaTest > 0) {
          standard.alphaToCoverage = true;
        }
      }
      if (materials.some((m) => (m as ThreeModule.Material).transparent)) {
        mesh.renderOrder = 1;
        for (const m of materials) (m as ThreeModule.Material).depthWrite = false;
      } else {
        // Only the opaque body casts: a windscreen that casts a shadow map
        // casts it solid black, and the car ends up sitting on a dark blob.
        mesh.castShadow = true;
      }
    });

    // Cadrage calculé, jamais codé en dur : les mods ont des échelles très
    // variables et un cadrage fixe en couperait la moitié (§8.1).
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

    const camera = new THREE.PerspectiveCamera(FRAMING_FOV, 16 / 9, radius / 100, radius * 40);

    const controls = new OrbitControls(camera, renderer.domElement);
    controls.target.copy(center);
    controls.enableDamping = true;
    controls.dampingFactor = 0.08;
    controls.enablePan = false;
    controls.minDistance = radius * 1.1;
    // Roomy enough for the whole zoom range of the settings: at 50 % the camera
    // sits at nearly ten radii, and a tighter bound would silently cancel the
    // setting on the first frame.
    controls.maxDistance = radius * 12;
    // Borne l'angle polaire pour qu'on ne puisse pas passer sous le sol.
    controls.maxPolarAngle = Math.PI * 0.495;

    // The ground: a pool of light with the contact shadow in its middle, drawn
    // as one gradient rather than a shadow map (§8.1). Two things at once,
    // because they are two halves of the same thing — the car sits on a lit
    // floor and blocks part of that light.
    const ground = new THREE.Mesh(
      new THREE.PlaneGeometry(radius * 5, radius * 5),
      new THREE.MeshBasicMaterial({
        map: groundTexture(THREE),
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
    sun.shadow.mapSize.set(512, 512);
    // Sans ce biais, la carte d'ombre s'auto-ombre en fines rayures sur les
    // surfaces presque parallèles à la lumière (le capot, le toit).
    sun.shadow.bias = -0.0015;

    const shadowCatcher = new THREE.Mesh(
      new THREE.PlaneGeometry(radius * 5, radius * 5),
      new THREE.ShadowMaterial({ opacity: 0.5 }),
    );
    shadowCatcher.receiveShadow = true;
    shadowCatcher.rotation.x = -Math.PI / 2;
    shadowCatcher.position.set(center.x, box.min.y + radius / 300, center.z);
    shadowCatcher.renderOrder = -1;
    scene.add(shadowCatcher);

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
      composer: null,
      introAt: 0,
      disposed: false,
    };
    // Chaîne SMAA d'emblée si le niveau de qualité la demande : la monter
    // après la première image ferait clignoter le panneau à l'ouverture.
    if (quality().smaa) built.composer = await buildComposer(THREE, renderer, scene, camera);
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
      renderer.setSize(width, height, false);
      // `setPixelRatio` avant `setSize` : la chaîne multiplie la taille reçue
      // par le ratio qu'elle connaît, et un ratio périmé lui ferait allouer
      // des cibles de la mauvaise taille après un changement de qualité.
      built.composer?.setPixelRatio(renderer.getPixelRatio());
      built.composer?.setSize(width, height);
      camera.aspect = width / height;
      camera.updateProjectionMatrix();
      requestRender(built);
    };
    built.resize = resize;
    built.observer = new ResizeObserver(resize);
    built.observer.observe(host);
    resize();

    controls.addEventListener("change", () => requestRender(built));
    // Première image, puis la boucle s'entretient tant que le plateau tourne.
    requestRender(built);
    // La main de l'utilisateur prime sur le plateau : il s'arrête à la prise,
    // et ne repart qu'après un temps de repos.
    controls.addEventListener("start", () => {
      spinning = false;
      stopResume();
    });
    controls.addEventListener("end", () => {
      if (reducedMotion) return;
      stopResume();
      resumeTimer = setTimeout(() => {
        spinning = true;
        // La scène en place, pas celle qui a armé le minuteur : un changement
        // de skin peut l'avoir remplacée entre-temps.
        const live = liveScene();
        if (live) requestRender(live);
      }, SPIN_RESUME_MS);
    });

    // Fiche sortie de l'écran par le scroll : plus rien à rendre.
    built.visibility = new IntersectionObserver((entries) => {
      onScreen = entries.some((e) => e.isIntersecting);
      if (turning()) requestRender(built);
    });
    built.visibility.observe(host);

    return built;
  }

  /**
   * The floor under the car, drawn on the fly — no file to ship (§8.1).
   *
   * Two gradients on one texture. The wide, pale one is the pool of light a
   * showroom floor returns under the lamps: it is what the Kunos photos show
   * around the car, and without it the car sits on nothing. The tight dark one
   * on top is the contact shadow, which is what makes it sit on the floor
   * rather than hover above it.
   *
   * Stops are in fractions of the plane's half-width, so changing the plane's
   * size keeps their proportions.
   */
  function groundTexture(THREE: typeof ThreeModule): ThreeModule.Texture {
    const size = 512;
    const canvas = document.createElement("canvas");
    canvas.width = size;
    canvas.height = size;
    const ctx = canvas.getContext("2d");
    if (ctx) {
      const middle = size / 2;
      // Parti de la photo — le fond d'un `preview.jpg` passe de rgb(2,3,5)
      // dans les coins à rgb(12,13,15) sous la voiture — puis remonté d'un
      // cran : ici le sol est aussi le seul repère de profondeur, alors que
      // la photo, elle, montre le décor du showroom autour.
      const pool = ctx.createRadialGradient(middle, middle, 0, middle, middle, middle);
      pool.addColorStop(0, "rgba(255,255,255,0.11)");
      pool.addColorStop(0.45, "rgba(255,255,255,0.066)");
      pool.addColorStop(0.75, "rgba(255,255,255,0.018)");
      pool.addColorStop(1, "rgba(255,255,255,0)");
      ctx.fillStyle = pool;
      ctx.fillRect(0, 0, size, size);

      // Assombrissement de contact seulement. L'ombre de la voiture, elle, est
      // désormais **projetée** (voir la lumière directionnelle plus haut) ; ce
      // dégradé ne fait plus que noircir le dernier centimètre sous la caisse,
      // là où une carte d'ombre manque toujours de résolution.
      const contact = ctx.createRadialGradient(middle, middle, 0, middle, middle, middle * 0.34);
      contact.addColorStop(0, "rgba(0,0,0,0.3)");
      contact.addColorStop(1, "rgba(0,0,0,0)");
      ctx.fillStyle = contact;
      ctx.fillRect(0, 0, size, size);
    }
    const texture = new THREE.CanvasTexture(canvas);
    texture.colorSpace = THREE.SRGBColorSpace;
    return texture;
  }

  // Chargement, et rechargement complet à chaque changement de voiture ou de
  // skin. `untrack` sur tout le reste : un effet Svelte 5 suit **toute** valeur
  // réactive lue pendant son exécution, pas seulement celles nommées en tête —
  // c'est exactement ce qui avait fait se refermer l'ancien aperçu natif dès
  // qu'il s'ouvrait (voir showroom-3d-preview-research.md, test réel n°5).
  $effect(() => {
    const car = carId;
    const skin = skinId;

    // Garde-fou : une scène déjà posée sur ce couple voiture/skin n'est pas
    // reconstruite. Recharger coûte le retour à la photo puis une conversion,
    // pour finir exactement là où on était — et ça s'est produit pour de bon,
    // un effet parent réévalué relançant tout (voir `untrack` dans
    // `DetailPage`). La cause est corrigée là-bas ; ceci empêche la classe
    // entière de se voir à l'écran.
    if (untrack(() => loaded) === `${car}|${skin}`) return;

    // Remplacement **à chaud** : même voiture, seul le skin change, et un
    // modèle tourne déjà à l'écran. Il y reste, et continue de tourner, le
    // temps que le nouveau se convertisse ; le remplaçant reprend le plateau
    // et la caméra là où celui-ci les avait laissés. Sans ça, changer de skin
    // repassait par la photo puis par un modèle remis droit — trois sauts
    // visibles pour repeindre une voiture.
    const hot = untrack(() => scene !== null && !scene.disposed && loadedCar === car);

    untrack(() => {
      if (hot) {
        swapping = true;
      } else {
        disposeScene(scene);
        scene = null;
        stopResume();
        loadedCar = "";
        phase = "loading";
      }
      loaded = "";
      stage = null;
      reason = null;
    });

    if (!webglAvailable()) {
      untrack(() => {
        phase = "unavailable";
        reason = null;
      });
      return;
    }

    let cancelled = false;
    (async () => {
      try {
        const handle = await prepareCarPreview(car, skin);
        // La fiche a pu changer pendant la conversion : ne jamais poser le
        // modèle d'une voiture sur la fiche d'une autre.
        if (cancelled || car !== untrack(() => carId)) return;
        const host = untrack(() => canvasHost);
        if (!host) return;
        // Les réglages de cadrage avant la scène : construire sur les valeurs
        // par défaut ferait sauter l'aperçu d'un cadrage à l'autre.
        await preview3dReady();
        const built = await build(handle.url, host, () => {
          const old = untrack(() => scene);
          if (!hot || !old || old.disposed) return null;
          return {
            rotationY: old.turntable.rotation.y,
            position: old.camera.position.clone(),
            target: old.controls.target.clone(),
          };
        });
        if (cancelled || car !== untrack(() => carId)) {
          disposeScene(built);
          return;
        }
        // L'ancien modèle ne part qu'une fois le nouveau posé et rendu : c'est
        // ce qui évite le trou noir d'une image entre les deux.
        disposeScene(untrack(() => scene));
        scene = built;
        loaded = `${car}|${skin}`;
        loadedCar = car;
        phase = "ready";
        swapping = false;
        // Armé ici et non dans `build` : le modèle n'apparaît qu'à partir de
        // cette ligne, et un effet d'entrée commencé pendant la conversion
        // serait à moitié joué avant d'être visible. Jamais sur un changement
        // de skin à chaud — la voiture en place tourne déjà, la relancer
        // serait un défaut, pas un effet.
        if (!hot) armIntro(built);
        requestRender(built);
      } catch (e) {
        if (cancelled) return;
        swapping = false;
        // `errors.previewSuperseded` n'est pas une panne : une demande plus
        // récente a pris la main, l'écran ne doit rien signaler.
        const failure = String(e) === "errors.previewSuperseded" ? null : String(e);
        reason = failure;
        // Skin non converti alors qu'un modèle est en place : il reste à
        // l'écran avec son ancienne peinture, ce qui vaut mieux qu'un retour à
        // la photo — le badge dit ce qui a échoué.
        if (untrack(() => scene)) return;
        phase = "unavailable";
      }
    })();

    return () => {
      cancelled = true;
    };
  });

  /**
   * Applique un niveau de qualité à la scène en place, sans la reconstruire :
   * rien de ce que le niveau change ne dépend du modèle, et recharger coûterait
   * le retour à la photo pour finir sur la même voiture.
   */
  async function applyQuality(current: ThreeScene | null, level: PreviewQuality) {
    if (!current || current.disposed) return;
    applyPixelRatio(current.renderer);
    const wanted = QUALITY[level].smaa;
    if (wanted && !current.composer) {
      const composer = await buildComposer(current.THREE, current.renderer, current.scene, current.camera);
      // Les imports dynamiques laissent le temps de changer d'avis : la scène
      // a pu être libérée, ou le réglage repasser à un niveau sans SMAA.
      if (current.disposed || !QUALITY[preview3dPrefs().quality].smaa) {
        disposeComposer(composer);
        return;
      }
      current.composer = composer;
    } else if (!wanted && current.composer) {
      disposeComposer(current.composer);
      current.composer = null;
    }
    if (current.disposed) return;
    current.resize();
    requestRender(current);
  }

  // Niveau de qualité changé pendant qu'une fiche est ouverte : même principe
  // que le cadrage ci-dessous, l'aperçu suit sans être remonté.
  $effect(() => {
    const level = preview3dPrefs().quality;
    untrack(() => void applyQuality(scene, level));
  });

  // Un réglage de cadrage changé pendant qu'une fiche est ouverte s'applique
  // tout de suite : recadrer ne coûte qu'un rendu, alors que remonter le
  // composant relancerait tout le chargement du modèle.
  $effect(() => {
    // Un effet ne suit que ce qu'il lit : les valeurs sont donc lues ici, à
    // découvert, et pas seulement à l'intérieur de `placeCamera`.
    const prefs = preview3dPrefs();
    void [prefs.zoom, prefs.azimuth, prefs.elevation, prefs.height, prefs.spin];
    untrack(() => {
      if (!scene) return;
      placeCamera(scene);
    });
  });

  // Bouton « replacer » : le compteur de remises à zéro change, la voiture
  // revient au cadrage réglé et repart. Passer par le module de préférences
  // plutôt que par une référence au composant permet de déclencher la remise à
  // zéro depuis n'importe où — la fiche comme l'écran Réglages.
  $effect(() => {
    preview3dResets();
    untrack(() => {
      if (!scene) return;
      stopResume();
      scene.turntable.rotation.y = 0;
      spinning = !reducedMotion;
      armIntro(scene);
      placeCamera(scene);
    });
  });

  // Étapes de conversion, pour que le squelette dise où on en est plutôt que
  // de tourner dans le vide pendant une seconde et demie (§7.3).
  let unlisten: (() => void) | null = null;
  onPreviewProgress((s) => {
    stage = s;
  }).then((off) => {
    unlisten = off;
  });

  // Fenêtre en arrière-plan ou app minimisée : le plateau s'arrête, et repart
  // au retour. Sans ça, une app laissée ouverte sur une fiche tournerait dans
  // le vide toute la journée.
  function onVisibilityChange() {
    if (!scene) return;
    // Le temps passé masqué ne compte pas : sans cette remise à zéro, la
    // première image du retour ferait avancer le plateau de tout ce temps.
    scene.lastFrameAt = 0;
    if (turning()) requestRender(scene);
  }

  $effect(() => {
    document.addEventListener("visibilitychange", onVisibilityChange);
    return () => document.removeEventListener("visibilitychange", onVisibilityChange);
  });

  onDestroy(() => {
    disposeScene(scene);
    scene = null;
    stopResume();
    unlisten?.();
  });
</script>

<div class="preview3d" class:ready={phase === "ready"}>
  {#if fallbackSrc}
    <!-- La photo reste dessous, telle quelle : c'est déjà l'aperçu habituel de
         la fiche, et la flouter pendant la préparation la rendait illisible
         juste au moment où elle sert le plus (§8.5). -->
    <img class="fallback" src={fallbackSrc} alt="" />
  {/if}

  <div class="host" bind:this={canvasHost}></div>

  {#if phase === "loading" || swapping}
    <div class="badge">
      <span class="spinner"></span>
      <span class="mono">{stage ? t(`detail.preview3dStage.${stage}`) : t("detail.preview3dLoading")}</span>
    </div>
  {:else if reason}
    <div class="badge quiet" title={errorText(reason)}>
      <span class="mono">{t("detail.preview3dUnavailable")}</span>
    </div>
  {/if}
</div>

<style>
  .preview3d {
    position: absolute;
    inset: 0;
  }
  .fallback {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    transition: opacity 0.25s ease;
  }
  /* Fondu depuis l'image une fois le modèle en place (§8.5). */
  .preview3d.ready .fallback {
    opacity: 0;
  }
  .host {
    position: absolute;
    inset: 0;
    opacity: 0;
    transition: opacity 0.35s ease;
  }
  .preview3d.ready .host {
    opacity: 1;
  }
  /* Absolu, pas dans le flux : deux canevas cohabitent le temps d'un
     changement de skin, et empilés dans le flux le second doublerait la
     hauteur du conteneur au lieu de recouvrir le premier. */
  .host :global(canvas) {
    position: absolute;
    inset: 0;
    display: block;
    width: 100%;
    height: 100%;
  }
  .badge {
    position: absolute;
    top: 10px;
    right: 10px;
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 4px 8px;
    background: rgba(8, 8, 12, 0.62);
    border: 1px solid var(--line);
    font-size: 11px;
    color: var(--text-dim);
    z-index: 3;
  }
  .badge.quiet {
    opacity: 0.75;
  }
  .spinner {
    width: 11px;
    height: 11px;
    border: 2px solid var(--line);
    border-top-color: var(--rosso);
    border-radius: 50%;
    animation: preview3d-spin 0.8s linear infinite;
  }
  @keyframes preview3d-spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
