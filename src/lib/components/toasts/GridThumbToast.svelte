<script lang="ts">
  // Suivi de la génération des vignettes, dans la pile bas-droite
  // (docs/SPEC-grille.md GRILLE§8).
  //
  // **Ce n'est pas un toast, et c'est tout le sujet du GRILLE§8.1.** Trois cents
  // voitures à environ une seconde, c'est cinq minutes et plus ; un toast est
  // éphémère par définition. Il faut une tâche de fond : elle ne se ferme pas
  // seule, elle survit à la navigation, elle se réduit au lieu de disparaître,
  // et elle porte une annulation.
  //
  // Elle emprunte quand même le cadre de `Toast` — c'est la pile qui décide de
  // la position (SPEC §4.2bis), et deux cartes fixées au même coin se
  // recouvrent en silence. **Écart assumé au GRILLE§8.2** : réduite, elle est une
  // barre d'une ligne et non une pastille circulaire à anneau. La pastille
  // aurait été une deuxième forme pour la même chose dans une pile qui n'en a
  // qu'une, et le chantier « composants partagés » dit exactement pourquoi
  // c'est cher. Ce que la spec protégeait vraiment est gardé : **le nom de la
  // voiture ne paraît pas dans l'état réduit**, il ferait clignoter le coin de
  // l'écran chaque seconde.
  import Toast from "./Toast.svelte";
  import {
    cancelGridThumbs,
    dismissGridThumbReport,
    gridThumbEta,
    gridThumbProgress,
    gridThumbsPaused,
  } from "$lib/gridthumbs/gridThumbs.svelte";
  import { t } from "$lib/i18n/index.svelte";

  const p = $derived(gridThumbProgress());

  /** Replié : l'utilisateur l'a réduit à la main. Il ne se replie jamais tout
   * seul — une tâche qui se cache d'elle-même laisse croire qu'elle a fini. */
  let collapsed = $state(false);

  const settled = $derived(p.done + p.failed);
  const ratio = $derived(p.total > 0 ? settled / p.total : 0);
  const percent = $derived(Math.round(ratio * 100));

  /** Temps restant en unités grossières : à la seconde près il sauterait à
   * chaque voiture, pour une précision que l'estimation n'a pas. Même
   * traitement que la progression d'import. */
  function etaText(secs: number): string {
    if (secs < 60) return t("gridThumbs.etaSeconds", { n: String(Math.max(5, Math.round(secs / 5) * 5)) });
    return t("gridThumbs.etaMinutes", { n: String(Math.max(1, Math.round(secs / 60))) });
  }

  const eta = $derived(gridThumbEta());

  /**
   * Suspendue — session lancée, ou atelier ouvert.
   *
   * **Une barre figée sans un mot passe pour une panne.** C'est le défaut que
   * la tâche de fond existe pour éviter : elle dure des minutes, donc tout ce
   * qu'elle ne dit pas, l'utilisateur le devine, et il devine mal. La barre
   * garde sa position — ce qui est fait reste fait — et la ligne du dessous dit
   * pourquoi plus rien n'avance.
   */
  const paused = $derived(gridThumbsPaused());

  /** Le rapport de fin. Une ligne factuelle, **sans tonalité d'échec** : les
   * voitures protégées ne sont pas un problème à régler, l'utilisateur n'y peut
   * rien et sa grille reste parfaitement utilisable (GRILLE§7). */
  const report = $derived.by(() => {
    const parts = [t("gridThumbs.reportDone", { count: String(p.done) })];
    if (p.failed > 0) parts.push(t("gridThumbs.reportFailed", { count: String(p.failed) }));
    if (p.cancelling) parts.push(t("gridThumbs.reportResume"));
    return parts.join(" · ");
  });
</script>

{#if p.running}
  <Toast
    title={collapsed
      ? t("gridThumbs.taskCollapsed", { percent: String(percent) })
      : paused
        ? t("gridThumbs.taskTitlePaused")
        : t("gridThumbs.taskTitle")}
    collapsed={collapsed}
    ontoggle={() => (collapsed = !collapsed)}
  >
    {#snippet actions()}
      <button class="btn-ghost g-cancel" type="button" onclick={cancelGridThumbs} disabled={p.cancelling}>
        {p.cancelling ? t("gridThumbs.cancelling") : t("common.cancel")}
      </button>
    {/snippet}
    <div class="g-bar">
      <div class="g-fill" class:paused style:width="{ratio * 100}%"></div>
    </div>
    <div class="g-row">
      <span class="mono">{t("gridThumbs.taskCount", { done: String(settled), total: String(p.total) })}</span>
      {#if eta !== null}<span class="g-eta mono">{etaText(eta)}</span>{/if}
    </div>
    <!-- La voiture en cours : ici et jamais dans l'état réduit. Remplacée par
         la raison de l'arrêt quand il y en a une — c'est ce qu'on vient lire. -->
    {#if paused}
      <div class="g-car g-paused">{t("gridThumbs.paused")}</div>
    {:else if p.current}
      <div class="g-car">{p.current}</div>
    {/if}
  </Toast>
{:else if p.finished}
  <Toast title={report} onclose={dismissGridThumbReport} />
{/if}

<style>
  .g-bar {
    height: 4px;
    background: var(--raised);
    overflow: hidden;
    margin-top: 2px;
  }
  .g-fill {
    height: 100%;
    background: var(--blue);
    transition: width 0.25s;
  }
  /* Grise plutôt que colorée : la barre ne ment pas sur ce qui est fait, elle
     dit seulement que rien n'avance. */
  .g-fill.paused {
    background: var(--muted2);
  }
  .g-paused {
    color: var(--txt2);
  }
  .g-row {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    margin-top: 7px;
    font-size: 11px;
    color: var(--txt2);
  }
  .g-eta {
    color: var(--muted);
  }
  /* Le nom tient sur une ligne : il change à chaque voiture, et un nom long qui
     repousserait la barre ferait sauter la carte entière. */
  .g-car {
    margin-top: 4px;
    font-size: 11px;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 260px;
  }
  .g-cancel {
    font-size: 11px;
  }
</style>
