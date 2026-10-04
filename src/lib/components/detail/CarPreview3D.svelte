<script lang="ts">
  // Aperçu 3D interactif d'une voiture (docs/SPEC-preview-3d-kn5.md PREVIEW§8).
  //
  // Le modèle est converti côté Rust en glTF binaire, mis en cache et servi
  // par le protocole `carpreview` : ici on ne reçoit qu'une URL, jamais les
  // octets (PREVIEW§7.2).
  //
  // The three.js scene itself — building, render loop, turntable, camera,
  // floor, steering, freeing the GPU — is `$lib/preview3d/scene.ts` and its
  // neighbours, and every rendering number is in `tuning.ts`. This component
  // decides which model to load and when, swaps a skin without a cut, and
  // carries the scene's settings over from the preferences.
  import { onDestroy, untrack } from "svelte";
  import { prepareCarPreview, onPreviewProgress, type DriverView, type PreviewStage } from "$lib/preview3d/preview";
  import { carClassOf, driverOverridePayload } from "$lib/driver/driverOverride.svelte";
  import {
    preview3dGraftsDriver,
    preview3dPrefs,
    preview3dReady,
    preview3dResets,
  } from "$lib/preview3d/preview3dPrefs.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { errorText } from "$lib/errors";
  import { engineRunning } from "$lib/detail/enginePlayer.svelte";
  import {
    applyQuality,
    applyScene,
    armIntro,
    build,
    disposeScene,
    placeCamera,
    REDUCED_MOTION,
    requestRender,
    turning,
    webglAvailable,
    type StageHost,
    type ThreeScene,
  } from "$lib/preview3d/scene";
  import { attachMirror } from "$lib/preview3d/floor";
  import { applySteer } from "$lib/preview3d/rig";

  let {
    carId,
    skinId = null,
    fallbackSrc = null,
    /** Classe de la voiture (`ui_car.json`), qui décide de laquelle des deux
     * tenues par défaut habille son pilote — course ou rue. La fiche la
     * connaît, l'aperçu non. */
    carClass = null,
    /** Incrémentée quand le contenu déployé de la voiture a changé sans que son
     * id ni son skin ne bougent — activation d'une couche, typiquement, qui
     * remplace le `.kn5` et les skins en place. Le backend s'en aperçoit tout
     * seul (la clé de cache porte la date du modèle), mais l'écran, lui, ne
     * redemandait rien : le modèle d'avant restait à tourner. */
    revision = 0,
    /** Montre le pilote sans attendre de clé de contact. L'écran Réglages n'en
     * a pas à tourner, et y régler un pilote qu'on ne voit jamais n'aurait pas
     * de sens — c'est le seul appelant qui le pose. */
    driverAlways = false,
  }: {
    carId: string;
    skinId?: string | null;
    fallbackSrc?: string | null;
    carClass?: string | null;
    revision?: number;
    driverAlways?: boolean;
  } = $props();

  type Phase = "loading" | "ready" | "unavailable";

  let phase = $state<Phase>("loading");
  let stage = $state<PreviewStage | null>(null);
  /** Clé i18n ou message technique, affiché en infobulle du badge (PREVIEW§8.5). */
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
    return `${car}|${skin}|${revision}|${driver ? JSON.stringify(driver) : ""}`;
  }
  /** Voiture du modèle en place — la moitié de `loaded` qui décide si un
   * changement de skin peut se faire à chaud (même géométrie) ou non. */
  let loadedCar = "";

  /** Le pilote doit-il être visible **maintenant** ?
   *
   * `always` le montre tout le temps ; `ignition` le fait arriver quand une
   * clé de contact tourne. `driverAlways` court-circuite le second : l'écran
   * Réglages n'a pas de clé à tourner, et y régler un pilote qu'on ne voit
   * jamais n'aurait pas de sens.
   *
   * Rien ici ne parle de `never` : dans ce mode la conversion ne greffe aucun
   * mannequin, donc il n'y a rien à montrer et `driver` est vide. */
  function driverWanted(): boolean {
    const mode = preview3dPrefs().driver;
    if (mode === "never") return false;
    return mode === "always" || driverAlways || engineRunning();
  }

  /** What the scenes read and write of this component: one for all of them,
   * so that a stand the user stopped stays stopped across a skin swap. */
  const stageHost: StageHost = { spinning: !REDUCED_MOTION, onScreen: true, driverWanted };

  // Chargement, et rechargement complet à chaque changement de voiture, de
  // skin ou de pilote — les trois décident du `.glb` demandé. `untrack` sur tout le reste : un effet Svelte 5 suit **toute** valeur
  // réactive lue pendant son exécution, pas seulement celles nommées en tête —
  // c'est exactement ce qui avait fait se refermer l'ancien aperçu natif dès
  // qu'il s'ouvrait (voir showroom-3d-preview-research.md, test réel n°5).
  $effect(() => {
    const car = carId;
    const skin = skinId;
    // Lue ici pour que l'effet s'y abonne : sans cette ligne, une couche
    // activée ne relancerait rien tant que la voiture et le skin ne bougent pas.
    void revision;
    // Lus à découvert, et volontairement : ce sont les seuls réglages qui
    // changent le `.glb` lui-même — le pilote y est greffé et sa pose y est
    // cuite — donc les bouger doit relancer une conversion. Les autres
    // s'appliquent à la scène en place, plus bas.
    // `preview3dGraftsDriver` et non le mode brut : `always` et `ignition`
    // convertissent tous deux **avec** le mannequin, seule la vue les
    // distingue. Sans ça, tourner une clé de contact demanderait une
    // conversion de quatorze mégaoctets avant que le pilote n'arrive.
    const driver = preview3dGraftsDriver() ? (driverOverridePayload(carId, carClassOf(carClass)) ?? {}) : null;
    // **Le braquage n'est pas lu ici, et surtout pas** : un effet suit tout ce
    // qu'il lit, donc le nommer suffirait à relancer ce chargement à chaque pas
    // de curseur. Roues, volant et bras du pilote tournent tous à l'affichage,
    // le `.glb` ne dépend plus de l'angle — c'est justement ce qui a fait
    // tomber la conversion par valeur essayée.

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
        const built = await build(handle.url, host, stageHost, () => {
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

  // Niveau de qualité changé pendant qu'une fiche est ouverte : même principe
  // que le cadrage ci-dessous, l'aperçu suit sans être remonté.
  $effect(() => {
    // Lu à découvert : un effet ne suit que ce qu'il lit lui-même.
    void preview3dPrefs().quality;
    untrack(() => applyQuality(scene));
  });

  // Le braquage s'applique **sans reconversion** : une rotation de nœud sur le
  // modèle en place. Seuls les bras du pilote font encore exception — ils sont
  // écrits en sommets figés, donc les bouger demande une conversion, et c'est
  // l'effet de chargement plus bas qui s'en charge.
  $effect(() => {
    void preview3dPrefs().steer;
    untrack(() => {
      if (!scene || scene.disposed) return;
      applySteer(scene);
      requestRender(scene);
    });
  });

  // Le pilote entre et sort **sans reconversion** : la clé de contact d'un mod
  // son le fait arriver, la couper le fait partir, et entre les deux le `.glb`
  // ne bouge pas. Seule la cible change ici ; c'est la boucle de rendu qui
  // mène le fondu, elle seule connaissant le temps écoulé.
  $effect(() => {
    // Lus à découvert pour que l'effet s'y abonne — le mode et la clé.
    const wanted = driverWanted();
    untrack(() => {
      if (!scene || scene.disposed || scene.driver.length === 0) return;
      if (scene.driverTarget === (wanted ? 1 : 0)) return;
      scene.driverTarget = wanted ? 1 : 0;
      requestRender(scene);
    });
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
      scene.turntable.rotation.y = 0;
      stageHost.spinning = !REDUCED_MOTION;
      armIntro(scene);
      placeCamera(scene);
    });
  });

  // Étapes de conversion, pour que le squelette dise où on en est plutôt que
  // de tourner dans le vide pendant une seconde et demie (PREVIEW§7.3).
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
    if (turning(scene)) requestRender(scene);
  }

  $effect(() => {
    document.addEventListener("visibilitychange", onVisibilityChange);
    return () => document.removeEventListener("visibilitychange", onVisibilityChange);
  });

  onDestroy(() => {
    disposeScene(scene);
    scene = null;
    unlisten?.();
  });
</script>

<div class="preview3d" class:ready={phase === "ready"}>
  {#if fallbackSrc && phase === "unavailable"}
    <!-- **La photo n'est plus qu'un recours**, et non plus le fond permanent
         de l'aperçu 3D. Elle servait de patience pendant la préparation, mais
         montrer une voiture pour en montrer une autre trois secondes plus tard
         fait deux images là où on en attend une, et le fondu de l'une à
         l'autre attirait l'œil sur le remplacement plutôt que sur le modèle.
         Elle reste, entière, quand la 3D ne peut pas aboutir — modèle chiffré,
         pas de WebGL, conversion en échec (PREVIEW§8.5). -->
    <img class="fallback" src={fallbackSrc} alt="" />
  {/if}

  <div class="host" bind:this={canvasHost}></div>

  {#if phase === "loading"}
    <!-- Au centre, et non dans le coin : sans la photo dessous, il n'y a plus
         rien d'autre à regarder, et un témoin réfugié en haut à droite d'une
         zone vide se cherche. -->
    <div class="preparing">
      <span class="spinner"></span>
      <span class="mono">{stage ? t(`detail.preview3dStage.${stage}`) : t("detail.preview3dLoading")}</span>
    </div>
  {:else if swapping}
    <!-- Le changement de skin garde le badge de coin : le modèle précédent est
         toujours à l'écran, et c'est lui qu'on regarde — un témoin centré le
         recouvrirait. -->
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
  }
  /* Le témoin de préparation, au centre de la zone vide. Même serpent que
     partout ailleurs ; seule la place change. */
  .preparing {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    font-size: 11px;
    color: var(--muted);
    z-index: 3;
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
