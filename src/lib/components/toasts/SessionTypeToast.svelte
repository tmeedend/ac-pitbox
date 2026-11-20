<script lang="ts">
  // Opponents sent from the library to a session that had no grid (SPEC §7.4):
  // the session became a Race, and this says so, with the way back. Gone as
  // soon as a type is picked by hand, the way back included.
  import Toast from "./Toast.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { dismissTypeSwitch, revertTypeSwitch, typeSwitchedFrom } from "$lib/shell/sessionNav.svelte";

  const from = $derived(typeSwitchedFrom());
</script>

{#if from}
  <Toast title={t("launch.switchedToRace")} tone="info" onclose={dismissTypeSwitch}>
    {#snippet actions()}
      <button class="btn" type="button" onclick={revertTypeSwitch}>
        {t("launch.switchBack", { type: t(`launch.type.${from}`) })}
      </button>
    {/snippet}
  </Toast>
{/if}
