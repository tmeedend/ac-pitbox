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
  import { prepareCarPreview, onPreviewProgress, type DriverView, type PreviewStage } from "$lib/preview";
  import { driverOverridePayload } from "$lib/driverOverride.svelte";
  import {
    preview3dPrefs,
    preview3dReady,
    preview3dResets,
  } from "$lib/preview3dPrefs.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { errorText } from "$lib/errors";
  // Le seul lien de l'aperçu vers le son : une fonction qui ne fait rien tant
  // que rien ne joue. L'aperçu n'a pas à savoir ce qu'est une écoute native.
  import { reportListenerAngle } from "$lib/enginePlayer.svelte";
  import type * as ThreeModule from "three";
  import type { Reflector } from "three/addons/objects/Reflector.js";
  import { applyFloorMirror, floorMirrorShader } from "./floorMirror";

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
  /** Ce qui est réellement en place — voiture, skin, pilote — pour ne pas
   * reconstruire une scène identique. Vide tant que rien n'est chargé. */
  let loaded = "";

  /** Les trois choses dont dépend le `.glb` demandé, en une clé comparable.
   * `driver` vaut l'angle du volant, ou `null` quand il n'y a pas de pilote. */
  function sceneKey(car: string, skin: string | null | undefined, driver: DriverView | null): string {
    return `${car}|${skin}|${driver ? JSON.stringify(driver) : ""}`;
  }
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
     * qu'il est terminé (§15 — effet d'intro). */
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
  // **Un seul levier, le suréchantillonnage**, et c'est le résultat d'un essai
  // mené jusqu'au bout plutôt qu'un choix de départ. Une passe SMAA a été
  // ajoutée, déplacée d'un espace colorimétrique à l'autre, puis retirée :
  // comparée à l'écran sur le cas le plus défavorable qui soit — un jonc
  // chromé quasi horizontal d'un pixel de haut sur fond noir — elle n'a jamais
  // produit de différence visible, alors qu'elle imposait un `EffectComposer`,
  // donc **deux** cibles RGBA16F multi-échantillonnées (il clone la sienne)
  // plus ses deux tampons internes : près d'un gigaoctet de mémoire graphique
  // sur une fiche large. Le suréchantillonnage, lui, se voit (§15 point 8).
  //
  // Ce qu'il faut retenir si l'idée revient : il ne suffit pas d'ajouter la
  // passe, il faut prouver qu'elle se voit — et sur ce panneau, elle ne se
  // voyait pas.
  //
  // Retirer la chaîne rend au passage le MSAA du contexte (`antialias: true`)
  // à tous les niveaux, et avec lui `alphaToCoverage` sur les découpes en
  // alpha : le montage post-traitement les avait justement contournés.
  // ------------------------------------------------------------------------
  // **Et le facteur ne peut valoir que la densité de l'écran, ou son double.**
  //
  // Ce n'est pas un choix de confort, c'est une contrainte mesurée, et elle a
  // coûté trois niveaux de réglage qui dégradaient l'image au lieu de
  // l'améliorer. Le canevas est dessiné plus grand que le panneau, puis c'est
  // le **compositeur du navigateur** qui le réduit à la taille écran — avec
  // une seule prise bilinéaire, sans mipmap. Le résultat ne dépend donc pas du
  // facteur mais du **rapport de réduction**, et il n'est bon qu'à 2 :
  //
  //   réduction 2 → le centre du pixel de sortie tombe sur le coin entre
  //                 quatre texels : la bilinéaire les lit à poids égaux, c'est
  //                 un filtre boîte 2×2 exact, et il est gratuit ;
  //   réduction 3 → il tombe sur le **centre** d'un texel : la bilinéaire
  //                 dégénère en plus proche voisin et ne lit qu'un texel sur
  //                 neuf. Le pire cas de tous ;
  //   réduction 4 → coin à nouveau, mais 4 texels lus sur 16 ;
  //   non entier  → les prises dérivent, les poids se déséquilibrent, des
  //                 texels sont sautés.
  //
  // Mesuré hors application (rendu de lignes claires quasi horizontales à
  // chaque facteur, réduction bilinéaire sur GPU, comparaison à un filtre
  // boîte depuis un rendu 16×), écart quadratique moyen sur 255 :
  //
  //   réduction  1,00  1,33  1,50  1,67  2,00  2,50  2,67  3,00  4,00
  //   RMS        7,69  9,98  8,87 11,08  4,97 12,17 13,47 21,34 15,43
  //
  // Sur un écran à 1,5 — le cas de l'utilisateur — les anciens niveaux
  // donnaient 1,00 / 1,67 / 2,67 : les deux niveaux « qualité » étaient
  // **mesurablement pires que de ne rien faire**, et Ultra le pire des deux.
  // C'est exactement ce que l'utilisateur voyait, et ça explique après coup le
  // « aucune différence entre Standard et Ultra » du début du chantier ainsi
  // que le « 5× ne se distingue pas de 4× ».
  //
  // Il ne reste donc que deux valeurs utiles, d'où deux niveaux et non trois.
  // Aller au-delà demanderait de faire la réduction soi-même (cible hors écran
  // à 4×, passe de filtre boîte) — donc un tone mapping à refaire à la main,
  // `alphaToCoverage` perdu, et 133 Mio de cible : le montage qui a déjà été
  // construit puis retiré une fois. Le gain irait de 4 à 16 échantillons par
  // pixel écran ; à reprendre le jour où quelqu'un prouve qu'il se voit.
  const QUALITY = {
    /** Un pixel de tampon pour un pixel d'écran : rien de plus. */
    standard: { oversampling: 1 },
    /** Le double, la seule autre valeur que le compositeur sache réduire. */
    high: { oversampling: 2 },
  } as const;

  /**
   * **Every rendering knob, gathered here on purpose.** Change a value, reopen
   * a car sheet, look.
   *
   * Nothing below enters the model conversion, so no value here invalidates a
   * cache entry: a change shows up on the next frame, never after a reconversion.
   * Each entry says what it does and what is worth trying.
   *
   * Two things deliberately stay outside this object:
   *
   * - the **oversampling factor**, in `QUALITY` just above. It is the one value
   *   that cannot be freely picked — see the long comment there, and the table
   *   of measurements that goes with it;
   * - the **anisotropic filtering**, left at whatever the card's maximum is
   *   (16 in practice). There is no reason to want less, and no way to want
   *   more.
   */
  const TUNING = {
    /**
     * Budget of the drawing buffer, in pixels.
     *
     * On the **area**, not on the factor: the window decides the size of the
     * panel, and the level must not multiply it without limit. An allocation
     * that fails does not degrade the image — it loses the WebGL context and
     * leaves the panel black. 16 Mpx of multisampled RGBA fits in ~256 MiB.
     * Lower it if a very large window ever turns the preview black.
     */
    drawingPixels: 16_000_000,

    /**
     * Budget of the mirror's own target, in pixels.
     *
     * Lower than the drawing buffer's on purpose: the reflection is blurred
     * then faded out, so it gains nothing from a resolution far past the
     * screen. 8 Mpx without MSAA fits in ~96 MiB. **This is the first knob to
     * turn if the floor shimmers or if the panel drops frames** — halving it
     * halves the cost of the mirror pass, which is a second render of the whole
     * scene.
     */
    mirrorPixels: 8_000_000,

    /**
     * MSAA samples on the mirror pass. Zero on purpose.
     *
     * The memory goes into resolution instead: MSAA samples triangle coverage
     * but still shades once per texel, so it does nothing for a sub-pixel
     * specular highlight, while the target now follows a supersampled drawing
     * buffer. Set it to 4 to trade the resolution back for coverage quality —
     * it costs four times the memory of the mirror target.
     */
    mirrorSamples: 0,

    /**
     * Shadow map resolution — **and the softness of the shadow**, which is the
     * counter-intuitive part.
     *
     * `PCFSoftShadowMap` has a fixed filter kernel counted in *texels*, so
     * fewer texels means a wider blur. 512 gives the soft edge of a studio
     * light; raising it makes the shadow harder, not better. It also crawls as
     * the car turns, so lowering it further trades a soft edge for a mushy one.
     */
    shadowMapSize: 512,

    /**
     * Shadow map depth bias. Without it the map self-shadows in fine stripes on
     * surfaces nearly parallel to the light — the bonnet, the roof. More
     * negative pushes the stripes away but detaches the shadow from the car.
     */
    shadowBias: -0.0015,

    /**
     * **The two knobs against shimmer in motion**, and they are the only two
     * that measured as working. Both attack the same cause and they add up.
     *
     * The cause: the car turns while the studio stays put, so the reflection of
     * a ceiling strip sweeps across the bodywork. On a near-mirror surface that
     * reflection is a band **narrower than a pixel**, and a pixel-wide band
     * crossing a pixel grid flickers. It is perfectly still on a screenshot,
     * which is what makes it so hard to chase — and why neither supersampling
     * nor any post-pass ever touched it.
     *
     * Measured on a bench outside the app (a near-mirror knot turning half a
     * degree between two frames, eight pairs, counting the pixels that jump by
     * more than 40 out of 255 — a few pixels flipping hard is what reads as
     * sparkle, not many pixels drifting a little):
     *
     * | `environmentBlur` | `roughnessFloor` | pixels jumping >40 | luminance |
     * | --- | --- | --- | --- |
     * | 0,04 | 0 | 1,50 % (référence) | 36,1 |
     * | 0,08 | 0 | 1,34 % (−11 %) | 38,0 |
     * | 0,04 | 0,15 | 1,21 % (−19 %) | 39,1 |
     * | **0,08** | **0,15** | **0,97 % (−35 %)** | 40,5 |
     * | 0,15 | 0,15 | 0,92 % (−38 %) | 41,0 |
     * | 0,30 | 0,15 | 0,92 % (−38 %) | 41,1 |
     *
     * Two things to read off that table. The effect **saturates around
     * 0,08–0,15**: past that, more blur costs contrast and buys nothing. And the
     * last column is the price — the surfaces come out about 12 % brighter and
     * flatter, chrome least like a mirror.
     *
     * **The shipped pair is 0,08 / 0,15**, the marked row: the last one that
     * still buys something. Since the price is a matter of taste and not of
     * correctness, the values were put to the user and are his — same as the
     * framing defaults (point 14). `environmentBlur: 0.04` with
     * `roughnessFloor: 0` restores exactly the look the app had before.
     *
     * ⚠️ **The quality level has nothing to do with any of this.** Measured on
     * the same bench, modelling the compositor: 1,49 % of violent pixels at
     * Standard, 1,47 % at Élevée, 1,48 % at the old Ultra — flat. Oversampling
     * decides the *static* quality of edges and nothing else. A sharper image
     * simply makes the same flicker easier to read, which is what made it look
     * worse at the higher level. Lowering the level to hide it would blur the
     * whole image to mask a defect that has its own remedy.
     */

    /**
     * Blur of the studio environment map — the `sigma` of `PMREMGenerator`.
     *
     * Softens every reflection at once, and costs nothing per frame: it is baked
     * into the environment map when the scene is built. 0,04 was the value the
     * app shipped with before the flicker was measured.
     */
    environmentBlur: 0.08,

    /**
     * Floor under the roughness of every material, 0 to disable.
     *
     * A perfect mirror (roughness near zero) has a highlight of *zero* width,
     * which no amount of sampling can resolve — AC's chrome and glass land
     * there. A floor gives those highlights a width. Applied per pixel, inside
     * the shader, because a plain `material.roughness` would only *scale* a
     * roughness map instead of lifting its dark parts.
     *
     * ⚠️ **Geometric specular antialiasing was tried here and removed.** The
     * textbook remedy (Kaplanyan/Frostbite: fold the screen derivative of the
     * normal into the roughness) measured at **exactly nothing** — 1,50 % of
     * violent pixels against 1,51 %, unchanged even at four times the standard
     * strength. It keys on the normal varying fast across one pixel, which
     * happens when geometry is undersampled; these cars are densely tessellated
     * and fill the frame, so their normals barely move from pixel to pixel. The
     * sparkle is in the sharpness of the reflection, not in the geometry. Do not
     * re-add it without measuring first.
     */
    roughnessFloor: 0.15,
  };

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

  /**
   * Applies `TUNING.roughnessFloor`, per pixel, inside the shader.
   *
   * Injected rather than configured, because three has no such setting — and a
   * plain `material.roughness` would not do: three *multiplies* it by the green
   * channel of the roughness map (`roughnessmap_fragment`), so it scales the
   * map instead of lifting its floor, and this project reads its roughness from
   * `txMaps` on most materials.
   *
   * The insertion point is right after `<roughnessmap_fragment>`, which is where
   * `roughnessFactor` is declared; `<lights_physical_fragment>` consumes it much
   * further down. Verified against three r185.
   */
  /**
   * Is this material physical glass, i.e. does it carry
   * `KHR_materials_transmission`?
   *
   * Glass declared by a mod's `ext_config.ini` is converted as transmissive
   * rather than blended (`kn5-gltf`, SPEC §4.5ter): blending attenuates the
   * whole surface response, specular reflection included, and it is that
   * reflection that makes a pane read as glass. The consequence here is that
   * such a material is **not** `transparent`, so every rule keyed on that flag
   * misses it.
   */
  function isTransmissive(material: ThreeModule.Material): boolean {
    return ((material as ThreeModule.MeshPhysicalMaterial).transmission ?? 0) > 0;
  }

  function applyRoughnessFloor(material: ThreeModule.MeshStandardMaterial): void {
    const floor = TUNING.roughnessFloor;
    if (floor <= 0) return;
    material.onBeforeCompile = (shader) => {
      shader.fragmentShader = shader.fragmentShader.replace(
        "#include <roughnessmap_fragment>",
        `#include <roughnessmap_fragment>
        roughnessFactor = max( roughnessFactor, ${floor.toFixed(4)} );`,
      );
    };
    // Sans cette clé, three met en commun le programme compilé de deux
    // matériaux qu'il croit identiques : il ne regarde pas ce qu'`onBeforeCompile`
    // a changé.
    material.customProgramCacheKey = () => `pitbox-roughfloor-${floor}`;
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
  function sizeMirror(current: ThreeScene): void {
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
  async function attachMirror(current: ThreeScene): Promise<void> {
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
      // into resolution instead. Same argument as §15: MSAA samples triangle
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

  /**
   * Reporte sur la scène tout ce qui se règle sans la reconstruire : le décor
   * (exposition, éclairage) et le sol (reflet, flaque, ombre).
   *
   * Une seule fonction pour la construction et pour les changements de
   * réglage — deux chemins auraient divergé au premier ajout, et c'est
   * exactement ce qui rend un réglage « qui ne marche que si on rouvre la
   * fiche ».
   */
  function applyScene(current: ThreeScene) {
    const prefs = preview3dPrefs();
    current.renderer.toneMappingExposure = prefs.exposure / 100;
    // `scene.environmentIntensity`, et **pas** `material.envMapIntensity` :
    // mesuré au banc, ce dernier n'a aucun effet quand l'environnement vient
    // de la scène (voir `docs/SPEC-preview-3d-kn5.md` §15).
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
  function turning(): boolean {
    return spinning && onScreen && !document.hidden && preview3dPrefs().spin > 0;
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
      reportEar(current);
      current.renderer.render(current.scene, current.camera);
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
    // facteur vient du niveau de qualité (§15) — c'était 1,5 à 2 avant lui.
    applyPixelRatio(renderer, host);
    renderer.toneMapping = THREE.ACESFilmicToneMapping;
    renderer.outputColorSpace = THREE.SRGBColorSpace;
    renderer.shadowMap.enabled = true;
    renderer.shadowMap.type = THREE.PCFSoftShadowMap;

    const scene = new THREE.Scene();
    // Image-based lighting, and no asset to ship for it (§8.1). The showroom is
    // dark on purpose — see `showroomEnvironment` for what a white room did to
    // the paint.
    const pmrem = new THREE.PMREMGenerator(renderer);
    scene.environment = pmrem.fromScene(showroomEnvironment(THREE), TUNING.environmentBlur).texture;

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
        // L'un des deux leviers contre le scintillement en mouvement — voir le
        // tableau de mesures devant `TUNING.roughnessFloor`.
        // Physical glass is exempt: three blurs the transmitted image by the
        // same roughness, so a 0.15 floor would frost every windowpane.
        if (standard.isMeshStandardMaterial && !isTransmissive(material)) applyRoughnessFloor(standard);
      }
      if (materials.some((m) => (m as ThreeModule.Material).transparent)) {
        mesh.renderOrder = 1;
        for (const m of materials) (m as ThreeModule.Material).depthWrite = false;
      } else if (!materials.some(isTransmissive)) {
        // Only the opaque body casts: a windscreen that casts a shadow map
        // casts it solid black, and the car ends up sitting on a dark blob.
        //
        // Transmissive glass is opaque as far as the sorting goes — its
        // transparency lives in `KHR_materials_transmission`, not in an alpha —
        // so it lands in this branch and has to be excluded by hand, or the
        // dark blob comes straight back.
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

    const camera = new THREE.PerspectiveCamera(FRAMING_FOV, 16 / 9, radius / 100, radius * 90);

    const controls = new OrbitControls(camera, renderer.domElement);
    controls.target.copy(center);
    controls.enableDamping = true;
    controls.dampingFactor = 0.08;
    controls.enablePan = false;
    controls.minDistance = radius * 1.1;
    // Assez large pour toute la plage des réglages, sinon la borne annulerait
    // le réglage en silence dès la première image. Le pire cas cumule le zoom
    // le plus faible (50 %) et la focale la plus longue (10°, soit deux fois
    // plus loin qu'à 20°) : environ vingt rayons.
    controls.maxDistance = radius * 26;
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
      disposed: false,
    };
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

  // Chargement, et rechargement complet à chaque changement de voiture, de
  // skin ou de pilote — les trois décident du `.glb` demandé. `untrack` sur tout le reste : un effet Svelte 5 suit **toute** valeur
  // réactive lue pendant son exécution, pas seulement celles nommées en tête —
  // c'est exactement ce qui avait fait se refermer l'ancien aperçu natif dès
  // qu'il s'ouvrait (voir showroom-3d-preview-research.md, test réel n°5).
  $effect(() => {
    const car = carId;
    const skin = skinId;
    // Lus à découvert, et volontairement : ce sont les seuls réglages qui
    // changent le `.glb` lui-même — le pilote y est greffé et sa pose y est
    // cuite — donc les bouger doit relancer une conversion. Les autres
    // s'appliquent à la scène en place, plus bas.
    const driver = preview3dPrefs().driver
      ? { steer: preview3dPrefs().steer, ...(driverOverridePayload() ?? {}) }
      : null;

    // Garde-fou : une scène déjà posée sur ce couple voiture/skin n'est pas
    // reconstruite. Recharger coûte le retour à la photo puis une conversion,
    // pour finir exactement là où on était — et ça s'est produit pour de bon,
    // un effet parent réévalué relançant tout (voir `untrack` dans
    // `DetailPage`). La cause est corrigée là-bas ; ceci empêche la classe
    // entière de se voir à l'écran.
    if (untrack(() => loaded) === sceneKey(car, skin, driver)) return;

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
        const handle = await prepareCarPreview(car, skin, driver);
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
        loaded = sceneKey(car, skin, driver);
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
  function applyQuality(current: ThreeScene | null) {
    if (!current || current.disposed) return;
    applyPixelRatio(current.renderer, current.host);
    current.resize();
    requestRender(current);
  }

  // Niveau de qualité changé pendant qu'une fiche est ouverte : même principe
  // que le cadrage ci-dessous, l'aperçu suit sans être remonté.
  $effect(() => {
    // Lu à découvert : un effet ne suit que ce qu'il lit lui-même.
    void preview3dPrefs().quality;
    untrack(() => applyQuality(scene));
  });

  // Décor et sol : mêmes règles que le cadrage, l'aperçu suit sans être
  // remonté. Le reflet fait exception sur un point — passer de 0 % à autre
  // chose demande de **construire** le miroir, ce qu'on ne fait qu'au
  // chargement : la scène est donc reconstruite dans ce seul cas.
  $effect(() => {
    const prefs = preview3dPrefs();
    void [prefs.exposure, prefs.light, prefs.pool, prefs.shadow];
    void [prefs.reflection, prefs.reflectionBlur, prefs.reflectionReach];
    untrack(() => {
      if (!scene) return;
      const live = scene;
      applyScene(live);
      requestRender(live);
      // Remonter le reflet depuis 0 demande de construire le miroir, ce que
      // `applyScene` ne peut pas faire — il ne règle que ce qui existe.
      if (prefs.reflection > 0 && !live.mirror) {
        void attachMirror(live).then(() => {
          if (!live.disposed) requestRender(live);
        });
      }
    });
  });

  // Un réglage de cadrage changé pendant qu'une fiche est ouverte s'applique
  // tout de suite : recadrer ne coûte qu'un rendu, alors que remonter le
  // composant relancerait tout le chargement du modèle.
  $effect(() => {
    // Un effet ne suit que ce qu'il lit : les valeurs sont donc lues ici, à
    // découvert, et pas seulement à l'intérieur de `placeCamera`.
    const prefs = preview3dPrefs();
    void [prefs.zoom, prefs.azimuth, prefs.elevation, prefs.height, prefs.spin, prefs.fov];
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
