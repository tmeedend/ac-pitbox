<script lang="ts">
  import { onMount, untrack } from "svelte";
  import Settings from "$lib/components/settings/Settings.svelte";
  import About from "$lib/components/settings/About.svelte";
  import Library from "$lib/components/library/Library.svelte";
  import Launch from "$lib/components/launch/Launch.svelte";
  import DriverScreen from "$lib/components/driver/DriverScreen.svelte";
  import Inventory from "$lib/components/inventory/Inventory.svelte";
  import Apps from "$lib/components/inventory/Apps.svelte";
  import NavRail from "./NavRail.svelte";
  import Workshop from "$lib/components/workshop/Workshop.svelte";
  import ImportOverlay from "$lib/components/workshop/ImportOverlay.svelte";
  import PendingDialog from "$lib/components/workshop/PendingDialog.svelte";
  import ImportToasts from "$lib/components/toasts/ImportToasts.svelte";
  import ToastStack from "$lib/components/toasts/ToastStack.svelte";
  import ControllerToast from "$lib/components/toasts/ControllerToast.svelte";
  import GridThumbToast from "$lib/components/toasts/GridThumbToast.svelte";
  import { FEATURE_GRID_THUMBS } from "$lib/features";
  import { gridMods, loadGridCars } from "$lib/launch/gridMods.svelte";
  import {
    BALLAST_MAX,
    RESTRICTOR_MAX,
    loadPlayerHandicap,
    playerHandicap,
    setPlayerHandicap,
  } from "$lib/launch/playerHandicap.svelte";
  import { carAssists, assistsTouched } from "$lib/launch/carAssists.svelte";
  import {
    SESSION_TYPES,
    hasOpponents,
    openOpponentsPage,
    openSetupPage,
    pickSessionType,
    sessionNav,
  } from "$lib/shell/sessionNav.svelte";
  import Seg from "$lib/components/ui/Seg.svelte";
  import Tooltip from "$lib/components/ui/Tooltip.svelte";
  import { bumpLibraryVersion } from "$lib/library/libraryVersion.svelte";
  import { PAUSE_SESSION, pauseGridThumbs, resumeGridThumbs } from "$lib/gridthumbs/gridThumbs.svelte";
  import { onAcRunning } from "$lib/launch/launch";
  import PrefsToast from "$lib/components/toasts/PrefsToast.svelte";
  import BulkToasts from "$lib/components/toasts/BulkToasts.svelte";
  import RepairToast from "$lib/components/toasts/RepairToast.svelte";
  import TitleBar from "./TitleBar.svelte";
  import ControllerSetup from "$lib/components/settings/ControllerSetup.svelte";
  import ImageSelectDropdown from "$lib/components/ui/ImageSelectDropdown.svelte";
  import ModIdentity from "$lib/components/library/ModIdentity.svelte";

  import { carClassOf, driverFor, isEmpty, wearsFallback } from "$lib/driver/driverOverride.svelte";
  import { bodyThumb, requestBodyThumb } from "$lib/driver/driverThumbs.svelte";
  import { wornOutfit } from "$lib/driver/driverOutfits.svelte";
  import TrackSkinChecklistDropdown from "./TrackSkinChecklistDropdown.svelte";
  import { nav, requestSection, openInSection, pickSession } from "$lib/shell/nav.svelte";
  import { recordScreen, goBack, goForward } from "$lib/shell/navHistory";
  import { previewSrc, getModDetail, activateMod } from "$lib/library/library";
  import { withoutBrand } from "$lib/library/displayName";
  import { peekUiPref } from "$lib/uiPrefs.svelte";
  import { StorageKey } from "$lib/storage";
  import { message } from "@tauri-apps/plugin-dialog";
  import { errorText } from "$lib/errors";
  import { initGlobalDragDrop } from "$lib/workshop/importState.svelte";
  import { watchShellScroll } from "$lib/shell/shellScroll";
  import { initBulkProgress } from "$lib/library/bulkState.svelte";
  import { initRepairProgress } from "$lib/workshop/repairState.svelte";
  import {
    listModSkins,
    carFactoryAssists,
    type AssistLevel,
    type FactoryAssists,
    type SessionType,
    type SkinItem,
  } from "$lib/launch/launch";
  import { listOtherMods } from "$lib/inventory/others";
  import { setPreferredSkin, setPreferredLayout } from "$lib/preferred";
  import { syncTrackSkins, listTrackSkinOptions, setTrackSkinActive, type TrackSkinOption } from "$lib/inventory/submods";
  import { t, setLocale } from "$lib/i18n/index.svelte";
  import { setZoom, zoomFactor } from "$lib/shell/zoom.svelte";
  import { getConfig, validateConfig } from "$lib/config";
  import { LAUNCH_BUTTON_ATTR, startGamepadNav } from "$lib/shell/gamepadNav";
  import { controllers, startControllerWatch } from "$lib/shell/gamepadDevices.svelte";
  import { bigPictureState, exitBigPicture } from "$lib/shell/bigpicture.svelte";
  import { musicEnterMenu, musicEnterGrid } from "$lib/shell/music";
  import { libraryVersion } from "$lib/library/libraryVersion.svelte";

  // Trois territoires étanches (SPEC §7.2) : le RAIL porte les lieux
  // (`NavRail.svelte`), la BARRE DE TITRE la forme de la fenêtre, cette
  // COLONNE ce qu'on lance. Les deux grilles de boutons « Add-ons » et
  // « Atelier » qui vivaient ici sont parties dans le rail : elles n'étaient
  // pas mal dessinées, elles étaient mal placées — la colonne de session
  // faisait office de navigation en plus de son travail propre.
  // Glisser-déposer disponible partout : un seul listener, monté ici à la
  // racine, plutôt que dans chaque écran susceptible de recevoir un drop.
  onMount(() => initGlobalDragDrop());

  // La coquille ne défile jamais (§13) : le filet qui l'y ramène est monté ici
  // parce que c'est elle qu'il protège — voir `shellScroll.ts` pour le
  // pourquoi, et pour les deux chemins par lesquels le décalage est arrivé.
  onMount(() => watchShellScroll());

  // Progression des actions groupées (§6.3bis) : un seul écouteur, monté ici
  // comme le glisser-déposer — un lot lancé depuis la bibliothèque doit rester
  // visible même si on change d'écran pendant.
  onMount(() => initBulkProgress());
  onMount(() => initRepairProgress());
  // Plateau du dernier réglage de session : la garde d'activation doit savoir
  // ce qu'elle protège même quand l'écran de réglages n'est pas monté.
  onMount(() => void loadGridCars());
  onMount(() => void loadPlayerHandicap());

  // **La fin de la session lève la pause de la génération.**
  //
  // Premier essai : le retour du focus dans la fenêtre, au motif qu'on ne
  // saurait pas voir la fin d'une course. On sait très bien — l'app surveille
  // déjà le process d'Assetto Corsa depuis le début, pour couper et reprendre
  // la musique de Big Picture. Le focus était donc à la fois moins juste (une
  // fenêtre reprise en alt-tab pendant une course aurait relancé les trois
  // cents conversions) et redondant.
  //
  // Le process, et non le statut « en piste » : ce dernier retombe à chaque
  // retour aux stands. C'est la fermeture du jeu qui rend la machine.
  onMount(() => {
    let stop: (() => void) | null = null;
    void onAcRunning((running) => {
      if (running) pauseGridThumbs(PAUSE_SESSION);
      else resumeGridThumbs(PAUSE_SESSION);
    }).then((off) => (stop = off));
    return () => stop?.();
  });

  // Navigation manette dans toute l'app (croix/stick = déplace le focus,
  // A/Croix = valide, B/Rond = ferme la fiche pleine page). Un seul scrutin
  // global, monté une fois ici.
  onMount(() => startGamepadNav());

  // Détection des périphériques et décision « lequel pilote l'interface »
  // (§7.4). Démarrage, branchement à chaud et première installation sont le
  // même événement — un périphérique visible sans décision enregistrée — donc
  // une seule surveillance, montée ici comme le scrutin ci-dessus.
  onMount(() => startControllerWatch());

  // Supprime le menu contextuel natif du navigateur (Actualiser/Enregistrer
  // sous/Imprimer…) partout dans l'app — une appli desktop n'en a pas besoin,
  // et il apparaîtrait sinon là où aucun menu contextuel maison n'est posé
  // (celui-ci, lui, s'affiche AVANT — donc gagne toujours). Un seul listener
  // global plutôt qu'un `preventDefault` à poser sur chaque écran.
  onMount(() => {
    const suppress = (e: MouseEvent) => e.preventDefault();
    document.addEventListener("contextmenu", suppress);
    return () => document.removeEventListener("contextmenu", suppress);
  });

  // Langue forcée par l'utilisateur (Réglages), sinon langue système (déjà
  // appliquée par défaut par le module i18n). Zoom d'interface, idem.
  onMount(async () => {
    const cfg = await getConfig();
    if (cfg.prefs.language) setLocale(cfg.prefs.language);
    setZoom(cfg.prefs.ui_zoom);
  });

  // --- Colonne de session : ce que les deux emplacements ont à dire ---------
  //
  // Trois états, pas deux (SPEC SESSION§1) : un mod choisi, rien de
  // choisi, ou des chemins cassés. Le troisième existe parce que l'invitation
  // « choisir une voiture » mène à une bibliothèque vide quand Assetto Corsa
  // est introuvable — ce n'est pas le même problème, donc pas la même
  // destination (Réglages › Chemins).
  type SlotState = "picked" | "empty" | "broken";
  let pathsBroken = $state(false);
  /** Content Manager introuvable : le lien de sortie ne s'affiche pas du tout
   * (SPEC SESSION§1). Ni bouton grisé ni message d'erreur au clic — une sortie vers
   * un outil absent n'a pas à occuper une ligne dans une colonne dont la
   * hauteur est comptée. */
  let cmAvailable = $state(false);

  async function refreshPaths() {
    try {
      const v = await validateConfig(await getConfig());
      pathsBroken = !v.ac_install.ok || !v.content_dir.ok;
      cmAvailable = v.content_manager.ok;
    } catch {
      // Le diagnostic lui-même a échoué : ne pas accuser les chemins pour
      // autant, l'état vide normal reste plus juste qu'une fausse impasse.
      pathsBroken = false;
      cmAvailable = false;
    }
  }
  onMount(refreshPaths);
  // Les chemins ne se réparent que depuis les Réglages : relire en sortant de
  // cet écran suffit, plutôt que de valider à chaque rendu. Le nettoyage d'un
  // effet dont la seule dépendance est `nav.section` s'exécute exactement au
  // moment où l'on quitte l'écran.
  $effect(() => {
    if (nav.section !== "settings") return;
    return () => void refreshPaths();
  });

  // --- Pastilles d'alerte du rail (SPEC §7.2) -------------------------------
  //
  // Un signal de rubrique, pas un agrégat : il faut voir LAQUELLE aller
  // regarder. Une seule source réelle aujourd'hui — les conflits de fichiers
  // entre « autres mods », que le backend calcule déjà. Les autres inventaires
  // n'ont pas de notion de « problème » à remonter ; leur en inventer une
  // serait une décision produit, pas une conséquence de cette spec.
  let alerts = $state<Record<string, boolean>>({});
  $effect(() => {
    libraryVersion();
    listOtherMods()
      .then((rows) => (alerts = { others: rows.some((o) => o.conflicts.length > 0) }))
      .catch(() => (alerts = {}));
  });

  const carSlot = $derived<SlotState>(nav.sessionCar ? "picked" : pathsBroken ? "broken" : "empty");
  const trackSlot = $derived<SlotState>(nav.sessionTrack ? "picked" : pathsBroken ? "broken" : "empty");
  /** Tant que le duo n'est pas complet, il n'y a ni session à paramétrer ni
   * session à lancer (SPEC SESSION§1) — l'app ne choisit pas une voiture à la place de
   * l'utilisateur pour se donner un bouton à activer. */
  const sessionReady = $derived(nav.sessionCar != null && nav.sessionTrack != null);

  // --- Le type de session EST la navigation (L5§1) ----------------------
  //
  // Le bouton « Paramétrage de la session » et le segmenté « Type de session »
  // de l'écran de réglages ont disparu tous les deux : ils disaient la même
  // chose à deux endroits, et aucun des deux ne disait ce que l'app sait
  // faire. Les quatre types sont désormais une liste, toujours dépliée — c'est
  // la seule chose de cette colonne qui ne se replie pas.
  //
  // Un clic fait deux gestes en un : il choisit le type **et** ouvre ses
  // réglages. Le second est ce qui remplace le bouton supprimé.
  async function goToType(type: SessionType) {
    pickSessionType(type);
    await requestSection("race");
  }
  /** La sous-entrée. Elle sert aussi de retour : depuis la page adversaires,
   * cliquer le type parent ramène aux réglages sans changer de type. */
  async function goToOpponents() {
    openOpponentsPage();
    await requestSection("race");
  }
  function backToSetup(type: SessionType) {
    if (type !== sessionNav.type) {
      void goToType(type);
      return;
    }
    openSetupPage();
    void requestSection("race");
  }
  /** `6 AI · 87% ± 3` (L5§1.2). Ce n'est pas un ornement : sans elle on ne peut
   * plus savoir combien d'adversaires on affronte sans changer de page, alors
   * qu'on peut lancer la session sans y être allé. */
  const opponentsSummary = $derived(
    t("session.opponentsSummary", {
      count: sessionNav.count,
      center: sessionNav.center,
      spread: sessionNav.spread,
    }),
  );
  /**
   * Deux marques, deux choses différentes — et c'est ce qui permet au filet
   * rouge de cohabiter avec celui du rail sans dire la même chose que lui.
   *
   * - **Choisi** (libellé en pleine lumière) : c'est le type qui partira, quel
   *   que soit l'écran qu'on regarde. La colonne répond toujours à « qu'est-ce
   *   que je lance ? », comme la voiture et le circuit au-dessus — et sans
   *   cette marque, la sous-entrée « Adversaires » pendait sous quatre lignes
   *   identiques, sans qu'on voie à laquelle elle appartenait.
   * - **Ouvert** (filet rouge d'attaque) : on est en train de le regarder. Ça,
   *   c'est un repère d'écran actif, et il ne s'allume que sur l'écran de
   *   session.
   */
  const onSessionScreen = $derived(nav.section === "race");
  const typeChosen = (type: SessionType) => sessionNav.type === type;
  const typeSelected = (type: SessionType) =>
    onSessionScreen && sessionNav.type === type && sessionNav.page === "setup";

  // --- Repli « Performance » de la carte voiture (L5§2.1) ---------------
  //
  // Lest, bride, ABS et contrôle de traction sous une seule ligne, au gabarit
  // de LIVRÉE et PILOTE. Les deux assistances viennent de l'écran de réglages,
  // où elles vivaient parmi les règles de la course : ce sont des **capacités
  // de la voiture**, et le réglage n'existe que parce que la voiture les
  // possède — ce que dit déjà la ligne « Factory » qui les suit ici.
  //
  // Le repli n'est pas mémorisé, et il n'a pas à l'être : la ligne **affiche
  // toujours son état**, replié ou non, donc l'ouvrir ne révèle rien qu'on ne
  // sache déjà — elle ne fait que rendre les champs modifiables.
  let perfOpen = $state(false);
  const perfTouched = $derived(playerHandicap.ballast > 0 || playerHandicap.restrictor > 0 || assistsTouched());
  /** Ce que la ligne dit repliée. `Stock` quand rien n'est posé — « absent »
   * et « à zéro » ne doivent pas se ressembler, donc jamais un champ vide. */
  const perfSummary = $derived.by(() => {
    if (!perfTouched) return t("session.perfStock");
    const parts: string[] = [];
    if (playerHandicap.ballast > 0)
      parts.push(t("session.perfBallast", { kg: playerHandicap.ballast }));
    if (playerHandicap.restrictor > 0)
      parts.push(t("session.perfRestrictor", { pct: playerHandicap.restrictor }));
    if (carAssists.abs !== "factory")
      parts.push(`${t("launch.absLabel")} ${t(`launch.assist${carAssists.abs === "on" ? "On" : "Off"}`)}`);
    if (carAssists.tractionControl !== "factory")
      parts.push(
        `${t("launch.tcShort")} ${t(`launch.assist${carAssists.tractionControl === "on" ? "On" : "Off"}`)}`,
      );
    return parts.join(" · ");
  });
  const assistLevels = $derived([
    { value: "off", label: t("launch.assistOff") },
    { value: "factory", label: t("launch.assistFactory") },
    { value: "on", label: t("launch.assistOn") },
  ]);
  /** Ce que `Factory` vaut pour cette voiture, lu dans son `electronics.ini`.
   * La ligne suit le repli : c'est elle qui justifie que le réglage soit là.
   * Une voiture qui ne dit rien n'a pas de ligne du tout — jamais d'« inconnu ». */
  let factoryAssists = $state<FactoryAssists | null>(null);
  $effect(() => {
    const carId = nav.sessionCar?.id;
    if (!carId) {
      factoryAssists = null;
      return;
    }
    let current = true;
    carFactoryAssists(carId)
      .then((found) => {
        if (current) factoryAssists = found;
      })
      .catch(() => {
        if (current) factoryAssists = null;
      });
    return () => {
      current = false;
    };
  });
  const factoryLine = $derived.by(() => {
    const f = factoryAssists;
    if (!f || !sessionCarName) return null;
    const key = f.abs
      ? f.tractionControl
        ? "launch.factoryBoth"
        : "launch.factoryAbsOnly"
      : f.tractionControl
        ? "launch.factoryTcOnly"
        : "launch.factoryNeither";
    return t(key, { car: sessionCarName });
  });

  // --- Colonne d'intitulés partagée (SPEC SESSION§1) -----------------------------
  //
  // Les quatre champs alignent leurs valeurs sur une même colonne d'intitulé :
  // c'est ce qui fait lire le bloc comme une fiche technique plutôt que comme
  // une pile de menus, et ça disparaît si chaque ligne se dimensionne seule.
  // 60 px conviennent au français ; une autre langue peut demander plus
  // (LACKIERUNG en allemand), d'où une mesure une fois par langue plutôt
  // qu'une constante — plafonnée à 88 px, au-delà l'intitulé tronque.
  const FIELD_LABELS = $derived([
    t("session.fieldLivery"),
    t("session.fieldDriver"),
    t("session.fieldPerformance"),
    t("session.fieldLayout"),
    t("session.fieldTrackSkin"),
  ]);
  let labelProbe = $state<HTMLElement | null>(null);
  let labelWidth = $state(60);
  $effect(() => {
    // Dépendance explicite : c'est le changement de langue qui doit relancer
    // la mesure, et il ne passe que par le contenu du gabarit caché.
    FIELD_LABELS;
    const el = labelProbe;
    if (!el) return;
    // `getBoundingClientRect` rend des pixels RÉELS de fenêtre, déjà
    // multipliés par le zoom d'interface, alors que la valeur repart dans un
    // `style` en pixels CSS que le zoom multipliera à son tour — sans cette
    // division, la colonne s'élargirait à chaque cran de zoom.
    const f = zoomFactor();
    let widest = 0;
    for (const child of Array.from(el.children)) {
      widest = Math.max(widest, (child as HTMLElement).getBoundingClientRect().width / f);
    }
    labelWidth = Math.min(88, Math.max(60, Math.ceil(widest)));
  });

  /**
   * **Un clic ouvre la bibliothèque, deux ouvrent la fiche — et les deux gestes
   * sont comptés ici, jamais confiés à `ondblclick`.**
   *
   * Les deux étaient posés côte à côte sur le même bouton, `onclick` agissant
   * tout de suite. Deux défauts en découlaient, dont l'un se voyait :
   *
   * - **Le double-clic n'ouvrait pas la fiche** quand on n'était pas déjà sur
   *   la bibliothèque visée (signalé à l'usage). Le premier clic changeait de
   *   section, donc remontait tout l'écran principal et remplaçait le contenu
   *   sous le curseur, entre les deux moitiés du geste — et un `dblclick` ne
   *   part que si les deux clics tombent sur le même élément, dans le temps
   *   système ET sans que la cible ait bougé. Impossible à reproduire en
   *   entrée synthétique, où les deux clics partent avant tout rendu : c'est
   *   précisément le genre de course qu'on ne corrige pas en la retentant.
   * - **L'historique enregistrait un écran que personne n'a regardé** : la
   *   liste de la bibliothèque, ouverte par le premier clic, puis la fiche.
   *   « Précédent » y ramenait, ce que `openInSection` existe justement pour
   *   éviter (§7.2bis).
   *
   * Compter les clics soi-même règle les deux d'un coup : rien ne part avant
   * que le geste ne soit fini, donc aucun rendu ne s'intercale et aucun écran
   * intermédiaire n'existe. Le prix est un quart de seconde d'attente sur le
   * clic simple — le prix habituel d'une cible qui porte deux gestes.
   */
  const DOUBLE_CLICK_MS = 260;
  let slotClicks = 0;
  let slotTimer: ReturnType<typeof setTimeout> | null = null;

  function pressSlot(section: "cars" | "tracks", state: SlotState, id: string | null | undefined) {
    slotClicks += 1;
    if (slotTimer) clearTimeout(slotTimer);
    slotTimer = setTimeout(() => {
      const clicks = slotClicks;
      slotClicks = 0;
      slotTimer = null;
      // Un emplacement vide ou en impasse n'a pas de fiche à ouvrir : le
      // double-clic y vaut le simple, il mène où le simple mène.
      if (clicks >= 2 && state === "picked" && id) void openSessionDetail(section, id);
      else void openSlot(section, state);
    }, DOUBLE_CLICK_MS);
  }

  /** Clic sur la vignette ou le nom : c'est la zone qui NAVIGUE (SPEC SESSION§1). Les
   * menus de livrée/layout et la ligne « Mon pilote » sont ses frères dans le
   * DOM, jamais ses enfants — un clic qui visait un menu ne doit pas éjecter
   * vers la bibliothèque. */
  async function openSlot(section: "cars" | "tracks", state: SlotState) {
    if (state === "broken") {
      nav.settingsTab = "paths";
      await requestSection("settings");
      return;
    }
    await requestSection(section);
  }

  // Historique de navigation (§7.2bis) : l'écran affiché est noté à chaque
  // fois qu'il change, quel que soit le chemin emprunté pour y arriver — la
  // douzaine d'endroits qui posent `nav.openFull` ou appellent
  // `requestSection` n'a donc rien à déclarer. Le détail est dans
  // `navHistory.ts`.
  $effect(() => {
    recordScreen({ section: nav.section, openFull: nav.openFull, openPack: nav.openPack });
  });

  // Boutons latéraux de la souris = précédent/suivant, comme dans un
  // navigateur. `preventDefault` sur le `mousedown` ET sur l'`auxclick` :
  // WebView2 mappe ces deux boutons sur SON historique de navigation, et une
  // app à route unique (adapter-static, SPA) n'a rien où reculer — au mieux il
  // ne se passe rien, au pire la webview quitte la page et l'app se retrouve
  // devant une fenêtre blanche.
  onMount(() => {
    const onMouseDown = (e: MouseEvent) => {
      // 3 et 4 = les deux boutons latéraux (« précédent » et « suivant » chez
      // Chromium), pas les trois boutons principaux.
      if (e.button !== 3 && e.button !== 4) return;
      e.preventDefault();
      if (e.button === 3) void goBack();
      else void goForward();
    };
    const swallow = (e: MouseEvent) => {
      if (e.button === 3 || e.button === 4) e.preventDefault();
    };
    window.addEventListener("mousedown", onMouseDown);
    window.addEventListener("auxclick", swallow);
    return () => {
      window.removeEventListener("mousedown", onMouseDown);
      window.removeEventListener("auxclick", swallow);
    };
  });

  // Sortie du mode Big Picture au clavier — pas d'autre chrome de fenêtre
  // visible une fois en plein écran pour cliquer un bouton "retour" évident.
  onMount(() => {
    const onKeydown = (e: KeyboardEvent) => {
      if (e.key === "Escape" && bigPictureState.active) exitBigPicture();
    };
    document.addEventListener("keydown", onKeydown);
    return () => document.removeEventListener("keydown", onKeydown);
  });

  // Ambiance musicale suit l'écran affiché tant que Big Picture est actif
  // (§4 de la spec musique) : GRID sur l'écran de paramétrage de la session
  // ("race", l'équivalent Pit Box de la grille de départ), MENU partout
  // ailleurs.
  $effect(() => {
    if (!bigPictureState.active) return;
    if (nav.section === "race") musicEnterGrid();
    else musicEnterMenu();
  });

  const carPrev = $derived(previewSrc(nav.sessionCar?.preview ?? null));
  const trackPrev = $derived(previewSrc(nav.sessionTrack?.preview ?? null));
  const trackOutline = $derived(previewSrc(nav.sessionTrack?.outline ?? null));

  const isLibrary = $derived(nav.section === "cars" || nav.section === "tracks");
  // Écrans « pleine page » qui gèrent leur propre défilement interne plutôt
  // que de compter sur le padding + le scroll de `.content` (évite le hack
  // de marge négative — cause probable de l'espace perdu au-dessus du titre
  // « Réglages » signalé par l'utilisateur, cf. Launch.svelte).
  // « Règles » en fait partie depuis que sa barre d'action du bas est un vrai
  // frère de la zone qui défile : en `position: fixed`, elle se plaçait dans
  // le repère de `100vh` (non divisé par le zoom d'interface) et passait donc
  // sous le bord bas de la fenêtre — voir le commentaire de `.r-footer`.
  const noPad = $derived(isLibrary || nav.section === "race" || nav.section === "rules");

  // Double-clic sur le slot de session : ouvre directement la fiche détail de
  // l'entité choisie (skin, layout…) plutôt que la liste de la bibliothèque.
  async function openSessionDetail(section: "cars" | "tracks", id: string | null | undefined) {
    // `openInSection` plutôt que `requestSection` puis `openMod` : les deux
    // écritures d'affilée n'exposent qu'un seul état, sinon l'historique
    // enregistre la liste de la bibliothèque comme un écran traversé et
    // « précédent » y ramène au lieu de rendre l'écran d'où l'on vient.
    if (id) await openInSection(section, id);
    else await requestSection(section);
  }

  // --- Sélecteurs rapides skin voiture / layout+skins circuit, directement
  // depuis le bloc SESSION (évite de passer par la fiche détail pour un
  // changement rapide). Mêmes actions que DetailPage.svelte (mémorise le
  // choix + met à jour le duo de session), réutilisées à l'identique.
  let carSkins = $state<SkinItem[]>([]);
  let carDetail = $state<Awaited<ReturnType<typeof getModDetail>>>(null);
  let trackDetail = $state<Awaited<ReturnType<typeof getModDetail>>>(null);
  let trackSkinOptions = $state<TrackSkinOption[]>([]);
  let trackSkinBusy = $state(false);

  $effect(() => {
    const carId = nav.sessionCar?.id ?? null;
    if (!carId) {
      carSkins = [];
      return;
    }
    listModSkins(carId).then((s) => {
      if (nav.sessionCar?.id === carId) carSkins = s;
    });
  });

  // État d'activation frais du duo de session (§ garde-fou lancement
  // ci-dessous) : jamais déduit de `nav.sessionCar`/`sessionTrack` eux-mêmes
  // (juste id/nom/preview pour l'affichage, persistés tels quels — une donnée
  // d'activation qui y serait figée resterait fausse dès que l'état change
  // ailleurs, ex. désactivé depuis la fiche détail sans repasser par ce
  // sélecteur). Repose sur le même effet-clé-sur-id que `trackDetail`
  // ci-dessous, qui écarte déjà les réponses obsolètes.
  $effect(() => {
    const carId = nav.sessionCar?.id ?? null;
    // Dépendance explicite : sans elle, désactiver la voiture de session
    // depuis sa fiche (ou en masse) ne rafraîchirait `carDetail` qu'au
    // prochain changement d'id — l'avertissement resterait faux jusqu'à ce
    // qu'on resélectionne la même voiture.
    libraryVersion();
    if (!carId) {
      carDetail = null;
      return;
    }
    getModDetail(carId).then((d) => {
      if (nav.sessionCar?.id === carId) carDetail = d;
    });
  });

  $effect(() => {
    const trackId = nav.sessionTrack?.id ?? null;
    libraryVersion();
    if (!trackId) {
      trackDetail = null;
      trackSkinOptions = [];
      return;
    }
    getModDetail(trackId).then((d) => {
      if (nav.sessionTrack?.id === trackId) trackDetail = d;
    });
    loadTrackSkinOptions(trackId);
  });

  // `null` tant que le détail n'est pas encore chargé (juste après une
  // sélection) : pas d'avertissement affiché dans ce court intervalle plutôt
  // que de risquer un faux positif pendant le chargement.
  const carInactive = $derived(nav.sessionCar != null && carDetail != null && !carDetail.active);

  /**
   * Le nom de la voiture tel que la colonne l'écrit — sans sa marque, qui est
   * juste au-dessus.
   *
   * Suit le réglage de la bibliothèque plutôt que de trancher dans son coin :
   * c'est la même décision d'affichage, elle n'a pas à se prendre deux fois.
   * `!== "0"` parce que l'option est désormais active par défaut, et
   * `peekUiPref` parce qu'un nom se rend, il ne s'attend pas — la valeur est
   * dans le cache réactif dès que la bibliothèque a chargé ses préférences, et
   * son absence donne le défaut, pas un écran vide.
   */
  const hideBrandPref = $derived(peekUiPref(StorageKey.gridHideBrand) !== "0");
  const sessionCarName = $derived.by(() => {
    const name = nav.sessionCar?.name ?? "";
    return hideBrandPref ? withoutBrand(name, carDetail?.brand ?? null) : name;
  });

  // --- Point d'entrée de l'écran Pilote (PILOTE§3.2) ---------
  //
  // Une ligne, pas trois menus : le choix a quitté cette colonne pour son
  // propre écran, parce que la hauteur y est la ressource rare et que
  // l'arrivée du corps y portait le nombre de listes à quatre (§D1). La ligne
  // porte le libellé, et un badge qui dit en un mot où en est le pilote.
  //
  // Ouvert aussi sur une voiture de course : le corps s'y pose comme sur
  // n'importe quelle voiture (§DRIVER3D_MODEL, docs/csp-driver-research.md),
  // et un verrou qui n'empêchait plus rien — un simple clic le franchissait —
  // ne faisait que décrire un état qui n'était même plus vrai. Retiré avec
  // l'utilisateur : le PILOTE§11.2 de la spec (voiture de course grisée) est donc
  // un écart assumé.
  /** La tenue de **cette** voiture, cascade résolue : la sienne si elle en a
   * une, la tenue par défaut si l'option est active, la livrée sinon. */
  /** La classe décide de **laquelle** des deux tenues par défaut s'applique
   * (course ou rue) ; `carDetail` la porte déjà. */
  const sessionCarClass = $derived(carClassOf(carDetail?.car_class));
  const driverPrefs = $derived(driverFor(nav.sessionCar?.id ?? null, sessionCarClass));

  /** Rien de choisi : la voiture et sa livrée décident de tout. */
  const driverUntouched = $derived(isEmpty(driverPrefs));

  /**
   * Ce que la ligne annonce.
   *
   * « Mon pilote » ne disait rien de ce qu'on porte. Trois cas, trois
   * réponses : le **nom de la tenue** quand on en a enregistré une et qu'on la
   * porte — c'est l'information la plus utile et c'est l'utilisateur qui l'a
   * écrite —, sinon la mention que rien n'a été touché, sinon qu'on a composé
   * quelque chose sans le nommer.
   */
  const driverLabel = $derived.by(() => {
    if (driverUntouched) return t("session.driverStock");
    const named = wornOutfit(driverPrefs)?.name;
    if (named) return named;
    // Une tenue héritée du défaut sans nom ne devrait pas exister — le défaut
    // *est* une tenue enregistrée — mais le dire plutôt que de mentir coûte
    // une ligne.
    return wearsFallback(nav.sessionCar?.id ?? null, sessionCarClass)
      ? t("session.driverFallback")
      : t("session.driverCustom");
  });

  /** Clé du badge, ou `null` (PILOTE§3.2). « Modifié » a disparu de la liste : le
   * libellé le dit déjà, et un badge qui répète la ligne qu'il accompagne
   * n'est que du bruit. */
  const driverBadge = $derived(driverPrefs.body ? "substituted" : null);

  /**
   * Vignette du corps substitué, dans la même colonne que celle de la livrée
   * juste au-dessus.
   *
   * La ligne « Mon pilote » disait qui pilote sans le montrer, seule de la
   * colonne dans ce cas : la voiture, la livrée et le circuit ont tous leur
   * image. Rien à rendre en plus pour autant — c'est la vignette de la
   * galerie de l'écran Pilote (`driverThumbs`), déjà sur disque dès qu'on y
   * est passé une fois, et la clé est celle de cet écran.
   *
   * **Vide quand aucun corps n'est substitué**, et c'est exact : le pilote
   * est alors celui que la voiture embarque, dont on ne connaît pas
   * l'identifiant côté interface. La case reste, la colonne tient.
   */
  const driverBodyThumb = $derived(
    nav.sessionCar && driverPrefs.body ? bodyThumb(nav.sessionCar.id + "|" + driverPrefs.body) : null,
  );
  $effect(() => {
    const car = nav.sessionCar;
    const body = driverPrefs.body;
    // `requestBodyThumb` est idempotent (il sort tout de suite si la vignette
    // est faite ou en cours) : cet effet, abonné à toutes les préférences par
    // `driverPrefs`, peut donc se redéclencher sans rien coûter.
    if (car && body) requestBodyThumb(car.id, car.skin ?? null, body);
  });

  const trackInactive = $derived(nav.sessionTrack != null && trackDetail != null && !trackDetail.active);

  // --- Garde d'activation (SESSION§3) ---
  //
  // La bibliothèque montre les mods désactivés, Assetto Corsa ne les voit pas :
  // lancer une session qui en contient échoue, et c'est un trou propre à Pit
  // Box — Content Manager ne montre que ce qui est installé.
  //
  // **Une ligne au-dessus du bouton, pas un dialogue au lancement.** C'était
  // un `confirm()` posé au clic : au moment où on l'ouvre, on est déjà parti
  // mentalement, et il ne couvrait que la voiture et le circuit. La ligne
  // couvre les trois, se voit avant de cliquer, et porte son remède ; le
  // bouton reste verrouillé tant qu'elle est là, donc aucune session ne peut
  // partir en échec.
  //
  // **Les doublons comptent pour un** : trois adversaires sur la même voiture
  // inactive font une seule activation, et la ligne annonce un mod, pas trois.
  let opponentDetails = $state<Record<string, Awaited<ReturnType<typeof getModDetail>>>>({});
  $effect(() => {
    const ids = gridMods.carIds;
    for (const id of ids) {
      if (id in untrack(() => opponentDetails)) continue;
      getModDetail(id).then((d) => {
        opponentDetails = { ...untrack(() => opponentDetails), [id]: d };
      });
    }
  });

  const inactiveMods = $derived.by(() => {
    const out = new Map<string, string>();
    if (carInactive && nav.sessionCar) out.set(nav.sessionCar.id, nav.sessionCar.name);
    if (trackInactive && nav.sessionTrack) out.set(nav.sessionTrack.id, nav.sessionTrack.name);
    for (const id of gridMods.carIds) {
      const d = opponentDetails[id];
      if (d && !d.active) out.set(id, d.display_name ?? id);
    }
    return [...out].map(([id, name]) => ({ id, name }));
  });

  let activating = $state(false);
  async function activateInactive() {
    if (activating) return;
    activating = true;
    try {
      for (const m of inactiveMods) await activateMod(m.id);
      // Relit tout de suite l'état frais : la ligne doit disparaître au clic,
      // pas au prochain changement de sélection.
      if (nav.sessionCar) carDetail = await getModDetail(nav.sessionCar.id);
      if (nav.sessionTrack) trackDetail = await getModDetail(nav.sessionTrack.id);
      const fresh: Record<string, Awaited<ReturnType<typeof getModDetail>>> = {};
      for (const id of gridMods.carIds) fresh[id] = await getModDetail(id);
      opponentDetails = fresh;
      bumpLibraryVersion();
    } catch (e) {
      await message(errorText(e), { title: t("session.activateFailedTitle"), kind: "error" });
    } finally {
      activating = false;
    }
  }

  // Bouton rouge « Démarrer la session » : lance directement avec les
  // réglages courants (dernier preset du type de session), sans repasser par
  // l'écran Paramétrage — pose le drapeau consommé par Launch.svelte une fois
  // monté et prêt (mêmes valeurs que si l'écran avait été ouvert normalement).
  //
  // L'activation n'est plus demandée ici : la garde ci-dessus verrouille le
  // bouton tant qu'un mod de la session est inactif, donc ce chemin n'est
  // atteint que sur une session lançable. L'activation reste **explicite** —
  // lancer une course ne doit pas modifier la bibliothèque dans le dos de
  // l'utilisateur, même si les liens durs rendent l'opération réversible.
  async function launchNow() {
    nav.autoLaunch = true;
    if (!(await requestSection("race"))) nav.autoLaunch = false;
  }

  async function loadTrackSkinOptions(trackId: string) {
    await syncTrackSkins(trackId);
    if (nav.sessionTrack?.id !== trackId) return;
    const opts = await listTrackSkinOptions(trackId);
    if (nav.sessionTrack?.id === trackId) trackSkinOptions = opts;
  }

  // `livery.png` (couleurs/motif du skin seul) plutôt que `preview` (photo de
  // la voiture entière, SESSION§1) : à 20px dans ce menu compact, la voiture
  // entière écrasée était illisible — repli sur `preview` si le skin n'a pas
  // de livery (convention pas garantie sur tous les skins).
  const carSkinOptions = $derived(
    carSkins.map((s) => ({ id: s.id, name: s.name, image: previewSrc(s.livery ?? s.preview) })),
  );
  // Le tracé (outline), pas la photo de fond : plus lisible en petite
  // miniature pour distinguer les layouts d'un même circuit d'un coup d'œil.
  const trackLayoutOptions = $derived(
    (trackDetail?.track?.layouts ?? []).map((l) => ({ id: l.id, name: l.name, image: previewSrc(l.outline) })),
  );
  const trackSkinChecklist = $derived(
    trackSkinOptions.map((o) => ({ name: o.name, image: previewSrc(o.image), active: o.active })),
  );

  function pickCarSkin(skinId: string) {
    const car = nav.sessionCar;
    const sk = carSkins.find((s) => s.id === skinId);
    if (!car || !sk) return;
    setPreferredSkin(car.id, sk);
    // `meta` ne porte plus la livrée (SPEC SESSION§1) : elle a sa propre ligne
    // juste dessous, il n'y a donc plus rien à y réécrire.
    pickSession("Car", {
      ...car,
      preview: sk.preview ?? car.preview,
      skin: sk.id,
    });
  }

  function pickTrackLayout(layoutId: string) {
    const track = nav.sessionTrack;
    const d = trackDetail;
    const l = d?.track?.layouts.find((x) => x.id === layoutId);
    if (!track || !d || !l) return;
    setPreferredLayout(d.id_interne, l);
    // Idem pour le tracé : sa ligne est juste dessous.
    pickSession("Track", {
      ...track,
      preview: l.preview ?? track.preview,
      layout: l.id,
      outline: l.outline,
    });
  }

  async function toggleTrackSkinFromSlot(name: string, active: boolean) {
    const trackId = nav.sessionTrack?.id;
    if (!trackId || trackSkinBusy) return;
    trackSkinBusy = true;
    try {
      await setTrackSkinActive(trackId, name, active);
      trackSkinOptions = await listTrackSkinOptions(trackId);
    } finally {
      trackSkinBusy = false;
    }
  }
</script>

{#if !bigPictureState.active}
  <TitleBar />
{/if}
<div class="frame" class:bigpicture={bigPictureState.active}>
  <div class="topbar"></div>
  <div class="shell">
    <!-- Zones parcourues par les gâchettes hautes de la manette (§7.4bis) :
         la barre latérale d'un côté, l'écran actif de l'autre. La
         bibliothèque redécoupe sa moitié en deux (liste et fiche) — les zones
         imbriquées les plus internes gagnent, voir `regions()`. -->
    <NavRail {alerts} {cmAvailable} />
    <aside class="side" data-gp-region="sidebar">
      <!-- La colonne répond à une seule question — « qu'est-ce que je
           lance ? » — et les deux blocs de mods ont exactement la même
           anatomie : vignette, nom, source, champs. Plus de traitement
           d'exception sur le bloc voiture.

           **L'ordre suit celui de la décision** : le circuit, puis la voiture
           qu'on y emmène, puis le genre de séance qu'on y fait — et le bouton
           de lancement tombe juste sous la liste des types, qui est le dernier
           choix avant de partir. -->
      <div class="session" style="--sess-lblw:{labelWidth}px">
        <div class="nsec">{t("session.trackTag")}</div>
        <div class="blk">
          <button
            class="pick"
            type="button"
            onclick={() => pressSlot("tracks", trackSlot, nav.sessionTrack?.id)}
            title={trackSlot === "picked" ? t("session.trackTooltip") : undefined}
            aria-label={nav.sessionTrack ? `${nav.sessionTrack.name} — ${t("session.changeTrack")}` : undefined}
          >
            <div class="thumb track" class:vacant={trackSlot !== "picked"} class:photo={trackSlot === "picked" && trackPrev}>
              {#if trackSlot === "picked"}
                {#if trackPrev}<img src={trackPrev} alt="" />{:else}<span class="thumb-ic">🏁</span>{/if}
                {#if trackOutline}<img class="outline" src={trackOutline} alt="" />{/if}
                <span class="veil"><span aria-hidden="true">✎</span>{t("session.changeTrack")}</span>
              {:else if trackSlot === "empty"}
                <span class="invite"><span aria-hidden="true">＋</span>{t("session.chooseTrack")}</span>
              {:else}
                <span class="invite">
                  {t("session.noTrackDetected")}
                  <small>{t("session.checkPaths")}</small>
                </span>
              {/if}
            </div>
            {#if nav.sessionTrack}
              <!-- Même composant que la voiture : les deux blocs ont la même
                   anatomie, et un circuit n'a simplement ni marque ni année.
                   Son auteur reste en dessous — c'est une source, pas une
                   partie de son nom. -->
              <ModIdentity name={nav.sessionTrack.name} year={trackDetail?.year}>
                {#snippet after()}
                  {#if trackInactive}<span class="warn" title={t("session.inactiveTooltip")}>⚠</span>{/if}
                {/snippet}
              </ModIdentity>
              {#if nav.sessionTrack.meta}<div class="psrc">{nav.sessionTrack.meta}</div>{/if}
            {/if}
          </button>
          {#if nav.sessionTrack}
            <ImageSelectDropdown
              label={t("session.fieldLayout")}
              options={trackLayoutOptions}
              selectedId={nav.sessionTrack.layout}
              placeholder={t("session.pickLayout")}
              emptyText={t("session.noLayoutsAvailable")}
              staticWhenSingle
              singleNote={t("session.layoutSingle")}
              onselect={pickTrackLayout}
              fit="contain"
            />
            <TrackSkinChecklistDropdown
              label={t("session.fieldTrackSkin")}
              options={trackSkinChecklist}
              busy={trackSkinBusy}
              ontoggle={toggleTrackSkinFromSlot}
            />
          {/if}
        </div>

        <div class="nsec section">{t("session.carTag")}</div>
        <div class="blk">
          <button
            class="pick"
            type="button"
            onclick={() => pressSlot("cars", carSlot, nav.sessionCar?.id)}
            title={carSlot === "picked" ? t("session.carTooltip") : undefined}
            aria-label={nav.sessionCar ? `${nav.sessionCar.name} — ${t("session.changeCar")}` : undefined}
          >
            <div class="thumb car" class:vacant={carSlot !== "picked"} class:photo={carSlot === "picked" && carPrev}>
              {#if carSlot === "picked"}
                {#if carPrev}<img src={carPrev} alt="" />{:else}<span class="thumb-ic">🚗</span>{/if}
                <!-- Le libellé n'est pas supprimé, il est différé (SPEC SESSION§1) : au
                     survol et au focus clavier seulement, sur un voile PLEIN — au
                     moment où l'on décide de changer, la voiture actuelle n'est
                     plus l'information utile, et un voile partiel rendrait le
                     libellé illisible sur une photo imprévisible. -->
                <span class="veil"><span aria-hidden="true">✎</span>{t("session.changeCar")}</span>
              {:else if carSlot === "empty"}
                <span class="invite"><span aria-hidden="true">＋</span>{t("session.chooseCar")}</span>
              {:else}
                <!-- Impasse (SPEC SESSION§1) : même trame que l'état initial, autre
                     destination — ici l'invitation à choisir mènerait à une
                     bibliothèque vide. -->
                <span class="invite">
                  {t("session.noCarDetected")}
                  <small>{t("session.checkPaths")}</small>
                </span>
              {/if}
            </div>
            {#if nav.sessionCar}
              <!-- Exactement la carte de bibliothèque, composant compris.
                   `meta` (qui valait « Nissan · 1999 ») disparaît du même
                   coup — cette ligne le dit mieux, et le répéter ferait deux
                   fois la même phrase dans huit pixels de haut. -->
              <ModIdentity
                name={sessionCarName}
                badge={carDetail?.badge}
                brand={carDetail?.brand}
                year={carDetail?.year}
                reserve
              >
                {#snippet after()}
                  {#if carInactive}<span class="warn" title={t("session.inactiveTooltip")}>⚠</span>{/if}
                {/snippet}
              </ModIdentity>
              <!-- L'auteur, comme sous le circuit : les deux blocs ont la même
                   anatomie, et il n'y avait pas de raison que la voiture soit
                   la seule à taire d'où elle vient. -->
              {#if carDetail?.author}<div class="psrc">{carDetail.author}</div>{/if}
            {/if}
          </button>
          {#if nav.sessionCar}
            <ImageSelectDropdown
              label={t("session.fieldLivery")}
              options={carSkinOptions}
              selectedId={nav.sessionCar.skin}
              placeholder={t("session.pickSkin")}
              emptyText={t("session.noSkinsAvailable")}
              staticWhenSingle
              onselect={pickCarSkin}
            />
            <!-- Le chevron est `›` et non `▾` : ce champ n'ouvre pas un menu
                 mais l'écran Pilote. Bas pour un menu, droite pour une
                 destination — la distinction est ténue mais constante. -->
            <button
              class="field"
              type="button"
              title={driverUntouched ? t("session.driverStockTooltip") : t("session.driverTooltip")}
              onclick={() => requestSection("driver")}
            >
              <span class="k">{t("session.fieldDriver")}</span>
              <span class="dthumb">
                {#if driverBodyThumb}<img src={driverBodyThumb} alt="" />{/if}
              </span>
              <span class="v" class:stock={driverUntouched}>{driverLabel}</span>
              {#if driverBadge}
                <span class="dl-badge">{t("session.driverBadge." + driverBadge)}</span>
              {/if}
              <span class="chev" aria-hidden="true">›</span>
            </button>

            <!-- PERFORMANCE (L5§2.1) : lest, bride, ABS et contrôle de
                 traction sous une ligne unique, au gabarit de LIVRÉE et
                 PILOTE. Quatre réglages valaient quatre lignes dans une colonne
                 dont la hauteur est la ressource rare, et les deux assistances
                 vivaient à l'autre bout de l'écran parmi les règles de la
                 course alors que ce sont des capacités de la VOITURE.

                 **La ligne affiche toujours son état**, repliée ou non : rien
                 n'est masqué, seulement rendu non modifiable — « absent » et
                 « à zéro » ne doivent pas se ressembler.

                 Chevron vers le bas et non vers la droite, contrairement à la
                 maquette : dans cette colonne, `›` annonce une destination
                 (l'écran Pilote) et `▾` un dépliement sur place. La distinction
                 est ténue mais constante, et c'est elle qui fait foi. -->
            <button
              class="field"
              type="button"
              aria-expanded={perfOpen}
              onclick={() => (perfOpen = !perfOpen)}
            >
              <span class="k">{t("session.fieldPerformance")}</span>
              <span class="v" class:stock={!perfTouched} class:set={perfTouched}>{perfSummary}</span>
              <span class="chev" aria-hidden="true">{perfOpen ? "▴" : "▾"}</span>
            </button>
            {#if perfOpen}
              <div class="perf">
                <label class="field">
                  <span class="k">{t("session.fieldBallast")}</span>
                  <input
                    class="hcap mono"
                    class:set={playerHandicap.ballast > 0}
                    type="number"
                    min="0"
                    max={BALLAST_MAX}
                    value={playerHandicap.ballast}
                    onchange={(e) => setPlayerHandicap(Number(e.currentTarget.value), playerHandicap.restrictor)}
                  />
                  <span class="unit">{t("session.ballastUnit")}</span>
                </label>
                <label class="field">
                  <span class="k">{t("session.fieldRestrictor")}</span>
                  <input
                    class="hcap mono"
                    class:set={playerHandicap.restrictor > 0}
                    type="number"
                    min="0"
                    max={RESTRICTOR_MAX}
                    value={playerHandicap.restrictor}
                    onchange={(e) => setPlayerHandicap(playerHandicap.ballast, Number(e.currentTarget.value))}
                  />
                  <span class="unit">%</span>
                </label>
                <!-- Trois états et non une case : une case ne saurait pas
                     distinguer « ce que la vraie voiture avait » de « forcé »,
                     et c'est le milieu qui est le défaut. -->
                <div class="assist">
                  <span class="k"
                    >{t("launch.absLabel")}<Tooltip text={t("launch.absTooltip")} align="left"
                      ><button type="button" class="info-i">ⓘ</button></Tooltip
                    ></span
                  >
                  <Seg
                    size="mini"
                    value={carAssists.abs}
                    onselect={(v) => (carAssists.abs = v as AssistLevel)}
                    items={assistLevels}
                  />
                </div>
                <div class="assist">
                  <span class="k"
                    >{t("launch.tcShort")}<Tooltip text={t("launch.tractionTooltip")} align="left"
                      ><button type="button" class="info-i">ⓘ</button></Tooltip
                    ></span
                  >
                  <Seg
                    size="mini"
                    value={carAssists.tractionControl}
                    onselect={(v) => (carAssists.tractionControl = v as AssistLevel)}
                    items={assistLevels}
                  />
                </div>
                {#if factoryLine}
                  <p class="factory-note"><span class="fw">{t("launch.assistFactory")}</span> — {factoryLine}</p>
                {/if}
              </div>
            {/if}
          {/if}
        </div>

        <div class="nsec section">{t("nav.session")}</div>

        <!-- LE TYPE DE SESSION EST LA NAVIGATION (L5§1). Les quatre types
             sont toujours visibles, jamais repliés derrière un sélecteur : ils
             annoncent ce que l'application sait faire, et c'est la seule chose
             de cette colonne qui ne se replie pas quand la hauteur manque.

             La sous-entrée « Adversaires » ne paraît que sous Course et Track
             day, et seulement quand ce type est sélectionné — en Essais et en
             Hotlap, la liste fait quatre lignes. -->
        <nav class="types" aria-label={t("nav.session")}>
          {#each SESSION_TYPES as type (type)}
            <button
              class="type"
              class:chosen={typeChosen(type)}
              class:on={typeSelected(type)}
              class:parent={onSessionScreen && sessionNav.type === type && sessionNav.page === "opponents"}
              type="button"
              onclick={() => (sessionNav.page === "opponents" ? backToSetup(type) : void goToType(type))}
              >{t(`launch.type.${type}`)}</button
            >
            {#if sessionNav.type === type && hasOpponents(type)}
              <!-- Indentée, et ce qui la rend lisible comme une descente est le
                   filet vertical qui la rattache à son parent : sans lui, deux
                   entrées de même gabarit à quelques pixels d'écart se lisent
                   comme deux destinations sœurs. -->
              <button
                class="type sub"
                class:on={onSessionScreen && sessionNav.page === "opponents"}
                type="button"
                onclick={() => void goToOpponents()}
              >
                <span class="sub-n">{t("launch.opponentsLabel")}</span>
                <!-- Le résumé passe en rouge et porte un marqueur quand la page
                     adversaires porte une alerte (L5§1.3) : une alerte sur une
                     page qu'on ne regarde pas ne vaut pas mieux que pas
                     d'alerte. -->
                <span class="sub-v" class:alert={sessionNav.alert}
                  >{#if sessionNav.alert}<span aria-hidden="true">⚠ </span>{/if}{opponentsSummary}</span
                >
              </button>
            {/if}
          {/each}
        </nav>

        {#if inactiveMods.length}
          <div class="warnbox guard">
            <span aria-hidden="true">⚠</span>
            <span class="guard-txt"
              >{inactiveMods.length === 1
                ? t("session.inactiveOne")
                : t("session.inactiveMany", { count: inactiveMods.length })}</span
            >
            <button class="guard-btn" type="button" disabled={activating} onclick={activateInactive}
              >{t("common.activate")}</button
            >
          </div>
        {/if}
        <!-- Cible du bouton Start de la manette (§7.4bis) : il y amène le
             curseur depuis n'importe quel écran, il ne lance pas lui-même. -->
        <button
          class="btn-launch"
          disabled={!sessionReady || inactiveMods.length > 0}
          {...{ [LAUNCH_BUTTON_ATTR]: "" }}
          onclick={launchNow}>{t("session.start")}</button
        >
        <!-- La sortie vers Content Manager a rejoint le pied du rail
             (`NavRail`), entre Réglages et À propos : cette colonne n'a plus
             de hauteur à donner à ce qui n'est pas la session. -->
      </div>

      <!-- Gabarit de mesure des intitulés de champ : hors flux, invisible, et
           surtout NON contraint en largeur — c'est la largeur naturelle du
           plus long qui décide de la colonne. -->
      <div class="lbl-probe" aria-hidden="true" bind:this={labelProbe}>
        {#each FIELD_LABELS as l}<span>{l}</span>{/each}
      </div>

      {#if bigPictureState.active}
        <!-- Seule sortie visible du mode Big Picture (plein écran, pas de
             chrome OS, barre de titre custom masquée) : bouton collant en
             bas de la barre latérale, position:sticky reste dans le flux
             normal donc ne peut jamais recouvrir les boutons au-dessus s'il
             manque de hauteur — il défile avec eux au lieu de les cacher. -->
        <button class="bigpicture-exit" type="button" onclick={exitBigPicture}>{t("bigpicture.exit")}</button>
      {/if}
    </aside>

    <div class="main-col">
      <main class="content" class:fixed={noPad} data-gp-region="main">
        {#if nav.section === "settings"}
          <Settings />
        {:else if nav.section === "about"}
          <About />
        {:else if nav.section === "cars"}
          <Library kind="Car" />
        {:else if nav.section === "tracks"}
          <Library kind="Track" />
        {:else if nav.section === "rules" || nav.section === "categories" || nav.section === "import" || nav.section === "profiles" || nav.section === "maintenance"}
          <Workshop />
        {:else if nav.section === "driver"}
          <DriverScreen />
        {:else if nav.section === "race"}
          <Launch />
        {:else if nav.section === "apps"}
          <!-- Les apps ont leur écran (REFONTE§3.2) : une app a un nom, une
               identité, on l'installe volontairement — elle n'est la dépendance
               de rien, et n'avait rien à faire dans un tiroir avec les polices
               et les fragments de config. -->
          <Apps />
        {:else if nav.section === "others"}
          <Inventory />
        {/if}
      </main>
    </div>
  </div>
</div>

<ImportOverlay />
<PendingDialog />

<!-- Tout ce que l'app a à dire sans interrompre, dans une seule colonne en bas
     à droite : progression et rapports d'import, nouveau périphérique. -->
<ToastStack>
  <PrefsToast />
  <ControllerToast />
  <BulkToasts />
  <RepairToast />
  <ImportToasts />
  <!-- La génération des vignettes en dernier, donc au plus près du coin : elle
       dure des minutes là où les autres passent, et c'est celle qu'on revient
       consulter. -->
  {#if FEATURE_GRID_THUMBS}<GridThumbToast />{/if}
</ToastStack>
{#if controllers.setupOpen}
  <ControllerSetup onclose={() => (controllers.setupOpen = false)} />
{/if}

<style>
  .frame {
    background: var(--panel);
    border: 1px solid var(--rosso);
    /* `zoom` (voir zoom.svelte.ts) agrandit tout le rendu, mais vh/vw restent
       relatifs à la fenêtre réelle — sans cette division, .frame devient plus
       haut que la fenêtre à >100% (rien à scroller pour atteindre le bas :
       bouton Enregistrer hors champ, coquille tronquée un peu partout). */
    height: calc(100vh / var(--ui-zoom, 1));
    /* Barre de titre custom en position fixe (voir TitleBar.svelte) : réserve
       sa hauteur ici plutôt que de la compter comme un enfant flex, pour
       qu'elle reste toujours à l'écran quel que soit ce qui défile en dessous. */
    padding-top: 32px;
    display: flex;
    flex-direction: column;
  }
  /* Big Picture : pas de barre de titre custom (masquée, gagne en hauteur)
     ni de bordure — rendu bord à bord, immersif. */
  .frame.bigpicture {
    padding-top: 0;
    border: none;
  }
  .topbar {
    background: var(--rosso);
    height: 3px;
    flex: none;
  }
  .frame.bigpicture .topbar {
    display: none;
  }
  .shell {
    flex: 1;
    min-height: 0;
    display: grid;
    /* Rail (lieux) · colonne de session (ce qu'on lance) · contenu.
       222px tant que la bibliothèque gardait son panneau de détail à droite ;
       celui-ci retiré, la zone principale n'a plus besoin d'autant de largeur
       et la colonne de session peut respirer — c'est elle qui porte le duo
       voiture/circuit et ses menus. Sa contrainte reste la **hauteur** : tout
       ce qu'on y ajoute doit tenir sans allonger la colonne, d'où la largeur
       prise ici (réglée à l'œil avec l'utilisateur). */
    grid-template-columns: 74px 328px 1fr;
  }
  .side {
    background: var(--bg);
    border-right: 1px solid var(--line);
    overflow-y: auto;
    /* **La colonne ne doit jamais défiler** (L5§2.3), et ce qu'on réduit
       quand le compte n'y est pas, ce sont les deux vignettes. Le seuil est
       donc une requête de CONTENEUR et non de média : une `@media
       (max-height)` interroge la fenêtre, que le zoom d'interface ne touche
       pas — à 150 %, une fenêtre de 1080 px n'offre plus que 720 px de mise en
       page et la règle ne se déclencherait pas. Le conteneur, lui, mesure la
       hauteur réellement disponible.
       960 et non les 900 de la spec : c'est une **mesure**, pas un nombre rond
       — la colonne la plus chargée (Course sélectionnée, sous-entrée affichée,
       replis fermés) faisait un millier de pixels, relevés à l'écran par
       bissection du seuil jusqu'à ce qu'il bascule, et le départ du bandeau de
       marque vers la barre de titre lui en a rendu une cinquantaine. En dessous
       de ce seuil elle n'a plus de marge, et c'est exactement là qu'il faut
       réagir : 900 l'aurait laissée déborder de quelques pixels — le bas de la
       colonne passe alors sous le bord de la fenêtre — avant que la règle ne se
       déclenche. Une fenêtre de 1920 × 1080 au zoom d'origine offre 1044 px à
       la colonne : elle garde ses vignettes entières, ce que la spec demande. */
    container: sidecol / size;
  }
  /* Titres de section du rail : mono, majuscules espacées, séparateur.
     Plus rouges depuis le barème de l'accent (SPEC §7.2ter) : un titre de section est de la
     STRUCTURE, pas un état — et ils sont assez nombreux, répartis sur toute
     la hauteur de la colonne, pour que leur filet mette le seul bouton rouge
     de l'écran (« Démarrer la session ») en concurrence avec quatre titres.
     Les capitales espacées suffisent à les faire lire comme des titres. */
  .nsec {
    color: var(--muted);
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 2px;
    padding: 14px 13px 8px;
    font-family: var(--mono);
    text-transform: uppercase;
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .nsec::after {
    content: "";
    flex: 1;
    height: 1px;
    background: var(--line);
  }

  .session {
    padding: 0 13px 4px;
  }
  .session .nsec {
    padding-left: 0;
    padding-right: 0;
  }
  /* Le second titre suit un champ, pas une marge de bloc : il lui faut un peu
     plus d'air pour que les deux blocs se lisent comme deux blocs. */
  .session .nsec.section {
    padding-top: 18px;
  }
  /* Un bloc = vignette cliquable + champs. Aucune bordure de SÉLECTION : elle
     n'aurait de sens que parmi des pairs, or il n'y a qu'une voiture, et
     l'entrée active se dit dans le rail (SPEC §7.2).
     Mais un plan, oui — et le même que la carte de bibliothèque (`--cell` /
     `--cell-line`), puisque les deux montrent désormais la même chose avec le
     même composant. Le `.blk` global posait l'inverse : un fond PLUS SOMBRE
     que la colonne, sans marge intérieure — donc un texte collé au filet, et
     une boîte qui se lisait comme un trou plutôt que comme une carte. Les
     8 px sont ceux de `.card`, pas une valeur de plus à faire diverger. */
  .blk {
    display: block;
    background: var(--cell);
    border-color: var(--cell-line);
    padding: 8px;
    /* La colonne de session est large de 328 px : le nom y a la place de la
       grille confortable, pas celle de la dense. */
    --ident-size: 12.5px;
  }
  /* **`:global` obligatoire, et ce n'était pas une précaution de style.** Les
     enfants de ce bloc sont des COMPOSANTS (`ImageSelectDropdown`,
     `TrackSkinChecklistDropdown`) : leur élément racine porte le hachage de
     leur propre fichier, pas celui-ci. Svelte compilait donc
     `.blk > * + *` en `.blk.svelte-xxx > :where(.svelte-xxx) + :where(.svelte-xxx)`,
     que le `<div class="isd">` du sélecteur ne satisfait jamais — **la règle
     des cinq pixels n'a donc jamais rien espacé**, et la correction de la
     marge sous le nom, écrite de la même façon, n'a rien corrigé non plus.
     Cherché dans le CSS compilé après un premier essai infructueux ; c'est le
     seul endroit où ça se voit. */
  .blk > :global(* + *) {
    margin-top: 5px;
  }
  /* **Le nom du mod ne touche pas le champ qui suit.** Cinq pixels séparent
     bien deux champs entre eux — ils forment une liste — mais pas une identité
     d'un contrôle : le nom se lisait collé à la liste déroulante « Livrée »,
     comme s'il en était l'étiquette. L'écart marque la frontière entre ce
     qu'on a choisi et ce qu'on règle dessus. */
  .blk > :global(.pick + *) {
    margin-top: 12px;
  }
  /* Zone qui NAVIGUE (SPEC SESSION§1) : vignette + nom + source, et rien d'autre.
     C'est un `<button>` FRÈRE des champs, jamais leur parent — un bouton qui
     contient des contrôles interactifs est invalide en HTML et casse la
     navigation clavier, et un clic qui visait un menu ne doit jamais éjecter
     vers la bibliothèque. */
  .pick {
    display: block;
    width: 100%;
    padding: 0;
    background: none;
    border: none;
    text-align: left;
    cursor: pointer;
  }
  .thumb {
    /* Rapport de repli, pour les états qui n'ont pas d'image à montrer
       (emplacement vide, chemins cassés, mod sans photo) : sans lui la boîte
       n'aurait aucune hauteur. Dès qu'il y a une photo, c'est ELLE qui donne
       la hauteur — voir `.thumb.photo`. */
    aspect-ratio: 2.3;
    display: flex;
    align-items: center;
    justify-content: center;
    position: relative;
    border: 1px solid var(--line);
    overflow: hidden;
    background: linear-gradient(135deg, #1a0808, var(--panel));
  }
  .thumb.track {
    aspect-ratio: 2.9;
    background: linear-gradient(135deg, #0a1a14, var(--panel));
  }
  /* Tracé du layout superposé à la photo du circuit (comme la fiche). */
  /* Le tracé est un CALQUE, pas la photo : il garde la boîte entière, quelle
     que soit la hauteur que la photo lui a donnée (sans les deux dimensions
     explicites, le `height: auto` de la règle du dessus le ferait retomber
     sur sa taille intrinsèque et déborder). */
  .thumb img.outline {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: contain;
    padding: 8px;
  }
  /* **La photo est montrée entière.** Elle l'était en `cover` sous un rapport
     imposé, donc rognée en haut et en bas — les roues d'une voiture et les
     bords d'un tracé y passaient, et ça se voyait (signalé à l'écran). C'est
     donc la boîte qui prend le rapport de l'image, jamais l'inverse : ni
     recadrage, ni bandes vides. */
  .thumb.photo {
    aspect-ratio: auto;
  }
  .thumb.photo img {
    display: block;
    width: 100%;
    height: auto;
  }
  /* Fenêtre basse : la photo et le plan tombent à la moitié de leur hauteur
     (L5§2.3, premier des trois recours). Un plafond de hauteur plutôt
     qu'un rapport d'image, parce que la boîte prend le rapport de SA photo
     dès qu'il y en a une (`.thumb.photo`) — c'est donc la hauteur qu'il faut
     borner, et le recadrage est préférable à une image écrasée. */
  @container sidecol (max-height: 960px) {
    /* Hauteur EXPLICITE et non un plafond : la boîte tire sa hauteur de son
       image (`aspect-ratio: auto`), donc un `max-height` ne donnerait à
       l'image aucune hauteur de référence à laquelle se rapporter. */
    .thumb.photo {
      height: 68px;
    }
    .thumb.photo img {
      height: 100%;
      object-fit: cover;
    }
    .thumb:not(.photo) {
      aspect-ratio: 4.6;
    }
    .thumb.track:not(.photo) {
      aspect-ratio: 5.8;
    }
  }
  .thumb-ic {
    font-size: 34px;
    opacity: 0.6;
  }
  /* Voile du libellé différé (SPEC SESSION§1) : plein, pas dégradé — au moment où
     l'on décide de changer, la photo n'est plus l'information utile, et un
     voile partiel rendrait le texte illisible sur une image imprévisible. Au
     focus clavier comme au survol : le libellé doit être atteignable sans
     souris. */
  .veil {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 7px;
    background: rgba(8, 8, 10, 0.72);
    color: var(--txt);
    font-size: 10.5px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    opacity: 0;
    transition: opacity 0.13s;
  }
  .pick:hover .veil,
  .pick:focus-visible .veil {
    opacity: 1;
  }
  .pick:hover .thumb {
    border-color: var(--faint2);
  }
  /* Rien de choisi : trame diagonale et bordure pointillée disent
     « emplacement à remplir » sans imiter une vignette vide, qui se lirait
     comme un mod sans photo. */
  .thumb.vacant {
    background: repeating-linear-gradient(135deg, #141518, #141518 6px, #17181c 6px, #17181c 12px);
    border-style: dashed;
    border-color: var(--faint2);
  }
  .invite {
    color: var(--txt2);
    font-size: 11.5px;
    letter-spacing: 0.06em;
    text-align: center;
    padding: 0 10px;
  }
  /* Le glyphe est un frère en ligne du libellé, pas un élément de grille :
     l'écart se pose ici plutôt qu'avec une espace dans la chaîne traduite. */
  .invite span {
    margin-right: 5px;
  }
  .invite small {
    display: block;
    margin-top: 3px;
    color: var(--muted);
    font-size: 10px;
    letter-spacing: 0;
  }
  /* Mod sélectionné mais non activé (§ garde-fou lancement) : jaune = alerte,
     cohérent avec les couleurs sémantiques du projet. */
  .warn {
    color: var(--yellow);
    margin-left: 4px;
  }
  /* Source, pas résumé : la marque et l'année pour une voiture, l'auteur pour
     un circuit. Ce qui est déjà écrit au-dessus (le nom) ou juste en dessous
     (la livrée, le tracé) n'y est pas répété — on ne paie pas des caractères
     pour une information présente à quelques pixels. */
  .psrc {
    margin-top: 1px;
    font-size: 10.5px;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  /* Champ nommé : l'intitulé est une COLONNE, pas une ligne au-dessus. Coût
     en hauteur : zéro — la ligne reste à 30 px, là où un intitulé posé
     au-dessus aurait coûté 14 px par champ pour le même service. Mêmes
     valeurs que `ImageSelectDropdown` : ces lignes doivent s'aligner au pixel
     avec les siennes. */
  .field {
    display: flex;
    align-items: center;
    /* 8 px, comme `.isd-trigger.labelled` juste au-dessus — qui était resté à
       9 malgré ce commentaire, d'où 2 px d'écart sur le début de la valeur.
       Les deux lignes partagent la colonne d'intitulé et portent chacune une
       vignette de 28 px — un pixel d'écart ici décale la valeur de l'une par
       rapport à l'autre, ce qui se voit d'autant mieux qu'elles sont
       voisines. */
    gap: 8px;
    width: 100%;
    height: 30px;
    padding: 0 9px;
    background: var(--panel2);
    border: 1px solid var(--line);
    color: var(--txt2);
    font-size: 11px;
    text-align: left;
    cursor: pointer;
  }
  .field:hover {
    border-color: var(--faint2);
  }
  .field .k {
    flex: 0 0 var(--sess-lblw, 60px);
    max-width: 88px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 8.5px;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--muted);
  }
  /* Même case que `.isd-thumb` du sélecteur de livrée, aux mêmes dimensions :
     c'est leur alignement vertical qui fait tout l'intérêt. Elle est posée
     même vide — un cadre qui apparaît et disparaît décalerait le nom du
     pilote d'une voiture à l'autre. Pleine hauteur de ligne depuis qu'à 13 px
     on ne reconnaissait rien de ce qu'elle montre, et donc sans cadre à elle :
     le raisonnement complet est dans `ImageSelectDropdown`. */
  .dthumb {
    flex: none;
    width: 28px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--raised);
    overflow: hidden;
  }
  .dthumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    /* **Recadrage sur la tête, en CSS et pas au rendu.** La vignette source est
       un buste (du haut du casque à la poitrine). Mesuré sur les 85 vignettes
       du cache : la tête va du bord haut (médiane 4 %, au pire 16 %) à 39 % de
       la hauteur (p90 47 %). Ce couple montre donc la tranche 0-52 % de
       l'image source, soit la tête entière et un doigt d'épaules.
       **Il tient même à 28 px**, et l'essai inverse a été fait : ouvert au
       haut du buste, la ligne montrait le torse et les jambes, qui ne
       distinguent aucun mannequin d'un autre. Ce qu'on reconnaît d'un pilote
       est la forme de son casque — sur une ligne d'une seule hauteur de
       texte, le reste n'est que du remplissage.
       En CSS et non dans `driverThumbs` parce que le même PNG sert la galerie
       de l'écran Pilote, où il est affiché à 104 px et où le buste est le bon
       cadrage — et parce que le recalculer invaliderait les 85 vignettes déjà
       sur disque pour un problème qui n'existe qu'ici. */
    transform: translateY(46%) scale(1.9);
  }
  /* Champ nu : c'est la ligne qui porte le cadre, comme les cellules du
     plateau. Un `NumberStepper` y mettrait un second cadre dans le premier. */
  .hcap {
    flex: 1;
    min-width: 0;
    background: transparent;
    border: 0;
    padding: 0;
    color: var(--faint);
    font-size: 11px;
    text-align: right;
    appearance: textfield;
  }
  .hcap::-webkit-outer-spin-button,
  .hcap::-webkit-inner-spin-button {
    appearance: none;
    margin: 0;
  }
  /* Zéro est éteint — il n'y a pas de handicap —, toute autre valeur est rouge.
     C'est le seul moyen qu'un lest oublié se voie sans lire la ligne, et le
     rouge est ici au sens du barème : un réglage qui change la course. */
  .hcap.set {
    color: var(--rosso-bright);
  }
  .field .unit {
    flex: none;
    font-size: 9px;
    color: var(--muted);
  }
  .field .v {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--txt);
  }
  /* Valeur par défaut : un possessif en italique grise, jamais une négation —
     même formulation que « Celle de la livrée » de l'écran Pilote. */
  .field .v.stock {
    color: var(--muted);
    font-style: italic;
  }
  /* Un réglage posé sur la voiture se voit sans lire la ligne, comme le lest
     dans son champ : rouge au sens du barème (§7.2ter) — un réglage qui change
     la course. */
  .field .v.set {
    color: var(--rosso-bright);
    font-size: 10px;
  }
  .field .chev {
    flex: none;
    color: var(--faint);
    font-size: 9px;
  }
  .dl-badge {
    flex: 0 0 auto;
    font-size: 9.5px;
    letter-spacing: 0.12em;
    color: var(--muted);
    border: 1px solid var(--line);
    border-radius: 2px;
    padding: 1px 5px;
  }
  /* Hors flux et sans contrainte de largeur : sert uniquement à mesurer le
     plus long intitulé de la locale courante. `visibility: hidden` et non
     `display: none` — un élément non rendu n'a pas de largeur à lire. */
  .lbl-probe {
    position: absolute;
    visibility: hidden;
    pointer-events: none;
    font-size: 8.5px;
    letter-spacing: 0.16em;
    text-transform: uppercase;
  }
  .lbl-probe span {
    display: block;
    width: max-content;
    white-space: nowrap;
  }
  /* --- La liste des types de session (L5§1) ---------------------------
     Même langage que le rail de navigation : le repos est en retrait, l'entrée
     retenue s'éclaircit et prend un filet rouge sur son bord d'attaque —
     niveau 2 du barème (SPEC §7.2ter), jamais un fond plein, qui reste au seul
     bouton de lancement. */
  .types {
    display: flex;
    flex-direction: column;
    margin-bottom: 4px;
  }
  .type {
    position: relative;
    display: flex;
    align-items: baseline;
    gap: 8px;
    /* **Pas de `width: 100%`** : la colonne est un `flex` vertical, ses enfants
       s'étirent déjà d'eux-mêmes. Posée en dur, cette largeur s'ajoutait au
       `margin-left` de la sous-entrée — une marge vit hors de la boîte, même en
       `border-box` — et la colonne débordait de quatorze pixels. `.side` étant
       en `overflow-y: auto`, une règle CSS lui calcule l'autre axe en `auto`
       aussi : d'où une barre de défilement horizontale pour quatorze pixels. */
    padding: 6px 10px;
    background: none;
    border: none;
    color: var(--muted);
    font-size: 12px;
    text-align: left;
  }
  .type:hover {
    color: var(--txt2);
    background: var(--panel2);
  }
  /* Le type qui partira : lisible en pleine lumière, sans rouge — c'est une
     valeur, pas un écran. */
  .type.chosen {
    color: var(--txt);
  }
  .type.on::before {
    content: "";
    position: absolute;
    left: 0;
    top: 3px;
    bottom: 3px;
    width: 2px;
    background: var(--rosso);
  }
  /* Le type qui porte la sous-entrée ouverte : en retrait d'un cran par
     rapport à celle-ci, mais toujours lisible — c'est lui qu'on clique pour
     remonter. */
  .type.parent {
    color: var(--txt2);
  }
  /* **L'indentation seule ne dit pas « descente »** — c'est la faiblesse connue
     de cette structure, et le filet vertical est ce qui la corrige : la
     sous-entrée est visiblement accrochée au type au-dessus d'elle, donc
     cliquer celui-ci se lit comme une remontée. */
  .sub {
    flex-direction: column;
    align-items: stretch;
    gap: 1px;
    margin-left: 14px;
    padding-left: 12px;
    border-left: 1px solid var(--line);
    font-size: 11px;
  }
  .sub.on {
    border-left-color: var(--rosso);
  }
  /* Le filet d'attaque appartient au type, pas à sa sous-entrée : celle-ci a
     déjà le sien, à gauche, et deux traits rouges à 14 px l'un de l'autre se
     liraient comme deux sélections. */
  .sub.on::before {
    display: none;
  }
  .sub-v {
    font-size: 9.5px;
    color: var(--muted);
    font-family: var(--mono);
  }
  /* Une alerte vivant sur la page adversaires (L5§1.3). Niveau 2 du barème : le
     libellé passe en rouge, rien de plein. */
  .sub-v.alert {
    color: var(--rosso-bright);
  }

  /* --- Repli « Performance » (L5§2.1) -------------------------------- */
  .perf {
    display: flex;
    flex-direction: column;
    gap: 5px;
    /* Retrait et filet : les quatre réglages appartiennent à la ligne qui les
       a ouverts, comme la sous-entrée appartient à son type. */
    margin-left: 8px;
    padding-left: 8px;
    border-left: 1px solid var(--line);
  }
  /* Même gouttière d'intitulé que `.field`, sans son cadre : ce n'est pas un
     champ mais un réglage posé sous la ligne qui le commande. */
  .assist {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 26px;
  }
  .assist .k {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    flex: 0 0 var(--sess-lblw, 60px);
    max-width: 88px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 8.5px;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--muted);
  }
  /* Même ⓘ que partout : une explication permanente vit là, jamais dans un
     encart jaune. */
  .info-i {
    background: transparent;
    border: none;
    padding: 0;
    color: var(--muted2);
    font-size: 10px;
    line-height: 1;
  }
  .info-i:hover {
    background: transparent;
    color: var(--txt2);
  }
  /* Ce que l'app sait et que le volant seul apprendrait — même registre que la
     note implicite de la météo. Le bleu est l'information au barème (§7.2ter) :
     rien ne va mal ici. */
  .factory-note {
    color: var(--muted);
    font-size: 10px;
    margin: 2px 0 0;
    line-height: 1.45;
  }
  .factory-note .fw {
    color: var(--blue);
  }
  .btn-launch {
    width: 100%;
    height: 40px;
    background: var(--rosso);
    color: #fff;
    font-size: 10.5px;
    letter-spacing: 1.5px;
    font-weight: 600;
    font-family: var(--mono);
    /* Détaché de la liste des types : il suit désormais le dernier choix qu'on
       fait avant de partir, et deux pixels le faisaient lire comme une
       cinquième entrée de cette liste. */
    margin-top: 12px;
  }
  /* L'encadré vient de `.warnbox` (global) : jaune, parce que ce n'est pas une
     erreur mais une condition réparable d'un clic, et parce que le rouge de
     cette colonne appartient au lancement (§7.2ter). Ne reste ici que la mise
     en ligne et le bouton, que `.warnbox` ne connaît pas. */
  .guard {
    display: flex;
    align-items: center;
    gap: 9px;
    margin-bottom: 8px;
    font-size: 10.5px;
  }
  .guard-txt {
    flex: 1;
  }
  .guard-btn {
    background: transparent;
    border: 1px solid currentcolor;
    color: inherit;
    font-size: 9px;
    letter-spacing: 0.09em;
    text-transform: uppercase;
    padding: 5px 9px;
    flex: none;
  }
  .guard-btn:hover:not(:disabled) {
    background: rgb(241 216 60 / 12%);
  }
  .btn-launch:hover:not(:disabled) {
    background: var(--rosso-bright);
  }
  /* Garde son fond rouge à l'état désactivé, en opacité réduite (SPEC SESSION§1) : il
     reste la destination visible de l'écran, et le griser complètement
     effacerait le but à atteindre. */
  .btn-launch:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .main-col {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }
  .content {
    flex: 1;
    min-height: 0;
    padding: 28px 32px;
    overflow: auto;
  }
  /* Bibliothèques : hauteur fixe + défilement interne (évite la double scrollbar). */
  .content.fixed {
    padding: 0;
    overflow: hidden;
  }

  /* Seule sortie visible du mode Big Picture (plein écran, pas de chrome
     OS, barre de titre custom masquée). Dernier enfant de .side : `sticky`
     reste dans le flux normal (contrairement à `fixed`), donc ne peut pas
     recouvrir les boutons de navigation au-dessus s'il manque de hauteur —
     il défile avec eux au lieu de les cacher. Couleur bleue (secondaire,
     "info" — le rouge est déjà pris par primaire/destructif partout
     ailleurs) pour ne pas se confondre avec Lancer/Paramétrage. */
  .bigpicture-exit {
    position: sticky;
    bottom: 0;
    width: 100%;
    margin-top: 8px;
    padding: 12px 13px;
    background: var(--blue-dim);
    border-top: 1px solid var(--blue-border);
    color: var(--blue);
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 1px;
    font-family: var(--mono);
  }
  .bigpicture-exit:hover {
    background: var(--blue-border);
    color: var(--txt);
  }
</style>
