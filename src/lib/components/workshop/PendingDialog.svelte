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
  import { importState, closePendingDialog, refreshPendingCount } from "$lib/workshop/importState.svelte";
  import { errorText } from "$lib/errors";
  import {
    groupPending,
    listPendingFolders,
    resolvePendingFolder,
    type PendingAction,
    type PendingFolder,
    type PendingGroup,
  } from "$lib/workshop/pending";
  import { t } from "$lib/i18n/index.svelte";
  import PendingCard from "./PendingCard.svelte";

  let folders = $state<PendingFolder[]>([]);
  /** Key of the question being answered — one folder or a whole group. */
  let busy = $state<string | null>(null);
  let error = $state<string | null>(null);
  /**
   * Answer given to a folder during THIS opening of the dialog.
   *
   * The card stays in place once answered, showing what was chosen, instead of
   * leaving the list. Reloading the list on each answer made every card below
   * jump up under the pointer - reported, and it is what made a run of ten
   * folders unpleasant. The answer is already written on disk: this map only
   * decides what the card shows.
   */
  let settled = $state<Record<string, PendingAction>>({});
  /** Groups the user chose to answer folder by folder, for this opening. */
  let split = $state<string[]>([]);
  /** The questions: identical folders asked once (§4.6ter). Forty questions
   * for twenty VRC cars, each delivering the same two folders, was the
   * reported case — twenty times the same answer is not a decision. */
  const groups = $derived(groupPending(folders, new Set(split)));

  /** The answer shared by every folder of a question, if they all have one. */
  function answerOf(g: PendingGroup): PendingAction | undefined {
    const first = settled[g.folders[0].id];
    return first && g.folders.every((f) => settled[f.id] === first) ? first : undefined;
  }

  /** Ce qui attend encore une réponse : le décompte de l'en-tête, et ce que dit
   * le bouton de fermeture. Des questions, pas des dossiers : c'est ce que
   * l'utilisateur a devant lui. */
  const waiting = $derived(groups.filter((g) => !answerOf(g)));

  // Rechargée à chaque ouverture : un lot a pu en ajouter, et un autre écran a
  // pu en trancher entre-temps.
  $effect(() => {
    if (!importState.pendingOpen) return;
    void refresh();
  });

  async function refresh(): Promise<void> {
    try {
      folders = await listPendingFolders();
      // A fresh opening lists only what is still pending, so no answer from a
      // previous opening has a card to sit on any more.
      settled = {};
      split = [];
    } catch (e) {
      error = errorText(e);
      folders = [];
    }
    // Le bandeau du rapport lit le même compte : sans cette mise à jour, il
    // continuait d'annoncer un dossier à trancher après le dernier arbitrage,
    // et son bouton n'ouvrait plus rien.
    await refreshPendingCount();
  }

  /** Gives the answer to every folder of the question, one after the other:
   * each is its own move on disk, and one failure must not cost the others. */
  async function settle(g: PendingGroup, action: PendingAction): Promise<void> {
    busy = g.key;
    error = null;
    let failed = false;
    for (const f of g.folders) {
      try {
        await resolvePendingFolder(f.id, action);
        settled = { ...settled, [f.id]: action };
      } catch (e) {
        failed = true;
        error ??= errorText(e);
      }
    }
    // A group answered only in part is shown folder by folder: the answered
    // ones say so, and the ones that failed are still there to answer.
    if (failed && g.folders.length > 1) splitGroup(g);
    // The list is NOT reloaded: the card keeps its place. Only the count the
    // import report reads has to follow.
    //
    // And NOTHING scrolls. Bringing the next question up was tried and taken
    // back out: the list holds still under the answer, so a view that moves
    // by itself right after a click reads as a consequence of that click,
    // and whoever wanted to re-read what they just answered has to find it
    // again. The card that keeps its size is the whole point - moving the
    // viewport instead gives back the disorientation it removed.
    await refreshPendingCount();
    busy = null;
  }

  function splitGroup(g: PendingGroup): void {
    if (!split.includes(g.key)) split = [...split, g.key];
  }
</script>

{#if importState.pendingOpen && folders.length}
  <div class="backdrop">
    <div class="dlg">
      <header class="dlg-h">
        <h3>{t("importOverlay.pendingTitle")}</h3>
        {#if waiting.length}
          <span class="dlg-n">{t("importOverlay.pendingRemaining", { count: waiting.length })}</span>
        {/if}
      </header>
      <p class="dlg-note">{t("importOverlay.pendingNote")}</p>

      <div class="dlg-body">
        {#each groups as g (g.key)}
          <PendingCard
            folders={g.folders}
            done={answerOf(g)}
            busy={busy === g.key}
            onsettle={(a) => settle(g, a)}
            onsplit={() => splitGroup(g)}
            onerror={(m) => (error = m)}
          />
        {/each}
      </div>

      {#if error}<p class="dlg-err">{error}</p>{/if}
      <footer class="dlg-f">
        <!-- « Plus tard » tant qu'il reste une question : fermer n'est alors pas
             une fin, c'est un report (§4.6bis). Quand tout est tranché, le même
             bouton ne reporte plus rien. -->
        <button class="btn" type="button" onclick={closePendingDialog}>
          {waiting.length ? t("importOverlay.pendingClose") : t("importOverlay.pendingDone")}
        </button>
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
