<script lang="ts">
  // One question of the pending-folders dialog (§4.6ter): what the author
  // offers, what it would do, and the answers that make sense for it.
  //
  // Out of `PendingDialog` because it has its own reasons to change — what a
  // question shows — while the dialog decides which questions there are and
  // when it closes.
  //
  // One question may stand for several folders (`groupPending`): twenty mods
  // delivering the same `Wallpapers/` are asked once, and the answer goes to
  // each. The card then shows what they share — the name, the shape, the
  // notice of the first — and totals the rest.
  import { errorText } from "$lib/errors";
  import { fmtSize } from "$lib/format";
  import { folderName, readPendingDocument, type PendingAction, type PendingFolder } from "$lib/workshop/pending";
  import { t } from "$lib/i18n/index.svelte";

  let {
    folders,
    done,
    busy,
    onsettle,
    onsplit,
    onerror,
  }: {
    /** One folder, or several asked as one question — never empty. */
    folders: PendingFolder[];
    /** Answer given during this opening of the dialog, if any. */
    done: PendingAction | undefined;
    busy: boolean;
    onsettle: (action: PendingAction) => void;
    /** Asks a group folder by folder instead. */
    onsplit: () => void;
    onerror: (message: string) => void;
  } = $props();

  const f = $derived(folders[0]);
  const many = $derived(folders.length > 1);
  const fileCount = $derived(folders.reduce((n, x) => n + x.file_count, 0));
  const sizeBytes = $derived(folders.reduce((n, x) => n + x.size_bytes, 0));
  /** Who the group's folders belong to. Three names read at a glance; the
   * rest is a count. */
  const SHOWN_OWNERS = 3;
  const owners = $derived(folders.map((x) => x.owner_id).filter((o): o is string => !!o));

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
    {:else if many}
      <!-- The archive paths differ from one mod to the next; the name the
           authors gave the folder is what they share. -->
      <span class="c-title mono">{folderName(f.rel_path)}</span>
    {:else}
      <span class="c-title mono">{f.rel_path}</span>
    {/if}
    {#if many}<span class="c-count">{t("importOverlay.pendingGroupCount", { count: folders.length })}</span>{/if}
    <span class="c-shape">{t(SHAPE_LABEL[f.shape] ?? SHAPE_LABEL.unknown)}</span>
  </div>
  {#if f.title && !many}<div class="c-path mono">{f.rel_path}</div>{/if}
  {#if f.description}<p class="c-desc">{f.description}</p>{/if}

  <div class="c-facts">
    <span>{t("importOverlay.pendingFiles", { count: fileCount, size: fmtSize(sizeBytes) })}</span>
    {#if many}
      {#if owners.length > SHOWN_OWNERS}
        <span class="info">
          {t("importOverlay.pendingForMore", {
            names: owners.slice(0, SHOWN_OWNERS).join(", "),
            count: owners.length - SHOWN_OWNERS,
          })}
        </span>
      {:else if owners.length}
        <span class="info">{t("importOverlay.pendingFor", { name: owners.join(", ") })}</span>
      {/if}
    {:else if f.skin_target}
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
  {#if many && !done}
    <button class="c-split" type="button" disabled={busy} onclick={onsplit}>
      {t("importOverlay.pendingSplit")}
    </button>
  {/if}
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
  .c-count {
    font-size: 11px;
    color: var(--muted);
    white-space: nowrap;
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
  /* Same discreet link as the notice: splitting is a way out, not a fifth
     answer competing with the four. */
  .c-notice-btn,
  .c-split {
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
  .c-notice-btn:focus-visible,
  .c-split:hover:not(:disabled),
  .c-split:focus-visible {
    color: var(--rosso-bright);
  }
  .c-split {
    margin-top: 10px;
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
