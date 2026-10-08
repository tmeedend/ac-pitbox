<script lang="ts">
  import { onMount, tick } from "svelte";
  import SetupWizard from "$lib/components/shell/SetupWizard.svelte";
  import AppShell from "$lib/components/shell/AppShell.svelte";
  import { getConfig, validateConfig } from "$lib/config";
  import { restoreStartScreen } from "$lib/shell/nav.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { applyStartupPrefs, dismissBootScreen } from "$lib/shell/boot";

  type View = "loading" | "wizard" | "app";
  let view = $state<View>("loading");

  // The boot screen of `app.html` covers all of this (§13): language and zoom
  // are applied before the shell is mounted, so its first frame is the final
  // one, and the boot screen only goes once that frame is painted. A failure
  // lets it go too - the "loading" line below beats a logo that never ends.
  onMount(async () => {
    try {
      const cfg = await getConfig();
      applyStartupPrefs(cfg.prefs);
      const v = await validateConfig(cfg);
      // Only for a configured app: one coming out of the wizard starts on the
      // car library, as every start did before.
      if (v.is_valid) await restoreStartScreen();
      view = v.is_valid ? "app" : "wizard";
      await tick();
    } finally {
      dismissBootScreen();
    }
  });
</script>

{#if view === "loading"}
  <div class="loading">{t("common.loading")}</div>
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
