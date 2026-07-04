<script lang="ts">
  // Vue transversale (§12bis.3) : liste tous les sous-éléments d'un type (skins
  // ou sons), avec l'entité cible affichée à côté, filtrable. Permet de
  // retrouver un pack sans ouvrir les fiches une à une. Ne pollue pas la
  // bibliothèque principale.
  import { onMount } from "svelte";
  import {
    listSubsByType,
    activateSound,
    restoreSound,
    deleteSubMod,
    type SubModRow,
  } from "$lib/submods";
  import { listLibrary, type ModCard } from "$lib/library";
  import { nav } from "$lib/nav.svelte";
  import { confirm } from "@tauri-apps/plugin-dialog";
  import { t } from "$lib/i18n/index.svelte";

  // "SKIN" | "SOUND"
  let { subType }: { subType: "SKIN" | "SOUND" } = $props();
  const isSound = subType === "SOUND";

  let subs = $state<SubModRow[]>([]);
  let cards = $state<ModCard[]>([]);
  let query = $state("");
  let busy = $state<string | null>(null);
  let error = $state("");

  const parents = $derived(
    new Map(cards.map((c) => [c.id_interne, c] as const)),
  );

  async function load() {
    // La vue Skins couvre skins de voitures (SKIN) et de circuits (TRACK_SKIN, §12bis.2).
    const types = isSound ? ["SOUND"] : ["SKIN", "TRACK_SKIN"];
    const [lists, lib] = await Promise.all([
      Promise.all(types.map((ty) => listSubsByType(ty))),
      listLibrary(),
    ]);
    subs = lists.flat();
    cards = lib;
  }
  onMount(load);

  function parentName(id: string): string {
    return parents.get(id)?.display_name ?? id;
  }

  function openParent(id: string) {
    const c = parents.get(id);
    nav.section = c?.kind === "Track" ? "tracks" : "cars";
    nav.openMod = id;
  }

  const filtered = $derived(
    subs.filter((s) => {
      if (!query.trim()) return true;
      const q = query.toLowerCase();
      return `${s.name} ${parentName(s.parent_id)} ${s.source_archive ?? ""}`
        .toLowerCase()
        .includes(q);
    }),
  );

  async function toggleSound(s: SubModRow) {
    busy = s.id;
    error = "";
    try {
      if (s.is_active) await restoreSound(s.parent_id);
      else await activateSound(s.id);
      await load();
    } catch (e) {
      error = String(e);
    } finally {
      busy = null;
    }
  }

  async function remove(s: SubModRow) {
    const msg = t(isSound ? "transversal.confirmDeleteSound" : "transversal.confirmDeleteSkin", { name: s.name });
    const ok = await confirm(msg, {
      title: t("common.delete"),
      kind: "warning",
    });
    if (!ok) return;
    busy = s.id;
    error = "";
    try {
      await deleteSubMod(s.id);
      await load();
    } catch (e) {
      error = String(e);
    } finally {
      busy = null;
    }
  }
</script>

<div class="trans">
  <header class="head">
    <div>
      <h2>{isSound ? t("nav.sounds") : t("nav.skins")}</h2>
      <p class="sub">
        {isSound ? t("transversal.soundSubtitle") : t("transversal.skinSubtitle")}
      </p>
    </div>
    <input class="input search" placeholder={t("transversal.searchPlaceholder")} bind:value={query} />
  </header>

  {#if error}<div class="err">{error}</div>{/if}

  {#if subs.length === 0}
    <div class="empty">
      <p>{isSound ? t("transversal.emptySound") : t("transversal.emptySkin")}</p>
      <p class="hint">{t("transversal.emptyHint")}</p>
    </div>
  {:else}
    <div class="count mono">{filtered.length} / {subs.length}</div>
    <ul class="list">
      {#each filtered as s (s.id)}
        <li class:active={s.is_active}>
          <div class="l-main">
            <span class="s-name">{s.name}</span>
            <button class="parent" type="button" onclick={() => openParent(s.parent_id)} title={t("detail.openSheetTooltip")}>
              → {parentName(s.parent_id)}
            </button>
            {#if s.source_archive}<span class="src mono">{s.source_archive}</span>{/if}
          </div>
          {#if isSound}
            {#if s.is_active}<span class="badge on">{t("common.active").toLowerCase()}</span>{/if}
            <button class="btn" type="button" onclick={() => toggleSound(s)} disabled={busy === s.id}>
              {busy === s.id ? t("common.working") : s.is_active ? t("transversal.restoreOriginal") : t("common.activate")}
            </button>
          {/if}
          <button class="btn del" type="button" title={t("common.delete")} onclick={() => remove(s)} disabled={busy === s.id}>✕</button>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .trans {
    max-width: 900px;
  }
  .head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 20px;
    margin-bottom: 18px;
  }
  h2 {
    font-size: 18px;
    font-weight: 600;
  }
  .sub {
    color: var(--muted);
    font-size: 12px;
    margin-top: 6px;
    line-height: 1.5;
    max-width: 540px;
  }
  .search {
    width: 240px;
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
  .count {
    color: var(--faint);
    font-size: 11px;
    margin-bottom: 8px;
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
  }
  .list li.active {
    border-color: var(--rosso-border);
    background: var(--rosso-dim);
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
</style>
