<script lang="ts">
  // "Layers back", in the notification stack (SPEC-play-online.md, "Couches
  // et versions": the game's end reactivates them "et le dit dans une
  // notification"). Sent by the backend when it gives back the layers an
  // online session set aside — at the game's end, or at the next start when
  // Pit Box was closed meanwhile; the screen does not have to be open.
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import Toast from "./Toast.svelte";
  import { t } from "$lib/i18n/index.svelte";

  let restored = $state<string[]>([]);

  onMount(() => {
    const stop = listen<string[]>("online://layers-restored", (e) => {
      restored = [...restored, ...e.payload];
    });
    return () => void stop.then((unlisten) => unlisten());
  });
</script>

{#if restored.length}
  <Toast title={t("online.layersRestored", { count: restored.length })} onclose={() => (restored = [])}>
    <p class="names">{restored.join(", ")}</p>
  </Toast>
{/if}

<style>
  .names {
    font-size: 12px;
    color: var(--txt2);
  }
</style>
