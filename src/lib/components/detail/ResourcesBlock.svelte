<script lang="ts">
  // Bloc « Ressources » de la fiche détail (§4.6) : fichiers annexes rangés à
  // part du contenu de jeu (PDF, changelog, templates de skin…).
  //
  // Lus **en direct sur disque** à chaque ouverture, jamais mémorisés en base :
  // un fichier déposé à la main dans le dossier apparaît sans réimport.
  import { listModResources, openModResource, type ResourceFile } from "$lib/library";
  import { errorText } from "$lib/errors";
  import { t } from "$lib/i18n/index.svelte";

  let {
    modId,
    onerror,
  }: {
    modId: string;
    onerror: (message: string) => void;
  } = $props();

  let files = $state<ResourceFile[]>([]);

  // La garde sur `modId` évite qu'une réponse tardive d'un mod précédent
  // n'écrase la liste du mod courant.
  $effect(() => {
    const current = modId;
    files = [];
    listModResources(current).then((rs) => {
      if (current === modId) files = rs;
    });
  });

  /** Taille lisible (Ko/Mo/Go, base 1024) — unités françaises, propres à ce bloc. */
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

  async function open(f: ResourceFile) {
    try {
      // Le chemin relatif est résolu et validé côté backend (anti-traversée).
      await openModResource(modId, f.rel_path);
    } catch (e) {
      onerror(errorText(e));
    }
  }
</script>

<div class="lbl section">{t("detail.resourcesLabel", { count: files.length })}</div>
{#if files.length}
  <div class="prov-note">{t("detail.resourcesNote")}</div>
  <ul class="res-list">
    {#each files as f (f.rel_path)}
      <li>
        <button class="res-row" type="button" onclick={() => open(f)} title={t("detail.resourceOpenTooltip")}>
          <span class="res-nm">{f.rel_path}</span>
          <span class="res-size mono">{fmtFileSize(f.size_bytes)}</span>
        </button>
      </li>
    {/each}
  </ul>
{:else}
  <div class="muted small">{t("detail.noResources")}</div>
{/if}

<style>
  /* Styles repris de la fiche : le CSS Svelte étant scopé par composant, un
     bloc extrait doit emporter les siens. */
  .lbl {
    color: var(--faint);
    font-size: 9px;
    letter-spacing: 1.5px;
    margin-bottom: 8px;
    display: flex;
    align-items: center;
    text-transform: uppercase;
  }
  .lbl.section {
    margin-top: 14px;
  }
  .muted {
    color: var(--muted);
  }
  .small {
    font-size: 11px;
  }
  .prov-note {
    margin-top: 8px;
    background: var(--blue-dim);
    border: 1px solid var(--blue-border);
    color: var(--blue);
    font-size: 9px;
    font-family: var(--mono);
    padding: 6px 9px;
  }
  .res-list {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin: 6px 0 0;
    padding: 0;
  }
  .res-row {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 8px;
    border: 1px solid var(--line);
    background: var(--panel2);
    padding: 5px 9px;
    text-align: left;
    cursor: pointer;
  }
  .res-row:hover {
    border-color: var(--rosso-border);
  }
  .res-nm {
    flex: 1;
    min-width: 0;
    font-size: 11px;
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
    font-size: 9px;
    color: var(--muted2);
  }
</style>
