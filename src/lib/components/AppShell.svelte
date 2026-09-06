<script lang="ts">
  import { onMount } from "svelte";
  import Settings from "./Settings.svelte";
  import About from "./About.svelte";
  import Library from "./Library.svelte";
  import Launch from "./Launch.svelte";
  import Transversal from "./Transversal.svelte";
  import DriverScreen from "./driver/DriverScreen.svelte";
  import OtherMods from "./OtherMods.svelte";
  import NavRail from "./NavRail.svelte";
  import Workshop from "./Workshop.svelte";
  import ImportOverlay from "./ImportOverlay.svelte";
  import PendingDialog from "./PendingDialog.svelte";
  import ImportToasts from "./ImportToasts.svelte";
  import ToastStack from "./ToastStack.svelte";
  import ControllerToast from "./ControllerToast.svelte";
  import PrefsToast from "./PrefsToast.svelte";
  import BulkToasts from "./BulkToasts.svelte";
  import TitleBar from "./TitleBar.svelte";
  import ControllerSetup from "./ControllerSetup.svelte";
  import ImageSelectDropdown from "./ImageSelectDropdown.svelte";
  import ModIdentity from "./ModIdentity.svelte";

  import { carClassOf, driverFor, isEmpty, wearsFallback } from "$lib/driverOverride.svelte";
  import { bodyThumb, requestBodyThumb } from "$lib/driverThumbs.svelte";
  import { wornOutfit } from "$lib/driverOutfits.svelte";
  import TrackSkinChecklistDropdown from "./TrackSkinChecklistDropdown.svelte";
  import { nav, requestSection, pickSession } from "$lib/nav.svelte";
  import { recordScreen, goBack, goForward } from "$lib/navHistory";
  import { previewSrc, getModDetail, activateMod } from "$lib/library";
  import { withoutBrand } from "$lib/displayName";
  import { peekUiPref } from "$lib/uiPrefs.svelte";
  import { StorageKey } from "$lib/storage";
  import { confirm, message } from "@tauri-apps/plugin-dialog";
  import { errorText } from "$lib/errors";
  import { initGlobalDragDrop } from "$lib/importState.svelte";
  import { initBulkProgress } from "$lib/bulkState.svelte";
  import { openContentManager, listModSkins, type SkinItem } from "$lib/launch";
  import { listOtherMods } from "$lib/others";
  import { setPreferredSkin, setPreferredLayout } from "$lib/preferred";
  import { syncTrackSkins, listTrackSkinOptions, setTrackSkinActive, type TrackSkinOption } from "$lib/submods";
  import { t, setLocale } from "$lib/i18n/index.svelte";
  import { setZoom, zoomFactor } from "$lib/zoom.svelte";
  import { getConfig, validateConfig } from "$lib/config";
  import { LAUNCH_BUTTON_ATTR, startGamepadNav } from "$lib/gamepadNav";
  import { controllers, startControllerWatch } from "$lib/gamepadDevices.svelte";
  import { bigPictureState, exitBigPicture } from "$lib/bigpicture.svelte";
  import { musicEnterMenu, musicEnterGrid } from "$lib/music";
  import { libraryVersion } from "$lib/libraryVersion.svelte";

  // Trois territoires étanches (SPEC §7.2) : le RAIL porte les lieux
  // (`NavRail.svelte`), la BARRE DE TITRE la forme de la fenêtre, cette
  // COLONNE ce qu'on lance. Les deux grilles de boutons « Add-ons » et
  // « Atelier » qui vivaient ici sont parties dans le rail : elles n'étaient
  // pas mal dessinées, elles étaient mal placées — la colonne de session
  // faisait office de navigation en plus de son travail propre.
  async function openCm() {
    try {
      await openContentManager();
    } catch (e) {
      console.error(e);
    }
  }

  // Glisser-déposer disponible partout : un seul listener, monté ici à la
  // racine, plutôt que dans chaque écran susceptible de recevoir un drop.
  onMount(() => initGlobalDragDrop());

  // Progression des actions groupées (§6.3bis) : un seul écouteur, monté ici
  // comme le glisser-déposer — un lot lancé depuis la bibliothèque doit rester
  // visible même si on change d'écran pendant.
  onMount(() => initBulkProgress());

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
  // Trois états, pas deux (SPEC §9.1) : un mod choisi, rien de
  // choisi, ou des chemins cassés. Le troisième existe parce que l'invitation
  // « choisir une voiture » mène à une bibliothèque vide quand Assetto Corsa
  // est introuvable — ce n'est pas le même problème, donc pas la même
  // destination (Réglages › Chemins).
  type SlotState = "picked" | "empty" | "broken";
  let pathsBroken = $state(false);
  /** Content Manager introuvable : le lien de sortie ne s'affiche pas du tout
   * (SPEC §9.1). Ni bouton grisé ni message d'erreur au clic — une sortie vers
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
   * session à lancer (SPEC §9.1) — l'app ne choisit pas une voiture à la place de
   * l'utilisateur pour se donner un bouton à activer. */
  const sessionReady = $derived(nav.sessionCar != null && nav.sessionTrack != null);

  // --- Colonne d'intitulés partagée (SPEC §9.1) -----------------------------
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

  /** Clic sur la vignette ou le nom : c'est la zone qui NAVIGUE (SPEC §9.1). Les
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
    if (await requestSection(section) && id) nav.openMod = id;
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

  // --- Point d'entrée de l'écran Pilote (SPEC-ecran-pilote §3.2) ---------
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
  // l'utilisateur : le §11.2 de la spec (voiture de course grisée) est donc
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

  /** Clé du badge, ou `null` (§3.2). « Modifié » a disparu de la liste : le
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

  // Bouton rouge « Démarrer la session » : lance directement avec les
  // réglages courants (dernier preset du type de session), sans repasser par
  // l'écran Paramétrage — pose le drapeau consommé par Launch.svelte une fois
  // monté et prêt (mêmes valeurs que si l'écran avait été ouvert normalement).
  //
  // Garde-fou activation (§ bug réel signalé) : lancer une session avec une
  // voiture/un circuit sélectionné mais non activé (jamais junctionné dans
  // `content/`) fait planter Content Manager/AC, qui ne trouve pas le contenu.
  // On bloque, on demande confirmation, et on active avant de laisser
  // continuer — jamais d'activation silencieuse sans accord explicite.
  async function launchNow() {
    const toActivate: { id: string; name: string }[] = [];
    if (carInactive && nav.sessionCar) toActivate.push({ id: nav.sessionCar.id, name: nav.sessionCar.name });
    if (trackInactive && nav.sessionTrack) toActivate.push({ id: nav.sessionTrack.id, name: nav.sessionTrack.name });
    if (toActivate.length) {
      const ok = await confirm(t("session.inactivePrompt", { names: toActivate.map((m) => m.name).join(", ") }), {
        title: t("session.inactiveTitle"),
        kind: "warning",
      });
      if (!ok) return;
      try {
        for (const m of toActivate) await activateMod(m.id);
      } catch (e) {
        await message(errorText(e), { title: t("session.activateFailedTitle"), kind: "error" });
        return;
      }
      // Recharge tout de suite l'état frais : efface l'icône d'alerte sans
      // attendre le prochain changement de sélection.
      if (nav.sessionCar) carDetail = await getModDetail(nav.sessionCar.id);
      if (nav.sessionTrack) trackDetail = await getModDetail(nav.sessionTrack.id);
    }
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
  // la voiture entière, §8.6) : à 20px dans ce menu compact, la voiture
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
    // `meta` ne porte plus la livrée (SPEC §9.1) : elle a sa propre ligne
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
    <NavRail {alerts} />
    <aside class="side" data-gp-region="sidebar">
      <div class="brand">
        <div class="logo"><span>PB</span></div>
        <div>
          <div class="brand-name">PIT BOX</div>
          <div class="brand-sub">AC MOD MANAGER</div>
        </div>
      </div>

      <!-- SESSION : le duo choisi, et rien d'autre. La colonne répond à une
           seule question — « qu'est-ce que je lance ? » — et les deux blocs
           ont exactement la même anatomie : vignette, nom, source, champs.
           Plus de traitement d'exception sur le bloc voiture. -->
      <div class="session" style="--sess-lblw:{labelWidth}px">
        <div class="nsec">{t("nav.session")}</div>
        <div class="blk">
          <button
            class="pick"
            type="button"
            onclick={() => openSlot("cars", carSlot)}
            ondblclick={() => openSessionDetail("cars", nav.sessionCar?.id)}
            title={carSlot === "picked" ? t("session.carTooltip") : undefined}
            aria-label={nav.sessionCar ? `${nav.sessionCar.name} — ${t("session.changeCar")}` : undefined}
          >
            <div class="thumb car" class:vacant={carSlot !== "picked"} class:photo={carSlot === "picked" && carPrev}>
              {#if carSlot === "picked"}
                {#if carPrev}<img src={carPrev} alt="" />{:else}<span class="thumb-ic">🚗</span>{/if}
                <!-- Le libellé n'est pas supprimé, il est différé (SPEC §9.1) : au
                     survol et au focus clavier seulement, sur un voile PLEIN — au
                     moment où l'on décide de changer, la voiture actuelle n'est
                     plus l'information utile, et un voile partiel rendrait le
                     libellé illisible sur une photo imprévisible. -->
                <span class="veil"><span aria-hidden="true">✎</span>{t("session.changeCar")}</span>
              {:else if carSlot === "empty"}
                <span class="invite"><span aria-hidden="true">＋</span>{t("session.chooseCar")}</span>
              {:else}
                <!-- Impasse (SPEC §9.1) : même trame que l'état initial, autre
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
          {/if}
        </div>

        <div class="nsec section">{t("session.trackTag")}</div>
        <div class="blk">
          <button
            class="pick"
            type="button"
            onclick={() => openSlot("tracks", trackSlot)}
            ondblclick={() => openSessionDetail("tracks", nav.sessionTrack?.id)}
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
              <ModIdentity name={nav.sessionTrack.name}>
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

        <button class="btn-configure" disabled={!sessionReady} onclick={() => requestSection("race")}
          >{t("session.configure")}</button
        >
        <!-- Cible du bouton Start de la manette (§7.4bis) : il y amène le
             curseur depuis n'importe quel écran, il ne lance pas lui-même. -->
        <button class="btn-launch" disabled={!sessionReady} {...{ [LAUNCH_BUTTON_ATTR]: "" }} onclick={launchNow}
          >{t("session.start")}</button
        >
        <!-- Sortie vers Content Manager : un lien texte, jamais un troisième
             bouton encadré — trois blocs de même gabarit empilés annuleraient
             la hiérarchie que le bordé et le plein viennent d'établir. Le
             libellé ne dit pas « dans » : CM ne reçoit ni la voiture ni le
             circuit, il s'ouvre sur son propre état, et « ouvrir dans »
             annoncerait un transfert de contexte qui n'a pas lieu. -->
        {#if cmAvailable}
          <div class="cm-sep"></div>
          <button class="cm-link" type="button" onclick={openCm}>
            <span aria-hidden="true">↗</span>{t("nav.openCm")}
          </button>
        {/if}
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
        {:else if nav.section === "rules" || nav.section === "import" || nav.section === "profiles" || nav.section === "maintenance"}
          <Workshop />
        {:else if nav.section === "driver"}
          <DriverScreen />
        {:else if nav.section === "race"}
          <Launch />
        {:else if nav.section === "carskins"}
          <Transversal variant="car" />
        {:else if nav.section === "trackskins"}
          <Transversal variant="track" />
        {:else if nav.section === "others" || nav.section === "apps"}
          <!-- Un seul écran pour les deux adresses : les apps sont un onglet
               de « Compléments » (SPEC §7.3), et `apps` reste une adresse
               valide — c'est ce qui la fait ouvrir directement sur son onglet. -->
          <OtherMods />
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
  <ImportToasts />
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
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 14px 13px;
    border-bottom: 1px solid var(--line);
  }
  .logo {
    width: 26px;
    height: 26px;
    background: var(--rosso);
    display: flex;
    align-items: center;
    justify-content: center;
    transform: skewX(-8deg);
    flex: none;
  }
  .logo span {
    transform: skewX(8deg);
    color: #fff;
    font-size: 10px;
    font-weight: 700;
    font-style: italic;
  }
  .brand-name {
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 1.5px;
    font-style: italic;
    line-height: 1;
  }
  .brand-sub {
    color: var(--muted);
    font-size: 6.5px;
    letter-spacing: 2.5px;
    margin-top: 3px;
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
  .blk > * + * {
    margin-top: 5px;
  }
  /* **Le nom du mod ne touche pas le champ qui suit.** Cinq pixels séparent
     bien deux champs entre eux — ils forment une liste — mais pas une identité
     d'un contrôle : le nom de la voiture se lisait collé à la liste déroulante
     « Livrée », comme s'il en était l'étiquette. L'écart marque la frontière
     entre ce qu'on a choisi et ce qu'on règle dessus. */
  .blk > .pick + * {
    margin-top: 12px;
  }
  /* Zone qui NAVIGUE (SPEC §9.1) : vignette + nom + source, et rien d'autre.
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
  .thumb-ic {
    font-size: 34px;
    opacity: 0.6;
  }
  /* Voile du libellé différé (SPEC §9.1) : plein, pas dégradé — au moment où
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
    /* 8 px et non 9 : la même gouttière que `.isd-trigger` juste au-dessus.
       Les deux lignes partagent la colonne d'intitulé et portent chacune une
       vignette de 13 px — un pixel d'écart ici décale la valeur de l'une par
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
  /* Même case que `.isd-thumb.labelled` du sélecteur de livrée, aux mêmes
     dimensions : c'est leur alignement vertical qui fait tout l'intérêt. Elle
     est posée même vide — un cadre qui apparaît et disparaît décalerait le
     nom du pilote d'une voiture à l'autre. */
  .dthumb {
    flex: none;
    width: 13px;
    height: 13px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--raised);
    border: 1px solid var(--line);
    overflow: hidden;
  }
  .dthumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    /* **Recadrage sur la tête, en CSS et pas au rendu.** La vignette est un
       buste (du haut du casque à la poitrine) : à 13 px, le casque n'en fait
       plus que trois et on ne distingue rien. Mesuré sur les 85 vignettes du
       cache : la tête va du bord haut (médiane 4 %, au pire 16 %) à 39 % de la
       hauteur (p90 47 %). Ce couple montre donc la tranche 0-52 % de l'image
       source, soit la tête entière et un doigt d'épaules.
       En CSS et non dans `driverThumbs` parce que le même PNG sert la galerie
       de l'écran Pilote, où il est affiché à 104 px et où le buste est le bon
       cadrage — et parce que le recalculer invaliderait les 85 vignettes déjà
       sur disque pour un problème qui n'existe qu'ici. */
    transform: translateY(46%) scale(1.9);
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
  .btn-configure {
    width: 100%;
    height: 36px;
    background: var(--raised);
    border: 1px solid var(--line);
    color: var(--txt2);
    font-size: 9.5px;
    letter-spacing: 1.5px;
    font-weight: 600;
    font-family: var(--mono);
    margin-top: 8px;
  }
  .btn-configure:hover:not(:disabled) {
    background: var(--card);
    border-color: var(--faint);
  }
  .btn-configure:disabled {
    opacity: 0.45;
    cursor: not-allowed;
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
    margin-top: 2px;
  }
  .btn-launch:hover:not(:disabled) {
    background: var(--rosso-bright);
  }
  /* Garde son fond rouge à l'état désactivé, en opacité réduite (SPEC §9.1) : il
     reste la destination visible de l'écran, et le griser complètement
     effacerait le but à atteindre. */
  .btn-launch:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  /* Un filet et une respiration séparent le lien du bloc de lancement :
     collé sous le bouton rouge, il se lirait comme la suite du bloc —
     l'adjacence promet toute seule, même sans le mot. */
  .cm-sep {
    height: 1px;
    background: var(--line);
    margin-top: 12px;
  }
  .cm-link {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    width: 100%;
    height: 26px;
    margin-top: 10px;
    background: none;
    border: none;
    color: var(--muted);
    font-size: 10px;
  }
  .cm-link:hover {
    color: var(--txt2);
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
