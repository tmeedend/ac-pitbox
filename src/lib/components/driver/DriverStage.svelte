<script lang="ts">
  // Plateau d'essayage de l'écran Pilote (docs/SPEC-ecran-pilote.md PILOTE§5).
  //
  // **Deux vitesses, et c'est la clé de tout l'écran.** La spec suppose que
  // l'essai au survol coûte « quelques millisecondes » (§D2) ; or habiller un
  // mannequin côté Rust, c'est une conversion complète — une bonne seconde.
  // Passer par là au survol rendrait la galerie inutilisable.
  //
  // D'où la répartition :
  //
  // | Geste | Ce qui se passe | Coût |
  // | --- | --- | --- |
  // | survol | la texture est échangée **ici**, sur le matériau déjà chargé | une image |
  // | clic | la tenue part au backend, qui reconvertit le `.glb` | ~1 s, puis cache |
  //
  // **Pas de volant.** La spec en prévoyait un (§D5, un tore générique dessiné
  // par l'app) pour donner aux doigts une géométrie de contact. Essayé deux
  // fois : sur les poignets d'abord — cerceau de 55 cm, personne ne le touche
  // —, puis sur les phalanges des majeurs, 13 cm plus en avant, ce qui le met
  // au bon endroit. Il n'est toujours pas convaincant à l'écran, et il n'a pas
  // assez d'intérêt pour qu'on continue à le chercher : le pilote seul se lit
  // mieux qu'un pilote accompagné d'un objet approximatif. Le rig continue de
  // servir au cadrage, `DriverRig.grip` compris — c'est lui que vise la piste
  // Gants.
  //
  // L'échange local est possible parce qu'AC range un `.jpg` à côté de chaque
  // `.dds` de garde-robe — **la même image, aux mêmes dimensions** (vérifié :
  // `HELMET_2012.dds` en 2048×512 DXT5 et son `.jpg` en 2048×512). C'est déjà
  // la vignette de la galerie, donc elle est souvent déjà en cache navigateur
  // quand le survol arrive.
  //
  // L'essai n'est pas rigoureusement le rendu final : la conversion multiplie
  // la diffuse par la couleur moyenne de la carte de détail du matériau (le
  // suffixe `#paint-babbba` des noms d'image), ce que l'échange local ne fait
  // pas. C'est une teinte, mesurée quasi neutre sur la combinaison et les
  // gants, et l'adoption rétablit le rendu exact. Un essai est un essai.
  import { onDestroy, untrack } from "svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { prepareDriverPreview, type DriverRig } from "$lib/preview";
  import { errorText } from "$lib/errors";
  import type * as ThreeModule from "three";

  /** Les quatre cadrages de PILOTE§5.2, dans le vocabulaire de l'écran. Déclaré ici
   * et non exporté : le mode runes réserve `export` aux props. */
  type StageLane = "body" | "helmet" | "suit" | "gloves";

  let {
    carId,
    skinId = null,
    /** La tenue **adoptée**, celle qui part au backend. */
    outfit,
    lane,
    /** Ce qu'on essaie en ce moment : la vignette d'AC et le nom de fichier de
     * la texture qu'elle remplace. `null` au repos. */
    trial = null,
    /** Nom de ce qui est appliqué — l'essai s'il y en a un, le choix retenu
     * sinon. Le nommer est obligatoire (PILOTE§5.4) : sans ça, rien ne distingue ce
     * qu'on survole de ce qu'on garde. */
    applied,
    /** Échantillon plat de ce qui est appliqué, seul recours quand la 3D ne
     * démarre pas (PILOTE§12.4). */
    sample = null,
    /** Un survol est en cours. Distinct de `trial` : on survole aussi des
     * corps, qui n'ont pas de texture à échanger, et la ligne d'état doit les
     * nommer comme les autres (PILOTE§5.4). */
    trying = false,
  }: {
    carId: string;
    skinId?: string | null;
    outfit: { model: string | null; suit: string | null; gloves: string | null; helmet: string | null };
    lane: StageLane;
    trial?: { url: string; texture: string } | null;
    applied: string;
    sample?: string | null;
    trying?: boolean;
  } = $props();

  type Phase = "loading" | "ready" | "unavailable";
  let phase = $state<Phase>("loading");
  /** Clé i18n ou message technique, en infobulle — jamais bloquant. */
  let reason = $state<string | null>(null);
  let host = $state<HTMLDivElement | null>(null);
  /** Un nouveau mannequin se prépare pendant que le précédent reste à
   * l'écran. Adopter un corps ou une pièce demande une conversion complète au
   * backend (~1 s) : sans ce témoin, le clic ne produit rien de visible et on
   * clique une deuxième fois. Même badge que l'aperçu 3D d'une voiture, qui a
   * exactement le même moment à couvrir. */
  let swapping = $state(false);

  /** Tout ce qui doit être libéré. Hors des runes : ce n'est pas de l'état
   * d'affichage, et le rendre réactif ne ferait que déclencher des effets. */
  let scene: Stage | null = null;
  /** La tenue réellement en place, pour ne pas reconstruire à l'identique. */
  let loaded = "";

  interface Stage {
    THREE: typeof ThreeModule;
    renderer: ThreeModule.WebGLRenderer;
    scene: ThreeModule.Scene;
    camera: ThreeModule.PerspectiveCamera;
    /** Le pilote, dans un groupe qu'on fait tourner (PILOTE§5.1 : glisser = pivoter
     * autour de l'axe vertical, rien d'autre). */
    pivot: ThreeModule.Group;
    rig: DriverRig;
    /** Les matériaux par nom de texture de base, pour l'échange au survol.
     * Clé : le nom de fichier sans extension, en minuscules. */
    slots: Map<string, { material: ThreeModule.MeshStandardMaterial; original: ThreeModule.Texture }[]>;
    /** Textures d'essai déjà chargées — un aller-retour par vignette, pas un
     * par survol. */
    trials: Map<string, ThreeModule.Texture>;
    /** Ce qui est actuellement substitué, pour savoir quoi rétablir. */
    tried: string | null;
    dispose: () => void;
    resize: () => void;
    frame: (lane: StageLane, instant: boolean) => void;
  }

  const outfitKey = $derived(
    [carId, skinId, outfit.model, outfit.suit, outfit.gloves, outfit.helmet].map((v) => v ?? "").join("|"),
  );

  // --- Chargement du mannequin --------------------------------------------

  $effect(() => {
    const key = outfitKey;
    const node = host;
    if (!node) return;
    if (untrack(() => loaded) === key) return;
    let cancelled = false;
    swapping = untrack(() => scene) != null;
    void (async () => {
      try {
        const preview = await prepareDriverPreview(carId, skinId, outfit);
        if (cancelled) return;
        if (!preview) {
          phase = "unavailable";
          reason = t("driver.stage.noBody");
          return;
        }
        // Le corps précédent reste affiché jusqu'à ce que le nouveau soit prêt
        // (PILOTE§9.2) : on ne démonte l'ancienne scène qu'ici, une fois le `.glb`
        // obtenu. Jamais de plateau vide.
        const built = await build(node, preview.url, preview.rig);
        if (cancelled) {
          built.dispose();
          return;
        }
        scene?.dispose();
        scene = built;
        loaded = key;
        phase = "ready";
        reason = null;
        built.frame(lane, true);
      } catch (e) {
        if (cancelled) return;
        // La galerie et la sélection restent pleinement fonctionnelles :
        // l'écran ne se bloque jamais sur l'absence de 3D (PILOTE§12.4).
        phase = "unavailable";
        reason = errorText(e);
        console.error("driver: plateau indisponible", e);
      } finally {
        if (!cancelled) swapping = false;
      }
    })();
    return () => {
      cancelled = true;
      swapping = false;
    };
  });

  // Le cadrage suit la piste active, et **seulement** elle : un cadrage qui
  // bouge pendant qu'on compare des options rendrait la comparaison
  // impossible (PILOTE§5.2).
  $effect(() => {
    const wanted = lane;
    untrack(() => scene)?.frame(wanted, prefersReducedMotion());
  });

  // L'essai, lui, ne touche jamais à la caméra — seulement aux textures.
  $effect(() => {
    const wanted = trial;
    const current = untrack(() => scene);
    if (!current) return;
    void applyTrial(current, wanted);
  });

  onDestroy(() => {
    scene?.dispose();
    scene = null;
  });

  function prefersReducedMotion(): boolean {
    return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  }

  // --- Échange de texture au survol ---------------------------------------

  /** Le nom de fichier d'une texture, sans extension ni suffixe de variante :
   * la conversion nomme ses images `2016_Suit_DIFF.dds#paint-babbba`, et c'est
   * `2016_suit_diff` qui identifie la pièce. */
  function stemOf(name: string): string {
    const base = name.split("#")[0] ?? name;
    const cut = base.lastIndexOf(".");
    return (cut > 0 ? base.slice(0, cut) : base).toLowerCase();
  }

  async function applyTrial(stage: Stage, wanted: { url: string; texture: string } | null): Promise<void> {
    const key = wanted ? stemOf(wanted.texture) : null;
    if (stage.tried === key) return;

    // Rétablit d'abord ce qui était là : passer d'un casque à l'autre ne doit
    // pas laisser le premier sur une pièce que le second ne recouvre pas.
    if (stage.tried) {
      for (const slot of stage.slots.get(stage.tried) ?? []) {
        slot.material.map = slot.original;
        slot.material.needsUpdate = true;
      }
    }
    stage.tried = null;
    if (!wanted || !key) return;

    const slots = stage.slots.get(key);
    if (!slots?.length) return;
    let texture = stage.trials.get(wanted.url);
    if (!texture) {
      const fetched = await loadTexture(stage, wanted.url);
      // Une vignette manquante — trois casques sur 176 n'en ont pas — laisse
      // simplement la texture en place (PILOTE§7.4).
      if (!fetched) return;
      texture = fetched;
      stage.trials.set(wanted.url, fetched);
    }
    // Le survol a pu changer pendant le chargement de l'image.
    if (stage.tried !== null || trial?.url !== wanted.url) return;
    for (const slot of slots) {
      slot.material.map = texture;
      slot.material.needsUpdate = true;
    }
    stage.tried = key;
  }

  function loadTexture(stage: Stage, url: string): Promise<ThreeModule.Texture | null> {
    return new Promise((resolve) => {
      new stage.THREE.TextureLoader().load(
        url,
        (texture) => {
          // glTF pose ses UV sans retournement : une texture chargée par
          // `TextureLoader` arrive avec `flipY` à vrai et se poserait à
          // l'envers sur un modèle glTF. Bug classique, invisible sur une
          // texture symétrique — la combinaison l'est presque.
          texture.flipY = false;
          texture.colorSpace = stage.THREE.SRGBColorSpace;
          texture.anisotropy = stage.renderer.capabilities.getMaxAnisotropy();
          texture.needsUpdate = true;
          resolve(texture);
        },
        undefined,
        () => resolve(null),
      );
    });
  }

  // --- Construction de la scène -------------------------------------------

  /** Champ de vision, en degrés. Assez long pour ne pas déformer un visage au
   * cadrage « casque », assez court pour ne pas demander trois mètres de
   * recul au cadrage « corps ». */
  const FOV = 32;
  /** Marge autour du sujet, en fraction du rayon cadré. */
  const MARGIN = 1.25;
  async function build(node: HTMLDivElement, url: string, rig: DriverRig): Promise<Stage> {
    const THREE = await import("three");
    const { GLTFLoader } = await import("three/examples/jsm/loaders/GLTFLoader.js");
    const { OrbitControls } = await import("three/examples/jsm/controls/OrbitControls.js");
    const { showroomEnvironment } = await import("../detail/showroomEnvironment");

    const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
    renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
    renderer.toneMapping = THREE.ACESFilmicToneMapping;
    renderer.outputColorSpace = THREE.SRGBColorSpace;

    const world = new THREE.Scene();
    const pmrem = new THREE.PMREMGenerator(renderer);
    world.environment = pmrem.fromScene(showroomEnvironment(THREE), 0.04).texture;

    // Le showroom des voitures est volontairement sombre : il a été calibré
    // pour que la peinture garde sa couleur. Sur une combinaison noire il ne
    // reste rien à voir, or ici **le contraste prime sur le réalisme** (PILOTE§5.1).
    // D'où deux lampes explicites par-dessus l'environnement, l'une de face à
    // gauche, l'autre en contre pour décoller la silhouette du fond.
    //
    // **Accrochées à la caméra, et c'est ce qui rend l'orbite possible.** Le
    // plateau tournait autrefois le sujet plutôt que la caméra, précisément
    // pour garder la lumière du côté du spectateur ; des lampes filles de la
    // caméra obtiennent le même résultat en laissant la caméra bouger, donc
    // sans interdire le second axe de rotation. Leurs positions sont désormais
    // relatives à la caméra, pas au monde.
    const key = new THREE.DirectionalLight(0xffffff, 2.4);
    key.position.set(-1.2, 1.4, 2.2);
    const rim = new THREE.DirectionalLight(0xffffff, 0.9);
    rim.position.set(1.4, 0.8, -1.8);
    world.add(new THREE.AmbientLight(0xffffff, 0.35));

    const gltf = await new GLTFLoader().loadAsync(url);

    const slots: Stage["slots"] = new Map();
    const maxAnisotropy = renderer.capabilities.getMaxAnisotropy();
    gltf.scene.traverse((object) => {
      const mesh = object as ThreeModule.Mesh;
      if (!mesh.isMesh) return;
      for (const raw of Array.isArray(mesh.material) ? mesh.material : [mesh.material]) {
        const material = raw as ThreeModule.MeshStandardMaterial;
        if (material.map) {
          material.map.anisotropy = maxAnisotropy;
          const stem = stemOf(material.map.name || "");
          if (stem) {
            const list = slots.get(stem) ?? [];
            list.push({ material, original: material.map });
            slots.set(stem, list);
          }
        }
        // La visière passe après l'opaque et n'écrit pas la profondeur, sinon
        // le visage disparaît derrière elle.
        if (material.transparent) {
          mesh.renderOrder = 1;
          material.depthWrite = false;
        }
      }
    });

    // Plateau tournant centré sur l'axe vertical du pilote. La pose vient de
    // la voiture, donc le mannequin n'est pas à l'origine — il est à sa place
    // dans l'habitacle, jusqu'à 40 cm sur le côté dans une conduite à droite.
    // Le faire tourner autour de l'origine lui ferait décrire un cercle au
    // lieu de pivoter sur lui-même.
    const axis = new THREE.Vector3(rig.head?.[0] ?? 0, 0, rig.head?.[2] ?? 0);
    const pivot = new THREE.Group();
    pivot.position.copy(axis);
    gltf.scene.position.copy(axis.clone().negate());
    pivot.add(gltf.scene);
    world.add(pivot);

    const camera = new THREE.PerspectiveCamera(FOV, 1, 0.02, 40);
    camera.add(key);
    camera.add(rim);
    // Une caméra rend même hors du graphe, mais **ses enfants non** : sans
    // cette ligne, les deux lampes n'éclaireraient rien.
    world.add(camera);
    world.add(camera);

    node.replaceChildren(renderer.domElement);
    renderer.domElement.style.width = "100%";
    renderer.domElement.style.height = "100%";
    renderer.domElement.style.display = "block";

    // --- cadrage ---
    let target = new THREE.Vector3();
    let distance = 2;
    let animation: { from: ThreeModule.Vector3; to: ThreeModule.Vector3; d0: number; d1: number; t: number } | null =
      null;

    // **C'est le pilote qui tourne, pas la caméra** (PILOTE§5.1), et ce n'est pas
    // qu'une façon de dire : l'éclairage est fixe, trois-quarts avant-gauche,
    // et faire orbiter la caméra le laisserait derrière le sujet dès qu'on
    // regarde son dos. Un plateau tournant garde la lumière du côté du
    // spectateur, quelle que soit la face montrée.
    /** Pose la caméra au cadrage voulu, vue de trois-quarts avant gauche. */
    const place = () => {
      camera.position.set(
        target.x + distance * Math.sin(VIEW_AZIMUTH) * Math.cos(VIEW_ELEVATION),
        target.y + distance * Math.sin(VIEW_ELEVATION),
        target.z + distance * Math.cos(VIEW_AZIMUTH) * Math.cos(VIEW_ELEVATION),
      );
      controls.target.copy(target);
      controls.update();
    };

    /** La piste cadrée en dernier : c'est là que le double-clic ramène. */
    let framed: StageLane = "body";
    const frame = (wanted: StageLane, instant: boolean) => {
      framed = wanted;
      const box = new THREE.Box3().setFromObject(gltf.scene);
      const shot = framingFor(THREE, wanted, rig, box);
      const wantedDistance = (shot.radius * MARGIN) / Math.tan((FOV * Math.PI) / 360);
      if (instant) {
        target = shot.target;
        distance = wantedDistance;
        animation = null;
        place();
        return;
      }
      animation = { from: target.clone(), to: shot.target, d0: distance, d1: wantedDistance, t: 0 };
    };

    // --- souris : les mêmes gestes que l'aperçu de la voiture -------------
    //
    // **Les mêmes, littéralement : les `OrbitControls` de three.** Le plateau
    // avait sa mécanique à lui — glisser à l'horizontale tournait le sujet, à
    // la verticale montait la caméra — et cette moitié verticale n'était pas
    // une rotation : impossible de regarder le pilote d'en haut ou d'en bas.
    // Deux écrans qui montrent la même chose ne doivent pas demander deux
    // gestes différents, et le second axe manquait vraiment.
    //
    // Ce que le plateau protégeait — la lumière reste du côté du spectateur —
    // est repris par les lampes filles de la caméra, plus haut.
    const controls = new OrbitControls(camera, renderer.domElement);
    controls.enableDamping = true;
    controls.dampingFactor = 0.08;
    // Clic droit : déplacer, dans le plan de l'écran.
    controls.enablePan = true;
    controls.screenSpacePanning = true;
    // Un pilote assis n'a pas de sol sous lui, mais passer par-dessus le pôle
    // retourne l'image : on s'arrête juste avant, des deux côtés.
    controls.minPolarAngle = 0.08;
    controls.maxPolarAngle = Math.PI - 0.08;
    controls.minDistance = 0.15;
    controls.maxDistance = 8;

    // Double-clic : remise de face **et** cadrage d'origine (PILOTE§5.1). Le zoom
    // n'ayant pas de commande à l'écran, il lui faut une marche arrière, et
    // c'est déjà le geste « remets tout comme c'était ».
    const onDouble = () => {
      frame(framed, true);
    };
    renderer.domElement.addEventListener("dblclick", onDouble);

    const resize = () => {
      const width = node.clientWidth;
      const height = node.clientHeight;
      if (width < 2 || height < 2) return;
      renderer.setSize(width, height, false);
      camera.aspect = width / height;
      camera.updateProjectionMatrix();
    };
    const observer = new ResizeObserver(resize);
    observer.observe(node);
    resize();

    let raf = 0;
    let previous = performance.now();
    const tick = (now: number) => {
      raf = requestAnimationFrame(tick);
      const dt = Math.min((now - previous) / 1000, 0.1);
      previous = now;
      if (animation) {
        // 220 ms, accélération douce (PILOTE§5.2).
        animation.t = Math.min(animation.t + dt / 0.22, 1);
        const e = animation.t < 0.5 ? 2 * animation.t * animation.t : 1 - (-2 * animation.t + 2) ** 2 / 2;
        target = animation.from.clone().lerp(animation.to, e);
        distance = animation.d0 + (animation.d1 - animation.d0) * e;
        place();
        if (animation.t >= 1) animation = null;
      }
      // L'amortissement des contrôles ne bouge que si on le lui demande, à
      // chaque image.
      controls.update();
      renderer.render(world, camera);
    };
    raf = requestAnimationFrame(tick);

    const dispose = () => {
      cancelAnimationFrame(raf);
      observer.disconnect();
      renderer.domElement.removeEventListener("dblclick", onDouble);
      controls.dispose();
      gltf.scene.traverse((object) => {
        const mesh = object as ThreeModule.Mesh;
        if (!mesh.isMesh) return;
        mesh.geometry.dispose();
        for (const raw of Array.isArray(mesh.material) ? mesh.material : [mesh.material]) {
          const material = raw as ThreeModule.MeshStandardMaterial;
          material.map?.dispose();
          material.dispose();
        }
      });
      pmrem.dispose();
      renderer.dispose();
    };

    return {
      THREE,
      renderer,
      scene: world,
      camera,
      pivot,
      rig,
      slots,
      trials: new Map(),
      tried: null,
      dispose,
      resize,
      frame,
    };
  }

  /** Trois-quarts avant **gauche**, comme toutes les photos du jeu, et
   * légèrement au-dessus de l'horizontale — en radians. */
  const VIEW_AZIMUTH = -0.42;
  const VIEW_ELEVATION = 0.14;

  /** Le cadrage d'une piste (PILOTE§5.2), déduit du rig et non codé en dur. */
  function framingFor(
    THREE: typeof ThreeModule,
    lane: StageLane,
    rig: DriverRig,
    box: ThreeModule.Box3,
  ): { target: ThreeModule.Vector3; radius: number } {
    const at = (p: [number, number, number] | null) => (p ? new THREE.Vector3(...p) : null);
    const head = at(rig.head);
    const hips = at(rig.hips);
    // Le cadrage « gants » vise les doigts, comme le volant : c'est là que se
    // passe ce qu'on regarde.
    const held = rig.grip ?? rig.hands;
    const grip = held ? at(held[0])!.add(at(held[1])!).multiplyScalar(0.5) : null;
    const whole = {
      target: box.getCenter(new THREE.Vector3()),
      radius: box.getSize(new THREE.Vector3()).length() / 2,
    };
    switch (lane) {
      case "helmet":
        return head ? { target: head, radius: 0.2 } : whole;
      case "suit":
        return head && hips
          ? { target: head.clone().add(hips).multiplyScalar(0.5), radius: 0.42 }
          : whole;
      case "gloves":
        return grip ? { target: grip, radius: 0.34 } : whole;
      default:
        return whole;
    }
  }
</script>

<div class="stage">
  <div class="canvas" bind:this={host} class:hidden={phase === "unavailable"}></div>

  {#if phase === "unavailable"}
    <!-- PILOTE§12.4 : l'échantillon plat de la pièce retenue, en grand. La galerie
         et la sélection restent pleinement fonctionnelles. -->
    <div class="flat">
      <div class="art">
        {#if sample}<img src={sample} alt="" />{:else}<span class="noart">{t("driver.stage.noSample")}</span>{/if}
      </div>
      <p class="why" title={reason ?? ""}>{t("driver.stage.no3d")}</p>
    </div>
  {:else if phase === "loading"}
    <div class="bar"></div>
  {/if}

  {#if swapping}
    <div class="badge">
      <span class="spinner"></span>
      <span class="mono">{t("driver.stage.updating")}</span>
    </div>
  {/if}

  <div class="foot">
    <span class="live">{t("driver.stage.live")}</span>
    <span class="hint">{trying ? t("driver.stage.trial", { name: applied }) : t("driver.stage.hint")}</span>
    {#if phase === "ready"}<span class="drag">{t("driver.stage.drag")}</span>{/if}
  </div>
</div>

<style>
  /* Dégradé radial du gris panneau vers le noir de fond (PILOTE§5.1) : une livrée
     sombre doit rester lisible, donc le contraste prime sur le réalisme. */
  /* Le plateau vit dans une carte `.blk`, dont le fond est `--panel2` : le
     dégradé part du gris de carte et y retombe, au lieu d'ouvrir un trou plus
     clair que tout le reste de l'écran. */
  .stage {
    position: relative;
    flex: 1;
    min-height: 340px;
    display: flex;
    flex-direction: column;
    background: radial-gradient(72% 68% at 50% 40%, var(--card) 0%, var(--panel2) 78%);
    border-bottom: 1px solid var(--line);
    overflow: hidden;
  }

  .canvas {
    flex: 1;
    min-height: 0;
    cursor: grab;
  }
  .canvas:active {
    cursor: grabbing;
  }
  .canvas.hidden {
    display: none;
  }


  /* Filet de progression en pied de plateau (PILOTE§9.2). Le corps précédent reste
     affiché derrière : jamais de plateau vide. */
  /* Même badge, même serpent que l'aperçu 3D d'une voiture (`CarPreview3D`) :
     c'est le même moment — un modèle se prépare pendant que le précédent
     reste affiché — et deux témoins différents pour un même état se lisent
     comme deux choses différentes. */
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
    color: var(--muted);
    z-index: 3;
  }
  .spinner {
    width: 11px;
    height: 11px;
    border: 2px solid var(--line);
    border-top-color: var(--rosso);
    border-radius: 50%;
    animation: driver-stage-spin 0.8s linear infinite;
  }
  @keyframes driver-stage-spin {
    to {
      transform: rotate(360deg);
    }
  }
  .bar {
    position: absolute;
    left: 0;
    bottom: 28px;
    height: 2px;
    width: 100%;
    background: linear-gradient(90deg, transparent, var(--rosso), transparent);
    animation: driver-stage-sweep 1.1s linear infinite;
  }
  @keyframes driver-stage-sweep {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(100%);
    }
  }

  .flat {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 14px;
    padding: 22px 18px 42px;
  }
  .art {
    width: 100%;
    max-width: 232px;
    aspect-ratio: 1;
    border: 1px solid var(--line);
    overflow: hidden;
    background: var(--card);
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .art img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .noart,
  .why {
    font-size: 11px;
    color: var(--faint);
    text-align: center;
    line-height: 1.5;
    padding: 0 16px;
    margin: 0;
    max-width: 260px;
  }

  .foot {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 28px;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 14px;
    font-size: 10.5px;
    color: var(--muted);
    border-top: 1px solid var(--line);
    background: color-mix(in srgb, var(--panel2) 82%, transparent);
  }
  /* Un des trois seuls emplois du rouge saturé sur cet écran (PILOTE§15). */
  .live {
    font-family: var(--mono);
    color: var(--rosso-bright);
    letter-spacing: 1px;
    text-transform: uppercase;
    flex: 0 0 auto;
  }
  .hint {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .drag {
    margin-left: auto;
    flex: 0 0 auto;
    color: var(--faint);
  }
</style>
