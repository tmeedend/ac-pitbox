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
  import { preview3dPrefs, preview3dReady } from "$lib/preview3dPrefs.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { errorText } from "$lib/errors";
  import type * as ThreeModule from "three";

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
  let canvasHost = $state<HTMLDivElement | null>(null);

  /** Tout ce qui doit être libéré : vit hors des runes, ce n'est pas de l'état
   * d'affichage et le rendre réactif ne ferait que déclencher des effets. */
  let scene: ThreeScene | null = null;
  let frame = 0;
  let observer: ResizeObserver | null = null;
  let visibility: IntersectionObserver | null = null;

  interface ThreeScene {
    THREE: typeof ThreeModule;
    renderer: ThreeModule.WebGLRenderer;
    scene: ThreeModule.Scene;
    camera: ThreeModule.PerspectiveCamera;
    controls: {
      update(): boolean;
      dispose(): void;
      addEventListener(type: string, listener: () => void): void;
    };
    pmrem: ThreeModule.PMREMGenerator;
    /** Le plateau : la voiture y est posée, l'ombre de contact non — un socle
     * de salon tourne sous la voiture, il n'emporte pas son ombre. */
    turntable: ThreeModule.Group;
    /** Centre et rayon du modèle, gardés pour recalculer le cadrage quand un
     * réglage change, sans reconstruire la scène. */
    center: ThreeModule.Vector3;
    radius: number;
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

  /** Une préférence système « moins d'animations » désactive le plateau : une
   * rotation permanente est exactement ce qu'elle demande d'éviter. */
  const reducedMotion =
    typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches;

  let spinning = !reducedMotion;
  let onScreen = true;
  let lastFrameAt = 0;
  let resumeTimer: ReturnType<typeof setTimeout> | null = null;

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
    if (frame) cancelAnimationFrame(frame);
    frame = 0;
    lastFrameAt = 0;
    if (resumeTimer) clearTimeout(resumeTimer);
    resumeTimer = null;
    visibility?.disconnect();
    visibility = null;
    if (!current) return;

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
    if (frame) return;
    frame = requestAnimationFrame((now) => {
      frame = 0;
      // Avance en fonction du temps écoulé, pas du nombre d'images : la vitesse
      // ne doit pas dépendre du taux de rafraîchissement de l'écran, sinon un
      // moniteur 144 Hz fait tourner la voiture deux fois plus vite.
      const elapsed = lastFrameAt ? Math.min((now - lastFrameAt) / 1000, 0.1) : 0;
      lastFrameAt = now;
      if (turning() && elapsed > 0) {
        // C'est la voiture qui tourne, pas la caméra : le cadrage reste
        // parfaitement stable, les reflets glissent sur la carrosserie, et
        // rien ne vient contrarier l'état interne d'OrbitControls quand
        // l'utilisateur prend la main.
        current.turntable.rotation.y += SPIN_SPEED * (preview3dPrefs().spin / 100) * elapsed;
      }
      const moving = current.controls.update();
      current.renderer.render(current.scene, current.camera);
      if (moving || turning()) requestRender(current);
      else lastFrameAt = 0;
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
    current.camera.position.set(
      current.center.x + distance * Math.cos(elevation) * Math.sin(azimuth),
      current.center.y + distance * Math.sin(elevation),
      current.center.z + distance * Math.cos(elevation) * Math.cos(azimuth),
    );
    current.controls.update();
    requestRender(current);
  }

  async function build(url: string, host: HTMLDivElement): Promise<ThreeScene> {
    const THREE = await import("three");
    const { GLTFLoader } = await import("three/examples/jsm/loaders/GLTFLoader.js");
    const { OrbitControls } = await import("three/examples/jsm/controls/OrbitControls.js");
    const { showroomEnvironment } = await import("./showroomEnvironment");

    const renderer = new THREE.WebGLRenderer({ antialias: true, powerPreference: "high-performance" });
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    renderer.toneMapping = THREE.ACESFilmicToneMapping;
    renderer.outputColorSpace = THREE.SRGBColorSpace;

    const scene = new THREE.Scene();
    // Image-based lighting, and no asset to ship for it (§8.1). The showroom is
    // dark on purpose — see `showroomEnvironment` for what a white room did to
    // the paint.
    const pmrem = new THREE.PMREMGenerator(renderer);
    scene.environment = pmrem.fromScene(showroomEnvironment(THREE), 0.04).texture;

    const gltf = await new GLTFLoader().loadAsync(url);

    // Les vitres passent après l'opaque et n'écrivent pas dans le tampon de
    // profondeur, sinon l'intérieur disparaît derrière le pare-brise (§8.2).
    gltf.scene.traverse((object) => {
      const mesh = object as ThreeModule.Mesh;
      if (!mesh.isMesh) return;
      const materials = Array.isArray(mesh.material) ? mesh.material : [mesh.material];
      if (materials.some((m) => (m as ThreeModule.Material).transparent)) {
        mesh.renderOrder = 1;
        for (const m of materials) (m as ThreeModule.Material).depthWrite = false;
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

    // Ombre de contact : un simple disque dégradé, pas de shadow map (§8.1).
    // Sans elle la voiture flotte visiblement au-dessus du vide.
    const shadow = new THREE.Mesh(
      new THREE.PlaneGeometry(radius * 3, radius * 3),
      new THREE.MeshBasicMaterial({ map: contactShadowTexture(THREE), transparent: true, depthWrite: false }),
    );
    shadow.rotation.x = -Math.PI / 2;
    shadow.position.set(center.x, box.min.y + radius / 400, center.z);
    scene.add(shadow);

    host.appendChild(renderer.domElement);
    const built: ThreeScene = { THREE, renderer, scene, camera, controls, pmrem, turntable, center, radius };
    placeCamera(built);

    const resize = () => {
      const width = host.clientWidth;
      const height = host.clientHeight;
      if (width === 0 || height === 0) return;
      renderer.setSize(width, height, false);
      camera.aspect = width / height;
      camera.updateProjectionMatrix();
      requestRender(built);
    };
    observer = new ResizeObserver(resize);
    observer.observe(host);
    resize();

    controls.addEventListener("change", () => requestRender(built));
    // Première image, puis la boucle s'entretient tant que le plateau tourne.
    requestRender(built);
    // La main de l'utilisateur prime sur le plateau : il s'arrête à la prise,
    // et ne repart qu'après un temps de repos.
    controls.addEventListener("start", () => {
      spinning = false;
      if (resumeTimer) clearTimeout(resumeTimer);
      resumeTimer = null;
    });
    controls.addEventListener("end", () => {
      if (reducedMotion) return;
      if (resumeTimer) clearTimeout(resumeTimer);
      resumeTimer = setTimeout(() => {
        spinning = true;
        requestRender(built);
      }, SPIN_RESUME_MS);
    });

    // Fiche sortie de l'écran par le scroll : plus rien à rendre.
    visibility = new IntersectionObserver((entries) => {
      onScreen = entries.some((e) => e.isIntersecting);
      if (turning()) requestRender(built);
    });
    visibility.observe(host);

    return built;
  }

  /** Dégradé radial dessiné à la volée — aucun fichier à embarquer. */
  function contactShadowTexture(THREE: typeof ThreeModule): ThreeModule.Texture {
    const size = 256;
    const canvas = document.createElement("canvas");
    canvas.width = size;
    canvas.height = size;
    const ctx = canvas.getContext("2d");
    if (ctx) {
      const gradient = ctx.createRadialGradient(size / 2, size / 2, 0, size / 2, size / 2, size / 2);
      gradient.addColorStop(0, "rgba(0,0,0,0.55)");
      gradient.addColorStop(0.7, "rgba(0,0,0,0.12)");
      gradient.addColorStop(1, "rgba(0,0,0,0)");
      ctx.fillStyle = gradient;
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

    untrack(() => {
      disposeScene(scene);
      scene = null;
      observer?.disconnect();
      observer = null;
      phase = "loading";
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
        const built = await build(handle.url, host);
        if (cancelled || car !== untrack(() => carId)) {
          disposeScene(built);
          return;
        }
        scene = built;
        phase = "ready";
      } catch (e) {
        if (cancelled) return;
        phase = "unavailable";
        // `errors.previewSuperseded` n'est pas une panne : une demande plus
        // récente a pris la main, l'écran ne doit rien signaler.
        reason = String(e) === "errors.previewSuperseded" ? null : String(e);
      }
    })();

    return () => {
      cancelled = true;
    };
  });

  // Un réglage de cadrage changé pendant qu'une fiche est ouverte s'applique
  // tout de suite : recadrer ne coûte qu'un rendu, alors que remonter le
  // composant relancerait tout le chargement du modèle.
  $effect(() => {
    // Un effet ne suit que ce qu'il lit : les quatre valeurs sont donc lues
    // ici, à découvert, et pas seulement à l'intérieur de `placeCamera`.
    const prefs = preview3dPrefs();
    void [prefs.zoom, prefs.azimuth, prefs.elevation, prefs.spin];
    untrack(() => {
      if (!scene) return;
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
    lastFrameAt = 0;
    if (scene && turning()) requestRender(scene);
  }

  $effect(() => {
    document.addEventListener("visibilitychange", onVisibilityChange);
    return () => document.removeEventListener("visibilitychange", onVisibilityChange);
  });

  onDestroy(() => {
    disposeScene(scene);
    scene = null;
    observer?.disconnect();
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

  {#if phase === "loading"}
    <div class="badge">
      <span class="spinner"></span>
      <span class="mono">{stage ? t(`detail.preview3dStage.${stage}`) : t("detail.preview3dLoading")}</span>
    </div>
  {:else if phase === "unavailable" && reason}
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
  .host :global(canvas) {
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
