<script lang="ts">
  // Vue transversale (§12bis.3) : liste tous les sous-éléments d'un type (skins
  // ou sons), regroupés par archive d'origine ou par voiture, avec sélection
  // multiple (Ctrl/Maj) et suppression groupée. Permet de gérer un pack entier
  // sans ouvrir les fiches une à une. Ne pollue pas la bibliothèque principale.
  import { onMount } from "svelte";
  import {
    listSubsByType,
    activateSound,
    restoreSound,
    deleteSubMod,
    type SubModRow,
  } from "$lib/submods";
  import { listLibrary, type ModCard } from "$lib/library";
  import { nav, requestSection } from "$lib/nav.svelte";
  import { confirm } from "@tauri-apps/plugin-dialog";
  import { t } from "$lib/i18n/index.svelte";
  import { fmtSize } from "$lib/format";
  import LayersSection from "./LayersSection.svelte";
  import LoadingState from "./LoadingState.svelte";
  import Tabs from "./Tabs.svelte";
  // Auto-import : la vue des sons est ce même composant, en sous-section.
  import Transversal from "./Transversal.svelte";

  import { errorText } from "$lib/errors";
  import { StorageKey } from "$lib/storage";
  import { getUiPrefs, setUiPref } from "$lib/uiPrefs.svelte";
  // "car" = skins de voitures (SKIN) · "track" = skins de circuits (TRACK_SKIN)
  // · "sound" = mods de son (SOUND). Voir menu ADD-ONS (§6.1ter).
  // `embedded` : rendu à l'intérieur d'un onglet d'un autre écran (les sons
  // dans « Add-ons voiture ») — ni titre d'écran, ni onglets à lui, l'onglet
  // qui le contient le nomme déjà.
  let { variant, embedded = false }: { variant: "car" | "track" | "sound"; embedded?: boolean } = $props();
  const isSound = $derived(variant === "sound");
  const isTrack = $derived(variant === "track");

  // Onglets de l'écran Add-ons (§6.1ter). Les trois rubriques s'empilaient sur
  // une seule page interminable ; elles ne se consultent jamais ensemble.
  // Les sons n'existent que pour les voitures.
  type AddonTab = "skins" | "sounds" | "layers";
  let activeTab = $state<AddonTab>("skins");
  const tabItems = $derived([
    { id: "skins", label: isTrack ? t("detail.trackSkinsLabelPlain") : t("detail.skinsLabel") },
    ...(isTrack ? [] : [{ id: "sounds", label: t("nav.sounds") }]),
    { id: "layers", label: t("transversal.layersTitle") },
  ]);

  let subs = $state<SubModRow[]>([]);
  let cards = $state<ModCard[]>([]);
  let query = $state("");
  let busy = $state(false);
  let loading = $state(true);
  let error = $state("");

  // Regroupement persisté : par archive d'origine (défaut) ou par voiture.
  // Défaut synchrone à l'affichage initial, remplacé par la valeur
  // sauvegardée dès que l'onMount plus bas répond (§6.2, même schéma que
  // les colonnes de bibliothèque).
  type GroupBy = "archive" | "car";
  const GKEY = StorageKey.transversalGroupBy;
  let groupBy = $state<GroupBy>("archive");
  function setGroupBy(g: GroupBy) {
    groupBy = g;
    setUiPref(GKEY, g);
    // Les clés de groupe changent de nature : ce qui était déplié n'a plus de sens.
    opened = new Set();
  }

  // Tri des groupes persisté : alphabétique (défaut) ou par poids décroissant,
  // pour repérer d'un coup d'œil le pack qui mange le plus de place.
  type SortBy = "name" | "size";
  const SKEY = StorageKey.transversalSortBy;
  let sortBy = $state<SortBy>("name");
  function setSortBy(s: SortBy) {
    sortBy = s;
    setUiPref(SKEY, s);
  }

  // Groupes repliés par défaut : la page liste les packs, on ouvre celui qu'on
  // veut détailler (voir `opened` plus bas).
  let opened = $state<Set<string>>(new Set());

  // Sélection multiple (ids de sous-éléments) + ancre pour la sélection par plage.
  let selected = $state<Set<string>>(new Set());
  let lastClicked: string | null = null;

  const parents = $derived(new Map(cards.map((c) => [c.id_interne, c] as const)));

  async function load() {
    // Chaque vue est spécialisée : voiture (SKIN), circuit (TRACK_SKIN) ou son.
    const types = isSound ? ["SOUND"] : isTrack ? ["TRACK_SKIN"] : ["SKIN"];
    try {
      const [lists, lib] = await Promise.all([
        Promise.all(types.map((ty) => listSubsByType(ty))),
        listLibrary(),
      ]);
      // Skins de circuit **fournis avec le mod** (§8) : reconnus sur disque
      // dans `cm_skins/`, jamais importés séparément, donc sans archive
      // d'origine — ils remplissaient à eux seuls la rubrique « Origine
      // inconnue ». Et rien ici ne s'applique à eux : ni sélection, ni
      // suppression (seul le mod entier les emporte), ni activation (elle se
      // fait depuis la barre latérale ou la fiche du circuit). Les lister
      // n'apprenait donc rien et noyait ce qui se gère vraiment.
      subs = lists.flat().filter((s) => !isTrack || s.removable);
      cards = lib;
      // Purge la sélection des ids disparus (après suppression).
      selected = new Set([...selected].filter((id) => subs.some((s) => s.id === id)));
    } finally {
      loading = false;
    }
  }
  onMount(load);
  onMount(async () => {
    const saved = await getUiPrefs([GKEY, SKEY]);
    if (saved[GKEY] === "car" || saved[GKEY] === "archive") groupBy = saved[GKEY];
    if (saved[SKEY] === "size" || saved[SKEY] === "name") sortBy = saved[SKEY];
  });

  function parentName(id: string): string {
    return parents.get(id)?.display_name ?? id;
  }

  const UNKNOWN = "unknown"; // clé interne, jamais affichée telle quelle
  function groupKeyOf(s: SubModRow): string {
    return groupBy === "archive" ? (s.source_archive ?? UNKNOWN) : s.parent_id;
  }
  function groupLabelOf(key: string): string {
    if (key === UNKNOWN) return t("transversal.unknownArchive");
    return groupBy === "car" ? parentName(key) : key;
  }

  const filtered = $derived(
    subs.filter((s) => {
      if (!query.trim()) return true;
      // Un terme par mot séparé par un espace, ET entre eux mais chacun en
      // simple "contains" (pas besoin d'être collés ni dans l'ordre) — même
      // correction que la bibliothèque (Library.svelte).
      const terms = query.toLowerCase().split(/\s+/).filter(Boolean);
      const hay = `${s.name} ${parentName(s.parent_id)} ${s.source_archive ?? ""}`.toLowerCase();
      return terms.every((term) => hay.includes(term));
    }),
  );

  // Groupes triés (alphabétique ou poids décroissant), items triés par nom,
  // + poids cumulé du groupe (les tailles inconnues comptent pour 0).
  const groups = $derived.by(() => {
    const map = new Map<string, SubModRow[]>();
    for (const s of filtered) {
      const k = groupKeyOf(s);
      let arr = map.get(k);
      if (!arr) {
        arr = [];
        map.set(k, arr);
      }
      arr.push(s);
    }
    const entries = [...map.entries()].map(([key, items]) => ({
      key,
      label: groupLabelOf(key),
      size: items.reduce((n, s) => n + (s.size_bytes ?? 0), 0),
      items: [...items].sort((a, b) => a.name.localeCompare(b.name)),
    }));
    entries.sort((a, b) => (sortBy === "size" ? b.size - a.size : a.label.localeCompare(b.label)));
    return entries;
  });

  // Une recherche en cours force l'ouverture : sinon on ne verrait pas ce qu'on
  // cherche, tout étant replié.
  const searching = $derived(query.trim().length > 0);
  const isOpen = (key: string) => searching || opened.has(key);
  function toggleGroup(key: string) {
    const next = new Set(opened);
    if (next.has(key)) next.delete(key);
    else next.add(key);
    opened = next;
  }
  const allOpen = $derived(groups.length > 0 && groups.every((g) => opened.has(g.key)));
  function toggleAll() {
    opened = allOpen ? new Set() : new Set(groups.map((g) => g.key));
  }

  // Ordre plat pour la sélection par plage : limité aux lignes réellement
  // visibles, une plage ne doit pas embarquer le contenu d'un groupe replié.
  const flatOrder = $derived(
    groups.filter((g) => isOpen(g.key)).flatMap((g) => g.items.map((s) => s.id)),
  );

  // Clic sur une ligne : clic simple = sélectionne seule ; Ctrl(ou Alt)+clic =
  // bascule ; Maj+clic = plage depuis la dernière ligne cliquée (ordre affiché).
  function onItemClick(id: string, e: MouseEvent) {
    // Fourni avec le mod (§8) : pas de sélection, rien à supprimer.
    if (!subs.find((s) => s.id === id)?.removable) return;
    if (e.shiftKey && lastClicked) {
      const a = flatOrder.indexOf(lastClicked);
      const b = flatOrder.indexOf(id);
      if (a !== -1 && b !== -1) {
        const [lo, hi] = a <= b ? [a, b] : [b, a];
        const next = new Set(selected);
        for (let i = lo; i <= hi; i++) next.add(flatOrder[i]);
        selected = next;
      }
    } else if (e.ctrlKey || e.altKey) {
      const next = new Set(selected);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      selected = next;
    } else {
      selected = new Set([id]);
    }
    lastClicked = id;
  }

  function clearSelection() {
    selected = new Set();
    lastClicked = null;
  }

  async function removeIds(ids: string[], message: string) {
    if (!ids.length || busy) return;
    const ok = await confirm(message, { title: t("common.delete"), kind: "warning" });
    if (!ok) return;
    busy = true;
    error = "";
    try {
      for (const id of ids) await deleteSubMod(id);
      await load();
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = false;
    }
  }

  const deleteSelected = () =>
    removeIds([...selected], t("transversal.confirmDeleteMany", { count: selected.size }));

  const deleteGroup = (g: { label: string; items: SubModRow[] }) => {
    const removable = g.items.filter((s) => s.removable);
    return removeIds(
      removable.map((s) => s.id),
      t("transversal.confirmDeleteGroup", { count: removable.length, name: g.label }),
    );
  };

  function removeOne(s: SubModRow) {
    const msg = t(isSound ? "transversal.confirmDeleteSound" : "transversal.confirmDeleteSkin", { name: s.name });
    return removeIds([s.id], msg);
  }

  async function openParent(id: string) {
    const c = parents.get(id);
    if (await requestSection(c?.kind === "Track" ? "tracks" : "cars")) {
      nav.openMod = id;
    }
  }

  async function toggleSound(s: SubModRow) {
    busy = true;
    error = "";
    try {
      if (s.is_active) await restoreSound(s.parent_id);
      else await activateSound(s.id);
      await load();
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = false;
    }
  }
</script>

<div class="trans">
  {#if !embedded}
    <header class="head">
      <h2 class="lbl-screen">{isTrack ? t("nav.trackAddons") : t("nav.carAddons")}</h2>
      <p class="sub">
        {isTrack ? t("transversal.trackSubtitle") : t("transversal.skinSubtitle")}
      </p>
    </header>
    <Tabs tabs={tabItems} active={activeTab} onselect={(v) => (activeTab = v as AddonTab)} />
  {/if}

  <!-- Un onglet à la fois. Une instance `embedded` (les sons) n'a pas de
       bandeau d'onglets, donc `activeTab` y reste sur "skins" : c'est bien sa
       propre liste qu'elle rend, et jamais les deux branches suivantes.
       Le corps de cette première branche garde volontairement son indentation
       d'origine — le ré-indenter d'un cran noierait le changement réel dans
       cent lignes de diff blanc, et avec lui `git blame`. -->
  {#if activeTab === "skins"}
  {#if error}<div class="err">{error}</div>{/if}

  {#if loading}
    <LoadingState />
  {:else if subs.length === 0}
    <div class="empty">
      <p>{isSound ? t("transversal.emptySound") : t("transversal.emptySkin")}</p>
      <p class="hint">{t("transversal.emptyHint")}</p>
    </div>
  {:else}
    <div class="toolbar">
      <!-- La recherche est descendue de l'en-tête dans la barre d'outils : là,
           elle accompagne la liste qu'elle filtre, et le fait aussi pour les
           sons — imbriqués, ils n'avaient aucun champ de recherche. -->
      <input class="input search" placeholder={t("transversal.searchPlaceholder")} bind:value={query} />
      <span class="seg-lbl lbl-key mono">{t("transversal.groupLabel")}</span>
      <div class="seg">
        <button class:on={groupBy === "archive"} type="button" onclick={() => setGroupBy("archive")}>{t("transversal.groupByArchive")}</button>
        <button class:on={groupBy === "car"} type="button" onclick={() => setGroupBy("car")}>{t("transversal.groupByCar")}</button>
      </div>
      <span class="seg-lbl lbl-key mono">{t("transversal.sortLabel")}</span>
      <div class="seg">
        <button class:on={sortBy === "name"} type="button" onclick={() => setSortBy("name")}>{t("transversal.sortByName")}</button>
        <button class:on={sortBy === "size"} type="button" onclick={() => setSortBy("size")}>{t("transversal.sortBySize")}</button>
      </div>
      <button class="btn" type="button" onclick={toggleAll} disabled={searching}>
        {allOpen ? t("transversal.collapseAll") : t("transversal.expandAll")}
      </button>
      <span class="count mono">{filtered.length} / {subs.length}</span>
      <span class="hint-inline">{t("transversal.selectHint")}</span>
      <div class="spacer"></div>
      {#if selected.size > 0}
        <span class="sel-count mono">{t("transversal.selectedCount", { count: selected.size })}</span>
        <button class="btn" type="button" onclick={clearSelection} disabled={busy}>{t("transversal.clearSelection")}</button>
        <button class="btn del-strong" type="button" onclick={deleteSelected} disabled={busy}>{t("transversal.deleteSelected")}</button>
      {/if}
    </div>

    <div class="groups">
      {#each groups as g (g.key)}
        {@const open = isOpen(g.key)}
        <section class="group">
          <div class="group-head" class:open>
            <button
              class="g-toggle"
              type="button"
              aria-expanded={open}
              onclick={() => toggleGroup(g.key)}
              disabled={searching}
            >
              <span class="g-chevron" aria-hidden="true">{open ? "▾" : "▸"}</span>
              <span class="g-label" title={g.label}>{g.label}</span>
              <span class="g-count mono">{t("transversal.itemCount", { count: g.items.length })}</span>
              <span class="g-size mono">{fmtSize(g.size)}</span>
            </button>
            {#if g.items.some((s) => s.removable)}
              <button class="btn del" type="button" onclick={() => deleteGroup(g)} disabled={busy}>{t("transversal.deleteGroup")}</button>
            {/if}
          </div>
          {#if open}
          <ul class="list">
            {#each g.items as s (s.id)}
              <!-- svelte-ignore a11y_no_noninteractive_element_to_interactive_role -->
              <li
                class:active={s.is_active}
                class:selected={selected.has(s.id)}
                class:bundled={!s.removable}
                role="button"
                tabindex="0"
                onclick={(e) => onItemClick(s.id, e)}
                onkeydown={(e) => (e.key === " " || e.key === "Enter") && onItemClick(s.id, e as unknown as MouseEvent)}
              >
                <div class="l-main">
                  <span class="s-name">{s.name}</span>
                  <button class="parent" type="button" onclick={(e) => { e.stopPropagation(); openParent(s.parent_id); }} title={t("detail.openSheetTooltip")}>
                    → {parentName(s.parent_id)}
                  </button>
                  {#if groupBy === "car" && s.source_archive}<span class="src mono">{s.source_archive}</span>{/if}
                  {#if !s.removable}<span class="badge bundled-badge" title={t("transversal.bundledHint")}>{t("transversal.bundledBadge")}</span>{/if}
                </div>
                <span class="s-size mono">{fmtSize(s.size_bytes)}</span>
                {#if isSound}
                  {#if s.is_active}<span class="badge on">{t("common.active").toLowerCase()}</span>{/if}
                  <button class="btn" type="button" onclick={(e) => { e.stopPropagation(); toggleSound(s); }} disabled={busy}>
                    {s.is_active ? t("transversal.restoreOriginal") : t("common.activate")}
                  </button>
                {/if}
                {#if s.removable}
                  <button class="btn del" type="button" title={t("common.delete")} onclick={(e) => { e.stopPropagation(); removeOne(s); }} disabled={busy}>✕</button>
                {/if}
              </li>
            {/each}
          </ul>
          {/if}
        </section>
      {/each}
    </div>
  {/if}
  {:else if activeTab === "sounds"}
    <!-- Les mods de son n'avaient pas de quoi remplir un écran à eux : ils sont
         un onglet d'« Add-ons voiture », rendu par ce même composant. L'onglet
         n'existe pas pour un circuit (voir `tabItems`), donc pas de garde ici. -->
    <Transversal variant="sound" embedded />
  {:else}
    <LayersSection kind={isTrack ? "Track" : "Car"} heading={false} />
  {/if}
</div>

<style>
  .trans {
    max-width: 900px;
  }
  .head {
    margin-bottom: 18px;
  }
  .sub {
    color: var(--muted);
    font-size: 12px;
    margin-top: 6px;
    line-height: 1.5;
    max-width: 540px;
  }
  .search {
    width: 200px;
    flex: none;
  }
  .err {
    background: var(--rosso-dim);
    border: 1px solid var(--rosso-border);
    color: var(--rosso-bright);
    padding: 10px 12px;
    font-size: 12px;
    margin-bottom: 14px;
  }
  .toolbar {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-bottom: 14px;
    flex-wrap: wrap;
  }
  .seg {
    display: flex;
    border: 1px solid var(--line);
  }
  .seg button {
    background: var(--panel2);
    color: var(--muted);
    padding: 6px 14px;
    font-size: 11px;
    border-right: 1px solid var(--line);
  }
  .seg button:last-child {
    border-right: none;
  }
  .seg button.on {
    background: var(--rosso);
    color: #fff;
  }
  /* Couleur/taille/interlettrage viennent de `.lbl-key` (global, harmonisation
     §chantier libellés) : ne reste ici que les majuscules, que la classe
     globale ne couvre pas. */
  .seg-lbl {
    text-transform: uppercase;
  }
  .count {
    color: var(--faint);
    font-size: 11px;
  }
  .hint-inline {
    color: var(--faint);
    font-size: 10.5px;
  }
  .spacer {
    flex: 1;
  }
  .sel-count {
    color: var(--blue);
    font-size: 11px;
  }
  .groups {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .group-head {
    display: flex;
    align-items: stretch;
    gap: 8px;
    border: 1px solid var(--line);
    background: var(--panel2);
  }
  /* Groupe déplié : l'en-tête devient le chapeau de sa liste. */
  .group-head.open {
    border-color: var(--faint);
    margin-bottom: 6px;
  }
  .g-toggle {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 10px;
    background: transparent;
    padding: 8px 12px;
    text-align: left;
    cursor: pointer;
  }
  .g-toggle:hover .g-label {
    color: var(--rosso-bright);
  }
  .g-toggle:disabled {
    cursor: default;
  }
  .g-chevron {
    color: var(--muted2);
    font-size: 10px;
    flex: none;
    width: 10px;
  }
  .g-label {
    font-size: 12px;
    font-weight: 600;
    color: var(--txt2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 1;
    min-width: 0;
  }
  .g-count {
    color: var(--faint);
    font-size: 10.5px;
    flex: none;
  }
  /* Poids cumulé du pack : c'est la colonne qu'on balaye du regard pour
     trouver ce qui prend le plus de place, d'où la largeur fixe. */
  .g-size {
    color: var(--muted);
    font-size: 11px;
    flex: none;
    width: 72px;
    text-align: right;
  }
  .group-head .del {
    flex: none;
    align-self: center;
    margin-right: 8px;
  }
  .s-size {
    color: var(--muted2);
    font-size: 10.5px;
    flex: none;
    width: 66px;
    text-align: right;
  }
  .list {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  .list li {
    display: flex;
    align-items: center;
    gap: 12px;
    border: 1px solid var(--line);
    background: var(--panel2);
    padding: 8px 12px;
    cursor: pointer;
    user-select: none;
  }
  .list li:hover {
    border-color: var(--faint);
  }
  .list li.active {
    border-color: var(--rosso-border);
    background: var(--rosso-dim);
  }
  .list li.selected {
    border-color: var(--blue);
    box-shadow: 0 0 0 1px var(--blue);
  }
  .l-main {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
  }
  .s-name {
    font-weight: 600;
    color: var(--txt);
    font-size: 12.5px;
  }
  .parent {
    background: transparent;
    color: var(--blue);
    font-size: 11.5px;
    padding: 0;
  }
  .parent:hover {
    color: var(--rosso-bright);
  }
  .src {
    color: var(--muted2);
    font-size: 10px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 220px;
  }
  .badge.on {
    font-size: 9px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--rosso-bright);
    border: 1px solid var(--rosso-border);
    padding: 1px 6px;
  }
  .badge.bundled-badge {
    font-size: 9px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--muted2);
    border: 1px solid var(--line);
    padding: 1px 6px;
  }
  .list li.bundled {
    cursor: default;
  }
  .btn {
    background: var(--raised);
    color: var(--txt2);
    border: 1px solid var(--line);
    font-size: 11px;
    padding: 6px 12px;
    flex: none;
  }
  .btn:hover {
    border-color: var(--rosso-border);
    color: var(--rosso-bright);
  }
  .btn:disabled {
    opacity: 0.5;
  }
  .btn.del {
    padding: 6px 9px;
    color: var(--muted);
  }
  .btn.del:hover {
    border-color: var(--rosso-border);
    color: var(--rosso-bright);
  }
  .btn.del-strong {
    color: var(--rosso-bright);
    border-color: var(--rosso-border);
  }
  .empty {
    color: var(--muted);
    text-align: center;
    padding: 50px 0;
  }
  .empty .hint {
    font-size: 12px;
    color: var(--faint);
    margin-top: 8px;
  }
</style>
