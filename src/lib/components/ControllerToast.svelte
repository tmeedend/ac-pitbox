<script lang="ts">
  // "New device detected" (§7.4), in the bottom-right notification stack.
  //
  // It used to be a full-width banner above the active screen. Same intent —
  // never interrupt, no deferral logic needed (AC running, import under way) —
  // but it pushed the whole screen down for a notice nobody answers right now,
  // and it was the one message of the app that did not look like the others.
  // What made the banner work is kept: it never fades on its own, so someone
  // who did not have time to read it still has a path to the setup panel.
  import Toast from "./Toast.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import {
    controllers,
    bannerVisible,
    pendingDevices,
    openControllerSetup,
  } from "$lib/gamepadDevices.svelte";

  const pendingCount = $derived(pendingDevices().length);
</script>

{#if bannerVisible()}
  <Toast
    tone="info"
    icon="🎮"
    title={pendingCount > 1 ? t("controller.banner.many", { n: pendingCount }) : t("controller.banner.one")}
    onclose={() => (controllers.bannerDismissed = true)}
    closeLabel={t("controller.banner.later")}
  >
    {#snippet actions()}
      <button class="btn" type="button" onclick={() => openControllerSetup()}>
        {t("controller.banner.configure")}
      </button>
    {/snippet}
  </Toast>
{/if}
