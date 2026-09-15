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
    openLayerFolder,
    type LayerRow,
    type LayerHostKind,
  } from "$lib/library/library";
  import { layerDisplayName } from "$lib/detail/layerName";
  import { confirm } from "@tauri-apps/plugin-dialog";
  import { errorText } from "$lib/errors";
  import { t } from "$lib/i18n/index.svelte";

  let {
    modId,
    hostKind = "Car",
    hostName = null,
    onchanged,
    onerror,
    onopen,
  }: {
    modId: string;
    /** Nom lisible de l'hôte : sert à retirer son préfixe du nom dérivé d'une
     * couche (REFONTE§8.3), quand l'archive le répète. */
    hostName?: string | null;
    /** Espace de noms de l'hôte (§4.4). Une app vit dans une autre table qu'un
     * mod : rien n'empêche un circuit et une app de porter le même id, et sans
     * ce paramètre les couches de l'un remonteraient sur l'autre. Voiture et
     * circuit, eux, partagent une table à clé primaire — ils ne peuvent pas se
     * confondre, d'où le défaut. */
    hostKind?: LayerHostKind;
    /** La recomposition de content/ change l'état du mod : la fiche se relit. */
    onchanged: () => void;
    onerror: (message: string) => void;
    /** Ouvre la fiche de la couche (REFONTE§8.4). C'est là que vit le détail de ce
     * qu'elle apporte — cette liste-ci ne fait que poser, activer et
     * ordonner. */
    onopen: (layer: LayerRow, siblingCount: number) => void;
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

  // La liste des fichiers apportés **ne se déplie plus ici** (REFONTE§8.2) : elle
  // vivait en monospace au milieu de la liste des couches — 392 lignes pour
  // une seule, poids par fichier compris. Elle est dans la fiche de la couche,
  // où elle est groupée par dossier et où ce qui écrase la base vient en
  // premier.
  async function openFolder(layer: LayerRow) {
    try {
      await openLayerFolder(layer.id);
    } catch (e) {
      onerror(errorText(e));
    }
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
            <button class="layer-main" type="button" title={t("detail.layerOpenDetail")} onclick={() => onopen(l, layers.length)}>
              <span class="layer-nm">{l.display_name_user ?? layerDisplayName(l.name, hostName)}</span>
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
  /* **Les cinq règles que l'extraction avait laissées derrière.** Ce bloc vient
     d'un composant depuis supprimé, et le CSS d'un composant Svelte est scopé :
     le markup a déménagé, son habillage non. Rien ne le signale — ni `npm run check`, ni la
     relecture du fichier d'arrivée — et le navigateur sert alors son propre
     style de bouton : des rectangles blancs au milieu d'un thème sombre. Vu à
     l'usage sur les couches d'une app et sur celles de Spa. */
  .layer-nm {
    font-size: 12px;
    color: var(--txt2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .layer-counts {
    font-size: 9px;
    color: var(--muted2);
  }
  /* Les deux flèches forment un seul contrôle, d'où l'interligne serré : elles
     se lisent comme une paire, pas comme deux boutons voisins. */
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
  /* Une flèche éteinte dit « on est déjà en bout de liste ». Elle reste
     lisible : ce qui est désactivé existe et attend, ce qui est absent n'existe
     pas (même distinction qu'à l'écran de session, SESSION§3). */
  .layer-arrow:disabled {
    opacity: 0.3;
    cursor: default;
  }
  /* Réordonner n'est ni destructif ni une action de fichier : le survol
     éclaircit son gris, sans introduire de couleur (§7.2ter). */
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
  /* Supprimer, en revanche, a droit au rouge : c'est le sens destructif du
     barème, et c'est le traitement déjà en place partout ailleurs. */
  .layer-x:hover {
    color: var(--rosso-bright);
  }
</style>
