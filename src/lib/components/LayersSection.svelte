<script lang="ts">
  // Vue transversale des couches/extensions (§4.4) d'un type (voiture ou circuit),
  // regroupées par entité de base. Mêmes actions que la fiche détail : activer/
  // désactiver (recompose en jeu), réordonner, supprimer — plus un lien vers la
  // fiche de la base. Complète la vue Skins dans la page « add-ons » d'un type.
  import { onMount } from "svelte";
  import {
    listLayersByKind,
    listLibrary,
    setLayerActive,
    reorderLayer,
    deleteLayer,
    type LayerRow,
    type ModCard,
    type ModKind,
  } from "$lib/library";
  import { nav, requestSection } from "$lib/nav.svelte";
  import { confirm } from "@tauri-apps/plugin-dialog";
  import { t } from "$lib/i18n/index.svelte";

  let { kind }: { kind: ModKind } = $props();

  let layers = $state<LayerRow[]>([]);
  let cards = $state<ModCard[]>([]);
  let busy = $state(false);
  let error = $state("");

  const parents = $derived(new Map(cards.map((c) => [c.id_interne, c] as const)));

  async function load() {
    const [ls, lib] = await Promise.all([listLayersByKind(kind), listLibrary()]);
    layers = ls;
    cards = lib;
  }
  onMount(load);

  function parentName(id: string): string {
    return parents.get(id)?.display_name ?? id;
  }

  // Regroupé par base ; dans chaque groupe, la plus prioritaire en haut.
  const groups = $derived.by(() => {
    const map = new Map<string, LayerRow[]>();
    for (const l of layers) {
      let arr = map.get(l.parent_id);
      if (!arr) {
        arr = [];
        map.set(l.parent_id, arr);
      }
      arr.push(l);
    }
    return [...map.entries()]
      .map(([parent, items]) => ({
        parent,
        label: parentName(parent),
        items: [...items].sort((a, b) => b.priority - a.priority),
      }))
      .sort((a, b) => a.label.localeCompare(b.label));
  });

  async function run(fn: () => Promise<void>) {
    busy = true;
    error = "";
    try {
      await fn();
      await load();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  const toggle = (l: LayerRow) => run(() => setLayerActive(l.id, !l.is_active));
  const move = (l: LayerRow, dir: "up" | "down") => run(() => reorderLayer(l.id, dir));

  async function remove(l: LayerRow) {
    const ok = await confirm(t("detail.layerDeleteConfirm", { name: l.source_archive ?? l.name }), {
      title: t("detail.layerDeleteTitle"),
      kind: "warning",
    });
    if (!ok) return;
    run(() => deleteLayer(l.id));
  }

  async function openParent(id: string) {
    if (await requestSection(kind === "Track" ? "tracks" : "cars")) {
      nav.openMod = id;
    }
  }
</script>

<section class="layers-sec">
  <div class="sec-head">
    <h3>{t("transversal.layersTitle")}</h3>
    <p class="sub">{t("transversal.layersSubtitle")}</p>
  </div>

  {#if error}<div class="err">{error}</div>{/if}

  {#if layers.length === 0}
    <div class="empty">{t("transversal.noLayers")}</div>
  {:else}
    <div class="hint">{t("detail.layersRecomposeNote")}</div>
    <div class="groups">
      {#each groups as g (g.parent)}
        <div class="group">
          <div class="group-head">
            <button class="parent" type="button" onclick={() => openParent(g.parent)} title={t("detail.openSheetTooltip")}>
              {g.label}
            </button>
            <span class="g-count mono">{g.items.length}</span>
          </div>
          <ul class="list">
            {#each g.items as l, i (l.id)}
              <li class="row" class:inactive={!l.is_active}>
                <label class="tog" title={l.is_active ? t("detail.layerActiveOn") : t("detail.layerActiveOff")}>
                  <input type="checkbox" checked={l.is_active} disabled={busy} onchange={() => toggle(l)} />
                </label>
                <div class="main">
                  <span class="nm">{l.source_archive ?? l.name}</span>
                  <span class="counts mono">{t("detail.layerCounts", { added: l.added_count, overwritten: l.overwritten_count })}</span>
                </div>
                <div class="ord">
                  <button class="arrow" type="button" title={t("detail.layerUp")} disabled={busy || i === 0} onclick={() => move(l, "up")}>▲</button>
                  <button class="arrow" type="button" title={t("detail.layerDown")} disabled={busy || i === g.items.length - 1} onclick={() => move(l, "down")}>▼</button>
                </div>
                <button class="x" type="button" title={t("detail.layerDeleteTitle")} disabled={busy} onclick={() => remove(l)}>✕</button>
              </li>
            {/each}
          </ul>
        </div>
      {/each}
    </div>
  {/if}
</section>

<style>
  .layers-sec {
    margin-top: 28px;
    padding-top: 20px;
    border-top: 1px solid var(--line);
    max-width: 900px;
  }
  .sec-head h3 {
    font-size: 15px;
    font-weight: 600;
  }
  .sub {
    color: var(--muted);
    font-size: 12px;
    margin-top: 4px;
  }
  .err {
    background: var(--rosso-dim);
    border: 1px solid var(--rosso-border);
    color: var(--rosso-bright);
    padding: 10px 12px;
    font-size: 12px;
    margin: 12px 0;
  }
  .empty {
    color: var(--faint);
    font-size: 12px;
    padding: 16px 0;
  }
  .hint {
    color: var(--blue);
    font-family: var(--mono);
    font-size: 9.5px;
    margin: 12px 0;
  }
  .groups {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .group-head {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 4px 2px 6px;
    border-bottom: 1px solid var(--line);
    margin-bottom: 6px;
  }
  .parent {
    background: transparent;
    color: var(--blue);
    font-size: 12.5px;
    font-weight: 600;
    padding: 0;
  }
  .parent:hover {
    color: var(--rosso-bright);
  }
  .g-count {
    color: var(--faint);
    font-size: 10.5px;
  }
  .list {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    border: 1px solid var(--line);
    background: var(--panel2);
    padding: 6px 11px;
  }
  .row.inactive {
    opacity: 0.5;
  }
  .tog {
    flex: none;
    display: flex;
    align-items: center;
    cursor: pointer;
  }
  .main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .nm {
    font-size: 12px;
    color: var(--txt2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .counts {
    font-size: 9px;
    color: var(--muted2);
  }
  .ord {
    flex: none;
    display: flex;
    flex-direction: column;
    line-height: 0.7;
  }
  .arrow {
    background: none;
    border: none;
    color: var(--muted);
    cursor: pointer;
    font-size: 8px;
    padding: 1px 2px;
  }
  .arrow:disabled {
    opacity: 0.3;
    cursor: default;
  }
  .arrow:not(:disabled):hover {
    color: var(--txt2);
  }
  .x {
    flex: none;
    background: none;
    border: none;
    color: var(--muted);
    cursor: pointer;
    font-size: 12px;
    padding: 2px 4px;
  }
  .x:hover {
    color: var(--rosso-bright);
  }
</style>
