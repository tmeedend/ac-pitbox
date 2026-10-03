<script lang="ts">
  // The sessions of a server in order (SPEC-play-online.md, "Frise des
  // sessions"): PRACTICE 10 min → QUALI 10 min → RACE 15 laps, the running
  // one marked with its time left — a race under way means waiting for the
  // next session, which is decisive. Mandatory pit and reversed grid as pills.
  import { t } from "$lib/i18n/index.svelte";
  import { durationText } from "$lib/online/labels";
  import { fromSeconds, sessionTimeline } from "$lib/online/sessions";
  import type { ServerSummary } from "$lib/online/online";

  let { server }: { server: ServerSummary } = $props();

  const steps = $derived(sessionTimeline(server));
</script>

{#if steps.length}
  <div class="timeline">
    <ol>
      {#each steps as step, i (i)}
        <li class:active={step.active}>
          <span class="kind">{t(`online.session.${step.kind}`)}</span>
          {#if step.duration}
            <span class="dur mono">
              {durationText(step.duration)}{#if step.extraLap}&nbsp;{t("online.extraLap")}{/if}
            </span>
          {/if}
          {#if step.active && server.time_left > 0}
            <span class="left mono">{t("online.timeLeft", { time: durationText(fromSeconds(server.time_left)) })}</span>
          {/if}
        </li>
      {/each}
    </ol>
    {#if server.mandatory_pit || server.inverted_grid}
      <p class="pills">
        {#if server.mandatory_pit}<span class="pill">{t("online.mandatoryPit")}</span>{/if}
        {#if server.inverted_grid}<span class="pill">{t("online.invertedGrid")}</span>{/if}
      </p>
    {/if}
  </div>
{/if}

<style>
  .timeline {
    padding: 10px 16px 12px;
    border-bottom: 1px solid var(--line);
  }
  ol {
    list-style: none;
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  li {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 5px 9px;
    border: 1px solid var(--line);
    background: var(--card);
    min-width: 0;
  }
  /* The running session: a raised frame, not red — red is kept for what the
     session retains (SPEC §7.2ter). */
  li.active {
    border-color: var(--txt2);
  }
  .kind {
    font-family: var(--mono);
    font-size: 10px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--txt2);
  }
  .dur {
    font-size: 11px;
    color: var(--txt);
  }
  .left {
    font-size: 10.5px;
    color: var(--muted);
  }
  .pills {
    display: flex;
    gap: 6px;
    margin-top: 8px;
  }
</style>
