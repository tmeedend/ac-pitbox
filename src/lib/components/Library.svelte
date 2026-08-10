<script lang="ts">
  import { tick, untrack } from "svelte";
  import ModDetail from "./ModDetail.svelte";
  import DetailPage from "./DetailPage.svelte";
  import BulkEditPanel from "./BulkEditPanel.svelte";
  import ContextMenu from "./ContextMenu.svelte";
  import LoadingState from "./LoadingState.svelte";
  import NumberStepper from "./NumberStepper.svelte";
  import {
    listLibrary,
    previewSrc,
    setFavorite,
    type ModCard,
    type ModKind,
  } from "$lib/library";
  import {
    columnsFor,
    loadVisible,
    saveVisible,
    type ColumnDef,
  } from "$lib/columns";
  import { nav, pickSession, requestSection, queueOpponentsAction } from "$lib/nav.svelte";
  import { importState } from "$lib/importState.svelte";
  import { getPreferredSkin, getPreferredLayout } from "$lib/preferred";
  import { buildModContextItems } from "$lib/modContextActions";
  import { t } from "$lib/i18n/index.svelte";

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
  // Bornes de la fourchette d'année (mêmes constantes que le vivier
  // d'adversaires de Launch.svelte, §8.6) : les flèches NumberStepper ont
  // besoin d'une valeur numérique, donc plus de `null` pour « pas de borne » —
  // rester aux bornes par défaut équivaut à l'ancien comportement non filtré.
  const YEAR_RANGE_MIN = 1950;
  const YEAR_RANGE_MAX = new Date().getFullYear();
  // Page détail pleine page (§6.3) : double-clic sur une carte, ou bouton
  // « Agrandir » du panneau latéral. État centralisé dans nav.openFull (voir
  // nav.svelte.ts) — la navigation manette globale (AppShell) doit savoir si
  // elle est ouverte pour céder gauche/droite au visualiseur et gérer B=fermer.

  // Filtres persistés par type (rechargés au retour sur la page).
  const FKEY = KEYS.filters;
  const sf: Record<string, unknown> = (() => {
    try {
      return JSON.parse(localStorage.getItem(FKEY) ?? "{}");
    } catch {
      return {};
    }
  })();
  let query = $state<string>((sf.query as string) ?? "");
  let categoryFilter = $state<string>((sf.category as string) ?? "all");
  let classFilter = $state<"all" | "race" | "street">((sf.class as "all" | "race" | "street") ?? "all");
  let stateFilter = $state<"all" | "active" | "inactive">((sf.state as "all" | "active" | "inactive") ?? "all");
  let authorFilter = $state<string>((sf.author as string) ?? "all");
  let countryFilter = $state<string>((sf.country as string) ?? "all");
  let favOnly = $state<boolean>((sf.fav as boolean) ?? false);
  let neverTried = $state<boolean>((sf.neverTried as boolean) ?? false);
  let hideBaseContent = $state<boolean>((sf.hideBaseContent as boolean) ?? false);
  let yearMin = $state<number>((sf.yearMin as number | null) ?? YEAR_RANGE_MIN);
  let yearMax = $state<number>((sf.yearMax as number | null) ?? YEAR_RANGE_MAX);

  // Persistance des filtres (champ libre + rubrique Filtres).
  $effect(() => {
    localStorage.setItem(
      FKEY,
      JSON.stringify({
        query,
        category: categoryFilter,
        class: classFilter,
        state: stateFilter,
        author: authorFilter,
        country: countryFilter,
        fav: favOnly,
        neverTried,
        hideBaseContent,
        yearMin,
        yearMax,
      }),
    );
  });
  let view = $state<"gallery" | "table">(
    (localStorage.getItem(KEYS.view) as "gallery" | "table") ?? "gallery",
  );

  // Colonnes (§6.2) : définitions propres au type + visibilité persistée par type.
  const columns: ColumnDef[] = untrack(() => columnsFor(kind));
  let visibleKeys = $state<string[]>(untrack(() => [...loadVisible(kind)]));
  let showColumns = $state(false);
  const visibleColumns = $derived(
    columns.filter((c) => c.fixed || visibleKeys.includes(c.key)),
  );
  function toggleColumn(key: string) {
    visibleKeys = visibleKeys.includes(key)
      ? visibleKeys.filter((k) => k !== key)
      : [...visibleKeys, key];
    saveVisible(kind, new Set(visibleKeys));
  }

  // Tri du tableau (par clé de colonne), persisté par type.
  let sortKey = $state<string>(localStorage.getItem(KEYS.sortKey) ?? "name");
  let sortDir = $state<1 | -1>(
    localStorage.getItem(KEYS.sortDir) === "-1" ? -1 : 1,
  );

  function toggleSort(key: string) {
    if (sortKey === key) sortDir = sortDir === 1 ? -1 : 1;
    else {
      sortKey = key;
      sortDir = 1;
    }
    localStorage.setItem(KEYS.sortKey, sortKey);
    localStorage.setItem(KEYS.sortDir, String(sortDir));
  }

  function setView(v: "gallery" | "table") {
    view = v;
    localStorage.setItem(KEYS.view, v);
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
    ctxMenu = { x: e.clientX, y: e.clientY, card: c };
  }
  const contextItems = $derived(ctxMenu ? buildModContextItems(ctxMenu.card, refresh) : []);

  // La bibliothèque EST le sélecteur (§8.6) : ouvrir une carte la définit comme
  // choix de session, affiché dans le bloc SESSION de la barre latérale.
  const sessionId = $derived(isCar ? nav.sessionCar?.id ?? null : nav.sessionTrack?.id ?? null);
  // Sélection effective du panneau : le clic explicite prime, sinon le choix
  // de session courant (le panneau reste toujours rempli, jamais vide).
  const effectiveId = $derived(selectedId ?? sessionId);
  // Défaut si aucune session n'a jamais été choisie (premier lancement) :
  // établit une vraie sélection de session plutôt que de laisser le panneau
  // vide indéfiniment. Ne se déclenche qu'une fois (sessionId devient non nul).
  $effect(() => {
    if (!sessionId && !selectedId && sorted.length) select(sorted[0]);
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

  // Envoi de la sélection groupée comme adversaires de la session course
  // (§6.3ter, voitures uniquement) — pose l'action puis navigue vers l'écran
  // de réglages, où Launch.svelte la consomme une fois prêt.
  async function sendAsOpponents(mode: "set" | "add") {
    queueOpponentsAction(mode, [...selectedIds]);
    selectedIds = new Set();
    // Garde de navigation (§10bis) : si le changement d'écran est refusé
    // (modifications non enregistrées ailleurs), ne pas laisser l'action
    // traîner pour se déclencher par surprise à une prochaine visite.
    if (!(await requestSection("race"))) nav.opponentsAction = null;
  }

  // Ouverture demandée depuis une vue transversale (§12bis.3) : on ouvre la
  // fiche pleine page de l'entité ciblée (la bonne bibliothèque est déjà active).
  $effect(() => {
    if (nav.openMod) {
      nav.openFull = nav.openMod;
      nav.openMod = null;
    }
  });

  // Recherche imposée depuis l'extérieur (ex. « filtrer par pack », §4.7).
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

  const filtered = $derived(
    typed.filter((c) => {
      if (categoryFilter !== "all") {
        const match = isCar ? c.category === categoryFilter : c.categories.includes(categoryFilter);
        if (!match) return false;
      }
      if (classFilter !== "all" && c.car_class !== classFilter) return false;
      if (stateFilter === "active" && !c.active) return false;
      if (stateFilter === "inactive" && c.active) return false;
      if (authorFilter !== "all" && c.author !== authorFilter) return false;
      if (countryFilter !== "all" && c.country !== countryFilter) return false;
      if (favOnly && !c.is_favorite) return false;
      if (neverTried && c.tried) return false;
      if (hideBaseContent && c.is_stock) return false;
      if (yearMin > YEAR_RANGE_MIN && (c.year ?? 0) < yearMin) return false;
      if (yearMax < YEAR_RANGE_MAX && (c.year ?? 9999) > yearMax) return false;
      if (query.trim()) {
        const q = query.toLowerCase();
        const tags = [...c.tags_from_mod, ...c.tags_from_rule, ...c.tags_manual].join(" ");
        // Inclut le pack (§4.7) : rechercher son nom remonte toutes ses voitures.
        const hay = `${c.display_name ?? ""} ${c.brand ?? ""} ${c.id_interne} ${c.category ?? ""} ${c.source_pack ?? ""} ${tags}`.toLowerCase();
        if (!hay.includes(q)) return false;
      }
      return true;
    }),
  );

  const activeFilterCount = $derived(
    (query.trim() !== "" ? 1 : 0) +
      (categoryFilter !== "all" ? 1 : 0) +
      (classFilter !== "all" ? 1 : 0) +
      (stateFilter !== "all" ? 1 : 0) +
      (authorFilter !== "all" ? 1 : 0) +
      (countryFilter !== "all" ? 1 : 0) +
      (favOnly ? 1 : 0) +
      (neverTried ? 1 : 0) +
      (hideBaseContent ? 1 : 0) +
      (yearMin !== YEAR_RANGE_MIN ? 1 : 0) +
      (yearMax !== YEAR_RANGE_MAX ? 1 : 0),
  );

  function clearFilters() {
    query = "";
    categoryFilter = "all";
    classFilter = "all";
    stateFilter = "all";
    authorFilter = "all";
    countryFilter = "all";
    favOnly = false;
    neverTried = false;
    hideBaseContent = false;
    yearMin = YEAR_RANGE_MIN;
    yearMax = YEAR_RANGE_MAX;
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

  // Recharge au montage, puis à chaque import déclenché depuis l'écran dédié
  // ou le glisser-déposer global (§4.6bis) — `version` sert de simple signal.
  $effect(() => {
    importState.version;
    refresh();
  });

  // --- Navigation clavier/manette dans la fiche pleine page ---
  // Flèche gauche/droite = mod précédent/suivant, dans l'ordre affiché
  // (tri courant). Ne touche pas à la sélection de session (comme le
  // double-clic qui ouvre la fiche) — juste la navigation dans la vue.
  function navigateFull(delta: 1 | -1) {
    // Une visionneuse plein écran ouverte par-dessus la fiche (§6.1) gère son
    // propre gauche/droite — sinon une même pression ferait défiler les
    // images ET changer de mod.
    if (!nav.openFull || nav.lightboxOpen) return;
    const ids = sorted.map((c) => c.id_interne);
    const idx = ids.indexOf(nav.openFull);
    if (idx === -1 || ids.length < 2) return;
    nav.openFull = ids[(idx + delta + ids.length) % ids.length];
  }

  function isTypingTarget(e: KeyboardEvent): boolean {
    const t = e.target as HTMLElement | null;
    return !!t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.tagName === "SELECT" || t.isContentEditable);
  }

  $effect(() => {
    function onKeydown(e: KeyboardEvent) {
      if (!nav.openFull || isTypingTarget(e)) return;
      if (e.key === "ArrowLeft") {
        e.preventDefault();
        navigateFull(-1);
      } else if (e.key === "ArrowRight") {
        e.preventDefault();
        navigateFull(1);
      }
    }
    window.addEventListener("keydown", onKeydown);
    return () => window.removeEventListener("keydown", onKeydown);
  });

  // Manette (API Gamepad standard, sans dépendance) : croix directionnelle ou
  // stick gauche gauche/droite navigue comme les flèches, tant que la fiche
  // pleine page est ouverte. Détection sur front montant (évite la répétition
  // continue tant que le bouton reste enfoncé).
  $effect(() => {
    if (!nav.openFull) return;
    let raf = 0;
    let last = { left: false, right: false };
    function poll() {
      for (const gp of navigator.getGamepads?.() ?? []) {
        if (!gp) continue;
        const axis = gp.axes[0] ?? 0;
        const left = (gp.buttons[14]?.pressed ?? false) || axis < -0.6;
        const right = (gp.buttons[15]?.pressed ?? false) || axis > 0.6;
        if (left && !last.left) navigateFull(-1);
        if (right && !last.right) navigateFull(1);
        last = { left, right };
      }
      raf = requestAnimationFrame(poll);
    }
    raf = requestAnimationFrame(poll);
    return () => cancelAnimationFrame(raf);
  });
</script>

<div class="library">
  {#if nav.openFull}
    <div class="full-wrap">
      <DetailPage
        id={nav.openFull}
        {kind}
        onclose={() => { nav.openFull = null; scrollToEffective(); }}
        onchange={refresh}
      />
    </div>
  {:else}
  <div class="main-wrap">
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
        <label>
          <span>{t("library.filterCategory")}</span>
          <select class="input" bind:value={categoryFilter}>
            <option value="all">{t("common.allFem")}</option>
            {#each categories as cat}<option value={cat}>{cat}</option>{/each}
          </select>
        </label>
        {#if isCar}
          <label>
            <span>{t("library.filterClass")}</span>
            <select class="input" bind:value={classFilter}>
              <option value="all">{t("common.allFem")}</option>
              <option value="race">race</option>
              <option value="street">street</option>
            </select>
          </label>
          <label>
            <span>{t("library.yearMin")}</span>
            <NumberStepper width={80} min={YEAR_RANGE_MIN} max={yearMax} bind:value={yearMin} />
          </label>
          <label>
            <span>{t("library.yearMax")}</span>
            <NumberStepper width={80} min={yearMin} max={YEAR_RANGE_MAX} bind:value={yearMax} />
          </label>
        {/if}
        <div class="filter-checks">
          <label class="fav-check">
            <input type="checkbox" bind:checked={favOnly} />
            <span>{t("library.favorites")}</span>
          </label>
          <label class="fav-check" title={t("library.neverTriedTooltip")}>
            <input type="checkbox" bind:checked={neverTried} />
            <span>{t("library.neverTried")}</span>
          </label>
          <label class="fav-check">
            <input type="checkbox" bind:checked={hideBaseContent} />
            <span>{t("library.hideBaseContent")}</span>
          </label>
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
              {#each visibleColumns as col}
                <th class:sortable={col.sortable} onclick={() => col.sortable && toggleSort(col.key)}>
                  {t(col.labelKey)}{#if sortKey === col.key}<span class="arrow">{sortDir === 1 ? "▲" : "▼"}</span>{/if}
                </th>
              {/each}
            </tr>
          </thead>
          <tbody>
            {#each sorted as c (c.id_interne)}
              <tr data-id={c.id_interne} class:sel={effectiveId === c.id_interne && selectedIds.size === 0} class:multisel={selectedIds.has(c.id_interne)} class:session={sessionId === c.id_interne} onclick={(e) => onCardClick(c, e)} ondblclick={() => (nav.openFull = c.id_interne)} oncontextmenu={(e) => openCardContextMenu(e, c)}>
                {#each visibleColumns as col}
                  <td
                    class:t-name={col.key === "name"}
                    class:mono={col.mono}
                    class:t-tags={col.key === "tags"}
                  >
                    {#if col.key === "active"}
                      {#if c.active}<span class="on-dot"></span>{t("common.active").toLowerCase()}{:else}—{/if}
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
      {isCar}
      onclose={() => (selectedIds = new Set())}
      onchange={refresh}
      onSetOpponents={() => sendAsOpponents("set")}
      onAddOpponents={() => sendAsOpponents("add")}
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
  .filter-checks {
    display: flex;
    flex-direction: row;
    gap: 16px;
    align-items: center;
  }
  .fav-check {
    flex-direction: row !important;
    align-items: center;
    gap: 6px !important;
    text-transform: none;
    font-size: 12px;
    color: var(--txt2);
    cursor: pointer;
  }
  .fav-check input[type="checkbox"] {
    width: 16px;
    height: 16px;
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
    width: 100%;
    border-collapse: collapse;
    font-size: 12px;
    /* Pas de surlignage de texte lors des clics de sélection (lignes + en-têtes triables). */
    user-select: none;
  }
  th {
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
  .on-dot {
    display: inline-block;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--green);
    margin-right: 5px;
  }
</style>
