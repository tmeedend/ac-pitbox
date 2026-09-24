<script lang="ts">
  // The progress bar of the notification stack: import, general repair, mod
  // update download.
  //
  // It was written three times, once per toast, with the same height and the
  // same red — and the third copy (the update download) already drifted: its
  // own "waiting" animation, no phase line. Three things that say "working on
  // it" in the same corner must look the same, so the bar lives here once.
  //
  // Knows nothing of what it measures: a ratio, or `null` for "moving, but
  // how far is not known" — the import while it sizes its batch, a download
  // whose server announced no length. No invented percentage in that case.
  import type { Snippet } from "svelte";

  interface Props {
    /** In [0, 1], or `null` when progress cannot be measured. */
    ratio: number | null;
    /** Current step, shown above the bar ("EXTRACTION", "DOWNLOAD"). */
    phase?: string;
    /** Right of the phase: what is being done, a count, a size. */
    detail?: Snippet;
    /** Secondary bar (a whole batch under its current item): thinner, so the
     * immediate information stays the one that stands out. */
    thin?: boolean;
  }
  const { ratio, phase, detail, thin = false }: Props = $props();

  const width = $derived(ratio == null ? undefined : `${Math.min(1, Math.max(0, ratio)) * 100}%`);
</script>

{#if phase || detail}
  <div class="row">
    {#if phase}<span class="mono phase">{phase}</span>{/if}
    {#if detail}<span class="detail">{@render detail()}</span>{/if}
  </div>
{/if}
<div class="bar" class:thin>
  <div class="fill" class:indeterminate={ratio == null} style:width></div>
</div>

<style>
  .row {
    display: flex;
    align-items: baseline;
    gap: 6px;
    margin-bottom: 6px;
    min-width: 0;
  }
  .phase {
    flex: none;
    color: var(--rosso-bright);
    font-size: 10px;
    text-transform: uppercase;
  }
  .detail {
    color: var(--muted);
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .bar {
    height: 4px;
    background: var(--line);
    overflow: hidden;
  }
  .bar.thin {
    height: 2px;
  }
  .fill {
    height: 100%;
    background: var(--rosso);
    transition: width 0.2s;
  }
  /* A segment going back and forth: "it is moving", without a figure. */
  .fill.indeterminate {
    width: 30%;
    animation: slide 1s ease-in-out infinite;
  }
  @keyframes slide {
    0% {
      margin-left: 0;
    }
    50% {
      margin-left: 70%;
    }
    100% {
      margin-left: 0;
    }
  }
</style>
