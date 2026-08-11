<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import ConfigFields from "./ConfigFields.svelte";
  import MusicTab from "./settings/MusicTab.svelte";
  import {
    emptyConfig,
    getConfig,
    saveConfig,
    validateConfig,
    type AppConfig,
    type ConfigValidation,
  } from "$lib/config";
  import { t, setLocale, availableLocales, localeNames } from "$lib/i18n/index.svelte";
  import { setZoom, ZOOM_LEVELS } from "$lib/zoom.svelte";
  import { setSectionGuard } from "$lib/nav.svelte";
  import { listShowrooms, type ShowroomOption } from "$lib/launch";
  import { confirm } from "@tauri-apps/plugin-dialog";

  import { errorText } from "$lib/errors";

  // Découpage en onglets (§ chantier "harmonisation" — Réglages était un seul
  // écran de 250+ lignes empilant chemins, préférences et, désormais,
  // musique). Général/Chemins/Import partagent le même AppConfig + garde de
  // navigation ci-dessous ; Musique gère son propre fichier séparé
  // (`music.json`, voir MusicTab.svelte) donc son propre état.
  const tabs = [
    { id: "general", labelKey: "settings.tabGeneral" },
    { id: "paths", labelKey: "settings.tabPaths" },
    { id: "import", labelKey: "settings.tabImport" },
    { id: "music", labelKey: "settings.tabMusic" },
  ] as const;
  let activeTab = $state<(typeof tabs)[number]["id"]>("general");

  let config = $state<AppConfig>(emptyConfig());
  // Instantané du dernier config enregistré (ou chargé) : sert à détecter les
  // modifications non sauvegardées et à revenir en arrière (zoom/langue,
  // appliqués en aperçu live avant même de cliquer Enregistrer) si l'utilisateur
  // choisit d'annuler en quittant l'écran.
  let savedConfig = emptyConfig();
  let validation = $state<ConfigValidation | null>(null);
  let saving = $state(false);
  let saved = $state(false);
  let error = $state("");

  const dirty = $derived(JSON.stringify(config) !== JSON.stringify(savedConfig));

  // Scènes de showroom installées dans AC (aperçu 3D). Lues une fois au montage,
  // depuis le dossier AC **enregistré** — changer le chemin ici ne rafraîchit la
  // liste qu'au prochain passage sur l'écran.
  let installedShowrooms = $state<ShowroomOption[]>([]);
  // Une scène choisie puis désinstallée doit rester visible dans la liste,
  // sinon le select s'affiche vide et donne l'illusion d'un réglage perdu.
  const showrooms = $derived.by(() => {
    const chosen = config.prefs.showroom_scene;
    if (!chosen || installedShowrooms.some((s) => s.id === chosen)) return installedShowrooms;
    return [...installedShowrooms, { id: chosen, name: `${chosen} (?)` }];
  });

  onMount(async () => {
    config = await getConfig();
    // `config` est un objet $state (proxy) : structuredClone() dessus lève
    // DataCloneError. $state.snapshot() en tire d'abord une copie brute.
    savedConfig = structuredClone($state.snapshot(config));
    installedShowrooms = await listShowrooms().catch(() => []);
  });

  // Garde de navigation (§10bis) : quitter Réglages avec des changements non
  // enregistrés propose d'enregistrer ou d'annuler (et dans ce cas, revient
  // sur l'aperçu live déjà appliqué — zoom, langue).
  setSectionGuard(async () => {
    if (!dirty) return true;
    const wantsSave = await confirm(t("settings.unsavedPrompt"), {
      title: t("settings.unsavedTitle"),
      okLabel: t("settings.save"),
      cancelLabel: t("settings.discard"),
    });
    if (wantsSave) {
      await save();
      return validation?.is_valid ?? false;
    }
    // Annulé : revient sur tout ce qui a été appliqué en aperçu live.
    setZoom(savedConfig.prefs.ui_zoom);
    setLocale(savedConfig.prefs.language);
    config = structuredClone(savedConfig);
    return true;
  });
  onDestroy(() => setSectionGuard(null));

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
      savedConfig = structuredClone($state.snapshot(config));
    } catch (e) {
      error = errorText(e);
    } finally {
      saving = false;
    }
  }

  function onLanguageChange(value: string) {
    config.prefs.language = value || null;
    setLocale(config.prefs.language);
  }

  function onZoomChange(value: string) {
    config.prefs.ui_zoom = value ? Number(value) : null;
    setZoom(config.prefs.ui_zoom);
  }
</script>

<div class="settings">
  <header>
    <h2 class="lbl-screen">{t("settings.title")}</h2>
  </header>

  <div class="tabs">
    {#each tabs as tab}
      <button class="tab" class:on={activeTab === tab.id} type="button" onclick={() => (activeTab = tab.id)}>
        {t(tab.labelKey)}
      </button>
    {/each}
  </div>

  {#if activeTab === "music"}
    <MusicTab />
  {:else}
    {#if activeTab === "general"}
      <p class="sub">{t("settings.tabGeneralHint")}</p>

      <section class="lang-section">
        <label>
          <span>{t("settings.language")}</span>
          <select class="input" value={config.prefs.language ?? ""} onchange={(e) => onLanguageChange(e.currentTarget.value)}>
            <option value="">{t("settings.languageAuto")}</option>
            {#each availableLocales as code}
              <option value={code}>{localeNames[code]}</option>
            {/each}
          </select>
        </label>
        <p class="hint">{t("settings.languageHint")}</p>
      </section>

      <section class="lang-section">
        <label>
          <span>{t("settings.zoom")}</span>
          <select class="input" value={config.prefs.ui_zoom ?? ""} onchange={(e) => onZoomChange(e.currentTarget.value)}>
            <option value="">{t("settings.zoomDefault")}</option>
            {#each ZOOM_LEVELS as level}
              <option value={level}>{level}%</option>
            {/each}
          </select>
        </label>
        <p class="hint">{t("settings.zoomHint")}</p>
      </section>

      <section class="lang-section">
        <label>
          <span>{t("settings.bigpictureZoom")}</span>
          <select
            class="input"
            value={config.prefs.bigpicture_zoom ?? ""}
            onchange={(e) => (config.prefs.bigpicture_zoom = e.currentTarget.value ? Number(e.currentTarget.value) : null)}
          >
            <option value="">{t("settings.bigpictureZoomDefault")}</option>
            {#each ZOOM_LEVELS as level}
              <option value={level}>{level}%</option>
            {/each}
          </select>
        </label>
        <p class="hint">{t("settings.bigpictureZoomHint")}</p>
      </section>

      <section class="lang-section">
        <label>
          <span>{t("settings.showroomScene")}</span>
          <select
            class="input"
            value={config.prefs.showroom_scene ?? ""}
            onchange={(e) => (config.prefs.showroom_scene = e.currentTarget.value || null)}
          >
            <option value="">{t("settings.showroomSceneDefault")}</option>
            {#each showrooms as s (s.id)}
              <option value={s.id}>{s.name}</option>
            {/each}
          </select>
        </label>
        <p class="hint">{t("settings.showroomSceneHint")}</p>
      </section>
    {:else if activeTab === "paths"}
      <p class="sub">{t("settings.tabPathsHint")}</p>
      <ConfigFields bind:config {validation} />
    {:else if activeTab === "import"}
      <p class="sub">{t("settings.tabImportHint")}</p>

      <section class="lang-section">
        <label>
          <span>{t("settings.resourceExtraction")}</span>
          <select class="input" bind:value={config.prefs.resource_extraction_mode}>
            <option value="none">{t("settings.resourceExtractionNone")}</option>
            <option value="info_only">{t("settings.resourceExtractionInfo")}</option>
            <option value="all">{t("settings.resourceExtractionAll")}</option>
          </select>
        </label>
        <p class="hint">{t("settings.resourceExtractionHint")}</p>
      </section>

      <section class="lang-section">
        <label class="check">
          <input type="checkbox" bind:checked={config.prefs.keep_source_archive} />
          <span>{t("settings.keepSourceArchive")}</span>
        </label>
        <p class="hint">{t("settings.keepSourceArchiveHint")}</p>
      </section>
    {/if}

    {#if error}<div class="error">{error}</div>{/if}

    <footer>
      {#if saved}<span class="pill pill-ok">{t("settings.saved")}</span>{/if}
      <button
        class="btn btn-primary"
        type="button"
        onclick={save}
        disabled={!validation?.is_valid || saving}
      >
        {saving ? t("settings.saving") : t("settings.save")}
      </button>
    </footer>
  {/if}
</div>

<style>
  .settings {
    max-width: 640px;
  }
  header {
    margin-bottom: 22px;
  }
  .sub {
    color: var(--muted);
    margin-top: 6px;
    line-height: 1.5;
  }
  .tabs {
    display: flex;
    gap: 1px;
    background: var(--line);
    border: 1px solid var(--line);
    margin-bottom: 20px;
  }
  .tab {
    flex: 1;
    background: var(--bg);
    color: var(--muted);
    padding: 9px 10px;
    font-size: 11px;
    letter-spacing: 0.5px;
  }
  .tab:hover {
    background: var(--raised);
    color: var(--txt);
  }
  .tab.on {
    background: var(--raised);
    color: var(--rosso-bright);
  }
  .lang-section {
    margin-bottom: 22px;
    padding-bottom: 18px;
    border-bottom: 1px solid var(--line);
  }
  .lang-section label {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 12px;
    color: var(--txt2);
    max-width: 260px;
  }
  .lang-section label.check {
    flex-direction: row;
    align-items: center;
    gap: 8px;
    max-width: none;
    cursor: pointer;
  }
  .lang-section .hint {
    margin-top: 8px;
    font-size: 11px;
    color: var(--faint);
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
