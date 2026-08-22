<script lang="ts">
  import { tick, untrack, onMount, onDestroy } from "svelte";
  import ModDetail from "./ModDetail.svelte";
  import DetailPage from "./DetailPage.svelte";
  import PackDetail from "./PackDetail.svelte";
  import TokenFilter, { type Token } from "./TokenFilter.svelte";
  import TriCheck, { type TriState } from "./TriCheck.svelte";
  import BulkEditPanel from "./BulkEditPanel.svelte";
  import ContextMenu from "./ContextMenu.svelte";
  import LoadingState from "./LoadingState.svelte";
  import NumberStepper from "./NumberStepper.svelte";
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
  import { getUiPrefs, setUiPref } from "$lib/uiPrefs.svelte";

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
    view: StorageKey.libraryView(kind),
    sortKey: StorageKey.librarySortKey(kind),
    sortDir: StorageKey.librarySortDir(kind),
  }));

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
  // Bornes de saisie de la fourchette d'année (mêmes constantes que le vivier
  // d'adversaires de Launch.svelte, §8.6).
  const YEAR_RANGE_MIN = 1950;
  const YEAR_RANGE_MAX = new Date().getFullYear();
  // « Pas de borne de ce côté », et c'est l'état par défaut : le champ est
  // alors **vide** (`emptyValue` du NumberStepper) et ne filtre rien.
  // Auparavant le défaut était 1950/2026, ce qui affichait deux bornes qu'on
  // n'avait pas demandées et ne pouvait pas effacer — vider le champ le
  // ramenait à sa borne, et vider « année max » l'écrasait même à 1950, ne
  // laissant plus rien remonter.
  const NO_YEAR = 0;
  // Page détail pleine page (§6.3) : double-clic sur une carte, ou bouton
  // « Agrandir » du panneau latéral. État centralisé dans nav.openFull (voir
  // nav.svelte.ts) — la navigation manette globale (AppShell) doit savoir si
  // elle est ouverte pour céder gauche/droite au visualiseur et gérer B=fermer.

  // Filtres persistés par type (rechargés au retour sur la page). Défauts
  // synchrones à l'affichage initial, remplacés par les valeurs sauvegardées
  // dès que l'onMount plus bas répond (même schéma que les colonnes, §6.2).
  const FKEY = KEYS.filters;
  let query = $state<string>("");
  let categoryFilter = $state<string>("all");
  let classFilter = $state<"all" | "race" | "street">("all");
  let stateFilter = $state<"all" | "active" | "inactive">("all");
  let authorFilter = $state<string>("all");
  let countryFilter = $state<string>("all");
  // Jetons inclure/exclure (§6.3). Ils remplacent un champ texte à virgules
  // (tags) et un `<select>` mono-valué (catégorie), qui partageaient la même
  // limite : on ne savait dire que « ceux-ci », jamais « tous sauf ceux-là ».
  let tagTokens = $state<Token[]>([]);
  let tagMode = $state<"and" | "or">("and");
  let catTokens = $state<Token[]>([]);
  // Trois états chacune : neutre, « uniquement ceux-ci », « tous sauf ceux-ci ».
  let favState = $state<TriState>(0);
  let triedState = $state<TriState>(0);
  let stockState = $state<TriState>(0);
  let yearMin = $state<number>(NO_YEAR);
  let yearMax = $state<number>(NO_YEAR);
  let view = $state<"gallery" | "table">("gallery");
  let sortKey = $state<string>("name");
  let sortDir = $state<1 | -1>(1);
  // Garde toutes les persistances ci-dessous tant que l'onMount plus bas n'a
  // pas fini de restaurer les valeurs sauvegardées — sans ça, l'effet des
  // filtres se déclenche dès le montage avec les défauts et les réécrit par-
  // dessus la sauvegarde avant même qu'elle soit lue (bug réel, même classe
  // que `ready` dans Launch.svelte).
  let prefsReady = false;

  // Persistance des filtres (champ libre + rubrique Filtres).
  $effect(() => {
    const snapshot = {
      query,
      category: categoryFilter,
      class: classFilter,
      state: stateFilter,
      author: authorFilter,
      country: countryFilter,
      // Sérialisés sous des clés distinctes des anciennes (`tag`, `fav`,
       // `neverTried`, `hideBaseContent`) : une préférence enregistrée par une
       // version antérieure garde son ancienne forme, que la relecture sait
       // encore convertir (voir l'onMount). Réutiliser les mêmes clés avec un
       // type différent aurait rendu les deux indiscernables.
      tagTokens,
      tagMode,
      catTokens,
      favState,
      triedState,
      stockState,
      yearMin,
      yearMax,
    };
    if (prefsReady) setUiPref(FKEY, JSON.stringify(snapshot));
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
    resizeStartWidth = currentWidth;
    window.addEventListener("mousemove", onResizeMove);
    window.addEventListener("mouseup", onResizeUp);
  }
  function onResizeMove(e: MouseEvent) {
    if (!resizingKey) return;
    const next = Math.max(MIN_COLUMN_WIDTH, Math.round(resizeStartWidth + (e.clientX - resizeStartX)));
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
    columnWidths = { ...columnWidths, [key]: Math.max(MIN_COLUMN_WIDTH, Math.round(currentWidth + delta)) };
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

  /** Borne d'année lue depuis les filtres enregistrés. `legacyNone` est
   * l'ancienne sentinelle « pas de borne » de ce côté (la borne de la plage
   * elle-même), traduite en champ vide : elle n'a jamais rien filtré, elle ne
   * doit pas se mettre à le faire ni à compter comme un filtre actif. */
  function readYearFilter(raw: unknown, legacyNone: number): number {
    const v = raw as number | null | undefined;
    if (v == null || v === legacyNone) return NO_YEAR;
    return v;
  }

  // Conversion des filtres enregistrés par une version antérieure. Les tags
  // étaient un texte à virgules, tous inclus ; la catégorie un choix unique,
  // `"all"` valant « pas de filtre ». Rien à jeter : les deux se traduisent
  // exactement en jetons « inclure ».
  function legacyTagTokens(raw: unknown): Token[] {
    if (typeof raw !== "string") return [];
    return raw
      .split(",")
      .map((s) => s.trim())
      .filter(Boolean)
      .map((value) => ({ value, mode: "inc" as const }));
  }
  function legacyCatTokens(raw: unknown): Token[] {
    return typeof raw === "string" && raw !== "" && raw !== "all" ? [{ value: raw, mode: "inc" }] : [];
  }

  // Restauration au montage (§6.2/§8.6) : colonnes (fichier dédié,
  // `columns.ts`) et le reste des petits réglages d'écran (`uiPrefs.ts`) en
  // parallèle, un seul aller-retour chacun. `prefsReady` n'est levé qu'une
  // fois tout appliqué, pour que l'effet de persistance des filtres plus haut
  // ne réécrive rien avant d'avoir vu les vraies valeurs sauvegardées.
  onMount(async () => {
    const [colPrefs, saved] = await Promise.all([
      loadColumnsPrefs(kind),
      getUiPrefs([FKEY, KEYS.view, KEYS.sortKey, KEYS.sortDir]),
    ]);
    visibleKeys = colPrefs.visible;
    columnOrder = colPrefs.order;
    columnWidths = colPrefs.widths;

    if (saved[FKEY]) {
      try {
        const sf: Record<string, unknown> = JSON.parse(saved[FKEY]);
        query = (sf.query as string) ?? "";
        classFilter = (sf.class as "all" | "race" | "street") ?? "all";
        stateFilter = (sf.state as "all" | "active" | "inactive") ?? "all";
        authorFilter = (sf.author as string) ?? "all";
        countryFilter = (sf.country as string) ?? "all";
        tagTokens = (sf.tagTokens as Token[]) ?? legacyTagTokens(sf.tag);
        tagMode = (sf.tagMode as "and" | "or") ?? "and";
        catTokens = (sf.catTokens as Token[]) ?? legacyCatTokens(sf.category);
        favState = (sf.favState as TriState) ?? (sf.fav ? 1 : 0);
        // Le sens s'est inversé : la case s'appelait « jamais essayé » (donc
        // cochée = jamais essayés), le tri-état s'appelle « déjà essayé ». Une
        // préférence enregistrée cochée devient donc l'état **rouge**.
        triedState = (sf.triedState as TriState) ?? (sf.neverTried ? -1 : 0);
        stockState = (sf.stockState as TriState) ?? (sf.hideBaseContent ? -1 : 0);
        // Un filtre enregistré avant que « vide » n'existe portait les bornes
        // de la plage comme sentinelle de « pas de borne » : elles se lisent
        // donc comme un champ vide, ce qu'elles ont toujours voulu dire.
        yearMin = readYearFilter(sf.yearMin, YEAR_RANGE_MIN);
        yearMax = readYearFilter(sf.yearMax, YEAR_RANGE_MAX);
      } catch {
        /* repli sur les défauts déjà en place */
      }
    }
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
  // Bandeau recherche+filtres épinglé : on mesure sa hauteur pour décaler
  // d'autant les en-têtes de tableau sticky (sinon ils passeraient dessous).
  let pinTopEl = $state<HTMLDivElement | undefined>();
  $effect(() => {
    const el = pinTopEl;
    const main = mainEl;
    if (!el || !main) return;
    // `- 18` = le padding-top de `.main`, que `.pin-top` compense par sa marge
    // négative (cf. son CSS `top: -18px`) ; sa hauteur visible une fois collé
    // est donc offsetHeight - 18. Exposé en variable CSS lue par `thead th`.
    const update = () => main.style.setProperty("--pin-h", `${el.offsetHeight - 18}px`);
    update();
    const ro = new ResizeObserver(update);
    ro.observe(el);
    window.addEventListener("resize", update);
    return () => {
      ro.disconnect();
      window.removeEventListener("resize", update);
    };
  });
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
  // de session) et efface toute sélection groupée en cours. Le panneau de droite
  // (ModDetail) reste rempli en permanence, y compris pendant une sélection
  // groupée : il suit le dernier mod cliqué (§6.3ter).
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

  // Catégories du filtre : voiture = catégorie unique (`category`) ; circuit =
  // multi-valué (`categories`, §5bis.2), on agrège toutes les valeurs vues.
  const categories = $derived(
    isCar
      ? [...new Set(typed.map((c) => c.category).filter((c): c is string => !!c))].sort()
      : [...new Set(typed.flatMap((c) => c.categories))].sort(),
  );
  const authors = $derived(
    [...new Set(typed.map((c) => c.author).filter((c): c is string => !!c))].sort((a, b) =>
      a.toLowerCase().localeCompare(b.toLowerCase()),
    ),
  );
  const countries = $derived(
    [...new Set(typed.map((c) => c.country).filter((c): c is string => !!c))].sort((a, b) =>
      a.toLowerCase().localeCompare(b.toLowerCase()),
    ),
  );
  const tags = $derived(
    [...new Set(typed.flatMap(modTags))].sort((a, b) => a.toLowerCase().localeCompare(b.toLowerCase())),
  );
  /** Valeurs d'un lot de jetons pour un sens donné, en minuscules. */
  function picked(tokens: Token[], mode: "inc" | "exc"): string[] {
    return tokens.filter((tk) => tk.mode === mode).map((tk) => tk.value.toLowerCase());
  }
  const tagsIncluded = $derived(picked(tagTokens, "inc"));
  const tagsExcluded = $derived(picked(tagTokens, "exc"));
  const catsIncluded = $derived(picked(catTokens, "inc"));
  const catsExcluded = $derived(picked(catTokens, "exc"));

  /** Catégories d'un mod, en minuscules — une seule pour une voiture,
   * plusieurs pour un circuit (§5bis.2). */
  function modCats(c: ModCard): string[] {
    return (isCar ? [c.category].filter((x): x is string => !!x) : c.categories).map((x) => x.toLowerCase());
  }

  /** Combien de mods portent ce tag / cette catégorie — le décompte affiché
   * dans l'autocomplétion. Calculé sur le type courant, pas sur les résultats
   * filtrés : un chiffre qui bouge à chaque jeton posé ne sert à rien pour
   * décider du jeton suivant. */
  const tagCounts = $derived.by(() => {
    const m = new Map<string, number>();
    for (const c of typed) for (const tg of modTags(c)) m.set(tg, (m.get(tg) ?? 0) + 1);
    return m;
  });
  const catCounts = $derived.by(() => {
    const m = new Map<string, number>();
    for (const c of typed) for (const cat of isCar ? [c.category].filter(Boolean) : c.categories) {
      m.set(cat as string, (m.get(cat as string) ?? 0) + 1);
    }
    return m;
  });

  const filtered = $derived(
    typed.filter((c) => {
      // Exclure l'emporte toujours sur inclure : un mod qui porte un critère
      // refusé sort, même s'il en porte un autre demandé. C'est ce qu'on attend
      // d'un « sauf » — sinon « avec jdm, sans wip » laisserait passer les mods
      // marqués les deux.
      const cats = modCats(c);
      if (catsExcluded.some((x) => cats.includes(x))) return false;
      // Plusieurs catégories incluses = OU. Une voiture n'en a qu'une, donc un
      // ET n'aurait jamais rien remonté ; un circuit en a plusieurs, mais on
      // garde le même sens des deux côtés.
      if (catsIncluded.length && !catsIncluded.some((x) => cats.includes(x))) return false;
      if (classFilter !== "all" && c.car_class !== classFilter) return false;
      if (stateFilter === "active" && !c.active) return false;
      if (stateFilter === "inactive" && c.active) return false;
      if (authorFilter !== "all" && c.author !== authorFilter) return false;
      if (countryFilter !== "all" && c.country !== countryFilter) return false;
      if (tagTokens.length) {
        const mine = modTags(c).map((tg) => tg.toLowerCase());
        if (tagsExcluded.some((x) => mine.includes(x))) return false;
        const ok =
          tagMode === "and" ? tagsIncluded.every((x) => mine.includes(x)) : tagsIncluded.some((x) => mine.includes(x));
        if (tagsIncluded.length && !ok) return false;
      }
      if (favState === 1 && !c.is_favorite) return false;
      if (favState === -1 && c.is_favorite) return false;
      if (triedState === 1 && !c.tried) return false;
      if (triedState === -1 && c.tried) return false;
      if (stockState === 1 && !c.is_stock) return false;
      if (stockState === -1 && c.is_stock) return false;
      // Champ vide = aucun filtre de ce côté, quelle que soit la plage de
      // saisie : c'est la borne saisie qui décide, pas sa distance aux bornes.
      if (yearMin !== NO_YEAR && (c.year ?? 0) < yearMin) return false;
      if (yearMax !== NO_YEAR && (c.year ?? 9999) > yearMax) return false;
      if (query.trim()) {
        // Un terme par mot séparé par un espace, ET entre eux mais chacun en
        // simple "contains" (pas besoin d'être collés ni dans l'ordre) — bug
        // réel signalé : « GT-M Evo » ne remontait pas « GT-M Adonis Evo »,
        // recherché comme une seule sous-chaîne collée.
        const terms = query.toLowerCase().split(/\s+/).filter(Boolean);
        // Inclut le pack (§4.4) : rechercher son nom remonte toutes ses voitures.
        const hay = `${c.display_name ?? ""} ${c.brand ?? ""} ${c.id_interne} ${c.category ?? ""} ${c.source_pack ?? ""} ${modTags(c).join(" ")}`.toLowerCase();
        if (!terms.every((term) => hay.includes(term))) return false;
      }
      return true;
    }),
  );

  const activeFilterCount = $derived(
    (query.trim() !== "" ? 1 : 0) +
      (catTokens.length ? 1 : 0) +
      (classFilter !== "all" ? 1 : 0) +
      (stateFilter !== "all" ? 1 : 0) +
      (authorFilter !== "all" ? 1 : 0) +
      (countryFilter !== "all" ? 1 : 0) +
      (tagTokens.length ? 1 : 0) +
      (favState !== 0 ? 1 : 0) +
      (triedState !== 0 ? 1 : 0) +
      (stockState !== 0 ? 1 : 0) +
      (yearMin !== NO_YEAR ? 1 : 0) +
      (yearMax !== NO_YEAR ? 1 : 0),
  );

  function clearFilters() {
    query = "";
    catTokens = [];
    classFilter = "all";
    stateFilter = "all";
    authorFilter = "all";
    countryFilter = "all";
    tagTokens = [];
    tagMode = "and";
    favState = 0;
    triedState = 0;
    stockState = 0;
    yearMin = NO_YEAR;
    yearMax = NO_YEAR;
  }

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
  <div class="main" bind:this={mainEl}>
    <div class="pin-top" bind:this={pinTopEl}>
    <div class="toolbar">
      <div class="search">
        <input class="input" placeholder={t("library.searchPlaceholder")} bind:value={query} />
      </div>

      <span class="count-pill mono">{filtered.length}</span>

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
    </div>

      <div class="filters">
        <label>
          <span>{t("library.filterState")}</span>
          <select class="input" bind:value={stateFilter}>
            <option value="all">{t("common.all")}</option>
            <option value="active">{t("common.active")}</option>
            <option value="inactive">{t("common.inactive")}</option>
          </select>
        </label>
        <label>
          <span>{t("library.filterAuthor")}</span>
          <select class="input" bind:value={authorFilter}>
            <option value="all">{t("common.all")}</option>
            {#each authors as a}<option value={a}>{a}</option>{/each}
          </select>
        </label>
        <label>
          <span>{t("library.filterCountry")}</span>
          <select class="input" bind:value={countryFilter}>
            <option value="all">{t("common.all")}</option>
            {#each countries as c}<option value={c}>{c}</option>{/each}
          </select>
        </label>
        <div class="tok-field">
          <div class="tok-head">
            <span>{t("library.filterTag")}</span>
            <!-- ET/OU : seul réglage du champ qui ne se lit pas sur les jetons
                 eux-mêmes, donc posé au-dessus d'eux plutôt que dedans. -->
            <span class="seg">
              <button type="button" class:on={tagMode === "and"} onclick={() => (tagMode = "and")}>
                {t("library.tagModeAnd")}
              </button>
              <button type="button" class:on={tagMode === "or"} onclick={() => (tagMode = "or")}>
                {t("library.tagModeOr")}
              </button>
            </span>
          </div>
          <TokenFilter
            options={tags}
            bind:tokens={tagTokens}
            placeholder={t("library.filterTagPlaceholder")}
            countOf={(v) => tagCounts.get(v) ?? 0}
          />
        </div>
        <div class="tok-field">
          <div class="tok-head"><span>{t("library.filterCategory")}</span></div>
          <TokenFilter
            options={categories}
            bind:tokens={catTokens}
            placeholder={t("library.filterCategoryPlaceholder")}
            countOf={(v) => catCounts.get(v) ?? 0}
          />
        </div>
        {#if isCar}
          <label>
            <span>{t("library.filterClass")}</span>
            <select class="input" bind:value={classFilter}>
              <option value="all">{t("common.allFem")}</option>
              <option value="race">race</option>
              <option value="street">street</option>
            </select>
          </label>
          <!-- Les deux champs se bornent l'un l'autre, mais un champ **vide**
               ne borne rien : sans ce repli, vider « année min » ramenait le
               plafond de « année max » à zéro. Pas de plancher ni de plafond
               à l'année courante : des voitures existent bien avant 1950, et
               un mod peut légitimement porter une année future (voiture
               concept, DLC annoncé) — `YEAR_RANGE_MIN` ne sert donc plus
               qu'à `emptyStart`, un point de départ, jamais une borne.
               `emptyStart` fait atterrir ▲ et ▼ sur le même repère depuis un
               champ vide — 1950 pour l'un, l'année courante pour l'autre —
               exactement comme taper cette valeur ; sans lui ▲ retombait sur
               `min` et ▼ restait désactivé faute de destination. -->
          <label>
            <span>{t("library.yearMin")}</span>
            <NumberStepper
              width={80}
              max={yearMax === NO_YEAR ? undefined : yearMax}
              emptyValue={NO_YEAR}
              emptyStart={YEAR_RANGE_MIN}
              bind:value={yearMin}
            />
          </label>
          <label>
            <span>{t("library.yearMax")}</span>
            <NumberStepper
              width={80}
              min={yearMin === NO_YEAR ? undefined : yearMin}
              emptyValue={NO_YEAR}
              emptyStart={YEAR_RANGE_MAX}
              bind:value={yearMax}
            />
          </label>
        {/if}
        <div class="filter-checks">
          <TriCheck
            label={t("library.favorites")}
            bind:value={favState}
            titleInclude={t("library.favOnly")}
            titleExclude={t("library.favExcluded")}
            titleNeutral={t("library.favNeutral")}
          />
          <TriCheck
            label={t("library.tried")}
            bind:value={triedState}
            titleInclude={t("library.triedOnly")}
            titleExclude={t("library.triedExcluded")}
            titleNeutral={t("library.triedNeutral")}
          />
          <TriCheck
            label={t("library.baseContent")}
            bind:value={stockState}
            titleInclude={t("library.baseOnly")}
            titleExclude={t("library.baseExcluded")}
            titleNeutral={t("library.baseNeutral")}
          />
        </div>
        {#if activeFilterCount > 0}
          <button class="btn-ghost clear" type="button" onclick={clearFilters}>{t("common.reset")}</button>
        {/if}
      </div>
    </div>

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
              {#if c.is_stock}
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
                      <StateBadge active={c.active} stock={c.is_stock} />
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

  {#if selectedIds.size >= 2}
    <!-- Panneau bas en surimpression (§6.3ter) : ne remplace plus le panneau
         de droite (ModDetail, toujours affiché) ni ne réduit la largeur de la
         grille — flotte par-dessus `.main`, indépendant de son défilement
         interne (sibling non-scrollant dans `.main-wrap`, cf. style). -->
    <BulkEditPanel
      ids={[...selectedIds]}
      cards={typed.filter((c) => selectedIds.has(c.id_interne))}
      onclose={() => (selectedIds = new Set())}
      onchange={refresh}
    />
  {/if}
  </div>

  <ModDetail
    id={effectiveId}
    onchange={refresh}
    onexpand={() => (nav.openFull = effectiveId)}
  />
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
  .main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    padding: 18px 22px;
    overflow-y: auto;
    /* Explicite plutôt que de compter sur la règle CSS qui promeut `visible`
       en `auto` quand l'autre axe ne l'est pas (§6.2, tableau large) — pas de
       doute possible sous WebView2. */
    overflow-x: auto;
  }
  .full-wrap {
    flex: 1;
    min-width: 0;
    overflow-y: auto;
    padding: 28px 32px; /* compense le margin négatif de .page */
  }
  /* Recherche + filtres épinglés en haut du scroll de `.main` : reste visible
     pendant qu'on défile la grille/le tableau. Marges négatives = compense le
     padding de `.main` pour venir affleurer les bords (fond opaque, sinon les
     cartes défileraient visiblement dessous) ; le `top` négatif équivalent
     ancre le point de décrochage du sticky sur ce même bord. */
  .pin-top {
    position: sticky;
    top: -18px;
    z-index: 6;
    margin: -18px -22px 0;
    padding: 18px 22px 2px;
    background: var(--bg);
  }
  .toolbar {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 16px;
    flex-wrap: wrap;
  }
  .search {
    flex: 1;
    min-width: 160px;
  }
  .count-pill {
    color: var(--faint);
    font-size: 11px;
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
  .seg button {
    background: var(--panel2);
    color: var(--muted);
    padding: 7px 11px;
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
    padding: 6px 10px;
  }
  .filters {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: 12px;
    padding: 12px;
    margin-bottom: 16px;
    background: var(--panel2);
    border: 1px solid var(--line);
  }
  .filters label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--muted);
  }
  .filters .input {
    width: 120px;
  }
  /* Les deux champs à jetons ne sont pas des `label` : leur ligne de titre
     porte aussi le sélecteur ET/OU, et le champ lui-même est un composant. */
  .tok-field {
    display: flex;
    flex-direction: column;
    gap: 4px;
    flex: 1 1 220px;
    max-width: 340px;
    min-width: 0;
  }
  .tok-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--muted);
    /* Même hauteur que les libellés des `<select>` voisins, pour que tous les
       champs de la rangée s'alignent sur leur bas. */
    min-height: 13px;
  }
  .seg {
    display: inline-flex;
    border: 1px solid var(--line);
  }
  .seg button {
    background: none;
    border: 0;
    color: var(--muted);
    font: inherit;
    font-size: 9.5px;
    letter-spacing: 0.5px;
    padding: 1px 6px;
    cursor: pointer;
  }
  .seg button.on {
    background: var(--raised);
    color: var(--txt);
  }
  .filter-checks {
    display: flex;
    flex-direction: row;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
  }
  .clear {
    font-size: 11px;
    margin-left: auto;
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
  /* En-têtes collés en haut du scroll de `.main`, juste sous le bandeau
     recherche+filtres (décalage --pin-h mesuré en JS). `box-shadow` = ligne
     de séparation fiable (les bordures collapse ne « suivent » pas le sticky
     sous Chromium/WebView2). */
  thead th {
    position: sticky;
    top: var(--pin-h, 0px);
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
