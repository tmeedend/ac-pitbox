<script lang="ts">
  // Bloc « Couches / extensions » de la fiche détail (§4.4) : contenus importés
  // par-dessus une base, activables et réordonnables, jamais destructifs.
  //
  // Le composant possède sa propre liste et la relit après chaque action —
  // activer, déplacer ou supprimer une couche **recompose `content/`**, donc la
  // fiche parente doit se relire elle aussi : d'où `onchanged`.
  import { listLayers, deleteLayer, setLayerActive, reorderLayer, type LayerRow } from "$lib/library";
  import { confirm } from "@tauri-apps/plugin-dialog";
  import { errorText } from "$lib/errors";
  import { t } from "$lib/i18n/index.svelte";

  let {
    modId,
    onchanged,
    onerror,
  }: {
    modId: string;
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
    listLayers(current).then((ls) => {
      if (current === modId) layers = ls;
    });
  });

  async function reload() {
    layers = await listLayers(modId);
  }

  /** Enveloppe commune : état occupé, remontée d'erreur, relecture des deux côtés. */
  async function run(action: () => Promise<unknown>) {
    busy = true;
    onerror("");
    try {
      await action();
      await reload();
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
            <div class="layer-main">
              <span class="layer-nm">{l.source_archive ?? l.name}</span>
              <span class="layer-counts mono">
                {t("detail.layerCounts", { added: l.added_count, overwritten: l.overwritten_count })}
              </span>
            </div>
            <div class="layer-ord">
              <button class="layer-arrow" type="button" title={t("detail.layerUp")} disabled={busy || i === 0} onclick={() => move(l, "up")}>▲</button>
              <button class="layer-arrow" type="button" title={t("detail.layerDown")} disabled={busy || i === ordered.length - 1} onclick={() => move(l, "down")}>▼</button>
            </div>
            <button class="layer-x" type="button" title={t("detail.layerDeleteTitle")} disabled={busy} onclick={() => remove(l)}>✕</button>
          </li>
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
  .layer-main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
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
