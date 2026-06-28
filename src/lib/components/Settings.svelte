<script lang="ts">
  import { onMount } from "svelte";
  import ConfigFields from "./ConfigFields.svelte";
  import {
    emptyConfig,
    getConfig,
    saveConfig,
    validateConfig,
    type AppConfig,
    type ConfigValidation,
  } from "$lib/config";

  let config = $state<AppConfig>(emptyConfig());
  let validation = $state<ConfigValidation | null>(null);
  let saving = $state(false);
  let saved = $state(false);
  let error = $state("");

  onMount(async () => {
    config = await getConfig();
  });

  $effect(() => {
    JSON.stringify(config);
    saved = false;
    const t = setTimeout(async () => {
      validation = await validateConfig(config);
    }, 250);
    return () => clearTimeout(t);
  });

  async function save() {
    if (!validation?.is_valid) return;
    saving = true;
    error = "";
    try {
      await saveConfig(config);
      saved = true;
    } catch (e) {
      error = String(e);
    } finally {
      saving = false;
    }
  }
</script>

<div class="settings">
  <header>
    <h2>Réglages — Chemins</h2>
    <p class="sub">Configuration de l'environnement (§12). Bibliothèque, overlay et règles sont des bases distinctes.</p>
  </header>

  <ConfigFields bind:config {validation} />

  {#if error}<div class="error">{error}</div>{/if}

  <footer>
    {#if saved}<span class="pill pill-ok">Enregistré</span>{/if}
    <button
      class="btn btn-primary"
      type="button"
      onclick={save}
      disabled={!validation?.is_valid || saving}
    >
      {saving ? "Enregistrement…" : "Enregistrer"}
    </button>
  </footer>
</div>

<style>
  .settings {
    max-width: 640px;
  }
  header {
    margin-bottom: 22px;
  }
  h2 {
    font-size: 15px;
    font-weight: 600;
    letter-spacing: 0.5px;
  }
  .sub {
    color: var(--muted);
    margin-top: 6px;
    line-height: 1.5;
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
    align-items: center;
    justify-content: flex-end;
    gap: 12px;
  }
</style>
