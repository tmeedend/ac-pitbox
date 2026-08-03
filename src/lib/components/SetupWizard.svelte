<script lang="ts">
  import { onMount } from "svelte";
  import ConfigFields from "./ConfigFields.svelte";
  import {
    autodetectPaths,
    emptyConfig,
    getConfig,
    saveConfig,
    validateConfig,
    type AppConfig,
    type ConfigValidation,
  } from "$lib/config";
  import { t } from "$lib/i18n/index.svelte";

  import { errorText } from "$lib/errors";
  interface Props {
    ondone: () => void;
  }
  let { ondone }: Props = $props();

  let config = $state<AppConfig>(emptyConfig());
  let validation = $state<ConfigValidation | null>(null);
  let detecting = $state(false);
  let saving = $state(false);
  let error = $state("");

  // Remplit uniquement les champs vides avec les valeurs détectées.
  async function detect() {
    detecting = true;
    try {
      const d = await autodetectPaths();
      config.ac_install_path ||= d.ac_install_path;
      config.content_manager_exe ||= d.content_manager_exe;
      config.sevenzip_exe ||= d.sevenzip_exe;
    } finally {
      detecting = false;
    }
  }

  onMount(async () => {
    config = await getConfig();
    await detect();
  });

  // Validation à la volée (anti-rebond).
  $effect(() => {
    JSON.stringify(config); // dépendance réactive
    const timer = setTimeout(async () => {
      validation = await validateConfig(config);
    }, 250);
    return () => clearTimeout(timer);
  });

  async function finish() {
    if (!validation?.is_valid) return;
    saving = true;
    error = "";
    try {
      await saveConfig(config);
      ondone();
    } catch (e) {
      error = errorText(e);
    } finally {
      saving = false;
    }
  }
</script>

<div class="wizard">
  <div class="frame">
    <div class="topbar"></div>
    <div class="inner">
      <header>
        <div class="logo"><span>PB</span></div>
        <div>
          <h1>Pit Box</h1>
          <p class="sub">{t("setup.title")}</p>
        </div>
      </header>

      <p class="intro">{t("setup.intro")}</p>

      <div class="toolbar">
        <button class="btn" type="button" onclick={detect} disabled={detecting}>
          {detecting ? t("setup.detecting") : t("setup.detect")}
        </button>
        {#if validation}
          <span class="pill {validation.is_valid ? 'pill-ok' : 'pill-err'}">
            {validation.is_valid ? t("setup.validConfig") : t("setup.missingPaths")}
          </span>
        {/if}
      </div>

      <ConfigFields bind:config {validation} />

      {#if error}
        <div class="error">{error}</div>
      {/if}

      <footer>
        <button
          class="btn btn-primary"
          type="button"
          onclick={finish}
          disabled={!validation?.is_valid || saving}
        >
          {saving ? t("settings.saving") : t("setup.finish")}
        </button>
      </footer>
    </div>
  </div>
</div>

<style>
  .wizard {
    min-height: 100vh;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 28px 16px;
  }
  .frame {
    width: 100%;
    max-width: 680px;
    background: var(--panel);
    border: 1px solid var(--rosso);
  }
  .topbar {
    background: var(--rosso);
    height: 3px;
  }
  .inner {
    padding: 26px 30px 30px;
  }
  header {
    display: flex;
    align-items: center;
    gap: 14px;
    margin-bottom: 18px;
  }
  .logo {
    width: 38px;
    height: 38px;
    background: var(--rosso);
    display: flex;
    align-items: center;
    justify-content: center;
    transform: skewX(-8deg);
  }
  .logo span {
    transform: skewX(8deg);
    color: #fff;
    font-weight: 700;
    font-size: 14px;
    font-style: italic;
  }
  h1 {
    font-size: 17px;
    font-weight: 600;
    letter-spacing: 1.5px;
    font-style: italic;
  }
  .sub {
    color: var(--rosso-bright);
    font-size: 9px;
    letter-spacing: 3px;
    text-transform: uppercase;
    margin-top: 3px;
  }
  .intro {
    color: var(--muted);
    line-height: 1.6;
    margin-bottom: 18px;
  }
  .toolbar {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-bottom: 22px;
  }
  .error {
    margin: 12px 0;
    padding: 10px;
    background: var(--rosso-dim);
    border: 1px solid var(--rosso-border);
    color: var(--rosso-bright);
    font-size: 12px;
  }
  footer {
    margin-top: 24px;
    padding-top: 18px;
    border-top: 1px solid var(--line);
    display: flex;
    justify-content: flex-end;
  }
</style>
