<script lang="ts">
  import { tick, untrack, onMount, onDestroy } from "svelte";
  import DetailPage from "./DetailPage.svelte";
  import PackDetail from "./PackDetail.svelte";
  import ColumnsMenu from "./ColumnsMenu.svelte";
  import FilterBar from "./filters/FilterBar.svelte";
  import { matchesQuery } from "$lib/cardSearch";
  import { hasOwnDriver } from "$lib/driverOverride.svelte";
  import BulkEditPanel from "./BulkEditPanel.svelte";
  import ContextMenu from "./ContextMenu.svelte";
  import LoadingState from "./LoadingState.svelte";
  import StateBadge from "./StateBadge.svelte";
  import Seg from "./Seg.svelte";
  import Tooltip from "./Tooltip.svelte";
  import {
    listLibrary,
    previewSrc,
    setFavorite,
    type ModCard,
    type ModKind,
  } from "$lib/library";
  import {
    columnsFor,
    loadColumnsPrefs,
    saveColumnsPrefs,
    type ColumnDef,
  } from "$lib/columns";
  import { nav, pickSession } from "$lib/nav.svelte";
  import { goBackOr } from "$lib/navHistory";
  import { bigPictureView } from "$lib/bigpicture.svelte";
  import { withoutBrand } from "$lib/displayName";
  import ModIdentity from "./ModIdentity.svelte";
  import { moveFocus } from "$lib/gamepadNav";
  import { registerModNav } from "$lib/screenActions";
  import { libraryVersion } from "$lib/libraryVersion.svelte";
  import { getPreferredSkin, getPreferredLayout } from "$lib/preferred";
  import { enqueueGridThumbs, gridThumb, requestGridThumb } from "$lib/gridThumbs.svelte";
  import { gridThumbsOn, presetForDensity, renderTemplate } from "$lib/gridThumbPrefs.svelte";
  import { buildModContextItems } from "$lib/modContextActions";
  import { t } from "$lib/i18n/index.svelte";
  import { zoomFactor } from "$lib/zoom.svelte";
  import { pinShell } from "$lib/shellScroll";
  import { getUiPrefs, setUiPref } from "$lib/uiPrefs.svelte";
  import {
    buildCardIndex,
    buildPredicate,
    filterDefs,
    parseFilters,
    parsePinned,
    serializeFilters,
    type FilterMap,
  } from "$lib/filters";

  import { StorageKey } from "$lib/storage";
  // Une bibliothèque par type (§6.1) : ce composant est rendu une fois pour les
  // voitures, une fois pour les circuits. Toute la persistance est suffixée par
  // type pour rester indépendante entre les deux.
  let { kind }: { kind: ModKind } = $props();
  // `kind` ne change jamais pour une instance montée (§6.1, deux instances
  // fixes voitures/circuits) — `untrack` documente que ces lectures ne
  // capturent la prop qu'une fois, volontairement, pour le compilateur.
  /** Les trois positions de la bascule de vue (§7.4). */
  type GridView = "dense" | "comfortable" | "table";

  const isCar = untrack(() => kind === "Car");
  // Clés de persistance bâties une fois, au même endroit : chaque réglage doit
  // rester indépendant entre voitures et circuits (§6.1).
  const KEYS = untrack(() => ({
    filters: StorageKey.libraryFilters(kind),
    pinned: StorageKey.libraryPinned(kind),
    view: StorageKey.libraryView(kind),
    hideBrand: StorageKey.gridHideBrand,
    sortKey: StorageKey.librarySortKey(kind),
    sortDir: StorageKey.librarySortDir(kind),
  }));
  /** Catalogue des filtres de CE type (§6.3) : marque, année, classe et
   * pilote n'existent que côté voitures. */
  const defs = untrack(() => filterDefs(kind));

  let cards = $state<ModCard[]>([]);
  // Distinct de « bibliothèque vide » : sans lui, la liste encore vide au
  // premier rendu (avant que listLibrary() ne réponde) affichait le message
  // « Aucune voiture… » pendant une fraction de seconde à chaque ouverture.
  // Ne repasse jamais à true après le premier chargement — un réimport ne
  // doit pas faire disparaître la liste déjà affichée.
  let loading = $state(true);
  let selectedId = $state<string | null>(null);
  // Édition groupée (§6.3bis) : Ctrl/Alt-clic ajoute/retire de la sélection
  // multiple. Un clic simple retombe toujours en sélection simple.
  let selectedIds = $state<Set<string>>(new Set());
  // Page détail pleine page (§6.3) : double-clic sur une carte, ou bouton
  // « Agrandir » du panneau latéral. État centralisé dans nav.openFull (voir
  // nav.svelte.ts) — la navigation manette globale (AppShell) doit savoir si
  // elle est ouverte pour céder gauche/droite au visualiseur et gérer B=fermer.

  // Filtres persistés par type (rechargés au retour sur la page). Défauts
  // synchrones à l'affichage initial, remplacés par les valeurs sauvegardées
  // dès que l'onMount plus bas répond (même schéma que les colonnes, §6.2).
  const FKEY = KEYS.filters;
  /** Recherche libre : le seul filtre qui n'a pas de puce. Il traverse
   * plusieurs champs à la fois (nom, marque, id, catégorie, pack, tags), ce
   * qu'aucune puce ne saurait résumer — c'est le rattrapage de ce que les
   * champs nommés ne couvrent pas, pas un filtre de plus. */
  let query = $state<string>("");
  /** Filtres posés (§6.3). Une clé absente = filtre inactif : jamais de valeur
   * vide conservée pour dire « indifférent ». */
  let filters = $state<FilterMap>({});
  /** Filtres épinglés : ils restent visibles en fantôme même sans valeur, et
   * leur ordre est celui des fantômes dans la barre. */
  let pinned = $state<string[]>([]);
  /** Trois positions et non deux (SPEC §7.4) : la grille a deux densités, la
   * liste reste la liste. Persistée **par type**, comme le tri et les colonnes
   * — voitures et circuits ne se regardent pas de la même façon, et c'est déjà
   * la règle de tout le reste de cet écran. */
  /** La vue que l'utilisateur a choisie, et que `ui_prefs.json` garde. */
  let view = $state<GridView>("dense");
  /** Celle qu'on AFFICHE : le Big Picture peut en imposer une autre le temps
   * qu'il dure, sans toucher au réglage (voir `bigPictureView`). */
  const shown = $derived(bigPictureView.forced ?? view);
  /** Préférence de présentation, pas d'identité : le nom stocké n'est jamais
   * touché (voir `displayName.ts`). */
  /** **Vrai par défaut** depuis que la marque a sa propre ligne au-dessus du
   * modèle : l'y laisser aussi écrirait « Nissan » deux fois par carte, et
   * c'est la répétition qui pousse le nom du modèle à la troncature. Un
   * utilisateur qui décoche garde son choix — seule l'absence de réglage
   * bascule (voir la lecture dans `loadPrefs`). */
  let hideBrandGrid = $state(true);
  /** Le même réglage pour le tableau, **et séparé**. La grille porte une ligne
   * d'identification, le tableau une colonne « Marque » : ce sont deux
   * présentations, et vouloir la marque dans le nom de l'une n'a aucune raison
   * de l'imposer à l'autre. Le tableau la lit aussi de son côté
   * (`columns.ts`), pour que le tri suive l'affichage. */
  let hideBrandTable = $state(true);
  /** Celui des deux que la vue courante commande — une seule case à l'écran,
   * qui agit là où on la voit agir. */
  const hideBrand = $derived(shown === "table" ? hideBrandTable : hideBrandGrid);
  let showDisplay = $state(false);
  const isGrid = $derived(shown !== "table");
  let sortKey = $state<string>("name");
  let sortDir = $state<1 | -1>(1);
  // Garde toutes les persistances ci-dessous tant que l'onMount plus bas n'a
  // pas fini de restaurer les valeurs sauvegardées — sans ça, l'effet des
  // filtres se déclenche dès le montage avec les défauts et les réécrit par-
  // dessus la sauvegarde avant même qu'elle soit lue (bug réel, même classe
  // que `ready` dans Launch.svelte).
  let prefsReady = false;

  // Persistance des filtres (champ libre + puces). L'instantané est écrit sous
  // une forme nouvelle (`{ query, filters }`) que la relecture distingue des
  // deux générations précédentes — voir `parseFilters`, qui les convertit
  // toutes plutôt que de repartir de zéro.
  $effect(() => {
    const snapshot = serializeFilters(query, filters);
    if (prefsReady) setUiPref(FKEY, snapshot);
  });
  $effect(() => {
    const list = JSON.stringify(pinned);
    if (prefsReady) setUiPref(KEYS.pinned, list);
  });

  // Colonnes (§6.2) : définitions propres au type + visibilité/ordre persistés
  // par type. Défauts synchrones à l'affichage initial (évite un vide le
  // temps du chargement Rust), remplacés par les valeurs sauvegardées dès que
  // `loadColumnsPrefs` répond (onMount plus bas).
  const columns: ColumnDef[] = untrack(() => columnsFor(kind));
  let visibleKeys = $state<string[]>(untrack(() => columns.filter((c) => c.fixed || c.defaultVisible).map((c) => c.key)));
  let columnOrder = $state<string[]>(untrack(() => columns.map((c) => c.key)));
  let columnWidths = $state<Record<string, number>>({});
  // Colonne en cours de glissement (réordonnancement d'en-tête, §6.2) : pilote
  // le retour visuel et la cible du drop, jamais persistée telle quelle.
  let dragKey = $state<string | null>(null);
  let dropTarget = $state<{ key: string; before: boolean } | null>(null);
  // Le clic natif qui suit un mousedown+mousemove+mouseup sur un en-tête (que
  // ce soit un redimensionnement ou un réordonnancement) ne doit jamais
  // déclencher son tri — bug réel constaté : redimensionner une colonne
  // changeait l'ordre de tri, parce que le curseur termine souvent au-dessus
  // d'un `<th>` voisin après un glissé horizontal, et le clic qui suit
  // toujours un mouseup cible cet en-tête-là. `click` est dispatché par le
  // navigateur juste après `mouseup`, dans la même séquence synchrone — pas
  // besoin d'attendre, le drapeau est déjà à jour quand `onclick` s'exécute.
  let suppressSortClick = false;
  function markSuppressSortClick() {
    suppressSortClick = true;
    setTimeout(() => (suppressSortClick = false), 0);
  }
  const visibleColumns = $derived(
    columnOrder
      .map((key) => columns.find((c) => c.key === key))
      .filter((c): c is ColumnDef => !!c && (c.fixed || visibleKeys.includes(c.key))),
  );
  function persistColumnsPrefs() {
    saveColumnsPrefs(kind, { visible: visibleKeys, order: columnOrder, widths: columnWidths });
  }
  function toggleColumn(key: string) {
    visibleKeys = visibleKeys.includes(key)
      ? visibleKeys.filter((k) => k !== key)
      : [...visibleKeys, key];
    persistColumnsPrefs();
  }
  /** Réordonnance : déplace `sourceKey` juste avant ou après `targetKey` dans
   * l'ordre complet (colonnes masquées comprises, pour qu'elles gardent leur
   * position relative une fois réaffichées).
   *
   * **Toutes les colonnes se déplacent, la colonne fixe comprise.** Elle ne
   * bougeait pas, et rien ne l'exigeait : `fixed` dit qu'on ne peut pas la
   * masquer, pas qu'elle est clouée en première position. La conséquence était
   * qu'aucune colonne ne pouvait passer avant le nom — or c'est la marque qu'on
   * lit d'abord. */
  function reorderColumn(sourceKey: string, targetKey: string, before: boolean) {
    if (sourceKey === targetKey) return;
    const rest = columnOrder.filter((k) => k !== sourceKey);
    const targetIdx = rest.indexOf(targetKey);
    const insertAt = before ? targetIdx : targetIdx + 1;
    columnOrder = [...rest.slice(0, insertAt), sourceKey, ...rest.slice(insertAt)];
    persistColumnsPrefs();
  }
  const HEADER_DRAG_THRESHOLD = 4;
  /** Réordonnancement au glissé souris (§6.2) — pas le drag HTML5 natif :
   * abandonné après deux tentatives (curseur « sens interdit » persistant,
   * jamais résolu malgré `setData`/`effectAllowed`/`-webkit-user-drag`).
   * Même technique, déjà éprouvée, que le redimensionnement juste en dessous
   * (`startResize`) : écouteurs `window` le temps du geste, seuil de
   * quelques pixels avant de considérer que c'est un vrai glissé et pas un
   * simple clic (sinon `toggleSort` ne se déclencherait plus jamais). */
  function startHeaderDrag(e: MouseEvent, key: string) {
    if (e.button !== 0) return;
    const startX = e.clientX;
    const startY = e.clientY;
    let moved = false;
    function onMove(ev: MouseEvent) {
      if (!moved) {
        if (Math.abs(ev.clientX - startX) < HEADER_DRAG_THRESHOLD && Math.abs(ev.clientY - startY) < HEADER_DRAG_THRESHOLD) return;
        moved = true;
        dragKey = key;
      }
      const target = (document.elementFromPoint(ev.clientX, ev.clientY) as HTMLElement | null)?.closest<HTMLElement>(
        "th[data-col-key]",
      );
      const targetKey = target?.dataset.colKey;
      if (!targetKey || targetKey === key) {
        dropTarget = null;
        return;
      }
      const rect = target!.getBoundingClientRect();
      dropTarget = { key: targetKey, before: ev.clientX - rect.left < rect.width / 2 };
    }
    function onUp() {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
      if (moved) {
        if (dropTarget) reorderColumn(key, dropTarget.key, dropTarget.before);
        markSuppressSortClick();
      }
      dragKey = null;
      dropTarget = null;
    }
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
  }

  // --- Redimensionnement de colonne (§6.2) : poignée à droite de l'en-tête,
  // glissé à la souris (pas de drag HTML5 ici — un redimensionnement est un
  // suivi continu du pointeur, pas un dépôt discret). Écouteurs posés sur
  // `window` le temps du geste : le curseur sort souvent de la poignée (large
  // mouvement horizontal), `mousemove`/`mouseup` doivent suivre partout. ---
  const MIN_COLUMN_WIDTH = 50;
  let resizingKey = $state<string | null>(null);
  let resizeStartX = 0;
  let resizeStartWidth = 0;
  /** Une largeur **mesurée** (pixels réels de la fenêtre) ramenée en pixels
   * CSS, seuls acceptés par la feuille de style — voir `zoomFactor`. Sans ça,
   * saisir une poignée à 110 % élargissait la colonne de 10 % d'un coup, et le
   * glissé avançait 10 % trop vite. */
  const measured = (width: number) => width / zoomFactor();

  function startResize(e: MouseEvent, key: string, currentWidth: number) {
    e.preventDefault();
    e.stopPropagation();
    // Idempotent avant tout ajout : si un geste précédent n'avait pas relâché
    // proprement (mouseup manqué hors fenêtre, par ex.), évite d'empiler des
    // écouteurs `window` en double qui recalculeraient la largeur en double à
    // chaque mouvement de souris.
    stopResizeListeners();
    resizingKey = key;
    resizeStartX = e.clientX;
    resizeStartWidth = measured(currentWidth);
    window.addEventListener("mousemove", onResizeMove);
    window.addEventListener("mouseup", onResizeUp);
  }
  function onResizeMove(e: MouseEvent) {
    if (!resizingKey) return;
    const next = Math.max(MIN_COLUMN_WIDTH, Math.round(resizeStartWidth + measured(e.clientX - resizeStartX)));
    columnWidths = { ...columnWidths, [resizingKey]: next };
  }
  function stopResizeListeners() {
    window.removeEventListener("mousemove", onResizeMove);
    window.removeEventListener("mouseup", onResizeUp);
  }
  function onResizeUp() {
    if (!resizingKey) return;
    resizingKey = null;
    stopResizeListeners();
    persistColumnsPrefs();
    markSuppressSortClick();
  }
  /** Redimensionnement au clavier (flèches gauche/droite), poignée focusable
   * — pas juste une alternative a11y de façade : sans ça, la poignée n'est
   * accessible qu'à la souris. `currentWidth` = largeur affichée actuelle
   * (naturelle si jamais redimensionnée), pas de branchement particulier. */
  function adjustColumnWidth(key: string, currentWidth: number, delta: number) {
    columnWidths = { ...columnWidths, [key]: Math.max(MIN_COLUMN_WIDTH, Math.round(measured(currentWidth) + delta)) };
    persistColumnsPrefs();
  }
  /** Double-clic (ou Entrée au clavier) sur la poignée = revenir à la largeur
   * naturelle (au contenu), convention standard des tableaux redimensionnables. */
  function resetColumnWidth(key: string) {
    const { [key]: _removed, ...rest } = columnWidths;
    columnWidths = rest;
    persistColumnsPrefs();
  }
  onDestroy(stopResizeListeners);

  // --- Vignettes régénérées : demandées à la visibilité (SPEC-grille §5.4) ---
  //
  // **Pas au chargement de la liste.** Trois cents conversions demandées d'un
  // coup, c'est cinq minutes de travail pour des cartes que personne ne
  // regarde ; demandées à l'entrée dans le champ de vision, la bibliothèque se
  // normalise pendant qu'on l'utilise. Un seul observateur pour toute la
  // grille : un par carte coûterait trois cents abonnements pour la même
  // information.
  // **Le preset suit la densité de la grille** (SPEC-grille FMOD§6bis). La densité
  // est déjà une déclaration d'intention : passer en dense, c'est dire « je
  // cherche » ; passer en confortable, c'est dire « je regarde ». Y accrocher le
  // style n'ajoute donc pas un réglage, ça donne un second sens à un contrôle
  // qui le portait déjà.
  //
  // Les deux jeux d'images coexistent sur disque, donc rebasculer est
  // instantané une fois les deux produits — c'est le balayage qui garde les
  // images de tous les presets vivants qui rend ça vrai.
  const preset = $derived(presetForDensity(shown === "comfortable" ? "comfortable" : "dense"));
  /** Le gabarit tel qu'il part au rendu : les valeurs du preset **plus son
   * mat**, que le fond cuit d'un preset comme Officiel reprend. */
  const template = $derived(renderTemplate(preset));

  type ThumbWant = { car: string; skin: string | null; name: string; want: boolean };
  const wanted = new WeakMap<Element, ThumbWant>();
  let visibility: IntersectionObserver | null = null;

  function whenVisible(node: HTMLElement, want: ThumbWant) {
    // `IntersectionObserver` manque dans certains contextes de test : son
    // absence doit coûter la fonctionnalité, jamais l'écran.
    if (typeof IntersectionObserver === "undefined") return;
    visibility ??= new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (!entry.isIntersecting) continue;
          const target = wanted.get(entry.target);
          if (target?.want) requestGridThumb(template, target.car, target.skin, target.name);
        }
      },
      // Un peu avant le bord : la vignette d'une carte qui arrive a une
      // chance d'être là quand elle arrive vraiment.
      { rootMargin: "300px" },
    );
    wanted.set(node, want);
    visibility.observe(node);
    return {
      update(next: ThumbWant) {
        wanted.set(node, next);
      },
      destroy() {
        visibility?.unobserve(node);
        wanted.delete(node);
      },
    };
  }

  onDestroy(() => {
    visibility?.disconnect();
    visibility = null;
  });

  // **Et tout le reste derrière**, une fois la liste filtrée connue (§5.4).
  // Sans cette seconde moitié, la génération ne produirait que ce qu'on a
  // regardé : le travail n'aurait pas de fin, et la tâche de fond pas de
  // dénominateur — sa file se viderait à chaque arrêt du défilement. Les
  // cartes visibles, elles, continuent de passer devant.
  //
  // `untrack` sur la mise en file : elle lit le cache des vignettes, qui est un
  // `$state` que la génération réécrit à chaque image — sans lui, cet effet se
  // redéclencherait trois cents fois pour trois cents vignettes.
  $effect(() => {
    if (!isCar || !gridThumbsOn()) return;
    const list = filtered;
    const wanted = template;
    // « Laisser le contenu de base tel quel » : les voitures Kunos ont déjà
    // exactement le rendu qu'un preset qui les imite produirait, donc les
    // régénérer coûte trois minutes pour un résultat identique. Avec un preset
    // qui cherche l'homogénéité, au contraire, les sauter ruine ce qu'on
    // cherche — d'où une propriété du preset et non un réglage global.
    const skipStock = preset.skipStock;
    untrack(() =>
      enqueueGridThumbs(
        wanted,
        list
          .filter((c) => !skipStock || !c.is_stock)
          .map((c) => ({
            id: c.id_interne,
            skin: getPreferredSkin(c.id_interne)?.id ?? null,
            name: cardName(c),
          })),
      ),
    );
  });

  // Restauration au montage (§6.2/SESSION§1) : colonnes (fichier dédié,
  // `columns.ts`) et le reste des petits réglages d'écran (`uiPrefs.ts`) en
  // parallèle, un seul aller-retour chacun. `prefsReady` n'est levé qu'une
  // fois tout appliqué, pour que l'effet de persistance des filtres plus haut
  // ne réécrive rien avant d'avoir vu les vraies valeurs sauvegardées.
  onMount(async () => {
    const [colPrefs, saved] = await Promise.all([
      loadColumnsPrefs(kind),
      getUiPrefs([FKEY, KEYS.pinned, KEYS.view, KEYS.sortKey, KEYS.sortDir, KEYS.hideBrand, StorageKey.tableHideBrand]),
    ]);
    visibleKeys = colPrefs.visible;
    columnOrder = colPrefs.order;
    columnWidths = colPrefs.widths;

    const restored = parseFilters(saved[FKEY], defs);
    query = restored.query;
    filters = restored.filters;
    pinned = parsePinned(saved[KEYS.pinned], defs);
    // `gallery` est l'ancienne valeur, d'avant la seconde densité : relue une
    // dernière fois pour ne pas renvoyer en grille dense quelqu'un qui était
    // en tableau, et réécrite au premier changement de vue.
    const savedView = saved[KEYS.view];
    if (savedView === "dense" || savedView === "comfortable" || savedView === "table") view = savedView;
    else if (savedView === "gallery") view = "dense";
    // `!== "0"` et non `=== "1"` : sans réglage enregistré, c'est le nouveau
    // défaut qui s'applique, pas l'ancien.
    hideBrandGrid = saved[KEYS.hideBrand] !== "0";
    hideBrandTable = saved[StorageKey.tableHideBrand] !== "0";
    const savedSortKey = saved[KEYS.sortKey];
    if (savedSortKey) sortKey = savedSortKey;
    const savedSortDir = saved[KEYS.sortDir];
    if (savedSortDir) sortDir = savedSortDir === "-1" ? -1 : 1;

    prefsReady = true;
  });

  function toggleSort(key: string) {
    if (sortKey === key) sortDir = sortDir === 1 ? -1 : 1;
    else {
      sortKey = key;
      sortDir = 1;
    }
    if (prefsReady) {
      setUiPref(KEYS.sortKey, sortKey);
      setUiPref(KEYS.sortDir, String(sortDir));
    }
  }

  function setView(v: GridView) {
    // Un choix explicite l'emporte sur le défaut du Big Picture : sans cette
    // ligne, cliquer sur « tableau » en mode salon ne changerait rien à
    // l'écran, la surcouche continuant de gagner.
    bigPictureView.forced = null;
    view = v;
    if (prefsReady) setUiPref(KEYS.view, v);
  }

  function setHideBrand(on: boolean) {
    if (shown === "table") {
      hideBrandTable = on;
      if (prefsReady) setUiPref(StorageKey.tableHideBrand, on ? "1" : "0");
    } else {
      hideBrandGrid = on;
      if (prefsReady) setUiPref(KEYS.hideBrand, on ? "1" : "0");
    }
  }

  /** Ce que la carte écrit, jamais ce sur quoi on trie ou on cherche. */
  function cardName(c: ModCard): string {
    const full = c.display_name ?? c.id_interne;
    // La préférence des GRILLES : une carte n'existe pas dans le tableau.
    return hideBrandGrid ? withoutBrand(full, c.brand) : full;
  }

  // Panneau latéral toujours ouvert (jamais de saut de largeur du panneau
  // central) : à défaut d'un clic explicite, on affiche le choix de session
  // courant. Défilement vers l'élément sélectionné à revenir sur cet écran.
  let mainEl = $state<HTMLDivElement | undefined>();
  let firstLoad = true;
  /** Ramène la carte sélectionnée au centre de la liste.
   *
   * **Jamais `scrollIntoView`** : il fait défiler *tous* les ancêtres
   * scrollables pour amener l'élément dans la **fenêtre**, document compris —
   * or `html`/`body` sont en `overflow: hidden` exprès, et un décalage posé là
   * est définitif (la molette ne peut plus le rattraper). C'est l'un des deux
   * chemins par lesquels toute la coquille se retrouvait remontée d'une
   * quinzaine de pixels, barre de titre à moitié sortie. On fait donc défiler
   * le conteneur, et lui seul.
   *
   * La division par `zoomFactor()` n'est pas décorative : le zoom d'interface
   * est un `zoom` CSS posé sur `<html>`, donc `getBoundingClientRect` rend des
   * pixels de fenêtre déjà multipliés quand `scrollTop` est en pixels CSS
   * (§13). */
  function scrollToEffective() {
    if (!effectiveId) return;
    tick().then(() => {
      const scroller = mainEl;
      const el = scroller?.querySelector(`[data-id="${CSS.escape(effectiveId!)}"]`);
      if (!scroller || !el) return;
      const f = zoomFactor();
      const top = (el.getBoundingClientRect().top - scroller.getBoundingClientRect().top) / f;
      const middle = scroller.clientHeight / 2 - el.getBoundingClientRect().height / f / 2;
      scroller.scrollTop = scroller.scrollTop + top - middle;
      // Le défilement dû au focus, lui, a pu avoir lieu avant : on remet la
      // coquille d'aplomb derrière.
      pinShell(scroller);
    });
  }

  async function refresh() {
    cards = await listLibrary();
    loading = false;
    // Purge de la sélection groupée : un mod supprimé pendant qu'il était
    // sélectionné (§6.3bis) ne doit pas laisser le panneau du bas affiché sur
    // une sélection en partie fantôme.
    if (selectedIds.size) {
      const ids = new Set(cards.map((c) => c.id_interne));
      const pruned = new Set([...selectedIds].filter((id) => ids.has(id)));
      if (pruned.size !== selectedIds.size) selectedIds = pruned;
    }
    if (firstLoad) {
      firstLoad = false;
      scrollToEffective();
    }
  }

  // Clic droit sur une carte/ligne (§ nettoyage panneaux) : mêmes actions que
  // le panneau compact, sans avoir à sélectionner le mod d'abord.
  let ctxMenu = $state<{ x: number; y: number; card: ModCard } | null>(null);
  function openCardContextMenu(e: MouseEvent, c: ModCard) {
    e.preventDefault();
    // Convention des gestionnaires de fichiers : viser un mod qui n'est pas
    // dans la sélection la ramène à lui seul. Sans ça, un clic droit un peu à
    // côté ferait porter « supprimer » sur douze mods qu'on ne regarde même
    // pas — et le menu n'a rien qui rappelle lesquels.
    if (!selectedIds.has(c.id_interne)) selectedIds = new Set();
    ctxMenu = { x: e.clientX, y: e.clientY, card: c };
  }
  // Le menu porte sur toute la sélection quand le mod visé en fait partie
  // (§6.3ter). C'est le chemin principal des actions groupées : le panneau du
  // bas ne garde que ce qu'un menu ne peut pas porter — un champ de saisie.
  const contextTargets = $derived.by(() => {
    if (!ctxMenu) return [];
    if (selectedIds.size >= 2 && selectedIds.has(ctxMenu.card.id_interne)) {
      return sorted.filter((c) => selectedIds.has(c.id_interne));
    }
    return [ctxMenu.card];
  });
  const contextItems = $derived(buildModContextItems(contextTargets, refresh));

  // La bibliothèque EST le sélecteur (SESSION§1) : ouvrir une carte la définit comme
  // choix de session, affiché dans le bloc SESSION de la barre latérale.
  const sessionId = $derived(isCar ? nav.sessionCar?.id ?? null : nav.sessionTrack?.id ?? null);
  // Sélection effective du panneau : le clic explicite prime, sinon le choix
  // de session courant (le panneau reste toujours rempli, jamais vide).
  const effectiveId = $derived(selectedId ?? sessionId);
  // Défaut si aucune session n'a jamais été choisie (premier lancement) :
  // établit une vraie sélection de session plutôt que de laisser le panneau
  // vide indéfiniment. Même repli si le mod affiché a été SUPPRIMÉ entre-temps
  // (§6.3bis) : sans lui, `selectedId`/la session continuaient de désigner un
  // id qui n'existe plus, et la fiche de droite comme le bloc SESSION
  // restaient bloqués dessus. L'existence se vérifie sur `typed` — la
  // bibliothèque de ce type SANS le filtre/la recherche courants — jamais sur
  // `sorted` : un mod simplement masqué par un filtre ne doit pas faire
  // sauter la session sur un autre mod.
  $effect(() => {
    if (!sorted.length) return;
    if (!sessionId && !selectedId) {
      select(sorted[0]);
      return;
    }
    if (effectiveId && !typed.some((c) => c.id_interne === effectiveId)) {
      select(sorted[0]);
    }
  });
  /**
   * Ce qui empêche de **jouer** cette voiture, ou `null`.
   *
   * Les autres états — installé, activé, déjà essayé, contenu de base — ont
   * quitté la grille (SPEC §7.4bis) : on joue dans la grille, on gère dans le
   * tableau, et un état d'installation ne change rien à la décision « je
   * prends celle-là ce soir ». Celui-ci reste parce qu'il la change : sans
   * marquage, on choisit la voiture, on lance, et on découvre le problème
   * dans le jeu.
   *
   * Un mod installé hors Pit Box n'est PAS concerné : il n'est pas « actif »
   * au sens du déploiement géré, mais il est bien dans le jeu et se lance
   * parfaitement — l'éteindre serait un contresens sur une install déjà
   * moddée, où il y en a des centaines.
   */
  function unusableReason(c: ModCard): string | null {
    if (c.broken) return "library.brokenTooltip";
    if (!c.active && !c.is_unmanaged) return "library.inactiveTooltip";
    return null;
  }

  function select(c: ModCard) {
    selectedId = c.id_interne;
    // Restaure les préférences mémorisées de l'entité (skin voiture, layout circuit).
    const sk = isCar ? getPreferredSkin(c.id_interne) : null;
    const lay = !isCar ? getPreferredLayout(c.id_interne) : null;
    // Source, pas résumé (SPEC SESSION§1) : la livrée et le tracé sont affichés
    // juste dessous dans la colonne de session, et le nom juste au-dessus —
    // les répéter ici coûtait des caractères pour rien. Le tag, lui, est un
    // critère de recherche : sa place est dans la bibliothèque et sur la
    // fiche, pas dans un résumé de session.
    const meta = isCar
      ? [c.brand, c.year].filter(Boolean).join(" · ")
      : (c.author ?? "");
    pickSession(kind, {
      id: c.id_interne,
      name: c.display_name ?? c.id_interne,
      meta,
      preview: sk?.preview ?? lay?.preview ?? c.preview,
      layout: lay?.id ?? (!isCar ? c.layouts[0] ?? null : null),
      skin: sk?.id ?? null,
      outline: !isCar ? (lay?.outline ?? c.outline) : null,
    });
  }

  // Sélection multiple (§6.3bis) ; clic simple = comportement normal (sélection
  // de session) et efface toute sélection groupée en cours. `effectiveId` suit
  // le dernier mod cliqué et ne sert plus qu'à le surligner : le panneau de
  // détail latéral a été retiré, la fiche s'ouvre en page pleine au
  // double-clic.
  // - Ctrl-clic : bascule un mod (ajout/retrait individuel).
  // - Maj-clic : sélectionne toute la plage entre l'ancre (dernier mod cliqué)
  //   et celui-ci, dans l'ordre affiché courant — combinable avec des Ctrl-clic
  //   (convention standard des gestionnaires de fichiers).
  function onCardClick(c: ModCard, e: MouseEvent) {
    if (e.ctrlKey) {
      e.preventDefault();
      const next = new Set(selectedIds);
      if (next.size === 0 && selectedId) next.add(selectedId);
      if (next.has(c.id_interne)) next.delete(c.id_interne);
      else next.add(c.id_interne);
      selectedIds = next;
      selectedId = c.id_interne;
      return;
    }
    if (e.shiftKey) {
      e.preventDefault();
      const ids = sorted.map((x) => x.id_interne);
      const to = ids.indexOf(c.id_interne);
      if (to === -1) return;
      const anchor = selectedId ?? sessionId;
      const from = anchor ? ids.indexOf(anchor) : -1;
      const next = new Set(selectedIds);
      if (from === -1) {
        next.add(c.id_interne);
      } else {
        const [lo, hi] = from <= to ? [from, to] : [to, from];
        for (let i = lo; i <= hi; i++) next.add(ids[i]);
      }
      selectedIds = next;
      selectedId = c.id_interne;
      return;
    }
    selectedIds = new Set();
    select(c);
  }

  // Ouverture demandée depuis une vue transversale (§8.3) : on ouvre la
  // fiche pleine page de l'entité ciblée (la bonne bibliothèque est déjà active).
  $effect(() => {
    if (nav.openMod) {
      nav.openFull = nav.openMod;
      nav.openMod = null;
    }
  });

  // Recherche imposée depuis l'extérieur (ex. « filtrer par pack », §4.4).
  $effect(() => {
    if (nav.search !== null) {
      query = nav.search;
      nav.openFull = null;
      nav.search = null;
    }
  });

  async function toggleFav(c: ModCard, e: Event) {
    e.stopPropagation();
    c.is_favorite = !c.is_favorite;
    await setFavorite(c.id_interne, c.is_favorite);
  }

  // N'expose que les mods du type de cette bibliothèque (§6.1).
  const typed = $derived(cards.filter((c) => c.kind === kind));

  // Index de recherche et valeurs proposées : partagés avec la modale
  // d'adversaires, qui pose exactement la même question sur le même vivier
  // (`filters.ts`/`cardSearch.ts`). Ils vivaient ici, en six blocs, et une
  // seconde copie aurait dérivé au premier champ ajouté — le nom du pack et la
  // note de l'utilisateur sont tous deux entrés dans la botte de foin après
  // coup.
  // La référence de la bande de performance est la **voiture de session**, sur
  // cet écran comme dans le bloc Adversaires : « montre-moi tout ce qui roule
  // au niveau de ma 488 » se pose aussi hors session, et la 488 en question
  // est toujours celle que la colonne de droite affiche (§3.4).
  const index = $derived(buildCardIndex(typed, defs, isCar, hasOwnDriver, isCar ? nav.sessionCar?.id ?? null : null));

  const matchesFilters = $derived(buildPredicate(defs, filters, index.ctx));

  const filtered = $derived(typed.filter((c) => matchesFilters(c) && matchesQuery(c, query)));

  const sorted = $derived.by(() => {
    const col = columns.find((c) => c.key === sortKey);
    const val = (c: ModCard): string | number => {
      if (!col) return (c.display_name ?? c.id_interne).toLowerCase();
      return col.sortValue ? col.sortValue(c) : col.value(c).toLowerCase();
    };
    return [...filtered].sort((a, b) => {
      const va = val(a), vb = val(b);
      if (va < vb) return -sortDir;
      if (va > vb) return sortDir;
      return 0;
    });
  });

  // Recharge au montage, puis à chaque changement de bibliothèque (import,
  // activation, suppression — §4.2/§ resynchronisation) — `libraryVersion()`
  // sert de simple signal.
  $effect(() => {
    libraryVersion();
    refresh();
  });

  // --- Navigation clavier/manette dans la fiche pleine page ---
  // Mod précédent/suivant, dans l'ordre affiché (tri et filtres courants). Ne
  // touche pas à la sélection de session (comme le double-clic qui ouvre la
  // fiche) — juste la navigation dans la vue.
  function navigateFull(delta: 1 | -1) {
    // Une visionneuse plein écran ouverte par-dessus la fiche (§6.1), ou le
    // panneau de périphérique (§7.4), consomme les entrées en exclusivité —
    // sinon une même pression ferait défiler les images ET changer de mod.
    if (!nav.openFull || nav.openPack || nav.inputCapture) return;
    const ids = sorted.map((c) => c.id_interne);
    const idx = ids.indexOf(nav.openFull);
    if (idx === -1 || ids.length < 2) return;
    nav.openFull = ids[(idx + delta + ids.length) % ids.length];
  }

  // Ouverte à la manette (boutons « mod précédent/suivant », §7.4bis) sans que
  // celle-ci ait à connaître le tri courant : seule la bibliothèque le sait.
  // Inscrite uniquement pendant qu'une fiche est ouverte — hors de là, ces
  // boutons n'ont rien à faire.
  $effect(() => {
    if (!nav.openFull) return;
    return registerModNav(navigateFull);
  });

  // Un champ de saisie garde ses flèches (déplacement du caret) — et un
  // curseur `range` aussi, c'est un `<input>` : les réglages de l'aperçu 3D,
  // posés sur la fiche, restent réglables au clavier.
  function isTypingTarget(e: KeyboardEvent): boolean {
    const t = e.target as HTMLElement | null;
    return !!t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.tagName === "SELECT" || t.isContentEditable);
  }

  const ARROW_DIRS: Record<string, "up" | "down" | "left" | "right"> = {
    ArrowUp: "up",
    ArrowDown: "down",
    ArrowLeft: "left",
    ArrowRight: "right",
  };

  // Clavier dans la fiche pleine page (§7.4bis) :
  // - Page préc./suiv. = mod précédent/suivant. Les flèches tenaient ce rôle
  //   avant, et c'était le mauvais choix : elles sont l'équivalent naturel de
  //   la croix directionnelle, donc du déplacement du curseur DANS la fiche.
  //   Tant qu'elles changeaient de mod, rien de la fiche (skins, onglets,
  //   boutons) n'était atteignable autrement qu'à la souris.
  // - Flèches = déplacement du curseur, exactement comme la croix
  //   directionnelle (`moveFocus`, partagé avec la manette : un seul
  //   comportement, pas deux implémentations qui divergent).
  $effect(() => {
    function onKeydown(e: KeyboardEvent) {
      if (!nav.openFull || nav.openPack || isTypingTarget(e) || e.ctrlKey || e.altKey || e.metaKey) return;
      if (e.key === "PageUp") {
        e.preventDefault();
        navigateFull(-1);
      } else if (e.key === "PageDown") {
        e.preventDefault();
        navigateFull(1);
      } else if (ARROW_DIRS[e.key]) {
        e.preventDefault();
        moveFocus(ARROW_DIRS[e.key]);
      }
    }
    window.addEventListener("keydown", onKeydown);
    return () => window.removeEventListener("keydown", onKeydown);
  });

  // Ctrl+A = sélectionner tout **ce qui est affiché** (filtres et recherche
  // courants), jamais la bibliothèque entière. C'est le geste attendu après un
  // filtre précis, et c'est aussi ce qui rend l'erreur peu probable : on ne
  // sélectionne que ce qu'on a sous les yeux, et le décompte est écrit dans le
  // panneau comme dans la demande de confirmation d'une suppression.
  $effect(() => {
    function onSelectAll(e: KeyboardEvent) {
      if (nav.openFull || nav.openPack || !e.ctrlKey || e.altKey || e.metaKey) return;
      if (e.key !== "a" && e.key !== "A") return;
      // Dans la recherche ou un filtre, Ctrl+A garde son sens : tout le texte.
      if (isTypingTarget(e)) return;
      e.preventDefault();
      selectedIds = new Set(sorted.map((c) => c.id_interne));
      if (sorted.length) selectedId = sorted[sorted.length - 1].id_interne;
    }
    window.addEventListener("keydown", onSelectAll);
    return () => window.removeEventListener("keydown", onSelectAll);
  });
</script>

<div class="library">
  <!-- La fiche d'un pack (§4.4) se pose PAR-DESSUS celle du mod d'où l'on
       vient : la fermer y ramène, au lieu de renvoyer à la liste. -->
  {#if nav.openPack}
    <div class="full-wrap">
      <PackDetail
        pack={nav.openPack}
        onclose={() => void goBackOr(() => (nav.openPack = null))}
        onopenmod={(id) => { nav.openPack = null; nav.openFull = id; }}
        onuninstalled={() => { nav.openFull = null; refresh(); }}
      />
    </div>
  {:else if nav.openFull}
    <div class="full-wrap">
      <DetailPage
        id={nav.openFull}
        {kind}
        onclose={() => void goBackOr(() => { nav.openFull = null; scrollToEffective(); })}
        onchange={refresh}
      />
    </div>
  {:else}
  <div class="main-wrap" data-gp-region="list">
  <div class="main">
    <!-- Header and scrolling list are SIBLINGS, and that is the whole point.
         Both used to live in one box scrolling on both axes, the header held in
         place by `position: sticky` - which only ever pins on the axis it is
         given an offset on. Vertically it worked; horizontally the header was
         an ordinary block, as wide as the VISIBLE area and not as the scrolled
         content, so a table wide enough to scroll sideways slid it out of the
         way and bared a strip of rows above it (reported, measured: scrolled
         150 px right in a 411 px viewport, the header ended 150 px short).
         Out of the scroller it cannot slide, because there is nothing left to
         slide it - and the three patches that state used to need go with it:
         negative margins, `top: -18px`, and a header height measured in JS to
         offset the sticky table heads. -->
    <div class="head">
    <!-- Toute la barre de filtres tient ici (§6.3) : une ligne permanente et
         une rangée de puces, quels que soient les filtres posés. Elle a
         remplacé onze contrôles affichés en permanence sur deux rangées.
         L'écran garde la fin de ligne — colonnes et bascule de vue — parce
         qu'elle ne parle pas de filtrage. -->
    <FilterBar
      {defs}
      bind:filters
      bind:pinned
      bind:query
      optionsFor={index.optionsFor}
      presets={index.yearPresets}
      resultCount={filtered.length}
      perfRef={index.ctx.perfRef}
      perfUnreadable={index.perfUnreadable}
    >
      {#snippet end()}
        {#if shown === "table"}
          <ColumnsMenu
            items={columns.map((c) => ({ key: c.key, label: t(c.labelKey), fixed: c.fixed }))}
            visible={visibleKeys}
            ontoggle={toggleColumn}
          />
        {/if}

        <!-- Chevron ACCOLÉ à la bascule, pas posé à côté : les deux bords se
             partagent, sinon le menu se lit comme un contrôle de plus au lieu
             du complément de celui-ci. -->
        <div class="view-wrap">
          <Seg
            size="toolbar"
            tone="neutral"
            icon
            value={shown}
            onselect={(v) => setView(v as GridView)}
            items={[
              { value: "dense", label: "▦", title: t("library.viewDense") },
              { value: "comfortable", label: "▤", title: t("library.viewComfortable") },
              { value: "table", label: "☰", title: t("library.viewList") },
            ]}
          />
        <!-- Les préférences de présentation vivent **ici** et non dans les
             réglages globaux : il faut en voir l'effet pour les juger, et un
             écran de réglages les rend invisibles. -->
        <div class="display-wrap">
          <button class="disp-toggle" type="button" aria-expanded={showDisplay} title={t("library.displayMenu")} onclick={() => (showDisplay = !showDisplay)}>▾</button>
          {#if showDisplay}
            <div class="display-menu">
              <span class="dm-title">{t("library.density")}</span>
              {#each [["dense", "library.viewDense"], ["comfortable", "library.viewComfortable"], ["table", "library.viewList"]] as [id, key] (id)}
                <label>
                  <!-- `name` partagé : sans lui les trois boutons sont trois
                       cases indépendantes pour le navigateur, et le clavier
                       n'y circule plus comme dans un groupe. -->
                  <input type="radio" name="pitbox-density" checked={shown === id} onchange={() => setView(id as GridView)} />
                  <span>{t(key)}</span>
                </label>
              {/each}
              {#if isCar}
                <span class="dm-sep"></span>
                <label>
                  <input type="checkbox" checked={hideBrand} onchange={(e) => setHideBrand(e.currentTarget.checked)} />
                  <span>{t("library.hideBrand")}</span>
                </label>
              {/if}
            </div>
          {/if}
        </div>
        </div>
      {/snippet}
    </FilterBar>
    </div>

    <div class="scroll" bind:this={mainEl}>
    {#if loading}
      <LoadingState />
    {:else if filtered.length === 0}
      <div class="empty">
        {#if typed.length === 0}
          <p>{isCar ? t("library.emptyCars") : t("library.emptyTracks")}</p>
          <p class="hint">{t("library.emptyHint")}</p>
        {:else}
          <p>{t("library.noResults")}</p>
        {/if}
      </div>
    {:else if isGrid}
      <div
        class="grid"
        class:comfortable={shown === "comfortable"}
        style:--mat-hi={isCar && gridThumbsOn() ? preset.mat.hi : undefined}
        style:--mat-lo={isCar && gridThumbsOn() ? preset.mat.lo : undefined}
      >
        {#each filtered as c (c.id_interne)}
          {@const prefSkin = isCar ? getPreferredSkin(c.id_interne) : null}
          {@const prefLayout = !isCar ? getPreferredLayout(c.id_interne) : null}
          {@const src = previewSrc(prefSkin?.preview ?? prefLayout?.preview ?? c.preview)}
          <!-- La vignette régénérée prend le pas sur la `preview.png` du mod
               quand elle existe (SPEC-grille §5). Quand elle n'existe pas — et
               une voiture chiffrée n'en aura jamais — la carte garde la photo
               d'origine : la grille reste mixte pour toujours, et c'est le mat
               qui les rend comparables. -->
          {@const regen = isCar ? gridThumb(template, c.id_interne, prefSkin?.id ?? null) : null}
          {@const ol = previewSrc(prefLayout?.outline ?? c.outline)}
          {@const blocked = unusableReason(c)}
          <button data-id={c.id_interne} class="card" class:unusable={blocked !== null} class:sel={effectiveId === c.id_interne && selectedIds.size === 0} class:multisel={selectedIds.has(c.id_interne)} class:session={sessionId === c.id_interne} onclick={(e) => onCardClick(c, e)} ondblclick={() => (nav.openFull = c.id_interne)} oncontextmenu={(e) => openCardContextMenu(e, c)} title={blocked ? `${t(blocked)}\n${t("library.cardTooltip")}` : t("library.cardTooltip")}>
            <div class="thumb" use:whenVisible={{ car: c.id_interne, skin: prefSkin?.id ?? null, name: cardName(c), want: isCar && !(preset.skipStock && c.is_stock) }}>
              {#if regen ?? src}<img src={regen ?? src} alt={c.display_name ?? c.id_interne} loading="lazy" />
              {:else}<div class="noprev">{isCar ? t("library.typeCar") : t("library.typeTrack")}</div>{/if}
              {#if !isCar && ol}<img class="outline" src={ol} alt="" loading="lazy" />{/if}
              {#if sessionId === c.id_interne}<span class="sessbadge">{t("library.sessionBadge")}</span>{/if}
              <!-- Marqueur de note (SESSION§5) : sans lui, une note est en écriture
                   seule — on ne saurait plus sur quel mod on en a laissé une.
                   Posé en bas à gauche, en face du cœur : les deux disent la
                   même sorte de chose, « j'ai touché à ce mod ». -->
              {#if c.notes_user}<span class="card-note" title={c.notes_user}>✎</span>{/if}
              <span
                class="card-fav"
                class:on={c.is_favorite}
                role="button"
                tabindex="-1"
                title={t("common.favorite")}
                onclick={(e) => toggleFav(c, e)}
                onkeydown={(e) => e.key === "Enter" && toggleFav(c, e)}
              >{c.is_favorite ? "♥" : "♡"}</span>
            </div>
            <!-- Marque, modèle et année d'un seul tenant : voir `ModIdentity`,
                 partagé avec la colonne de session. -->
            <ModIdentity name={cardName(c)} badge={c.badge} brand={c.brand} year={c.year} dim={blocked !== null} />
          </button>
        {/each}
      </div>
    {:else}
      <div class="table-wrap">
        <table>
          <thead>
            <tr>
              {#each visibleColumns as col (col.key)}
                <th
                  data-col-key={col.key}
                  class="draggable"
                  class:sortable={col.sortable}
                  class:dragging={dragKey === col.key}
                  class:resizing={resizingKey === col.key}
                  class:drop-before={dropTarget?.key === col.key && dropTarget.before}
                  class:drop-after={dropTarget?.key === col.key && !dropTarget.before}
                  style={columnWidths[col.key] ? `width:${columnWidths[col.key]}px; max-width:${columnWidths[col.key]}px;` : undefined}
                  title={t("library.dragColumnTooltip")}
                  onclick={() => col.sortable && !suppressSortClick && toggleSort(col.key)}
                  onmousedown={(e) => startHeaderDrag(e, col.key)}
                >
                  <span class="th-label">
                    <!-- Le libellé d'une colonne triable est un vrai bouton :
                         sans lui, la seule chose focusable de l'entête était
                         la poignée de redimensionnement, et trier restait
                         hors de portée de la manette et du clavier. Aucun
                         gestionnaire dessus — le clic remonte au `<th>`, qui
                         trie déjà (et applique sa garde anti-glissé). Le
                         `mousedown` remonte lui aussi : glisser une colonne
                         par son libellé continue de marcher. -->
                    {#if col.sortable}
                      <button class="th-sort" type="button">{t(col.labelKey)}</button>
                    {:else}
                      {t(col.labelKey)}
                    {/if}
                    {#if col.tooltipKey}
                      <!-- "published" = avant-dernière colonne par défaut (juste avant
                           "size"), sa bulle centrée déborderait sur le panneau de droite
                           (bug réel constaté) — bord droit aligné, la bulle grandit vers
                           la gauche à la place. -->
                      <Tooltip text={t(col.tooltipKey)} side="bottom" align={col.key === "published" ? "right" : "center"}>
                        <button type="button" class="th-info" onclick={(e) => e.stopPropagation()} onmousedown={(e) => e.stopPropagation()}>ⓘ</button>
                      </Tooltip>
                    {/if}
                    {#if sortKey === col.key}<span class="arrow">{sortDir === 1 ? "▲" : "▼"}</span>{/if}
                  </span>
                  <!-- Poignée de redimensionnement (§6.2) : `draggable="false"` explicite
                       coupe l'héritage du glisser-déposer de réordonnancement posé sur le
                       `<th>` — sans ça, saisir la poignée déclencherait aussi un drag de
                       colonne. Repère visuel permanent (pas seulement au survol) : sans
                       indice visible, rien ne suggère qu'on peut redimensionner ici.
                       `role="separator"` + `tabindex` + flèches clavier = le motif
                       « separator (focusable) » documenté par le WAI-ARIA APG pour les
                       poignées de redimensionnement — le linter a11y de Svelte ne le
                       reconnaît pas comme interactif (liste de rôles trop stricte),
                       d'où les deux ignores ci-dessous plutôt qu'un vrai souci. -->
                  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
                  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
                  <span
                    class="col-resize"
                    data-gp-skip
                    draggable="false"
                    role="separator"
                    aria-orientation="vertical"
                    tabindex="0"
                    title={t("library.resizeColumnTooltip")}
                    onmousedown={(e) => startResize(e, col.key, (e.currentTarget as HTMLElement).closest("th")!.getBoundingClientRect().width)}
                    onclick={(e) => e.stopPropagation()}
                    ondblclick={(e) => { e.stopPropagation(); resetColumnWidth(col.key); }}
                    onkeydown={(e) => {
                      if (e.key === "ArrowLeft" || e.key === "ArrowRight") {
                        e.preventDefault();
                        e.stopPropagation();
                        const width = (e.currentTarget as HTMLElement).closest("th")!.getBoundingClientRect().width;
                        adjustColumnWidth(col.key, width, e.key === "ArrowRight" ? 10 : -10);
                      } else if (e.key === "Enter") {
                        e.stopPropagation();
                        resetColumnWidth(col.key);
                      }
                    }}
                  ></span>
                </th>
              {/each}
            </tr>
          </thead>
          <tbody>
            {#each sorted as c (c.id_interne)}
              <tr data-id={c.id_interne} tabindex="0" class:sel={effectiveId === c.id_interne && selectedIds.size === 0} class:multisel={selectedIds.has(c.id_interne)} class:session={sessionId === c.id_interne} onclick={(e) => onCardClick(c, e)} ondblclick={() => (nav.openFull = c.id_interne)} oncontextmenu={(e) => openCardContextMenu(e, c)}>
                {#each visibleColumns as col}
                  <td
                    class:t-name={col.key === "name"}
                    class:mono={col.mono}
                    class:t-tags={col.key === "tags"}
                    class:col-resized={!!columnWidths[col.key]}
                    style={columnWidths[col.key] ? `width:${columnWidths[col.key]}px; max-width:${columnWidths[col.key]}px;` : undefined}
                  >
                    {#if col.key === "active"}
                      <StateBadge active={c.active} stock={c.is_stock} unmanaged={c.is_unmanaged} />
                    {:else if col.key === "brand"}
                      {#if c.badge}<img class="brand-badge" src={previewSrc(c.badge)} alt="" loading="lazy" />{/if}
                      {col.value(c)}
                    {:else if col.key === "name"}
                      {#if c.broken}<span class="broken-flag" title={t("library.brokenTooltip")}>⚠</span>{/if}
                      {col.value(c)}
                    {:else}
                      {col.value(c)}
                    {/if}
                  </td>
                {/each}
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
    </div>
  </div>

  {#if selectedIds.size >= 2}
    <!-- Panneau bas en surimpression (§6.3ter) : ne réduit pas la largeur de
         la grille — flotte par-dessus `.main`, indépendant de son défilement
         interne (sibling non-scrollant dans `.main-wrap`, cf. style). -->
    <BulkEditPanel
      ids={[...selectedIds]}
      cards={typed.filter((c) => selectedIds.has(c.id_interne))}
      onclose={() => (selectedIds = new Set())}
      onchange={refresh}
    />
  {/if}
  </div>

  {/if}
  {#if ctxMenu}
    <ContextMenu x={ctxMenu.x} y={ctxMenu.y} items={contextItems} onclose={() => (ctxMenu = null)} />
  {/if}
</div>

<style>
  .library {
    display: flex;
    height: 100%;
    min-height: 0;
  }
  .main-wrap {
    /* Non-scrollant : ancre le panneau bas en surimpression (BulkEditPanel)
       indépendamment du défilement interne de `.main`. */
    position: relative;
    flex: 1;
    min-width: 0;
    min-height: 0;
    display: flex;
  }
  /* Ne défile pas lui-même : il empile un en-tête fixe et une zone qui, elle,
     défile. `min-height: 0` sans quoi `.scroll` refuserait de rétrécir sous la
     hauteur de son contenu et déborderait la fenêtre au lieu de défiler. */
  .main {
    flex: 1;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .full-wrap {
    flex: 1;
    min-width: 0;
    overflow-y: auto;
    padding: 28px 32px; /* compense le margin négatif de .page */
  }
  /* Recherche + filtres. Hors du conteneur qui défile (voir le commentaire du
     markup) : il ne bouge donc plus, sans rien qui l'y force. `z-index` pour
     que les listes d'autocomplétion des champs à jetons, qui débordent
     forcément sur la liste en dessous, passent par-dessus elle.
     Le retrait à droite vaut celui des autres bords PLUS la gouttière de
     défilement que `.scroll` réserve en permanence : sans lui, le tableau —
     large de la zone moins sa barre de défilement — s'arrêtait 9 px avant le
     bord du panneau de filtres, et le décalage se voyait sur les circuits, dont
     le tableau tient dans la largeur. */
  .head {
    position: relative;
    z-index: 2;
    padding: 14px calc(22px + var(--scrollbar-w)) 2px 22px;
  }
  /* La seule chose qui défile, sur les deux axes : la grille ou le tableau.
     `overflow-x` explicite plutôt que de compter sur la règle CSS qui promeut
     `visible` en `auto` quand l'autre axe ne l'est pas (§6.2, tableau large) —
     pas de doute possible sous WebView2.
     `position` + `z-index` en font un contexte d'empilement à lui : sans ça,
     les en-têtes de tableau collants (`thead th`, z-index 5) se comparaient
     directement aux listes d'autocomplétion de l'en-tête et passaient
     par-dessus elles (bug signalé). Les deux frères deviennent deux couches, et
     ce que chacun empile à l'intérieur ne regarde plus que lui.
     `scrollbar-gutter` : la gouttière est réservée même sans barre, sinon
     l'apparition de celle-ci décalerait le tableau sous un en-tête, lui, fixe. */
  .scroll {
    position: relative;
    z-index: 1;
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overflow-x: auto;
    scrollbar-gutter: stable;
    padding: 0 22px 18px;
  }
  /* Le menu des colonnes est parti dans `ColumnsMenu` ; celui-ci reste, et il
     empruntait ses styles au précédent. Le CSS étant scopé, l'extraction les
     lui aurait retirés en silence — d'où cette copie, qui n'en est plus une :
     c'est désormais le seul menu de cet écran. */
  .display-menu {
    position: absolute;
    top: calc(100% + 4px);
    right: 0;
    z-index: 20;
    background: var(--panel);
    border: 1px solid var(--line);
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 200px;
    box-shadow: 0 6px 18px rgb(0 0 0 / 40%);
  }
  .display-menu label {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: var(--txt2);
    padding: 3px 4px;
    cursor: pointer;
  }
  .display-menu label:hover {
    background: var(--raised);
  }
  .view-wrap {
    display: flex;
    align-items: stretch;
  }
  .display-wrap {
    position: relative;
    display: flex;
  }
  .disp-toggle {
    height: 32px;
    padding: 0 7px;
    background: var(--panel2);
    border: 1px solid var(--line);
    border-left: none;
    color: var(--muted);
    font-size: 10px;
    line-height: 1;
  }
  .disp-toggle:hover,
  .disp-toggle[aria-expanded="true"] {
    color: var(--txt);
    border-color: var(--faint2);
  }
  .display-menu .dm-title {
    color: var(--muted);
    font-family: var(--mono);
    font-size: 9px;
    letter-spacing: 1.5px;
    text-transform: uppercase;
    padding: 2px 4px 4px;
  }
  .display-menu .dm-sep {
    height: 1px;
    background: var(--line);
    margin: 6px 0;
  }
  .empty {
    color: var(--muted);
    text-align: center;
    padding: 60px 0;
  }
  .empty .hint {
    font-size: 12px;
    color: var(--faint);
    margin-top: 8px;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(190px, 1fr));
    gap: 9px;
    /* Taille du nom (marque + modèle + année, `ModIdentity`). La grille dense
       en met deux fois plus par écran : elle prend la petite des deux tailles
       qui cohabitaient avant, la confortable la grande. */
    --ident-size: 10.5px;
    /* Le détachement de la carte, en une valeur : un liseré clair en haut
       (la lumière tombe d'en haut) et une ombre portée. Nommé ici parce que
       chaque état de carte redéfinit `box-shadow` en entier et doit le
       remettre — un `box-shadow` ne se cumule pas d'une règle à l'autre. */
    --card-lift: inset 0 1px 0 rgba(255, 255, 255, 0.025), 0 4px 14px rgba(0, 0, 0, 0.35);
  }
  .grid.comfortable {
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    --ident-size: 12.5px;
  }
  /* **La carte se détache de la page.** Fond au-dessus de celui de la grille,
     bordure enfin visible, et une élévation : c'est le CONTENANT qui répond au
     « les voitures sombres sont indiscernables », pas l'image. Les previews
     sont opaques et portent leur propre fond, cuit dedans — aucun réglage de
     la carte ne peut agir derrière la voiture, et certaines voitures sont
     chiffrées, donc la grille restera hétérogène pour toujours. Ce traitement
     n'attend donc rien : il confine la disparité à l'intérieur de l'image au
     lieu de la laisser contaminer la carte. */
  .card {
    background: var(--cell);
    border: 1px solid var(--cell-line);
    padding: 8px;
    text-align: left;
    overflow: hidden;
    box-shadow: var(--card-lift);
    transition: border-color 0.12s;
    /* Évite la sélection de texte (surlignage bleu) lors des clics de
       sélection multiple Ctrl/Maj. */
    user-select: none;
  }
  .card:hover {
    border-color: var(--faint);
  }
  .card.sel {
    border-color: var(--rosso);
  }
  .card.multisel {
    border-color: var(--blue);
    box-shadow: 0 0 0 1px var(--blue), var(--card-lift);
  }
  .card.session {
    border-color: var(--rosso);
    box-shadow: 0 0 0 1px var(--rosso), var(--card-lift);
  }
  /* Ce qui empêche de jouer se dit en éteignant la carte, pas par un badge :
     une carte éteinte se comprend au premier regard, sans légende et sans
     traduction, là où un badge demanderait un vocabulaire à apprendre dans six
     langues. La RAISON est dans l'infobulle et dans la fiche, pas ici. Et la
     carte n'est jamais masquée : une voiture qui disparaît produit le « où est
     passée ma Skyline », bien pire qu'une carte éteinte. */
  .card.unusable {
    opacity: 0.42;
  }
  .sessbadge {
    position: absolute;
    bottom: 5px;
    left: 5px;
    background: var(--rosso);
    color: #fff;
    font-size: 7px;
    letter-spacing: 1px;
    font-family: var(--mono);
    padding: 1px 5px;
    z-index: 1;
  }
  tbody tr.session {
    box-shadow: inset 2px 0 0 var(--rosso);
  }
  /* **Le mat.** L'image n'est plus à ras du bord : elle est posée sur un
     rectangle neutre un ton au-dessus de la carte. Il ne se voit presque pas
     quand la preview est sombre et remplit son cadre ; il devient le liant dès
     que les previews sont hétérogènes, parce qu'il donne à toutes les cartes
     le même encadrement. C'est aussi lui qui absorbe une preview absente ou de
     format inattendu : ce qui manque montre le mat plutôt qu'un trou noir. */
  .thumb {
    position: relative;
    aspect-ratio: 16 / 9;
    /* Dégradé radial, pas un ton plat : voir `--mat-hi`/`--mat-lo`. Le centre
       est légèrement au-dessus du milieu, là où se pose une voiture.
       Les deux bouts viennent du **preset** quand les vignettes régénérées sont
       allumées : la moitié de ce qui rend une preview d'Assetto Corsa belle est
       son fond, et le nôtre s'écrit ici plutôt que dans l'image. */
    background: radial-gradient(ellipse at 50% 44%, var(--mat-hi) 0%, var(--mat-lo) 76%);
    border: 1px solid var(--mat-line);
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
  }
  /* `contain` et non `cover` : les images ne sont JAMAIS retouchées — ni
     filtre, ni correction de luminosité, ni masque. Une preview soignée par
     son auteur ne doit pas être dégradée pour compenser celles qui ne le sont
     pas. Elle est donc montrée entière, et ce qui reste est du mat. */
  .thumb img {
    width: 100%;
    height: 100%;
    object-fit: contain;
  }
  /* Tracé du circuit superposé à la photo (§6.1). */
  .thumb img.outline {
    position: absolute;
    inset: 0;
    object-fit: contain;
    padding: 8px;
  }
  .noprev {
    color: var(--faint);
    font-size: 10px;
    letter-spacing: 1px;
    text-transform: uppercase;
  }
  .broken-flag {
    color: var(--yellow);
    margin-right: 4px;
  }
  .card-note {
    position: absolute;
    bottom: 5px;
    left: 6px;
    font-size: 12px;
    line-height: 1;
    color: var(--muted2);
    text-shadow: 0 0 3px var(--bg);
    pointer-events: auto;
  }
  .card-fav {
    position: absolute;
    bottom: 5px;
    right: 6px;
    font-size: 15px;
    line-height: 1;
    color: var(--muted2);
    cursor: pointer;
    text-shadow: 0 0 3px var(--bg);
  }
  .card-fav.on {
    color: var(--rosso-bright);
  }
  .card-fav:hover {
    color: var(--rosso-bright);
  }
  /* `.brand-badge` reste ici pour la colonne « Marque » du TABLEAU seule — la
     grille, elle, passe par `ModIdentity`, qui porte le sien. */
  .brand-badge {
    flex: none;
    width: 13px;
    height: 13px;
    object-fit: contain;
    /* Pour la colonne « Marque » du tableau, où le logo est en ligne dans du
       texte — sans effet dans la carte, qui est en flex. */
    vertical-align: -2px;
    margin-right: 4px;
  }

  .table-wrap {
    border: 1px solid var(--line);
    /* Pas d'overflow ici : ce serait un conteneur de scroll imbriqué et les
       en-têtes sticky se colleraient à lui (invisible) au lieu de `.main`. Le
       défilement horizontal des tableaux larges est géré par `.main`. */
  }
  table {
    /* `max-content` plutôt que `100%` : avec beaucoup de colonnes visibles
       (§6.2), un tableau capé à la largeur du conteneur se contente de
       compresser chaque colonne au lieu de déborder — au point qu'une
       colonne tout juste cochée peut devenir quasi invisible plutôt que de
       déclencher le défilement horizontal prévu par `.table-wrap`/`.main`
       (bug réel constaté). `min-width: 100%` garde un tableau à peu de
       colonnes étalé sur toute la largeur disponible, comme avant. */
    width: max-content;
    min-width: 100%;
    border-collapse: collapse;
    font-size: 12px;
    /* Pas de surlignage de texte lors des clics de sélection (lignes + en-têtes triables). */
    user-select: none;
  }
  th {
    /* Ancre la poignée de redimensionnement (position absolute). `sticky`
       (règle suivante) l'établirait déjà, mais autant ne pas en dépendre. */
    position: relative;
    text-align: left;
    padding: 8px 10px;
    color: var(--muted);
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    border-bottom: 1px solid var(--line);
    background: var(--panel2);
    white-space: nowrap;
  }
  /* En-têtes collés en haut de `.scroll`, dont le bord haut est déjà sous le
     bandeau recherche+filtres : `top: 0` suffit, plus rien à mesurer. C'est ce
     que la barre sortie du scroller a fait gagner. `box-shadow` = ligne de
     séparation fiable (les bordures collapse ne « suivent » pas le sticky sous
     Chromium/WebView2). */
  thead th {
    position: sticky;
    top: 0;
    z-index: 5;
    box-shadow: inset 0 -1px 0 var(--line);
  }
  th.sortable {
    cursor: pointer;
    user-select: none;
  }
  th.sortable:hover {
    color: var(--txt2);
  }
  th .arrow {
    margin-left: 4px;
    color: var(--rosso-bright);
  }
  .th-label {
    padding-right: 8px;
    display: inline-flex;
    align-items: center;
    gap: 3px;
  }
  /* Bouton uniquement pour être atteignable (focus manette/clavier) : il ne
     doit rien changer à l'apparence de l'entête, qui est déjà cliquable sur
     toute sa surface. */
  .th-sort {
    background: none;
    border: none;
    padding: 0;
    margin: 0;
    font: inherit;
    color: inherit;
    letter-spacing: inherit;
    text-transform: inherit;
    text-align: left;
    cursor: inherit;
  }
  .th-info {
    background: transparent;
    border: none;
    padding: 0;
    color: var(--faint);
    font-size: 10px;
    line-height: 1;
    cursor: help;
  }
  .th-info:hover,
  .th-info:focus-visible {
    color: var(--rosso-bright);
  }
  /* Réordonnement au glissé souris, pas le drag HTML5 natif (§6.2, abandonné
     après deux tentatives infructueuses sous WebView2 — voir `startHeaderDrag`).
     La colonne fixe (nom) n'a pas `.draggable`, donc jamais ce curseur —
     cohérent avec le fait qu'elle ne peut être ni déplacée ni servir de cible
     avant elle-même. */
  th.draggable {
    cursor: grab;
  }
  th.dragging {
    opacity: 0.4;
  }
  /* Repère de dépôt : ligne verticale du côté où la colonne glissée
     s'insérerait si on relâchait maintenant. */
  th.drop-before::before,
  th.drop-after::after {
    content: "";
    position: absolute;
    top: 0;
    bottom: 0;
    width: 2px;
    background: var(--rosso-bright);
    z-index: 3;
  }
  th.drop-before::before {
    left: 0;
  }
  th.drop-after::after {
    right: 0;
  }
  /* Poignée de redimensionnement (§6.2) : bande à la jonction de deux
     colonnes. Repère visuel permanent (pas seulement au survol) — un simple
     changement de curseur ne suffit pas à faire découvrir la fonction. */
  .col-resize {
    position: absolute;
    top: 0;
    bottom: 0;
    right: -4px;
    width: 8px;
    cursor: col-resize;
    z-index: 2;
  }
  .col-resize::after {
    content: "";
    position: absolute;
    top: 5px;
    bottom: 5px;
    left: 3px;
    width: 2px;
    background: var(--line);
  }
  .col-resize:hover::after,
  .col-resize:focus-visible::after,
  th.resizing .col-resize::after {
    background: var(--rosso-border);
  }
  .col-resize:focus-visible {
    outline: none;
  }
  td.col-resized {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  td {
    padding: 7px 10px;
    border-bottom: 1px solid var(--line);
    color: var(--txt2);
    white-space: nowrap;
  }
  tbody tr {
    cursor: pointer;
  }
  tbody tr:hover {
    background: var(--raised);
  }
  tbody tr.sel {
    background: var(--rosso-dim);
  }
  tbody tr.multisel {
    background: var(--blue-dim);
    box-shadow: inset 2px 0 0 var(--blue);
  }
  .t-name {
    font-weight: 600;
    color: var(--txt);
  }
  .t-tags {
    color: var(--muted);
    white-space: normal;
  }
  /* La pastille d'état vit dans `StateBadge.svelte`, partagée avec la fiche. */
</style>
