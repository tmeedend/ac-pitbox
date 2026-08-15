<script lang="ts">
  // Bloc « Type de session » de l'écran Lancement (§8.4) : bascule
  // Practice/Hotlap/Course. Vue de présentation pure — le changement de type
  // déclenche la sauvegarde du preset courant puis l'application de celui de
  // la cible (§8.4bis), orchestré par Launch.svelte via `onselect`.
  import type { SessionType } from "$lib/launch";
  import { t } from "$lib/i18n/index.svelte";

  let { sessionType, onselect }: { sessionType: SessionType; onselect: (type: SessionType) => void } = $props();

  const sessionTypes: { id: SessionType; labelKey: string }[] = [
    { id: "practice", labelKey: "launch.typePractice" },
    { id: "hotlap", labelKey: "launch.typeHotlap" },
    { id: "race", labelKey: "launch.typeRace" },
    { id: "trackday", labelKey: "launch.typeTrackday" },
  ];
</script>

<section class="blk">
  <header class="blk-h"><span class="blk-t">{t("launch.sessionTypeLabel")}</span></header>
  <div class="blk-b">
    <div class="seg types">
      {#each sessionTypes as st}
        <button class:on={sessionType === st.id} onclick={() => onselect(st.id)}>{t(st.labelKey)}</button>
      {/each}
    </div>
  </div>
</section>

<style>
  .seg,
  .types {
    display: flex;
    border: 1px solid var(--line);
    width: fit-content;
    margin-bottom: 16px;
  }
  .seg button {
    background: var(--panel2);
    color: var(--muted);
    padding: 9px 26px;
    font-size: 12px;
    letter-spacing: 1px;
    border-right: 1px solid var(--line);
  }
  .seg button:last-child {
    border-right: none;
  }
  .seg button.on {
    background: var(--rosso);
    color: #fff;
  }
</style>
