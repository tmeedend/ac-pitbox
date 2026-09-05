<script lang="ts">
  import { tick, untrack, onMount, onDestroy } from "svelte";
  import DetailPage from "./DetailPage.svelte";
  import PackDetail from "./PackDetail.svelte";
  import FilterBar from "./filters/FilterBar.svelte";
  import { hasOwnDriver } from "$lib/driverOverride.svelte";
  import BulkEditPanel from "./BulkEditPanel.svelte";
  import ContextMenu from "./ContextMenu.svelte";
  import LoadingState from "./LoadingState.svelte";
  import StateBadge from "./StateBadge.svelte";
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
  import { moveFocus } from "$lib/gamepadNav";
  import { registerModNav } from "$lib/screenActions";
  import { libraryVersion } from "$lib/libraryVersion.svelte";
  import { getPreferredSkin, getPreferredLayout } from "$lib/preferred";
  import { buildModContextItems } from "$lib/modContextActions";
  import { t } from "$lib/i18n/index.svelte";
  import { zoomFactor } from "$lib/zoom.svelte";
  import { getUiPrefs, setUiPref } from "$lib/uiPrefs.svelte";
  import {
    buildPredicate,
    decadePresets,
    filterDefs,
    optionsOf,
    parseFilters,
    parsePinned,
    serializeFilters,
    type FilterContext,
    type FilterMap,
    type FilterOption,
  } from "$lib/filters";

  import { StorageKey } from "$lib/storage";
  // Une bibliothèque par type (§6.1) : ce composant est rendu une fois pour les
  // voitures, une fois pour les circuits. Toute la persistance est suffixée par
  // type pour rester indépendante entre les deux.
  let { kind }: { kind: ModKind } = $props();
  // `kind` ne change jamais pour une instance montée (§6.1, deux instances
  // fixes voitures/circuits) — `untrack` documente que ces lectures ne
  // capturent la prop qu'une fois, volontairement, pour le compilateur.
  const isCar = untrack(() => kind === "Car");
  // Clés de persistance bâties une fois, au même endroit : chaque réglage doit
  // rester indépendant entre voitures et circuits (§6.1).
  const KEYS = untrack(() => ({
    filters: StorageKey.libraryFilters(kind),
    pinned: StorageKey.libraryPinned(kind),
    view: StorageKey.libraryView(kind),
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
  let view = $state<"gallery" | "table">("gallery");
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
  let showColumns = $state(false);
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
   * position relative une fois réaffichées). Colonne fixe jamais déplaçable —
   * ni comme source, ni comme cible ne bougeant elle-même (on peut déposer
   * dessus : la colonne déplacée vient alors juste après elle, seule place
   * valide avant la 1ʳᵉ colonne libre). */
  function reorderColumn(sourceKey: string, targetKey: string, before: boolean) {
    if (sourceKey === targetKey) return;
    if (columns.find((c) => c.key === sourceKey)?.fixed) return;
    const rest = columnOrder.filter((k) => k !== sourceKey);
    const targetIsFixed = columns.find((c) => c.key === targetKey)?.fixed;
    const targetIdx = rest.indexOf(targetKey);
    const insertAt = targetIsFixed ? targetIdx + 1 : before ? targetIdx : targetIdx + 1;
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
    if (columns.find((c) => c.key === key)?.fixed) return;
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

  // Restauration au montage (§6.2/§8.6) : colonnes (fichier dédié,
  // `columns.ts`) et le reste des petits réglages d'écran (`uiPrefs.ts`) en
  // parallèle, un seul aller-retour chacun. `prefsReady` n'est levé qu'une
  // fois tout appliqué, pour que l'effet de persistance des filtres plus haut
  // ne réécrive rien avant d'avoir vu les vraies valeurs sauvegardées.
  onMount(async () => {
    const [colPrefs, saved] = await Promise.all([
      loadColumnsPrefs(kind),
      getUiPrefs([FKEY, KEYS.pinned, KEYS.view, KEYS.sortKey, KEYS.sortDir]),
    ]);
    visibleKeys = colPrefs.visible;
    columnOrder = colPrefs.order;
    columnWidths = colPrefs.widths;

    const restored = parseFilters(saved[FKEY], defs);
    query = restored.query;
    filters = restored.filters;
    pinned = parsePinned(saved[KEYS.pinned], defs);
    const savedView = saved[KEYS.view];
    if (savedView === "gallery" || savedView === "table") view = savedView;
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

  function setView(v: "gallery" | "table") {
    view = v;
    if (prefsReady) setUiPref(KEYS.view, v);
  }

  // Panneau latéral toujours ouvert (jamais de saut de largeur du panneau
  // central) : à défaut d'un clic explicite, on affiche le choix de session
  // courant. Défilement vers l'élément sélectionné à revenir sur cet écran.
  let mainEl = $state<HTMLDivElement | undefined>();
  let firstLoad = true;
  function scrollToEffective() {
    if (!effectiveId) return;
    tick().then(() => {
      const el = mainEl?.querySelector(`[data-id="${CSS.escape(effectiveId!)}"]`);
      el?.scrollIntoView({ block: "center" });
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

  // La bibliothèque EST le sélecteur (§8.6) : ouvrir une carte la définit comme
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
  function select(c: ModCard) {
    selectedId = c.id_interne;
    // Restaure les préférences mémorisées de l'entité (skin voiture, layout circuit).
    const sk = isCar ? getPreferredSkin(c.id_interne) : null;
    const lay = !isCar ? getPreferredLayout(c.id_interne) : null;
    const meta = isCar
      ? [c.brand, sk ? `skin: ${sk.name}` : c.category].filter(Boolean).join(" · ")
      : [lay?.name ?? c.category, c.author].filter(Boolean).join(" · ");
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

  // Ouverture demandée depuis une vue transversale (§12bis.3) : on ouvre la
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

  // Les trois origines de tags (fichier mod, règle, manuel) sont équivalentes
  // pour filtrer/rechercher — seule la fiche détail les distingue par origine.
  function modTags(c: ModCard): string[] {
    return [...c.tags_from_mod, ...c.tags_from_rule, ...c.tags_manual];
  }

  // Descriptions ready to be matched, built ONCE per list load rather than on
  // every keystroke: ~400 KB of prose over a full library, which is cheap to
  // walk but not to lowercase again at each letter typed.
  //
  // Markup is stripped first: 116 of the 124 descriptions measured on a real
  // library carry HTML (`<br>`, `<b>`, `<font color=...>`), so matching the raw
  // text would make "b", "br", "font" or "color" hit nearly every mod.
  const descIndex = $derived.by(() => {
    const map = new Map<string, string>();
    for (const c of typed) {
      if (c.description) map.set(c.id_interne, c.description.replace(/<[^>]*>/g, " ").toLowerCase());
    }
    return map;
  });

  /** Ce que le moteur de filtres a besoin de savoir lire sur une carte. Les
   * trois origines de tags sont équivalentes ici ; seule la fiche détail les
   * distingue par origine. */
  const ctx: FilterContext = $derived({
    isCar,
    tagsOf: modTags,
    descOf: (c) => descIndex.get(c.id_interne),
    hasDriver: hasOwnDriver,
  });

  /** Valeurs proposées par filtre, avec leur décompte. Calculées sur le type
   * courant et **pas** sur les résultats filtrés : un chiffre qui bouge à
   * chaque jeton posé ne sert à rien pour décider du jeton suivant. */
  const optionIndex = $derived.by(() => {
    const map = new Map<string, FilterOption[]>();
    for (const def of defs) {
      if (def.type === "val") map.set(def.key, optionsOf(def, typed, ctx));
    }
    return map;
  });
  const yearPresets = $derived(isCar ? decadePresets(typed.map((c) => c.year ?? 0)) : []);

  const matchesFilters = $derived(buildPredicate(defs, filters, ctx));

  const filtered = $derived(
    typed.filter((c) => {
      if (!matchesFilters(c)) return false;
      if (query.trim()) {
        // Un terme par mot séparé par un espace, ET entre eux mais chacun en
        // simple "contains" (pas besoin d'être collés ni dans l'ordre) — bug
        // réel signalé : « GT-M Evo » ne remontait pas « GT-M Adonis Evo »,
        // recherché comme une seule sous-chaîne collée.
        const terms = query.toLowerCase().split(/\s+/).filter(Boolean);
        // Inclut le pack (§4.4) : rechercher son nom remonte toutes ses voitures.
        const hay =
          `${c.display_name ?? ""} ${c.brand ?? ""} ${c.id_interne} ${c.category ?? ""} ${c.source_pack ?? ""} ${modTags(c).join(" ")}`.toLowerCase();
        if (!terms.every((term) => hay.includes(term))) return false;
      }
      return true;
    }),
  );

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
        onclose={() => (nav.openPack = null)}
        onopenmod={(id) => { nav.openPack = null; nav.openFull = id; }}
        onuninstalled={() => { nav.openFull = null; refresh(); }}
      />
    </div>
  {:else if nav.openFull}
    <div class="full-wrap">
      <DetailPage
        id={nav.openFull}
        {kind}
        onclose={() => { nav.openFull = null; scrollToEffective(); }}
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
      optionsFor={(key) => optionIndex.get(key) ?? []}
      presets={yearPresets}
      resultCount={filtered.length}
    >
      {#snippet end()}
        {#if view === "table"}
          <div class="columns-wrap">
            <button class="btn" type="button" onclick={() => (showColumns = !showColumns)}>{t("library.columns")}</button>
            {#if showColumns}
              <div class="columns-menu">
                {#each columns as col}
                  <label class:fixed={col.fixed}>
                    <input
                      type="checkbox"
                      checked={col.fixed || visibleKeys.includes(col.key)}
                      disabled={col.fixed}
                      onchange={() => toggleColumn(col.key)}
                    />
                    <span>{t(col.labelKey)}</span>
                  </label>
                {/each}
              </div>
            {/if}
          </div>
        {/if}

        <div class="seg view">
          <button class:on={view === "gallery"} onclick={() => setView("gallery")} title={t("library.galleryView")}>▦</button>
          <button class:on={view === "table"} onclick={() => setView("table")} title={t("library.tableView")}>≣</button>
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
    {:else if view === "gallery"}
      <div class="grid">
        {#each filtered as c (c.id_interne)}
          {@const prefSkin = isCar ? getPreferredSkin(c.id_interne) : null}
          {@const prefLayout = !isCar ? getPreferredLayout(c.id_interne) : null}
          {@const src = previewSrc(prefSkin?.preview ?? prefLayout?.preview ?? c.preview)}
          {@const ol = previewSrc(prefLayout?.outline ?? c.outline)}
          <button data-id={c.id_interne} class="card" class:sel={effectiveId === c.id_interne && selectedIds.size === 0} class:multisel={selectedIds.has(c.id_interne)} class:session={sessionId === c.id_interne} onclick={(e) => onCardClick(c, e)} ondblclick={() => (nav.openFull = c.id_interne)} oncontextmenu={(e) => openCardContextMenu(e, c)} title={t("library.cardTooltip")}>
            <div class="thumb">
              {#if src}<img src={src} alt={c.display_name ?? c.id_interne} loading="lazy" />
              {:else}<div class="noprev">{isCar ? t("library.typeCar") : t("library.typeTrack")}</div>{/if}
              {#if !isCar && ol}<img class="outline" src={ol} alt="" loading="lazy" />{/if}
              {#if sessionId === c.id_interne}<span class="sessbadge">{t("library.sessionBadge")}</span>{/if}
              {#if c.is_unmanaged}
                <span class="sbadge unm" title={t("library.unmanagedTooltip")}>{t("library.unmanagedBadge")}</span>
              {:else if c.is_stock}
                <span class="sbadge" title={t("library.stockTooltip")}>{t("library.baseBadge")}</span>
              {:else}
                <span class="dot" class:active={c.active} title={c.active ? t("common.active") : t("common.inactive")}></span>
              {/if}
              {#if c.broken}<span class="brokenbadge" title={t("library.brokenTooltip")}>⚠ {t("library.brokenBadge")}</span>{/if}
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
            <div class="c-name">{c.display_name ?? c.id_interne}</div>
            <div class="c-sub">
              {#if c.badge}<img class="brand-badge" src={previewSrc(c.badge)} alt="" loading="lazy" />{/if}
              {c.brand ?? ""}{c.year ? ` · ${c.year}` : ""}
            </div>
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
                  class:sortable={col.sortable}
                  class:draggable={!col.fixed}
                  class:dragging={dragKey === col.key}
                  class:resizing={resizingKey === col.key}
                  class:drop-before={dropTarget?.key === col.key && dropTarget.before}
                  class:drop-after={dropTarget?.key === col.key && !dropTarget.before}
                  style={columnWidths[col.key] ? `width:${columnWidths[col.key]}px; max-width:${columnWidths[col.key]}px;` : undefined}
                  title={col.fixed ? undefined : t("library.dragColumnTooltip")}
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
  .columns-wrap {
    position: relative;
  }
  .columns-menu {
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
    min-width: 180px;
    box-shadow: 0 6px 18px rgba(0, 0, 0, 0.4);
  }
  .columns-menu label {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: var(--txt2);
    padding: 3px 4px;
    cursor: pointer;
  }
  .columns-menu label:hover {
    background: var(--raised);
  }
  .columns-menu label.fixed {
    color: var(--muted);
    cursor: default;
  }
  .seg {
    display: flex;
    border: 1px solid var(--line);
  }
  /* 32 px comme la recherche et le bouton « + Filtre » : ce sont des contrôles,
     et la barre de filtres n'en connaît que deux hauteurs (§7.1). */
  .seg button {
    background: var(--panel2);
    color: var(--muted);
    height: 32px;
    padding: 0 11px;
    font-size: 11.5px;
    border-right: 1px solid var(--line);
  }
  .seg button:last-child {
    border-right: none;
  }
  .seg button.on {
    background: var(--raised);
    color: var(--txt);
  }
  .seg.view button {
    font-size: 14px;
    padding: 0 10px;
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
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
    gap: 12px;
  }
  .card {
    background: var(--card);
    border: 1px solid var(--line);
    padding: 0;
    text-align: left;
    overflow: hidden;
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
    box-shadow: 0 0 0 1px var(--blue);
  }
  .card.session {
    border-color: var(--rosso);
    box-shadow: 0 0 0 1px var(--rosso);
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
  .thumb {
    position: relative;
    aspect-ratio: 16 / 9;
    background: var(--bg);
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
  }
  .thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
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
  .dot {
    position: absolute;
    top: 6px;
    left: 6px;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--muted2);
    box-shadow: 0 0 0 2px var(--bg);
  }
  .dot.active {
    background: var(--green);
  }
  .sbadge {
    position: absolute;
    top: 5px;
    left: 5px;
    background: var(--raised);
    color: var(--blue);
    border: 1px solid var(--blue-border);
    font-size: 8px;
    font-family: var(--mono);
    letter-spacing: 0.5px;
    padding: 1px 4px;
  }
  /* Mod installé hors Pit Box (§12bis.1bis) : même badge, en gris — la même
     grammaire de couleurs que la pastille d'état (StateBadge). */
  .sbadge.unm {
    color: var(--muted);
    border-color: var(--line);
  }
  /* Mod cassé (§6.4) : signalement visuel sur la carte, même détection que
     l'écran Maintenance. */
  .brokenbadge {
    position: absolute;
    top: 5px;
    right: 5px;
    background: #1a1708;
    color: var(--yellow);
    border: 1px solid #4a4426;
    font-size: 8px;
    font-family: var(--mono);
    letter-spacing: 0.5px;
    padding: 1px 4px;
    z-index: 1;
  }
  .broken-flag {
    color: var(--yellow);
    margin-right: 4px;
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
  .c-name {
    font-size: 12px;
    font-weight: 600;
    padding: 7px 8px 2px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .c-sub {
    font-size: 11px;
    color: var(--muted);
    padding: 0 8px 8px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .brand-badge {
    width: 13px;
    height: 13px;
    object-fit: contain;
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
