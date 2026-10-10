<script lang="ts">
  // The rev slider and the throttle-blip button of an engine audition (FMOD§2.4),
  // shared by the car's "Engine sound" card and the sound mod's own page.
  //
  // Renders nothing unless the game's real engine is playing AND its event
  // exposes a recognisable rev parameter: the Web Audio fallback plays a frozen
  // sample, there is nothing to tune. The range comes from **this** car's power
  // curve, hence an F1 going up to 19,500 and a diesel van stopping at 5,000.
  //
  // The root carries the `rev` class so that each host can place the row
  // (`:global(.rev)`) — only the spacing differs between the two.
  import Slider from "$lib/components/ui/Slider.svelte";
  import {
    engineControls,
    engineRev,
    engineShowcase,
    setEnginePedal,
    setEngineRev,
    setEngineShowcase,
  } from "$lib/detail/enginePlayer.svelte";
  import { t } from "$lib/i18n/index.svelte";

  const revControls = $derived.by(() => {
    const c = engineControls();
    return c && c.revParam ? c : null;
  });
</script>

{#if revControls}
  <div class="rev">
    <!-- The slider steps aside during the showcase: both drive the same
         parameter, and a slider that does not follow what is heard would be
         worse than none. -->
    {#if !engineShowcase()}
      <Slider
        compact
        label={t("detail.soundRev")}
        min={revControls.revFloor}
        max={revControls.revCeiling}
        step={50}
        value={engineRev()}
        display={t("detail.soundRevValue", { rpm: Math.round(engineRev()).toLocaleString() })}
        oninput={setEngineRev}
        onpress={() => setEnginePedal(true)}
        onrelease={() => setEnginePedal(false)}
      />
    {/if}
    <button class="blip" class:on={engineShowcase()} type="button" onclick={() => setEngineShowcase(!engineShowcase())}>
      {engineShowcase() ? t("detail.soundBlipStop") : t("detail.soundBlip")}
    </button>
  </div>
{/if}

<style>
  .rev {
    display: flex;
    align-items: flex-end;
    gap: 12px;
  }

  /* The slider takes the remaining room; the button keeps its own. */
  .rev :global(.slider) {
    flex: 1;
    min-width: 0;
  }

  .blip {
    flex: 0 0 auto;
    margin-left: auto;
    padding: 6px 12px;
    border: 1px solid var(--rosso-border);
    border-radius: 4px;
    background: var(--rosso-dim);
    color: var(--txt);
    font-size: 11.5px;
    cursor: pointer;
  }

  .blip:hover {
    border-color: var(--rosso-bright);
  }

  .blip.on {
    background: var(--rosso-bright);
    border-color: var(--rosso-bright);
    color: #fff;
  }
</style>
