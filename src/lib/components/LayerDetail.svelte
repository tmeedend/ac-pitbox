<script lang="ts">
  // Fiche d'une couche (REFONTE§8.3).
  //
  // Une couche n'avait pas de fiche : ses fichiers se déversaient dans la liste
  // de l'hôte, 392 lignes en monospace avec le poids de chacun. Ce déversement
  // disparaît (REFONTE§8.2) — il vit ici, et il y est organisé.
  //
  // L'ordre des blocs n'est pas neutre. **Ce qui écrase la base vient en
  // premier**, en clair et non en compteur, parce que c'est le seul endroit où
  // une couche inquiète : elle remplace des fichiers du jeu. La promesse du
  // §4.4 — l'original est sauvegardé, il revient à la désactivation — est donc
  // écrite là, une fois, au lieu d'être répétée en bandeau sur chaque écran.
  import { onMount } from "svelte";
  import { confirm } from "@tauri-apps/plugin-dialog";
  import { t } from "$lib/i18n/index.svelte";
  import { errorText } from "$lib/errors";
  import { fmtSize } from "$lib/format";
  import { layerDisplayName } from "$lib/layerName";
  import { setEntityDisplayName, setEntityNote } from "$lib/userMeta";
  import {
    listLayerFiles,
    setLayerActive,
    reorderLayer,
    deleteLayer,
    openLayerFolder,
    type LayerFile,
    type LayerRow,
  } from "$lib/library";
  import FicheHeader from "./FicheHeader.svelte";
  import NoteBlock from "./NoteBlock.svelte";
  import LoadingState from "./LoadingState.svelte";

  interface Props {
    layer: LayerRow;
    /** Nom lisible de l'hôte, pour en retirer le préfixe du nom dérivé. */
    hostName: string | null;
    /** Couches du même hôte : la carte Ordre n'a de sens qu'à partir de deux
     * (principe des lignes vides omises, §6). */
    siblingCount: number;
    onclose: () => void;
    /** La couche a changé d'état, d'ordre ou a disparu : l'hôte doit relire. */
    onchanged: () => void;
  }
  let { layer, hostName, siblingCount, onclose, onchanged }: Props = $props();

  let files = $state<LayerFile[]>([]);
  let loading = $state(true);
  let error = $state("");
  let busy = $state(false);
  /** Dossiers dépliés de « ce qui s'ajoute ». Repliés au départ : la liste dit
   * d'abord ce que la couche touche, le détail vient sur demande. */
  let opened = $state<Set<string>>(new Set());

  const name = $derived(layer.display_name_user ?? layerDisplayName(layer.name, hostName));
  const overwrites = $derived(files.filter((f) => f.overwrites));
  const additions = $derived(files.filter((f) => !f.overwrites));
  const totalBytes = $derived(files.reduce((sum, f) => sum + f.size_bytes, 0));

  /** Ce qui s'ajoute, groupé par **premier dossier** : c'est le niveau auquel
   * un mod se lit (`ui/`, `data/`, `skins/`). Le compteur et le poids vivent
   * sur le dossier, jamais sur le fichier — le poids d'un fichier ne décide
   * de rien. */
  const groups = $derived.by(() => {
    const map = new Map<string, LayerFile[]>();
    for (const f of additions) {
      const cut = f.rel_path.indexOf("/");
      const key = cut > 0 ? f.rel_path.slice(0, cut) : "";
      const list = map.get(key);
      if (list) list.push(f);
      else map.set(key, [f]);
    }
    return [...map.entries()]
      .map(([folder, list]) => ({
        folder,
        files: list,
        bytes: list.reduce((sum, f) => sum + f.size_bytes, 0),
      }))
      .sort((a, b) => a.folder.localeCompare(b.folder));
  });

  onMount(async () => {
    try {
      files = await listLayerFiles(layer.id);
    } catch (e) {
      error = errorText(e);
    } finally {
      loading = false;
    }
  });

  function toggle(folder: string) {
    const next = new Set(opened);
    if (next.has(folder)) next.delete(folder);
    else next.add(folder);
    opened = next;
  }

  async function run(action: () => Promise<void>) {
    busy = true;
    error = "";
    try {
      await action();
      onchanged();
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = false;
    }
  }

  async function remove() {
    const ok = await confirm(t("detail.layerDeleteConfirm", { name }), {
      title: t("detail.layerDeleteTitle"),
      kind: "warning",
    });
    if (!ok) return;
    await run(async () => {
      await deleteLayer(layer.id);
      onclose();
    });
  }

  async function rename(value: string | null) {
    error = "";
    try {
      await setEntityDisplayName("LAYER", layer.id, value ?? "");
      onchanged();
    } catch (e) {
      error = errorText(e);
    }
  }

  async function saveNote(value: string | null) {
    error = "";
    try {
      await setEntityNote("LAYER", layer.id, value ?? "");
      onchanged();
    } catch (e) {
      error = errorText(e);
    }
  }
</script>

<div class="page">
  <FicheHeader
    onback={onclose}
    backLabel={t("detail.layerBack")}
    glyph="▤"
    {name}
    subtitle={layer.source_archive ?? layer.name}
    rename={{ original: layerDisplayName(layer.name, hostName), overridden: !!layer.display_name_user, onsave: rename }}
    deployment={{ active: layer.is_active }}
    actions={[
      {
        label: layer.is_active ? t("common.deactivate") : t("common.activate"),
        onclick: () => run(() => setLayerActive(layer.id, !layer.is_active)),
        disabled: busy,
      },
      { label: t("detail.openFolder"), onclick: () => void openLayerFolder(layer.id) },
      { label: t("common.delete"), onclick: remove, disabled: busy, danger: true },
    ]}
  />

  {#if error}<div class="errbox">{error}</div>{/if}

  {#if loading}
    <LoadingState />
  {:else}
    <!-- Les trois chiffres qui disent ce que la couche fait, avant le détail. -->
    <div class="stats">
      <div><span class="lbl-key">{t("detail.layerAdded")}</span><span class="v mono">{additions.length}</span></div>
      <div>
        <span class="lbl-key">{t("detail.layerOverwritten")}</span>
        <span class="v mono" class:danger={overwrites.length > 0}>{overwrites.length}</span>
      </div>
      <div><span class="lbl-key">{t("detail.layerWeight")}</span><span class="v mono">{fmtSize(totalBytes)}</span></div>
    </div>

    {#if overwrites.length}
      <section class="blk">
        <header class="blk-h">
          <span class="blk-t">{t("detail.layerOverwritesTitle")}</span>
          <span class="blk-n">{overwrites.length}</span>
        </header>
        <div class="blk-b">
          <p class="assure">{t("detail.layerOverwritesSafe")}</p>
          <ul class="files">
            {#each overwrites as f (f.rel_path)}
              <li><span class="mono over">{f.rel_path}</span><span class="sz mono">{fmtSize(f.size_bytes)}</span></li>
            {/each}
          </ul>
        </div>
      </section>
    {/if}

    <section class="blk">
      <header class="blk-h">
        <span class="blk-t">{t("detail.layerAddsTitle")}</span>
        <span class="blk-n">{additions.length}</span>
      </header>
      <div class="blk-b">
        {#if !additions.length}
          <p class="muted small">{t("detail.layerAddsNone")}</p>
        {:else}
          <ul class="groups">
            {#each groups as g (g.folder)}
              <li>
                <button class="grp" type="button" onclick={() => toggle(g.folder)} aria-expanded={opened.has(g.folder)}>
                  <span class="caret">{opened.has(g.folder) ? "▾" : "▸"}</span>
                  <span class="gname mono">{g.folder || t("detail.layerRootFolder")}</span>
                  <span class="gmeta mono">{t("detail.layerFileCount", { count: g.files.length })} · {fmtSize(g.bytes)}</span>
                </button>
                {#if opened.has(g.folder)}
                  <ul class="files">
                    {#each g.files as f (f.rel_path)}
                      <li><span class="mono">{f.rel_path}</span><span class="sz mono">{fmtSize(f.size_bytes)}</span></li>
                    {/each}
                  </ul>
                {/if}
              </li>
            {/each}
          </ul>
        {/if}
      </div>
    </section>

    {#if siblingCount > 1}
      <!-- Carte Ordre : seulement à partir de deux couches sur le même hôte.
           Seule, une couche n'a pas d'ordre — et un contrôle qui ne peut rien
           changer est une question posée pour rien. -->
      <section class="blk">
        <header class="blk-h"><span class="blk-t">{t("detail.layerOrderTitle")}</span></header>
        <div class="blk-b order">
          <p class="muted small">{t("detail.layerOrderHint")}</p>
          <div class="order-btns">
            <button class="btn" type="button" disabled={busy} onclick={() => run(() => reorderLayer(layer.id, "up"))}>
              {t("detail.layerOrderUp")}
            </button>
            <button class="btn" type="button" disabled={busy} onclick={() => run(() => reorderLayer(layer.id, "down"))}>
              {t("detail.layerOrderDown")}
            </button>
          </div>
        </div>
      </section>
    {/if}

    <NoteBlock value={layer.notes_user} onsave={saveNote} />
  {/if}
</div>

<style>
  .page {
    max-width: 860px;
  }
  .stats {
    display: flex;
    gap: 28px;
    margin-bottom: 16px;
  }
  .stats > div {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .stats .v {
    font-size: 15px;
    color: var(--txt2);
  }
  /* Rouge sur le seul chiffre qui décrit un remplacement : c'est le barème du
     §7.2ter — le rouge dit « ça touche au jeu », pas « c'est cassé ». */
  .stats .v.danger {
    color: var(--rosso-bright);
  }
  .assure {
    color: var(--muted);
    font-size: 11.5px;
    margin-bottom: 8px;
  }
  .groups {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .grp {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 8px;
    background: transparent;
    color: var(--txt2);
    font-size: 11.5px;
    padding: 5px 4px;
    text-align: left;
  }
  .grp:hover {
    background: var(--raised);
  }
  .caret {
    flex: none;
    color: var(--muted2);
    font-size: 9px;
  }
  .gname {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .gmeta {
    flex: none;
    color: var(--muted2);
    font-size: 10.5px;
  }
  .files {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 1px;
    padding: 2px 0 6px 20px;
  }
  .files li {
    display: flex;
    gap: 10px;
    font-size: 10.5px;
    color: var(--muted);
  }
  .files li span:first-child {
    flex: 1;
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .over {
    color: var(--rosso-bright);
  }
  .sz {
    flex: none;
    color: var(--muted2);
  }
  .order-btns {
    display: flex;
    gap: 8px;
    margin-top: 8px;
  }
</style>
