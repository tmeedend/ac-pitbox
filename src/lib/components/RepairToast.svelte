<script lang="ts">
  // Progression et rapport de la réparation générale (§9.3), dans la pile de
  // notifications.
  //
  // La réparation était une commande synchrone : la fenêtre gelait pendant
  // toute sa durée — plusieurs minutes sur une install réelle — et on ne savait
  // ni où elle en était, ni même si elle avançait. Elle rend maintenant la main
  // tout de suite ; ce composant est ce qui reste à l'écran pendant ce temps.
  import Toast from "./Toast.svelte";
  import { repairState, dismissRepairResult } from "$lib/repairState.svelte";
  import { t } from "$lib/i18n/index.svelte";

  // Clés explicites plutôt que construites à la volée : `t()` renvoyant la clé
  // quand elle manque, une phase non prévue s'afficherait telle quelle.
  const PHASE_KEYS: Record<string, string> = {
    sizing: "maintenance.repairPhaseSizing",
    projections: "maintenance.repairPhaseProjections",
    redeploy: "maintenance.repairPhaseRedeploy",
    reinstall: "maintenance.repairPhaseReinstall",
  };

  /** Temps restant en unités grossières — à la seconde près il sauterait à
   * chaque événement, pour une précision que l'estimation n'a pas. Même
   * arrondi que l'import, et les mêmes clés : deux barres qui comptent le
   * temps de deux façons différentes se contrediraient à l'œil. */
  function etaText(secs: number): string {
    if (secs < 60) return t("importOverlay.etaSeconds", { n: Math.max(5, Math.round(secs / 5) * 5) });
    return t("importOverlay.etaMinutes", { n: Math.max(1, Math.round(secs / 60)) });
  }

  const resultTitle = $derived.by(() => {
    const r = repairState.result;
    if (!r) return "";
    const failed = r.projections.failed.length + r.redeploy_errors.length + r.reinstall_errors.length;
    return (
      t("maintenance.repairDone", { count: r.redeployed }) +
      (failed ? t("maintenance.repairFailedCount", { count: failed }) : "")
    );
  });

  /** Toutes les lignes en échec, quelle que soit l'étape : une réparation
   * ratée sur trois cents mods ne se résume pas à un compteur. */
  const failures = $derived.by(() => {
    const r = repairState.result;
    if (!r) return [];
    return [
      ...r.projections.failed.map((line) => ({ id: "", error: line })),
      ...r.redeploy_errors,
      ...r.reinstall_errors,
    ];
  });
</script>

{#if repairState.result}
  <Toast title={resultTitle} onclose={dismissRepairResult}>
    {#each failures as f, i (`${f.id}:${i}`)}
      <div class="fail">
        {#if f.id}<span class="fail-id mono">{f.id}</span>{/if}
        <span class="fail-err">{f.error}</span>
      </div>
    {/each}
  </Toast>
{/if}

{#if repairState.running && repairState.progress}
  {@const p = repairState.progress}
  <Toast title={t("maintenance.repairTitle")} truncate>
    <div class="r-row">
      <span class="r-phase">{t(PHASE_KEYS[p.phase] ?? p.phase)}</span>
      {#if p.total > 1}<span class="r-count mono">{Math.max(1, p.index)} / {p.total}</span>{/if}
    </div>
    {#if p.label}<div class="r-id mono">{p.label}</div>{/if}
    <div class="r-bar">
      <div class="r-fill" style:width="{p.ratio * 100}%"></div>
    </div>
    {#if p.etaSecs != null}<div class="r-eta mono">{etaText(p.etaSecs)}</div>{/if}
  </Toast>
{/if}

<style>
  .r-row {
    display: flex;
    align-items: baseline;
    gap: 10px;
    min-width: 0;
  }
  .r-phase {
    flex: 1;
    min-width: 0;
    font-size: 11.5px;
    color: var(--txt2);
  }
  .r-count {
    flex: none;
    color: var(--muted);
    font-size: 11px;
  }
  .r-id {
    color: var(--muted);
    font-size: 11px;
    margin: 2px 0 6px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .r-bar {
    height: 4px;
    background: var(--line);
    overflow: hidden;
  }
  .r-fill {
    height: 100%;
    background: var(--rosso);
    transition: width 0.2s;
  }
  .r-eta {
    margin-top: 4px;
    font-size: 11px;
    color: var(--muted);
  }
  .fail {
    display: flex;
    gap: 8px;
    padding: 2px 0;
    font-size: 11px;
  }
  .fail-id {
    flex: none;
    color: var(--txt2);
  }
  .fail-err {
    color: var(--rosso-bright);
    overflow-wrap: anywhere;
  }
</style>
