<script lang="ts">
  // Vue Apps (§12bis.4) : type autonome, simplement activable/désactivable par
  // junction. Pas de fiche ni de tags en v1 — nom, état, activation, ressources
  // annexes (§4.5.2, même mécanisme que les mods voiture/circuit).
  import { onMount } from "svelte";
  import {
    listApps,
    activateApp,
    deactivateApp,
    deleteApp,
    listAppResources,
    openAppResource,
    openAppFolder,
    type AppItem,
  } from "$lib/apps";
  import type { ResourceFile } from "$lib/library";
  import { confirm } from "@tauri-apps/plugin-dialog";
  import { t } from "$lib/i18n/index.svelte";
  import LoadingState from "./LoadingState.svelte";

  import { errorText } from "$lib/errors";
  let apps = $state<AppItem[]>([]);
  let query = $state("");
  let busy = $state<string | null>(null);
  let loading = $state(true);
  let error = $state("");

  // Ressources (§4.5.2) : chargées à la demande, une seule fois par app dépliée
  // (pas de parcours disque pour chaque app de la liste tant que personne ne
  // regarde), mémorisées ensuite pour un repli instantané.
  let expandedId = $state<string | null>(null);
  let resourcesById = $state<Record<string, ResourceFile[]>>({});
  let resourcesLoading = $state<string | null>(null);

  async function toggleResources(id: string) {
    if (expandedId === id) {
      expandedId = null;
      return;
    }
    expandedId = id;
    if (!resourcesById[id]) {
      resourcesLoading = id;
      try {
        resourcesById = { ...resourcesById, [id]: await listAppResources(id) };
      } finally {
        resourcesLoading = null;
      }
    }
  }

  async function openResource(id: string, f: ResourceFile) {
    try {
      await openAppResource(id, f.rel_path);
    } catch (e) {
      error = errorText(e);
    }
  }

  function fmtFileSize(bytes: number): string {
    if (bytes < 1024) return `${bytes} o`;
    const units = ["Ko", "Mo", "Go"];
    let v = bytes;
    let i = -1;
    do {
      v /= 1024;
      i++;
    } while (v >= 1024 && i < units.length - 1);
    return `${v.toFixed(v < 10 ? 1 : 0)} ${units[i]}`;
  }

  async function load() {
    try {
      apps = await listApps();
    } finally {
      loading = false;
    }
  }
  onMount(load);

  async function toggle(a: AppItem) {
    busy = a.id;
    error = "";
    try {
      if (a.active) await deactivateApp(a.id);
      else await activateApp(a.id);
      await load();
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = null;
    }
  }

  async function openFolder(id: string) {
    try {
      await openAppFolder(id);
    } catch (e) {
      error = errorText(e);
    }
  }

  async function remove(a: AppItem) {
    const ok = await confirm(t("apps.confirmDelete", { id: a.id }), {
      title: t("common.delete"),
      kind: "warning",
    });
    if (!ok) return;
    busy = a.id;
    error = "";
    try {
      await deleteApp(a.id);
      await load();
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = null;
    }
  }

  const filtered = $derived(
    apps.filter((a) => {
      if (!query.trim()) return true;
      // Un terme par mot séparé par un espace, ET entre eux (même correction
      // que la bibliothèque, Library.svelte).
      const terms = query.toLowerCase().split(/\s+/).filter(Boolean);
      const hay = a.id.toLowerCase();
      return terms.every((term) => hay.includes(term));
    }),
  );
</script>

<div class="apps">
  <header class="head">
    <div>
      <h2 class="lbl-screen">{t("nav.apps")}</h2>
      <p class="sub">{t("apps.subtitle")}</p>
    </div>
    {#if apps.length}
      <input class="input search" placeholder={t("apps.searchPlaceholder")} bind:value={query} />
    {/if}
  </header>

  {#if error}<div class="err">{error}</div>{/if}

  {#if loading}
    <LoadingState />
  {:else if apps.length === 0}
    <div class="empty">
      <p>{t("apps.empty")}</p>
      <p class="hint">{t("apps.emptyHint", { path: "apps/python/<App>/ · apps/lua/<App>/" })}</p>
    </div>
  {:else}
    <ul class="list">
      {#each filtered as a (a.id)}
        <li class:active={a.active}>
          <div class="row">
            <span class="a-name mono">{a.id}</span>
            {#if a.source_archive}<span class="src mono">{a.source_archive}</span>{/if}
            {#if a.active}<span class="state on">{t("common.active").toLowerCase()}</span>{:else}<span class="state">{t("common.inactive").toLowerCase()}</span>{/if}
            <button class="btn" type="button" onclick={() => toggleResources(a.id)}>
              {t("apps.resources")}{#if resourcesById[a.id]?.length} <span class="mono">({resourcesById[a.id].length})</span>{/if}
            </button>
            <button class="btn" type="button" onclick={() => openFolder(a.id)} title={t("apps.openFolderTooltip")}>
              {t("detail.openFolder")}
            </button>
            <button class="btn" type="button" onclick={() => toggle(a)} disabled={busy === a.id}>
              {busy === a.id ? t("common.working") : a.active ? t("common.deactivate") : t("common.activate")}
            </button>
            <button class="btn del" type="button" title={t("common.delete")} onclick={() => remove(a)} disabled={busy === a.id}>✕</button>
          </div>
          {#if expandedId === a.id}
            <div class="res-panel">
              {#if resourcesLoading === a.id}
                <p class="res-empty">{t("common.loading")}</p>
              {:else if !resourcesById[a.id]?.length}
                <p class="res-empty">{t("detail.noResources")}</p>
              {:else}
                <ul class="res-list">
                  {#each resourcesById[a.id] as f (f.rel_path)}
                    <li>
                      <button class="res-row" type="button" onclick={() => openResource(a.id, f)} title={t("detail.resourceOpenTooltip")}>
                        <span class="res-nm">{f.rel_path}</span>
                        <span class="res-size mono">{fmtFileSize(f.size_bytes)}</span>
                      </button>
                    </li>
                  {/each}
                </ul>
              {/if}
            </div>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .apps {
    max-width: 860px;
  }
  .head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 20px;
    margin-bottom: 18px;
  }
  .sub {
    color: var(--muted);
    font-size: 12px;
    margin-top: 6px;
    line-height: 1.5;
    max-width: 520px;
  }
  .search {
    width: 220px;
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
  .list {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  .list li {
    border: 1px solid var(--line);
    background: var(--panel2);
  }
  .list li.active {
    border-left: 3px solid var(--green-border);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 9px 12px;
  }
  .a-name {
    flex: 1;
    font-weight: 600;
    color: var(--txt);
    font-size: 12.5px;
  }
  .src {
    color: var(--muted2);
    font-size: 10px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 200px;
  }
  .state {
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--muted);
  }
  .state.on {
    color: var(--green);
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
  .res-panel {
    border-top: 1px solid var(--line);
    padding: 9px 12px;
  }
  .res-empty {
    color: var(--muted);
    font-size: 11.5px;
  }
  .res-list {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .res-row {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    border: 1px solid var(--line);
    background: var(--raised);
    padding: 7px 10px;
    text-align: left;
    cursor: pointer;
  }
  .res-row:hover {
    border-color: var(--rosso-border);
  }
  .res-nm {
    flex: 1;
    min-width: 0;
    font-size: 11.5px;
    color: var(--txt2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .res-row:hover .res-nm {
    color: var(--rosso-bright);
  }
  .res-size {
    flex: none;
    font-size: 10px;
    color: var(--muted2);
  }
</style>
