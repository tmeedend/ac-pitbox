<script lang="ts">
  // Bloc « Couches / extensions » de la fiche détail (§4.4) : contenus importés
  // par-dessus une base, activables et réordonnables, jamais destructifs.
  //
  // Le composant possède sa propre liste et la relit après chaque action —
  // activer, déplacer ou supprimer une couche **recompose `content/`**, donc la
  // fiche parente doit se relire elle aussi : d'où `onchanged`.
  import {
    listLayers,
    deleteLayer,
    setLayerActive,
    reorderLayer,
    listLayerFiles,
    openLayerFolder,
    type LayerRow,
    type LayerFile,
    type LayerHostKind,
  } from "$lib/library";
  import { confirm } from "@tauri-apps/plugin-dialog";
  import { errorText } from "$lib/errors";
  import { t } from "$lib/i18n/index.svelte";

  let {
    modId,
    hostKind = "Car",
    onchanged,
    onerror,
  }: {
    modId: string;
    /** Espace de noms de l'hôte (§4.4). Une app vit dans une autre table qu'un
     * mod : rien n'empêche un circuit et une app de porter le même id, et sans
     * ce paramètre les couches de l'un remonteraient sur l'autre. Voiture et
     * circuit, eux, partagent une table à clé primaire — ils ne peuvent pas se
     * confondre, d'où le défaut. */
    hostKind?: LayerHostKind;
    /** La recomposition de content/ change l'état du mod : la fiche se relit. */
    onchanged: () => void;
    onerror: (message: string) => void;
  } = $props();

  let layers = $state<LayerRow[]>([]);
  let busy = $state(false);

  // Rechargement au changement de mod. La garde sur `modId` évite qu'une
  // réponse tardive d'un mod précédent n'écrase la liste du mod courant.
  $effect(() => {
    const current = modId;
    layers = [];
    listLayers(current, hostKind).then((ls) => {
      if (current === modId) layers = ls;
    });
  });

  async function reload() {
    layers = await listLayers(modId, hostKind);
  }

  // --- Ce que la couche apporte --------------------------------------------
  //
  // Les deux décomptes (« n ajoutés · m écrasés ») disent qu'il se passe
  // quelque chose, pas quoi. Sur une couche d'app — un `.lua` de réglages, des
  // fichiers de caméras — c'est la liste des fichiers qui dit à quoi elle sert.
  // Repliée par défaut : une couche de circuit peut en apporter des centaines.
  let openId = $state<string | null>(null);
  let files = $state<LayerFile[]>([]);
  let filesFor = $state<string | null>(null);

  async function toggleFiles(layer: LayerRow) {
    if (openId === layer.id) {
      openId = null;
      return;
    }
    openId = layer.id;
    // Gardes sur l'id, comme au chargement de la liste : une réponse tardive ne
    // doit pas peupler la ligne d'une autre couche.
    if (filesFor !== layer.id) {
      files = [];
      try {
        const got = await listLayerFiles(layer.id);
        if (openId === layer.id) {
          files = got;
          filesFor = layer.id;
        }
      } catch (e) {
        onerror(errorText(e));
      }
    }
  }

  async function openFolder(layer: LayerRow) {
    try {
      await openLayerFolder(layer.id);
    } catch (e) {
      onerror(errorText(e));
    }
  }

  /** Taille lisible, même règle d'arrondi que le reste des fiches. */
  function size(bytes: number): string {
    if (bytes < 1024) return `${bytes} o`;
    if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} Ko`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} Mo`;
  }

  /** Enveloppe commune : état occupé, remontée d'erreur, relecture des deux côtés. */
  async function run(action: () => Promise<unknown>) {
    busy = true;
    onerror("");
    try {
      await action();
      await reload();
      // La liste de fichiers en cache décrit l'état d'avant l'action : une
      // couche supprimée ou recomposée doit la faire relire, pas la garder.
      openId = null;
      filesFor = null;
      onchanged();
    } catch (e) {
      onerror(errorText(e));
    } finally {
      busy = false;
    }
  }

  async function remove(layer: LayerRow) {
    const ok = await confirm(t("detail.layerDeleteConfirm", { name: layer.source_archive ?? layer.name }), {
      title: t("detail.layerDeleteTitle"),
      kind: "warning",
    });
    if (ok) await run(() => deleteLayer(layer.id));
  }

  const toggle = (layer: LayerRow) => run(() => setLayerActive(layer.id, !layer.is_active));
  const move = (layer: LayerRow, direction: "up" | "down") => run(() => reorderLayer(layer.id, direction));

  // Priorité décroissante à l'écran : la couche qui gagne est affichée en haut.
  const ordered = $derived([...layers].reverse());
</script>

<!-- Sans couche, la rubrique n'a rien à dire : on ne montre pas une carte vide. -->
{#if layers.length}
  <section class="blk">
    <header class="blk-h">
      <span class="blk-t">{t("detail.layersTitle")}</span>
      <span class="blk-n">{layers.length}</span>
    </header>
    <div class="blk-b">
      <p class="note">{t("detail.layersNote")}</p>
      <ul class="layer-list">
        {#each ordered as l, i (l.id)}
          <li class="layer-row" class:inactive={!l.is_active}>
            <label class="layer-tog" title={l.is_active ? t("detail.layerActiveOn") : t("detail.layerActiveOff")}>
              <input type="checkbox" checked={l.is_active} disabled={busy} onchange={() => toggle(l)} />
            </label>
            <button class="layer-main" type="button" aria-expanded={openId === l.id} onclick={() => toggleFiles(l)}>
              <span class="layer-nm">{l.source_archive ?? l.name}</span>
              <span class="layer-counts mono">
                {t("detail.layerCounts", { added: l.added_count, overwritten: l.overwritten_count })}
              </span>
            </button>
            <button class="layer-icon" type="button" title={t("detail.layerOpenFolder")} onclick={() => openFolder(l)}>🗀</button>
            <div class="layer-ord">
              <button class="layer-arrow" type="button" title={t("detail.layerUp")} disabled={busy || i === 0} onclick={() => move(l, "up")}>▲</button>
              <button class="layer-arrow" type="button" title={t("detail.layerDown")} disabled={busy || i === ordered.length - 1} onclick={() => move(l, "down")}>▼</button>
            </div>
            <button class="layer-x" type="button" title={t("detail.layerDeleteTitle")} disabled={busy} onclick={() => remove(l)}>✕</button>
          </li>
          {#if openId === l.id}
            <li class="layer-files">
              {#if filesFor !== l.id}
                <span class="fl-empty">{t("common.loading")}</span>
              {:else if !files.length}
                <span class="fl-empty">{t("detail.layerNoFiles")}</span>
              {:else}
                <ul class="fl">
                  {#each files as f (f.rel_path)}
                    <li class="fl-row" class:over={f.overwrites}>
                      <span class="fl-p mono">{f.rel_path}</span>
                      {#if f.overwrites}
                        <span class="fl-tag">{t("detail.layerFileOverwrites")}</span>
                      {/if}
                      <span class="fl-s mono">{size(f.size_bytes)}</span>
                    </li>
                  {/each}
                </ul>
              {/if}
            </li>
          {/if}
        {/each}
      </ul>
      <p class="note last">{t("detail.layersRecomposeNote")}</p>
    </div>
  </section>
{/if}

<style>
  /* Habillage propre au bloc. Encadré et bandeau viennent des classes
     globales `.blk*` (voir global.css). */
  .note {
    color: var(--blue);
    font-family: var(--mono);
    font-size: 10.5px;
    line-height: 1.5;
    margin-bottom: 12px;
  }
  .note.last {
    margin: 12px 0 0;
  }
  .layer-list {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin: 6px 0 0;
    padding: 0;
  }
  .layer-row {
    display: flex;
    align-items: center;
    gap: 8px;
    border: 1px solid var(--line);
    background: var(--panel2);
    padding: 5px 9px;
  }
  .layer-row.inactive {
    opacity: 0.5;
  }
  .layer-tog {
    flex: none;
    display: flex;
    align-items: center;
    cursor: pointer;
  }
  /* Devenu un bouton (déplie les fichiers) : on lui retire l'habillage de
     bouton, il doit rester la ligne de texte qu'il était. */
  .layer-main {
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
  .layer-main:hover .layer-nm {
    color: var(--txt);
  }
  .layer-icon {
    flex: none;
    background: none;
    border: none;
    color: var(--muted);
    cursor: pointer;
    font-size: 12px;
    padding: 2px 4px;
  }
  .layer-icon:hover {
    color: var(--blue);
  }
  .layer-files {
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
  .fl-row.over .fl-p {
    color: var(--orange);
  }
  .fl-tag {
    flex: none;
    font-size: 9px;
    color: var(--orange);
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
  .layer-nm {
    font-size: 11px;
    color: var(--txt2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .layer-counts {
    font-size: 9px;
    color: var(--muted2);
  }
  .layer-ord {
    flex: none;
    display: flex;
    flex-direction: column;
    line-height: 0.7;
  }
  .layer-arrow {
    background: none;
    border: none;
    color: var(--muted);
    cursor: pointer;
    font-size: 8px;
    padding: 1px 2px;
  }
  .layer-arrow:disabled {
    opacity: 0.3;
    cursor: default;
  }
  .layer-arrow:not(:disabled):hover {
    color: var(--txt2);
  }
  .layer-x {
    flex: none;
    background: none;
    border: none;
    color: var(--muted);
    cursor: pointer;
    font-size: 12px;
    padding: 2px 4px;
  }
  .layer-x:hover {
    color: var(--rosso-bright);
  }
</style>
