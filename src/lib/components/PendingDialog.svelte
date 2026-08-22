<script lang="ts">
  // Écran d'arbitrage des dossiers proposés par l'auteur (§4.6ter).
  //
  // Modale et non section du rapport : la pile de notifications fait 380 px,
  // et une décision se prend sur un titre, une description libre, une notice,
  // un avertissement et jusqu'à quatre réponses. Ça ne tient pas — et une
  // question illisible se répond au hasard.
  //
  // **Non bloquante**, contrairement aux arbitrages de `ImportOverlay` : ne
  // rien décider est une réponse valable (§4.6bis). Fermer laisse les dossiers
  // en attente, et le rapport d'import garde la ligne pour y revenir.
  import { importState, closePendingDialog, refreshPendingCount } from "$lib/importState.svelte";
  import { errorText } from "$lib/errors";
  import { fmtSize } from "$lib/format";
  import {
    listPendingFolders,
    readPendingDocument,
    resolvePendingFolder,
    type PendingAction,
    type PendingFolder,
  } from "$lib/pending";
  import { t } from "$lib/i18n/index.svelte";

  let folders = $state<PendingFolder[]>([]);
  let busy = $state<string | null>(null);
  let error = $state<string | null>(null);
  /** Notice dépliée, par dossier. Absent = repliée. */
  let notices = $state<Record<string, string>>({});

  // Rechargée à chaque ouverture : un lot a pu en ajouter, et un autre écran a
  // pu en trancher entre-temps.
  $effect(() => {
    if (!importState.pendingOpen) return;
    void refresh();
  });

  async function refresh(): Promise<void> {
    try {
      folders = await listPendingFolders();
    } catch (e) {
      error = errorText(e);
      folders = [];
    }
    // Le bandeau du rapport lit le même compte : sans cette mise à jour, il
    // continuait d'annoncer un dossier à trancher après le dernier arbitrage,
    // et son bouton n'ouvrait plus rien.
    await refreshPendingCount();
  }

  /** Notices rendues sur place. Un PDF ou un .docx ne se rend pas ici : son nom
   * est affiché et le dossier reste ouvrable depuis la fiche du mod. */
  const READABLE = /\.(txt|md|nfo|log|ini|cfg)$/i;

  async function toggleNotice(f: PendingFolder): Promise<void> {
    if (notices[f.id] !== undefined) {
      const { [f.id]: _shown, ...rest } = notices;
      notices = rest;
      return;
    }
    if (!f.readme) return;
    try {
      notices = { ...notices, [f.id]: await readPendingDocument(f.id, f.readme) };
    } catch (e) {
      error = errorText(e);
    }
  }

  async function settle(f: PendingFolder, action: PendingAction): Promise<void> {
    busy = f.id;
    error = null;
    try {
      await resolvePendingFolder(f.id, action);
      await refresh();
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = null;
    }
  }

  const ACTION_LABEL: Record<PendingAction, string> = {
    game: "importOverlay.pendingActionGame",
    layer: "importOverlay.pendingActionLayer",
    resources: "importOverlay.pendingActionResources",
    other: "importOverlay.pendingActionOther",
    discard: "importOverlay.pendingActionDiscard",
  };
  const ACTION_HINT: Record<PendingAction, string> = {
    game: "importOverlay.pendingActionGameHint",
    layer: "importOverlay.pendingActionLayerHint",
    resources: "importOverlay.pendingActionResourcesHint",
    other: "importOverlay.pendingActionOtherHint",
    discard: "importOverlay.pendingActionDiscardHint",
  };
  const SHAPE_LABEL: Record<string, string> = {
    jsgme: "importOverlay.pendingShapeJsgme",
    gameTree: "importOverlay.pendingShapeGameTree",
    skinVariant: "importOverlay.pendingShapeSkinVariant",
    documents: "importOverlay.pendingShapeDocuments",
    unknown: "importOverlay.pendingShapeUnknown",
  };
</script>

{#if importState.pendingOpen && folders.length}
  <div class="backdrop">
    <div class="dlg">
      <header class="dlg-h">
        <h3>{t("importOverlay.pendingTitle")}</h3>
        <span class="dlg-n">{t("importOverlay.pendingRemaining", { count: folders.length })}</span>
      </header>
      <p class="dlg-note">{t("importOverlay.pendingNote")}</p>

      <div class="dlg-body">
        {#each folders as f (f.id)}
          <article class="card">
            <!-- Le titre de l'auteur passe devant le chemin d'archive : c'est
                 la seule ligne écrite pour être lue par un humain. -->
            <div class="c-head">
              {#if f.title}
                <span class="c-title">{f.title}</span>
              {:else}
                <span class="c-title mono">{f.rel_path}</span>
              {/if}
              <span class="c-shape">{t(SHAPE_LABEL[f.shape] ?? SHAPE_LABEL.unknown)}</span>
            </div>
            {#if f.title}<div class="c-path mono">{f.rel_path}</div>{/if}
            {#if f.description}<p class="c-desc">{f.description}</p>{/if}

            <div class="c-facts">
              <span>{t("importOverlay.pendingFiles", { count: f.file_count, size: fmtSize(f.size_bytes) })}</span>
              {#if f.skin_target}
                <span class="info">{t("importOverlay.pendingOverwrites", { name: f.skin_target })}</span>
              {:else if f.owner_id}
                <span class="info">{t("importOverlay.pendingFor", { name: f.owner_id })}</span>
              {/if}
            </div>

            <!-- L'avertissement porte sur le FAIT, pas sur un bouton : c'est le
                 rayon d'action qui mérite du jaune, pas une réponse (§4.6bis). -->
            {#if f.replaced > 0}
              <div class="c-warn">
                <span class="c-warn-h">{t("importOverlay.pendingReplaces", { count: f.replaced })}</span>
                <span class="c-warn-b">{t("importOverlay.pendingReplacesWhy")}</span>
              </div>
            {/if}

            {#if f.readme && READABLE.test(f.readme)}
              <button class="c-notice-btn" type="button" onclick={() => toggleNotice(f)}>
                {notices[f.id] !== undefined
                  ? t("importOverlay.pendingHideNotice")
                  : t("importOverlay.pendingReadNotice", { name: f.readme })}
              </button>
              {#if notices[f.id] !== undefined}
                <div class="c-notice-h">{t("importOverlay.pendingNotice", { name: f.readme })}</div>
                <pre class="c-notice">{notices[f.id]}</pre>
              {/if}
            {:else if f.readme}
              <div class="c-notice-btn as-text">
                {t("importOverlay.pendingNoticeUnreadable", { name: f.readme })}
              </div>
            {/if}

            {#if !f.suggestion}
              <p class="c-neutral">{t("importOverlay.pendingNoSuggestion")}</p>
            {/if}

            <!-- Chaque réponse porte son explication en toutes lettres : dans
                 une modale on a la place, et une infobulle est invisible à la
                 manette. -->
            <div class="c-actions">
              {#each f.actions as a}
                <button
                  class="c-act"
                  class:suggested={a === f.suggestion}
                  type="button"
                  disabled={busy === f.id}
                  onclick={() => settle(f, a)}
                >
                  <span class="c-act-l">
                    {t(ACTION_LABEL[a])}
                    {#if a === f.suggestion}<em class="c-act-s">{t("importOverlay.pendingSuggested")}</em>{/if}
                  </span>
                  <span class="c-act-h">{t(ACTION_HINT[a])}</span>
                </button>
              {/each}
            </div>
          </article>
        {/each}
      </div>

      {#if error}<p class="dlg-err">{error}</p>{/if}
      <footer class="dlg-f">
        <button class="btn" type="button" onclick={closePendingDialog}>{t("importOverlay.pendingClose")}</button>
      </footer>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 92;
    padding: 24px;
  }
  /* Large, parce que c'est le point du changement : la même question dans la
     pile de notifications était illisible. La hauteur est bornée par l'écran et
     c'est le corps qui défile, jamais la page. */
  .dlg {
    width: 760px;
    max-width: 100%;
    max-height: 100%;
    display: flex;
    flex-direction: column;
    background: var(--panel);
    border: 1px solid var(--blue-border);
    padding: 20px 22px 16px;
  }
  .dlg-h {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
  }
  .dlg-h h3 {
    font-size: 14px;
    font-weight: 600;
  }
  .dlg-n {
    font-size: 11px;
    color: var(--muted);
    white-space: nowrap;
  }
  .dlg-note {
    margin-top: 6px;
    font-size: 12px;
    line-height: 1.5;
    color: var(--muted);
  }
  .dlg-body {
    margin-top: 14px;
    overflow-y: auto;
    min-height: 0;
  }
  .card {
    padding: 14px 0;
    border-top: 1px solid var(--line);
  }
  .card:first-child {
    border-top: none;
    padding-top: 0;
  }
  .c-head {
    display: flex;
    align-items: baseline;
    gap: 10px;
  }
  .c-title {
    flex: 1;
    min-width: 0;
    font-size: 13.5px;
    color: var(--txt);
    overflow-wrap: anywhere;
  }
  .c-shape {
    font-size: 9px;
    letter-spacing: 0.5px;
    text-transform: uppercase;
    padding: 1px 5px;
    border: 1px solid var(--line);
    color: var(--muted);
    white-space: nowrap;
  }
  .c-path {
    margin-top: 3px;
    font-size: 11px;
    color: var(--muted2);
    overflow-wrap: anywhere;
  }
  .c-desc {
    margin-top: 6px;
    font-size: 12px;
    line-height: 1.55;
    color: var(--txt2);
    white-space: pre-wrap;
  }
  .c-facts {
    display: flex;
    flex-wrap: wrap;
    gap: 14px;
    margin-top: 8px;
    font-size: 11px;
    color: var(--muted);
  }
  .c-facts .info {
    color: var(--blue);
  }
  .c-warn {
    margin-top: 8px;
    padding: 7px 9px;
    border: 1px solid #4a4426;
    background: var(--raised);
  }
  .c-warn-h {
    display: block;
    font-size: 11.5px;
    color: var(--yellow);
  }
  .c-warn-b {
    display: block;
    margin-top: 3px;
    font-size: 11px;
    line-height: 1.5;
    color: var(--muted);
  }
  .c-notice-btn {
    display: block;
    margin-top: 8px;
    background: none;
    border: none;
    padding: 0;
    font: inherit;
    font-size: 11px;
    color: var(--muted);
    cursor: pointer;
    text-decoration: underline;
    text-decoration-color: var(--line);
    text-underline-offset: 2px;
  }
  .c-notice-btn:hover,
  .c-notice-btn:focus-visible {
    color: var(--rosso-bright);
  }
  .c-notice-btn.as-text {
    cursor: default;
    text-decoration: none;
  }
  .c-notice-h {
    margin-top: 8px;
    font-size: 10px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--muted2);
  }
  .c-notice {
    margin-top: 4px;
    padding: 9px 11px;
    max-height: 220px;
    overflow: auto;
    background: var(--raised);
    border: 1px solid var(--line);
    font-size: 11.5px;
    line-height: 1.55;
    color: var(--txt2);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .c-neutral {
    margin-top: 8px;
    font-size: 11px;
    color: var(--muted);
  }
  .c-actions {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(210px, 1fr));
    gap: 8px;
    margin-top: 12px;
  }
  /* Aucune couleur sur les réponses : ce sont quatre réponses à une question,
     pas une bonne et trois mauvaises. Le seul repère est la mention
     « proposé », et elle disparaît quand l'app n'a pas d'avis (§4.6bis). */
  .c-act {
    display: block;
    text-align: left;
    padding: 8px 10px;
    background: var(--raised);
    border: 1px solid var(--line);
    color: var(--txt2);
    cursor: pointer;
    font: inherit;
  }
  .c-act:hover:not(:disabled),
  .c-act:focus-visible {
    border-color: var(--txt2);
    color: var(--txt);
  }
  .c-act:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .c-act.suggested {
    border-color: var(--blue-border);
  }
  .c-act-l {
    display: block;
    font-size: 12px;
    color: var(--txt);
  }
  .c-act-s {
    margin-left: 6px;
    font-size: 9px;
    font-style: normal;
    letter-spacing: 0.5px;
    text-transform: uppercase;
    color: var(--blue);
  }
  .c-act-h {
    display: block;
    margin-top: 4px;
    font-size: 10.5px;
    line-height: 1.45;
    color: var(--muted);
  }
  .dlg-err {
    margin-top: 10px;
    font-size: 11.5px;
    color: var(--rosso-bright);
  }
  .dlg-f {
    display: flex;
    justify-content: flex-end;
    margin-top: 14px;
  }
</style>
