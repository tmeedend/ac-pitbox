<script lang="ts">
  // Corps du rapport d'import (§4.2bis), sans son cadre : rendu à l'identique
  // dans le toast de fin (ImportToasts) et sur l'écran Import, qui garde le
  // dernier rapport consultable. Extrait justement pour ça — deux copies du
  // même balisage auraient divergé dès la première retouche.
  import { nav, requestSection } from "$lib/nav.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { activateOther } from "$lib/others";
  import { errorText } from "$lib/errors";
  import { fmtSize } from "$lib/format";
  import {
    listPendingFolders,
    readPendingDocument,
    resolvePendingFolder,
    type PendingAction,
    type PendingFolder,
  } from "$lib/pending";
  import type { ArchiveResult, OtherImported, SubImported } from "$lib/library";

  interface Props {
    report: ArchiveResult[];
    /** Appelé juste avant d'ouvrir une fiche : le toast s'en sert pour se
     * REPLIER, sans quoi il recouvrirait la fiche qu'on vient d'ouvrir. Il se
     * fermait, avant — et on ouvre souvent plusieurs mods du même lot l'un
     * après l'autre, sans aucun chemin pour revenir au rapport. */
    onnavigate?: () => void;
  }
  const { report, onnavigate }: Props = $props();

  // Libellé + classe CSS de la pastille d'issue d'un mod importé.
  function outcomeChip(o: string): { cls: string; label: string } {
    if (o === "UPDATE_REPLACE") return { cls: "upd", label: t("importOverlay.outcomeUpdate") };
    if (o === "DUPLICATE") return { cls: "dup", label: t("importOverlay.outcomeDuplicate") };
    if (o === "EXTENSION") return { cls: "ext", label: t("importOverlay.outcomeExtension") };
    return { cls: "new", label: t("importOverlay.outcomeNew") };
  }

  /** Ouvre la fiche d'un contenu. Pour une couche, `id` est déjà celui du
   * contenu de base (§4.4) : c'est donc lui qui s'ouvre. */
  async function openContent(id: string, kind: string): Promise<void> {
    onnavigate?.();
    if (await requestSection(kind === "Track" ? "tracks" : "cars")) nav.openMod = id;
  }

  async function openSection(section: string): Promise<void> {
    onnavigate?.();
    await requestSection(section);
  }

  // --- Composants optionnels (§4.6bis) ------------------------------------
  //
  // Livrés par l'auteur dans une archive à part **et** modifiant le jeu de
  // base : l'app les a rangés sans les activer, parce qu'aucun des deux
  // défauts n'est sûr. La question est posée ici, en fin de lot — jamais
  // pendant, un import de cinquante mods ne doit pas s'interrompre.
  //
  // Ne rien décider est une réponse valable : le composant reste en
  // bibliothèque, activable depuis l'écran « Autres mods » quand on veut.
  const optionals = $derived<OtherImported[]>(
    report.flatMap((a) => (a.others ?? []).filter((o) => o.optional)),
  );

  /** État local par composant — le rapport vit en mémoire, ces choix aussi. */
  let settled = $state<Record<string, "installed" | "skipped">>({});
  let installing = $state<string | null>(null);
  let optionalError = $state<string | null>(null);

  async function install(o: OtherImported) {
    installing = o.id;
    optionalError = null;
    try {
      await activateOther(o.id);
      settled = { ...settled, [o.id]: "installed" };
    } catch (e) {
      optionalError = errorText(e);
    } finally {
      installing = null;
    }
  }

  // --- Dossiers proposés (§4.6ter) ----------------------------------------
  //
  // Chargés depuis la base, jamais depuis le rapport en mémoire : ne rien
  // décider est une réponse valable, donc ce qui attend doit survivre à une
  // fermeture de l'app — et se retrouver ici au prochain import.
  let pending = $state<PendingFolder[]>([]);
  let pendingBusy = $state<string | null>(null);
  let pendingError = $state<string | null>(null);
  /** Notice dépliée, par dossier. `null` = repliée. */
  let notices = $state<Record<string, string>>({});

  $effect(() => {
    // Dépend du rapport : un nouveau lot peut avoir ajouté des dossiers.
    void report;
    void refreshPending();
  });

  async function refreshPending(): Promise<void> {
    try {
      pending = await listPendingFolders();
    } catch {
      // Rien à dire : l'absence de liste n'est pas une erreur à montrer, elle
      // signifie seulement qu'il n'y a rien à trancher.
      pending = [];
    }
  }

  /** Notices lisibles sans quitter l'écran. Un PDF ou un .docx ne se rend pas
   * ici — le nom seul est affiché, et le dossier reste ouvrable ailleurs. */
  const READABLE = /\.(txt|md|nfo|log|ini|cfg)$/i;

  async function toggleNotice(f: PendingFolder): Promise<void> {
    if (notices[f.id] !== undefined) {
      const { [f.id]: _dropped, ...rest } = notices;
      notices = rest;
      return;
    }
    if (!f.readme) return;
    try {
      notices = { ...notices, [f.id]: await readPendingDocument(f.id, f.readme) };
    } catch (e) {
      pendingError = errorText(e);
    }
  }

  async function settle(f: PendingFolder, action: PendingAction): Promise<void> {
    pendingBusy = f.id;
    pendingError = null;
    try {
      await resolvePendingFolder(f.id, action);
      await refreshPending();
    } catch (e) {
      pendingError = errorText(e);
    } finally {
      pendingBusy = null;
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

  /** Skins et sons regroupés par contenu parent : un pack de quarante livrées
   * ferait déborder le rapport à raison d'une ligne chacune, alors qu'il n'y a
   * qu'une seule fiche à ouvrir au bout. */
  function subsByParent(subs: SubImported[]): { id: string; kind: string; skins: number; sounds: number }[] {
    const groups = new Map<string, { id: string; kind: string; skins: number; sounds: number }>();
    for (const s of subs) {
      const g = groups.get(s.parent_id) ?? {
        id: s.parent_id,
        kind: s.sub_type === "TRACK_SKIN" ? "Track" : "Car",
        skins: 0,
        sounds: 0,
      };
      if (s.sub_type === "SOUND") g.sounds++;
      else g.skins++;
      groups.set(s.parent_id, g);
    }
    return [...groups.values()];
  }
</script>

{#if pending.length}
  <!-- En tête du rapport, avant même les composants optionnels : c'est ici que
       la question la plus fréquente se pose, et un dossier proposé n'a pas de
       défaut sûr — il attend, il n'agit pas. -->
  <section class="pend">
    <div class="pend-h">{t("importOverlay.pendingTitle")} ({pending.length})</div>
    <p class="pend-note">{t("importOverlay.pendingNote")}</p>
    {#each pending as f (f.id)}
      <article class="pend-row">
        <header class="pend-top">
          <span class="pend-nm mono">{f.rel_path}</span>
          <span class="pend-shape">{t(SHAPE_LABEL[f.shape] ?? SHAPE_LABEL.unknown)}</span>
          <span class="pend-meta">
            {t("importOverlay.pendingFiles", { count: f.file_count, size: fmtSize(f.size_bytes) })}
          </span>
        </header>

        {#if f.title}<div class="pend-title">{f.title}</div>{/if}
        {#if f.description}<p class="pend-desc">{f.description}</p>{/if}

        <div class="pend-tags">
          {#if f.skin_target}
            <span class="pend-tag info">{t("importOverlay.pendingOverwrites", { name: f.skin_target })}</span>
          {:else if f.owner_id}
            <span class="pend-tag info">{t("importOverlay.pendingFor", { name: f.owner_id })}</span>
          {/if}
          {#if f.replaced > 0}
            <span class="pend-tag warn">{t("importOverlay.pendingReplaces", { count: f.replaced })}</span>
          {/if}
          {#if f.readme && READABLE.test(f.readme)}
            <button class="pend-notice-btn" type="button" onclick={() => toggleNotice(f)}>
              {notices[f.id] !== undefined
                ? t("importOverlay.pendingHideNotice")
                : t("importOverlay.pendingReadNotice", { name: f.readme })}
            </button>
          {:else if f.readme}
            <span class="pend-tag">{f.readme}</span>
          {/if}
        </div>

        {#if notices[f.id] !== undefined}
          <pre class="pend-notice">{notices[f.id]}</pre>
        {/if}

        <div class="pend-actions">
          {#each f.actions as a}
            <button
              class="btn"
              class:btn-primary={a === f.suggestion}
              class:danger={a === "discard"}
              type="button"
              title={t(ACTION_HINT[a])}
              disabled={pendingBusy === f.id}
              onclick={() => settle(f, a)}
            >
              {t(ACTION_LABEL[a])}
            </button>
          {/each}
        </div>
      </article>
    {/each}
    {#if pendingError}<p class="opt-err">{pendingError}</p>{/if}
  </section>
{/if}

{#if optionals.length}
  <!-- En tête du rapport : c'est la seule chose qui attend une réponse. -->
  <section class="opt">
    <div class="opt-h">{t("importOverlay.optionalTitle")}</div>
    <p class="opt-note">{t("importOverlay.optionalNote")}</p>
    {#each optionals as o (o.id)}
      <div class="opt-row">
        <span class="opt-nm mono">{o.id}</span>
        <span class="opt-n">{t("importOverlay.optionalReplaces", { count: o.game_files_replaced ?? 0 })}</span>
        {#if settled[o.id] === "installed"}
          <span class="opt-done">{t("importOverlay.optionalInstalled")}</span>
        {:else if settled[o.id] === "skipped"}
          <span class="opt-skip">{t("importOverlay.optionalSkipped")}</span>
        {:else}
          <button class="btn" type="button" disabled={installing === o.id} onclick={() => install(o)}>
            {t("importOverlay.optionalInstall")}
          </button>
          <button
            class="btn ghost"
            type="button"
            onclick={() => (settled = { ...settled, [o.id]: "skipped" })}
          >
            {t("importOverlay.optionalSkip")}
          </button>
        {/if}
      </div>
    {/each}
    {#if optionalError}<p class="opt-err">{optionalError}</p>{/if}
  </section>
{/if}

{#each report as a}
  {#if a.error}
    <div class="r-line err">⚠ {a.archive} — {a.error}</div>
  {/if}
  {#each a.mods as m}
    {@const chip = outcomeChip(m.outcome)}
    <!-- Un mod resté AMBIGU n'a rien écrit (§4.4) : il n'y a pas de fiche à
         ouvrir tant que l'utilisateur n'a pas tranché. -->
    {@const openable = m.outcome !== "AMBIGUOUS"}
    <div class="r-line">
      <span class="r-out {chip.cls}">{chip.label}</span>
      {#if openable}
        <button class="r-open" type="button" onclick={() => openContent(m.id_interne, m.kind)}>
          {m.display_name ?? m.id_interne}
        </button>
      {:else}
        {m.display_name ?? m.id_interne}
      {/if}
      {#if m.outcome === "DUPLICATE"}
        <span class="r-conflict">{t("importOverlay.duplicateNote")}</span>
      {:else if m.outcome === "EXTENSION"}
        <span class="r-conflict">{t("importOverlay.extensionNote", { added: m.added_count ?? 0, overwritten: m.overwritten_count ?? 0 })}</span>
      {/if}
    </div>
  {/each}
  {#each subsByParent(a.subs ?? []) as g}
    <div class="r-line shared">
      {t("importOverlay.subsAttached", {
        parts: `${g.skins ? t("importOverlay.skinCount", { count: g.skins }) : ""}${g.skins && g.sounds ? " · " : ""}${g.sounds ? t("importOverlay.soundCount", { count: g.sounds }) : ""}`,
      })}
      <button class="r-open" type="button" onclick={() => openContent(g.id, g.kind)}>{g.id}</button>
    </div>
  {/each}
  {#if (a.apps ?? []).length}
    <div class="r-line shared">
      <button class="r-open" type="button" onclick={() => openSection("apps")}>
        {t("importOverlay.appsImported", { count: a.apps.length })}
      </button>
    </div>
  {/if}
  {#if (a.others ?? []).length}
    <div class="r-line shared">
      <button class="r-open" type="button" onclick={() => openSection("others")}>
        {t("importOverlay.othersImported", { count: a.others.length })}
      </button>
    </div>
  {/if}
  {@const resExtracted =
    a.mods.reduce((acc, m) => acc + (m.resources_extracted ?? 0), 0) +
    (a.subs ?? []).reduce((acc, s) => acc + (s.resources_extracted ?? 0), 0) +
    (a.apps ?? []).reduce((acc, s) => acc + (s.resources_extracted ?? 0), 0) +
    (a.others ?? []).reduce((acc, s) => acc + (s.resources_extracted ?? 0), 0)}
  {#if resExtracted > 0}
    <div class="r-line shared">{t("importOverlay.resourcesExtracted", { count: resExtracted })}</div>
  {/if}
  {@const extras = a.extras ?? 0}
  {#if extras > 0}
    <div class="r-line shared">{t("importOverlay.extrasAttached", { count: extras })}</div>
  {/if}
{/each}

<style>
  /* Dossiers proposés (§4.6ter). Même bleu que partout ailleurs pour
     l'information — ce n'est ni une alerte ni une erreur, c'est une question.
     Encadré, comme les composants optionnels, sans quoi une question se lirait
     comme un compte rendu de plus. */
  .pend {
    border: 1px solid var(--blue-border);
    background: var(--raised);
    padding: 9px 11px;
    margin-bottom: 10px;
  }
  .pend-h {
    font-size: 10px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--blue);
    margin-bottom: 4px;
  }
  .pend-note {
    font-size: 11px;
    color: var(--muted);
    margin-bottom: 8px;
  }
  .pend-row {
    padding: 8px 0;
    border-top: 1px solid var(--line);
  }
  .pend-row:first-of-type {
    border-top: none;
  }
  .pend-top {
    display: flex;
    align-items: baseline;
    flex-wrap: wrap;
    gap: 8px;
  }
  .pend-nm {
    flex: 1;
    min-width: 0;
    font-size: 11px;
    color: var(--txt2);
    overflow-wrap: anywhere;
  }
  .pend-shape {
    font-size: 9px;
    letter-spacing: 0.5px;
    text-transform: uppercase;
    padding: 1px 5px;
    border: 1px solid var(--line);
    color: var(--muted);
    white-space: nowrap;
  }
  .pend-meta {
    font-size: 10.5px;
    color: var(--muted2);
    white-space: nowrap;
  }
  /* Le titre vient de l'auteur, pas de nous : il porte l'information la plus
     utile de la ligne, donc il passe devant le chemin d'archive. */
  .pend-title {
    margin-top: 4px;
    font-size: 12.5px;
    color: var(--txt);
  }
  .pend-desc {
    margin-top: 2px;
    font-size: 11px;
    line-height: 1.5;
    color: var(--muted);
    white-space: pre-wrap;
  }
  .pend-tags {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 6px;
  }
  .pend-tag {
    font-size: 10.5px;
    color: var(--muted);
  }
  .pend-tag.info {
    color: var(--blue);
  }
  .pend-tag.warn {
    color: var(--yellow);
  }
  .pend-notice-btn {
    background: none;
    border: none;
    padding: 0;
    font: inherit;
    font-size: 10.5px;
    color: var(--muted);
    cursor: pointer;
    text-decoration: underline;
    text-decoration-color: var(--line);
    text-underline-offset: 2px;
  }
  .pend-notice-btn:hover,
  .pend-notice-btn:focus-visible {
    color: var(--rosso-bright);
  }
  /* La notice s'étend dans le flux, sans hauteur imposée ni défilement propre —
     même choix que la prévisualisation des ressources (§4.5.2) : c'est la page
     qui défile, pas une boîte dans la page. */
  .pend-notice {
    margin-top: 6px;
    padding: 8px 10px;
    background: var(--panel);
    border: 1px solid var(--line);
    font-size: 11px;
    line-height: 1.5;
    color: var(--txt2);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .pend-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 8px;
  }
  .pend-actions .danger {
    color: var(--rosso-bright);
  }

  /* Composants optionnels (§4.6bis). Jaune = alerte : ce n'est ni une erreur
     ni une action destructive, c'est la seule chose du rapport qui attend une
     réponse. Encadré plutôt que fondu dans les lignes, sans quoi la question
     se lirait comme un compte rendu de plus. */
  .opt {
    border: 1px solid var(--yellow);
    background: var(--raised);
    padding: 9px 11px;
    margin-bottom: 10px;
  }
  .opt-h {
    font-size: 10px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--yellow);
    margin-bottom: 4px;
  }
  .opt-note {
    font-size: 11px;
    color: var(--muted);
    margin-bottom: 8px;
  }
  .opt-row {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px;
    padding: 3px 0;
  }
  .opt-nm {
    flex: 1;
    min-width: 0;
    font-size: 11px;
    color: var(--txt2);
    overflow-wrap: anywhere;
  }
  .opt-n {
    font-size: 10.5px;
    color: var(--yellow);
    white-space: nowrap;
  }
  .opt-done,
  .opt-skip {
    font-size: 10.5px;
    white-space: nowrap;
  }
  .opt-done {
    color: var(--green);
  }
  .opt-skip {
    color: var(--muted2);
  }
  .opt-err {
    margin-top: 6px;
    font-size: 11px;
    color: var(--rosso-bright);
  }
  .r-line {
    padding: 2px 0;
    color: var(--txt2);
  }
  .r-line.err {
    color: var(--rosso-bright);
  }
  .r-line.shared {
    color: var(--muted);
    font-size: 11px;
    padding-top: 4px;
  }
  .r-out {
    font-size: 9px;
    letter-spacing: 0.5px;
    padding: 1px 5px;
    border: 1px solid var(--line);
    margin-right: 6px;
  }
  .r-out.new {
    color: var(--green);
    border-color: var(--green-border);
  }
  .r-out.upd {
    color: var(--yellow);
    border-color: #4a4426;
  }
  .r-out.dup {
    color: var(--muted);
    border-color: var(--line);
  }
  .r-out.ext {
    color: var(--green);
    border-color: var(--green-border);
  }
  .r-conflict {
    color: var(--yellow);
    margin-left: 6px;
    font-size: 11px;
  }
  /* Nom cliquable : ouvre la fiche du contenu. Bouton et non `<span onclick>` —
     le rapport doit rester atteignable au clavier et à la manette. */
  .r-open {
    background: none;
    border: none;
    padding: 0;
    font: inherit;
    color: var(--txt);
    text-align: left;
    cursor: pointer;
    text-decoration: underline;
    text-decoration-color: var(--line);
    text-underline-offset: 2px;
  }
  .r-open:hover,
  .r-open:focus-visible {
    color: var(--rosso-bright);
    text-decoration-color: currentColor;
  }
  .r-line.shared .r-open {
    color: var(--muted);
    font-size: 11px;
  }
  .r-line.shared .r-open:hover,
  .r-line.shared .r-open:focus-visible {
    color: var(--rosso-bright);
  }
</style>
