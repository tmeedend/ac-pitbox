<script lang="ts">
  // The sign of a value the user decided over what the rules compute: the
  // curation flag (`⚑`, the one of the taxonomy tabs) and, when there is a way
  // back, a `↺` that goes to the computed value. Its tooltip says which one.
  //
  // Content-agnostic, like the other bricks of `ui/`: what is decided, and what
  // it goes back to, is the caller's (`titles` are already worded).
  interface Props {
    /** Tooltip of the flag. */
    flagTitle: string;
    /** Tooltip of the `↺`: the value one goes back to. */
    resetTitle?: string;
    /** Absent: the mark shows the flag and offers no way back. */
    onreset?: () => void;
  }
  const { flagTitle, resetTitle, onreset }: Props = $props();
</script>

<span class="mark">
  <span class="flag" title={flagTitle}>⚑</span>
  {#if onreset}
    <button class="rev" type="button" title={resetTitle} aria-label={resetTitle} onclick={onreset}>↺</button>
  {/if}
</span>

<style>
  .mark {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    flex: none;
  }
  .flag {
    color: var(--rosso-border);
    font-size: 10px;
    line-height: 1;
  }
  .rev {
    background: transparent;
    border: none;
    color: var(--muted);
    font-size: 12px;
    line-height: 1;
    padding: 0 2px;
    cursor: pointer;
  }
  .rev:hover,
  .rev:focus-visible {
    color: var(--rosso-bright);
  }
</style>
