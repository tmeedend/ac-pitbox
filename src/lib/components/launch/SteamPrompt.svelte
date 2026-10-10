<script lang="ts">
  // The "start Steam first" modal in front of a launch (SESSION§2.3). A
  // blocking dialog rather than a line in the banner, because there is a
  // gesture to make outside the app and a check to redo after it — a passive
  // text would leave the user relaunching into the void. All its state lives
  // in the sequence, which also decides when it opens.
  import { t } from "$lib/i18n/index.svelte";
  import type { LaunchSequence } from "$lib/launch/launchSequence.svelte";

  let { sequence }: { sequence: LaunchSequence } = $props();
</script>

{#if sequence.steamPromptOpen}
  <div class="backdrop">
    <div class="modal">
      <h2>{t("launch.steamRequiredTitle")}</h2>
      <p>{t("launch.steamRequiredBody")}</p>
      {#if sequence.steamStillMissing}
        <p class="steam-missing">{t("launch.steamStillMissing")}</p>
      {/if}
      <div class="steam-actions">
        <button class="btn btn-ghost" type="button" onclick={() => sequence.dismissSteamPrompt()}>
          {t("common.cancel")}
        </button>
        <button
          class="btn btn-primary"
          type="button"
          disabled={sequence.steamChecking}
          onclick={() => void sequence.confirmSteamStarted()}
        >
          {sequence.steamChecking ? t("common.working") : t("launch.steamStarted")}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  /* Same visual language as `NamedListDialog`; component CSS being scoped, it
     is copied rather than inherited. */
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }
  .modal {
    width: 420px;
    max-width: calc(92 * var(--vw));
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 16px;
    background: var(--panel);
    border: 1px solid var(--rosso);
  }
  .modal h2 {
    font-size: 13px;
    letter-spacing: 0.5px;
    text-transform: uppercase;
    color: var(--txt2);
  }
  .modal p {
    font-size: 12px;
    line-height: 1.5;
    color: var(--txt2);
  }
  .steam-missing {
    color: var(--yellow);
  }
  .steam-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
</style>
