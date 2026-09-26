<script lang="ts">
  // One question of the pending-folders dialog (§4.6ter): what the author
  // offers, what it would do, and the answers that make sense for it.
  //
  // Out of `PendingDialog` because it has its own reasons to change — what a
  // question shows — while the dialog decides which questions there are and
  // when it closes.
  import { errorText } from "$lib/errors";
  import { fmtSize } from "$lib/format";
  import { readPendingDocument, type PendingAction, type PendingFolder } from "$lib/workshop/pending";
  import { t } from "$lib/i18n/index.svelte";

  let {
    folder: f,
    done,
    busy,
    onsettle,
    onerror,
  }: {
    folder: PendingFolder;
    /** Answer given during this opening of the dialog, if any. */
    done: PendingAction | undefined;
    busy: boolean;
    onsettle: (action: PendingAction) => void;
    onerror: (message: string) => void;
  } = $props();

  /** Notice text once unfolded; `null` = folded. */
  let notice = $state<string | null>(null);

  /** Notices rendues sur place. Un PDF ou un .docx ne se rend pas ici : son nom
   * est affiché et le dossier reste ouvrable depuis la fiche du mod. */
  const READABLE = /\.(txt|md|nfo|log|ini|cfg)$/i;

  async function toggleNotice(): Promise<void> {
    if (notice !== null) {
      notice = null;
      return;
    }
    if (!f.readme) return;
    try {
      notice = await readPendingDocument(f.id, f.readme);
    } catch (e) {
      onerror(errorText(e));
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

<!-- Répondue, la carte ne change **pas de hauteur** : c'est la
     réponse choisie qui s'allume et les autres qui s'éteignent. La
     rétracter en une ligne faisait remonter tout ce qui suit, soit
     exactement le saut qu'on cherchait à supprimer — et la question
     qu'on vient de trancher reste lisible, ce qui est la seule façon
     de vérifier qu'on a répondu ce qu'on croit. -->
<article class="card" class:done>
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
    <button class="c-notice-btn" type="button" onclick={toggleNotice}>
      {notice !== null
        ? t("importOverlay.pendingHideNotice")
        : t("importOverlay.pendingReadNotice", { name: f.readme })}
    </button>
    {#if notice !== null}
      <div class="c-notice-h">{t("importOverlay.pendingNotice", { name: f.readme })}</div>
      <pre class="c-notice">{notice}</pre>
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
        class:suggested={a === f.suggestion && !done}
        class:chosen={done === a}
        type="button"
        aria-pressed={done ? done === a : undefined}
        disabled={busy || !!done}
        onclick={() => onsettle(a)}
      >
        <span class="c-act-l">
          {t(ACTION_LABEL[a])}
          {#if a === f.suggestion && !done}<em class="c-act-s">{t("importOverlay.pendingSuggested")}</em>{/if}
        </span>
        <span class="c-act-h">{t(ACTION_HINT[a])}</span>
      </button>
    {/each}
  </div>
</article>

<style>
  .card {
    padding: 14px 0;
    border-top: 1px solid var(--line);
  }
  .card:first-child {
    border-top: none;
    padding-top: 0;
  }
  /* Une carte répondue s'éteint sans rien perdre de sa taille : même texte,
     mêmes réponses, mêmes pixels — seul le contraste tombe, pour qu'elle ne se
     dispute plus l'œil avec la question suivante. */
  .card.done .c-title,
  .card.done .c-desc,
  .card.done .c-facts,
  .card.done .c-warn,
  .card.done .c-neutral {
    opacity: 0.55;
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
  /* La réponse donnée : vert, comme le « installé » du rapport d'import —
     c'est un fait acquis, pas une proposition. Les autres retombent au gris
     des boutons désactivés, si bien qu'une carte répondue se lit d'un coup
     d'œil sans qu'une seule ligne ait bougé. */
  .c-act.chosen:disabled {
    opacity: 1;
    border-color: var(--green-border);
  }
  .c-act.chosen .c-act-l {
    color: var(--green);
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
</style>
