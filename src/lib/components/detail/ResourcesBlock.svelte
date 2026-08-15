<script lang="ts">
  // Bloc « Ressources » de la fiche détail (§4.5.2) : fichiers annexes rangés à
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

<section class="blk">
  <header class="blk-h">
    <span class="blk-t">{t("detail.resourcesTitle")}</span>
    <span class="blk-n">{files.length}</span>
  </header>
  <div class="blk-b">
    {#if files.length}
      <p class="note">{t("detail.resourcesNote")}</p>
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
      <p class="empty">{t("detail.noResources")}</p>
    {/if}
  </div>
</section>

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
  .empty {
    color: var(--muted);
    font-size: 12px;
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
    padding: 8px 11px;
    text-align: left;
    cursor: pointer;
  }
  .res-row:hover {
    border-color: var(--rosso-border);
  }
  .res-nm {
    flex: 1;
    min-width: 0;
    font-size: 12px;
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
    font-size: 10.5px;
    color: var(--muted2);
  }
</style>
