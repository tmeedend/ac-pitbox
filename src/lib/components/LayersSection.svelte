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
    listLayerFiles,
    openLayerFolder,
    type LayerFile,
    type LayerRow,
    type ModCard,
    type ModKind,
  } from "$lib/library";
  import { nav, requestSection } from "$lib/nav.svelte";
  import { confirm } from "@tauri-apps/plugin-dialog";
  import { t } from "$lib/i18n/index.svelte";
  import LoadingState from "./LoadingState.svelte";

  import { errorText } from "$lib/errors";
  // `heading` : le titre de rubrique n'a de sens que si la section est empilée
  // avec d'autres. Dans un onglet, l'onglet la nomme déjà — le répéter juste
  // en dessous est du bruit.
  let { kind, heading = true }: { kind: ModKind; heading?: boolean } = $props();

  let layers = $state<LayerRow[]>([]);
  let cards = $state<ModCard[]>([]);
  let busy = $state(false);
  let loading = $state(true);
  let error = $state("");

  const parents = $derived(new Map(cards.map((c) => [c.id_interne, c] as const)));

  async function load() {
    try {
      const [ls, lib] = await Promise.all([listLayersByKind(kind), listLibrary()]);
      layers = ls;
      cards = lib;
    } finally {
      loading = false;
    }
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
      error = errorText(e);
    } finally {
      busy = false;
    }
  }

  const toggle = (l: LayerRow) => run(() => setLayerActive(l.id, !l.is_active));
  const move = (l: LayerRow, dir: "up" | "down") => run(() => reorderLayer(l.id, dir));

  // Ce que la couche apporte, et son dossier — mêmes deux besoins que sur la
  // fiche détail, et pour la même raison : deux décomptes ne disent pas *quoi*.
  let openId = $state<string | null>(null);
  let files = $state<LayerFile[]>([]);
  let filesFor = $state<string | null>(null);

  async function toggleFiles(l: LayerRow) {
    if (openId === l.id) {
      openId = null;
      return;
    }
    openId = l.id;
    if (filesFor !== l.id) {
      files = [];
      const got = await listLayerFiles(l.id).catch(() => [] as LayerFile[]);
      if (openId === l.id) {
        files = got;
        filesFor = l.id;
      }
    }
  }

  function size(bytes: number): string {
    if (bytes < 1024) return `${bytes} o`;
    if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} Ko`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} Mo`;
  }

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

<section class="layers-sec" class:alone={!heading}>
  <div class="sec-head">
    {#if heading}<h3 class="sec-t">{t("transversal.layersTitle")}</h3>{/if}
    <p class="sub">{t("transversal.layersSubtitle")}</p>
  </div>

  {#if error}<div class="err">{error}</div>{/if}

  {#if loading}
    <LoadingState />
  {:else if layers.length === 0}
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
                <button class="main" type="button" aria-expanded={openId === l.id} onclick={() => toggleFiles(l)}>
                  <span class="nm">{l.source_archive ?? l.name}</span>
                  <span class="counts mono">{t("detail.layerCounts", { added: l.added_count, overwritten: l.overwritten_count })}</span>
                </button>
                <button class="icon" type="button" title={t("detail.layerOpenFolder")} onclick={() => openLayerFolder(l.id)}>🗀</button>
                <div class="ord">
                  <button class="arrow" type="button" title={t("detail.layerUp")} disabled={busy || i === 0} onclick={() => move(l, "up")}>▲</button>
                  <button class="arrow" type="button" title={t("detail.layerDown")} disabled={busy || i === g.items.length - 1} onclick={() => move(l, "down")}>▼</button>
                </div>
                <button class="x" type="button" title={t("detail.layerDeleteTitle")} disabled={busy} onclick={() => remove(l)}>✕</button>
              </li>
              {#if openId === l.id}
                <li class="files">
                  {#if filesFor !== l.id}
                    <span class="fl-empty">{t("common.loading")}</span>
                  {:else if !files.length}
                    <span class="fl-empty">{t("detail.layerNoFiles")}</span>
                  {:else}
                    <ul class="fl">
                      {#each files as f (f.rel_path)}
                        <li class="fl-row" class:over={f.overwrites}>
                          <span class="fl-p mono">{f.rel_path}</span>
                          {#if f.overwrites}<span class="fl-tag">{t("detail.layerFileOverwrites")}</span>{/if}
                          <span class="fl-s mono">{size(f.size_bytes)}</span>
                        </li>
                      {/each}
                    </ul>
                  {/if}
                </li>
              {/if}
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
  /* Seule dans son onglet : le trait et l'espace qui la détachaient de la
     section précédente n'ont plus rien à séparer. */
  .layers-sec.alone {
    margin-top: 0;
    padding-top: 0;
    border-top: none;
  }
  .sub {
    color: var(--muted);
    font-size: 12px;
    margin-top: 8px;
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
  /* Devenu un bouton (déplie les fichiers) : habillage de bouton retiré, il
     doit rester la ligne de texte qu'il était. */
  .main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    background: none;
    border: none;
    padding: 0;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .main:hover .nm {
    color: var(--txt);
  }
  .icon {
    flex: none;
    background: none;
    border: none;
    color: var(--muted);
    cursor: pointer;
    font-size: 12px;
    padding: 2px 4px;
  }
  .icon:hover {
    color: var(--blue);
  }
  .files {
    list-style: none;
    margin: 0 0 4px;
    padding: 6px 8px 8px 30px;
    border: 1px solid var(--line);
    border-top: none;
  }
  .fl {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
    max-height: 220px;
    overflow-y: auto;
  }
  .fl-row {
    display: flex;
    align-items: baseline;
    gap: 8px;
    font-size: 10px;
  }
  .fl-p {
    flex: 1;
    min-width: 0;
    color: var(--txt2);
    overflow-wrap: anywhere;
  }
  .fl-row.over .fl-p,
  .fl-tag {
    color: var(--orange);
  }
  .fl-tag {
    flex: none;
    font-size: 9px;
  }
  .fl-s {
    flex: none;
    color: var(--muted2);
    font-size: 9px;
  }
  .fl-empty {
    font-size: 10px;
    color: var(--muted);
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
