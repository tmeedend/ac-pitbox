<script lang="ts">
  import { onMount } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { listen } from "@tauri-apps/api/event";
  import ModDetail from "./ModDetail.svelte";
  import DetailPage from "./DetailPage.svelte";
  import BulkImport from "./BulkImport.svelte";
  import {
    importArchives,
    importFolders,
    listLibrary,
    previewSrc,
    resolveConflict,
    setFavorite,
    type ArchiveResult,
    type ImportProgress,
    type ModCard,
    type ModKind,
  } from "$lib/library";
  import {
    columnsFor,
    kindKey,
    loadVisible,
    saveVisible,
    type ColumnDef,
  } from "$lib/columns";
  import { nav } from "$lib/nav.svelte";

  // Une bibliothèque par type (§6.1) : ce composant est rendu une fois pour les
  // voitures, une fois pour les circuits. Toute la persistance est suffixée par
  // type pour rester indépendante entre les deux.
  let { kind }: { kind: ModKind } = $props();
  const isCar = kind === "Car";
  const kk = kindKey(kind);

  interface PendingConflict {
    newId: string;
    newName: string;
    oldId: string;
    oldName: string;
  }

  let cards = $state<ModCard[]>([]);
  let selectedId = $state<string | null>(null);
  // Page détail pleine page (§6.3) : double-clic sur une carte, ou bouton
  // « Agrandir » du panneau latéral.
  let fullId = $state<string | null>(null);
  let query = $state("");
  let categoryFilter = $state<string>("all");
  let classFilter = $state<"all" | "race" | "street">("all");
  let favOnly = $state(false);
  let yearMin = $state<number | null>(null);
  let yearMax = $state<number | null>(null);
  let showFilters = $state(false);
  let view = $state<"gallery" | "table">(
    (localStorage.getItem(`pitbox.view.${kk}`) as "gallery" | "table") ?? "gallery",
  );
  let importing = $state(false);
  let report = $state<ArchiveResult[] | null>(null);
  let progress = $state<ImportProgress | null>(null);
  let pendingConflicts = $state<PendingConflict[]>([]);
  // Mode d'import de dossier : copier (préserve la source) ou déplacer (§4.5).
  let copyMode = $state(localStorage.getItem("pitbox.import.copy") !== "false");
  function setCopyMode(v: boolean) {
    copyMode = v;
    localStorage.setItem("pitbox.import.copy", String(v));
  }

  // Colonnes (§6.2) : définitions propres au type + visibilité persistée par type.
  const columns: ColumnDef[] = columnsFor(kind);
  let visibleKeys = $state<string[]>([...loadVisible(kind)]);
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
  let sortKey = $state<string>(localStorage.getItem(`pitbox.sort.${kk}.key`) ?? "name");
  let sortDir = $state<1 | -1>(
    localStorage.getItem(`pitbox.sort.${kk}.dir`) === "-1" ? -1 : 1,
  );

  function toggleSort(key: string) {
    if (sortKey === key) sortDir = sortDir === 1 ? -1 : 1;
    else {
      sortKey = key;
      sortDir = 1;
    }
    localStorage.setItem(`pitbox.sort.${kk}.key`, sortKey);
    localStorage.setItem(`pitbox.sort.${kk}.dir`, String(sortDir));
  }

  function setView(v: "gallery" | "table") {
    view = v;
    localStorage.setItem(`pitbox.view.${kk}`, v);
  }

  async function refresh() {
    cards = await listLibrary();
  }

  // Ouverture demandée depuis une vue transversale (§12bis.3) : on ouvre la
  // fiche pleine page de l'entité ciblée (la bonne bibliothèque est déjà active).
  $effect(() => {
    if (nav.openMod) {
      fullId = nav.openMod;
      nav.openMod = null;
    }
  });

  async function toggleFav(c: ModCard, e: Event) {
    e.stopPropagation();
    c.is_favorite = !c.is_favorite;
    await setFavorite(c.id_interne, c.is_favorite);
  }

  // N'expose que les mods du type de cette bibliothèque (§6.1).
  const typed = $derived(cards.filter((c) => c.kind === kind));

  const categories = $derived(
    [...new Set(typed.map((c) => c.category).filter((c): c is string => !!c))].sort(),
  );

  const filtered = $derived(
    typed.filter((c) => {
      if (categoryFilter !== "all" && c.category !== categoryFilter) return false;
      if (classFilter !== "all" && c.car_class !== classFilter) return false;
      if (favOnly && !c.is_favorite) return false;
      if (yearMin !== null && (c.year ?? 0) < yearMin) return false;
      if (yearMax !== null && (c.year ?? 9999) > yearMax) return false;
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
    (categoryFilter !== "all" ? 1 : 0) +
      (classFilter !== "all" ? 1 : 0) +
      (favOnly ? 1 : 0) +
      (yearMin !== null ? 1 : 0) +
      (yearMax !== null ? 1 : 0),
  );

  function clearFilters() {
    categoryFilter = "all";
    classFilter = "all";
    favOnly = false;
    yearMin = null;
    yearMax = null;
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

  async function runImport(task: Promise<ArchiveResult[]>) {
    importing = true;
    progress = null;
    try {
      report = await task;
      pendingConflicts = report.flatMap((a) =>
        a.mods
          .filter((m) => m.conflict)
          .map((m) => ({
            newId: m.id_interne,
            newName: m.display_name ?? m.id_interne,
            oldId: m.conflict!.existing_id,
            oldName: m.conflict!.existing_name ?? m.conflict!.existing_id,
          })),
      );
      await refresh();
    } finally {
      importing = false;
      progress = null;
    }
  }

  async function resolve(c: PendingConflict, action: "keep_both" | "replace") {
    await resolveConflict(c.newId, c.oldId, action);
    pendingConflicts = pendingConflicts.filter((p) => p !== c);
    await refresh();
  }

  async function pickAndImport() {
    const sel = await open({
      multiple: true,
      filters: [{ name: "Archives", extensions: ["zip", "rar", "7z"] }],
    });
    if (!sel) return;
    await runImport(importArchives(Array.isArray(sel) ? sel : [sel]));
  }

  async function pickFolderAndImport() {
    const sel = await open({ directory: true, multiple: false });
    if (!sel || typeof sel !== "string") return;
    await runImport(importFolders([sel], copyMode));
  }

  // Import en masse (§4.6) : on ouvre le flux analyse → arbitrage → exécution.
  let bulkParent = $state<string | null>(null);
  async function pickBulkImport() {
    const sel = await open({ directory: true, multiple: false });
    if (sel && typeof sel === "string") bulkParent = sel;
  }
  async function onBulkDone(r: ArchiveResult[]) {
    report = r; // conflits déjà arbitrés dans le flux → pas de dialogue
    bulkParent = null;
    await refresh();
  }

  onMount(() => {
    refresh();
    // Import par glisser-déposer de fichiers sur la fenêtre.
    const unlistenDrop = getCurrentWebview().onDragDropEvent((event) => {
      if (event.payload.type === "drop") {
        const archives = event.payload.paths.filter((p) =>
          /\.(zip|rar|7z)$/i.test(p),
        );
        if (archives.length) runImport(importArchives(archives));
      }
    });
    // Progression de l'import.
    const unlistenProgress = listen<ImportProgress>("import:progress", (e) => {
      progress = e.payload;
    });
    return () => {
      unlistenDrop.then((f) => f());
      unlistenProgress.then((f) => f());
    };
  });

  function importSummary(r: ArchiveResult[]): string {
    const n = r.reduce((acc, a) => acc + a.mods.length, 0);
    const errs = r.filter((a) => a.error).length;
    return `${n} mod(s) importé(s)${errs ? `, ${errs} archive(s) en erreur` : ""}`;
  }

  const typeLabel = isCar ? "voiture" : "circuit";
</script>

<div class="library">
  {#if fullId}
    <div class="full-wrap">
      <DetailPage
        id={fullId}
        {kind}
        onclose={() => (fullId = null)}
        onchange={refresh}
      />
    </div>
  {:else}
  <div class="main">
    <div class="toolbar">
      <div class="import-group">
        <button class="btn btn-primary" type="button" onclick={pickAndImport} disabled={importing}>
          {importing ? "Import…" : "Importer une archive"}
        </button>
        <button class="btn" type="button" onclick={pickFolderAndImport} disabled={importing} title="Importer un dossier de mod déjà décompressé (§4.5)">
          Importer un dossier
        </button>
        <button class="btn" type="button" onclick={pickBulkImport} disabled={importing} title="Importer en masse : un dossier parent dont chaque sous-dossier est un mod (§4.6)">
          Import en masse
        </button>
        <div class="copy-toggle" title="Pour l'import de dossier">
          <button class:on={copyMode} onclick={() => setCopyMode(true)}>Copier</button>
          <button class:on={!copyMode} onclick={() => setCopyMode(false)}>Déplacer</button>
        </div>
      </div>

      <div class="search">
        <input class="input" placeholder="Rechercher (nom, marque, tag…)" bind:value={query} />
      </div>

      <span class="count-pill mono">{filtered.length}</span>

      <button class="btn filter-btn" class:active={activeFilterCount > 0} type="button" onclick={() => (showFilters = !showFilters)}>
        Filtres{#if activeFilterCount > 0}<span class="fc">{activeFilterCount}</span>{/if}
      </button>

      {#if view === "table"}
        <div class="columns-wrap">
          <button class="btn" type="button" onclick={() => (showColumns = !showColumns)}>Colonnes</button>
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
                  <span>{col.label}</span>
                </label>
              {/each}
            </div>
          {/if}
        </div>
      {/if}

      <div class="seg view">
        <button class:on={view === "gallery"} onclick={() => setView("gallery")} title="Galerie">▦</button>
        <button class:on={view === "table"} onclick={() => setView("table")} title="Tableau">≣</button>
      </div>
    </div>

    {#if showFilters}
      <div class="filters">
        {#if isCar}
          <label>
            <span>Catégorie</span>
            <select class="input" bind:value={categoryFilter}>
              <option value="all">Toutes</option>
              {#each categories as cat}<option value={cat}>{cat}</option>{/each}
            </select>
          </label>
          <label>
            <span>Classe</span>
            <select class="input" bind:value={classFilter}>
              <option value="all">Toutes</option>
              <option value="race">race</option>
              <option value="street">street</option>
            </select>
          </label>
          <label>
            <span>Année min</span>
            <input class="input" type="number" placeholder="—" bind:value={yearMin} />
          </label>
          <label>
            <span>Année max</span>
            <input class="input" type="number" placeholder="—" bind:value={yearMax} />
          </label>
        {/if}
        <label class="fav-check">
          <input type="checkbox" bind:checked={favOnly} />
          <span>Favoris</span>
        </label>
        {#if activeFilterCount > 0}
          <button class="btn-ghost clear" type="button" onclick={clearFilters}>Réinitialiser</button>
        {/if}
      </div>
    {/if}

    {#if importing && progress}
      <div class="progress">
        <div class="p-label">
          <span class="mono p-phase">{progress.phase}</span>
          {progress.archive} — {progress.label}
          {#if progress.total > 0 && progress.phase === "filing"}
            <span class="mono">({progress.current}/{progress.total})</span>
          {/if}
        </div>
        <div class="p-bar">
          <div
            class="p-fill"
            style:width={progress.total > 0 ? `${(progress.current / progress.total) * 100}%` : "30%"}
            class:indeterminate={progress.total === 0}
          ></div>
        </div>
      </div>
    {/if}

    {#if report}
      <div class="report">
        <div class="report-head">
          <span>{importSummary(report)}</span>
          <button class="btn-ghost" onclick={() => (report = null)}>✕</button>
        </div>
        {#each report as a}
          {#if a.error}
            <div class="r-line err">⚠ {a.archive} — {a.error}</div>
          {/if}
          {#each a.mods as m}
            <div class="r-line">
              <span class="r-out {m.outcome === 'UPDATE_REPLACE' ? 'upd' : 'new'}">
                {m.outcome === "UPDATE_REPLACE" ? "MAJ" : "NOUVEAU"}
              </span>
              {m.display_name ?? m.id_interne}
              {#if m.conflict}
                <span class="r-conflict">
                  ressemble à « {m.conflict.existing_name ?? m.conflict.existing_id} » —
                  les deux ont été conservés
                </span>
              {/if}
            </div>
          {/each}
          {@const replaced = (a.shared ?? []).filter((s) => s.disposition === "replaced")}
          {@const added = (a.shared ?? []).filter((s) => s.disposition === "installed")}
          {#if added.length}
            <div class="r-line shared">
              + {added.length} ressource(s) partagée(s) installée(s) (fonts/drivers, §4.8)
            </div>
          {/if}
          {#each replaced as s}
            <div class="r-line shared warn">
              ⚠ {s.kind === "fonts" ? "Font" : "Driver"} « {s.name} » remplacé par une version différente
            </div>
          {/each}
          {#if (a.subs ?? []).length}
            {@const skins = a.subs.filter((s) => s.sub_type === "SKIN").length}
            {@const sounds = a.subs.filter((s) => s.sub_type === "SOUND").length}
            <div class="r-line shared">
              + {skins ? `${skins} skin(s)` : ""}{skins && sounds ? " · " : ""}{sounds ? `${sounds} son(s)` : ""} rattaché(s) (§12bis)
            </div>
          {/if}
          {#if (a.apps ?? []).length}
            <div class="r-line shared">+ {a.apps.length} app(s) importée(s) (§12bis)</div>
          {/if}
        {/each}
      </div>
    {/if}

    {#if filtered.length === 0}
      <div class="empty">
        {#if typed.length === 0}
          <p>Aucune {typeLabel} dans la bibliothèque.</p>
          <p class="hint">Importe une archive (.zip / .rar / .7z) ou glisse-la sur la fenêtre.</p>
        {:else}
          <p>Aucun résultat pour ce filtre.</p>
        {/if}
      </div>
    {:else if view === "gallery"}
      <div class="grid">
        {#each filtered as c (c.id_interne)}
          {@const src = previewSrc(c.preview)}
          <button class="card" class:sel={selectedId === c.id_interne} onclick={() => (selectedId = c.id_interne)} ondblclick={() => (fullId = c.id_interne)} title="Double-clic : page détail">
            <div class="thumb">
              {#if src}<img src={src} alt={c.display_name ?? c.id_interne} loading="lazy" />
              {:else}<div class="noprev">{isCar ? "Voiture" : "Circuit"}</div>{/if}
              {#if c.active}<span class="dot" title="Actif"></span>{/if}
              {#if c.is_stock}<span class="sbadge" title="Contenu de base Kunos">BASE</span>{/if}
              {#if c.version_count > 1}<span class="vbadge">{c.version_count}</span>{/if}
              <span
                class="card-fav"
                class:on={c.is_favorite}
                role="button"
                tabindex="-1"
                title="Favori"
                onclick={(e) => toggleFav(c, e)}
                onkeydown={(e) => e.key === "Enter" && toggleFav(c, e)}
              >{c.is_favorite ? "♥" : "♡"}</span>
            </div>
            <div class="c-name">{c.display_name ?? c.id_interne}</div>
            <div class="c-sub">{c.brand ?? ""}{c.year ? ` · ${c.year}` : ""}</div>
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
                  {col.label}{#if sortKey === col.key}<span class="arrow">{sortDir === 1 ? "▲" : "▼"}</span>{/if}
                </th>
              {/each}
            </tr>
          </thead>
          <tbody>
            {#each sorted as c (c.id_interne)}
              <tr class:sel={selectedId === c.id_interne} onclick={() => (selectedId = c.id_interne)} ondblclick={() => (fullId = c.id_interne)}>
                {#each visibleColumns as col}
                  <td
                    class:t-name={col.key === "name"}
                    class:mono={col.mono}
                    class:t-tags={col.key === "tags"}
                  >
                    {#if col.key === "active"}
                      {#if c.active}<span class="on-dot"></span>actif{:else}—{/if}
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

  <ModDetail
    id={selectedId}
    onclose={() => (selectedId = null)}
    onchange={refresh}
    onexpand={() => (fullId = selectedId)}
  />
  {/if}
</div>

{#if bulkParent}
  <BulkImport parent={bulkParent} copy={copyMode} onclose={() => (bulkParent = null)} ondone={onBulkDone} />
{/if}

{#if pendingConflicts.length}
  {@const c = pendingConflicts[0]}
  <div class="modal-backdrop">
    <div class="modal">
      <h3>Nouvelle version possible</h3>
      <p>
        « <b>{c.newName}</b> » ressemble à un mod déjà présent
        (dossier différent : <span class="mono">{c.oldId}</span> →
        <span class="mono">{c.newId}</span>). Que faire ?
      </p>
      <div class="modal-actions">
        <button class="btn" type="button" onclick={() => resolve(c, "keep_both")}>
          Garder les deux
        </button>
        <button class="btn btn-primary" type="button" onclick={() => resolve(c, "replace")}>
          Écraser l'ancienne
        </button>
      </div>
      {#if pendingConflicts.length > 1}
        <div class="modal-rest">{pendingConflicts.length - 1} autre(s) à traiter</div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .library {
    display: flex;
    height: 100%;
    margin: -28px -32px; /* étend dans la zone de contenu du shell */
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
  .toolbar {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 16px;
    flex-wrap: wrap;
  }
  .import-group {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .copy-toggle {
    display: flex;
    border: 1px solid var(--line);
  }
  .copy-toggle button {
    background: var(--panel2);
    color: var(--muted);
    font-size: 10.5px;
    padding: 6px 9px;
    border-right: 1px solid var(--line);
  }
  .copy-toggle button:last-child {
    border-right: none;
  }
  .copy-toggle button.on {
    background: var(--raised);
    color: var(--rosso-bright);
  }
  .search {
    flex: 1;
    min-width: 160px;
  }
  .count-pill {
    color: var(--faint);
    font-size: 11px;
  }
  .filter-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .filter-btn.active {
    border-color: var(--rosso-border);
    color: var(--rosso-bright);
  }
  .filter-btn .fc {
    background: var(--rosso);
    color: #fff;
    font-family: var(--mono);
    font-size: 9px;
    padding: 0 4px;
    border-radius: 2px;
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
  .filters input[type="number"] {
    width: 80px;
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
  .clear {
    font-size: 11px;
    margin-left: auto;
  }

  .progress {
    margin-bottom: 16px;
  }
  .p-label {
    font-size: 12px;
    color: var(--txt2);
    margin-bottom: 6px;
  }
  .p-phase {
    color: var(--rosso-bright);
    font-size: 10px;
    text-transform: uppercase;
    margin-right: 6px;
  }
  .p-bar {
    height: 4px;
    background: var(--line);
    overflow: hidden;
  }
  .p-fill {
    height: 100%;
    background: var(--rosso);
    transition: width 0.2s;
  }
  .p-fill.indeterminate {
    animation: slide 1s ease-in-out infinite;
  }
  @keyframes slide {
    0% { margin-left: 0; }
    50% { margin-left: 70%; }
    100% { margin-left: 0; }
  }

  .report {
    border: 1px solid var(--line);
    background: var(--panel2);
    padding: 10px 12px;
    margin-bottom: 16px;
    font-size: 12px;
  }
  .report-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-weight: 600;
    margin-bottom: 6px;
  }
  .r-line {
    padding: 2px 0;
    color: var(--txt2);
  }
  .r-line.err {
    color: var(--rosso-bright);
  }
  .r-line.shared {
    color: var(--muted);
    font-size: 11px;
    padding-top: 4px;
  }
  .r-line.shared.warn {
    color: var(--yellow);
  }
  .r-out {
    font-size: 9px;
    letter-spacing: 0.5px;
    padding: 1px 5px;
    border: 1px solid var(--line);
    margin-right: 6px;
  }
  .r-out.new {
    color: var(--green);
    border-color: var(--green-border);
  }
  .r-out.upd {
    color: var(--yellow);
    border-color: #4a4426;
  }
  .r-conflict {
    color: var(--yellow);
    margin-left: 6px;
    font-size: 11px;
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
  }
  .card:hover {
    border-color: var(--faint);
  }
  .card.sel {
    border-color: var(--rosso);
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
    background: var(--green);
    box-shadow: 0 0 0 2px var(--bg);
  }
  .vbadge {
    position: absolute;
    top: 5px;
    right: 5px;
    background: var(--rosso);
    color: #fff;
    font-size: 10px;
    font-family: var(--mono);
    padding: 1px 5px;
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

  .table-wrap {
    border: 1px solid var(--line);
    overflow-x: auto;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 12px;
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

  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 50;
  }
  .modal {
    width: 440px;
    max-width: 90vw;
    background: var(--panel);
    border: 1px solid var(--rosso);
    padding: 22px 24px;
  }
  .modal h3 {
    font-size: 14px;
    font-weight: 600;
    margin-bottom: 12px;
  }
  .modal p {
    font-size: 12.5px;
    line-height: 1.6;
    color: var(--txt2);
    margin-bottom: 18px;
  }
  .modal-actions {
    display: flex;
    gap: 10px;
    justify-content: flex-end;
  }
  .modal-rest {
    margin-top: 12px;
    font-size: 11px;
    color: var(--muted);
    text-align: right;
  }
</style>
