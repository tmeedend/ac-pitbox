<script lang="ts">
  import { onMount } from "svelte";
  import SetupWizard from "$lib/components/SetupWizard.svelte";
  import AppShell from "$lib/components/AppShell.svelte";
  import { getConfig, validateConfig } from "$lib/config";

  type View = "loading" | "wizard" | "app";
  let view = $state<View>("loading");

  onMount(async () => {
    const cfg = await getConfig();
    const v = await validateConfig(cfg);
    view = v.is_valid ? "app" : "wizard";
  });
</script>

{#if view === "loading"}
  <div class="loading">Chargement…</div>
{:else if view === "wizard"}
  <SetupWizard ondone={() => (view = "app")} />
{:else}
  <AppShell />
{/if}

<style>
  .loading {
    min-height: 100vh;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--muted);
    font-size: 13px;
  }
</style>
