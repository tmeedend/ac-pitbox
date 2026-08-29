<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import ConfigFields from "./ConfigFields.svelte";
  import Tabs from "./Tabs.svelte";
  import MusicTab from "./settings/MusicTab.svelte";
  import PreviewTab from "./settings/PreviewTab.svelte";
  import {
    emptyConfig,
    getConfig,
    saveConfig,
    validateConfig,
    type AppConfig,
    type ConfigValidation,
  } from "$lib/config";
  import { t, setLocale, availableLocales, localeNames } from "$lib/i18n/index.svelte";
  import { ZOOM_LEVELS } from "$lib/zoom.svelte";
  import { applyZoomFor } from "$lib/bigpicture.svelte";
  import { nav, setSectionGuard } from "$lib/nav.svelte";
  import { preview3dDirty, revertPreview3dPrefs, savePreview3dPrefs } from "$lib/preview3dPrefs.svelte";
  import { listShowrooms, type ShowroomOption } from "$lib/launch";
  import { confirm } from "@tauri-apps/plugin-dialog";
  import { resolveProfile, type ProfileSource } from "$lib/gamepadNav";
  import {
    controllers,
    deviceRecords,
    forgetDevice,
    gamepadEnabled,
    openControllerSetup,
    setDeviceUse,
    setGamepadEnabled,
  } from "$lib/gamepadDevices.svelte";

  import { errorText } from "$lib/errors";

  // Découpage en onglets (§ chantier "harmonisation" — Réglages était un seul
  // écran de 250+ lignes empilant chemins, préférences et, désormais,
  // musique). Général/Chemins/Import partagent le même AppConfig + garde de
  // navigation ci-dessous ; Musique gère son propre fichier séparé
  // (`music.json`, voir MusicTab.svelte) donc son propre état.
  const TAB_IDS = ["general", "paths", "import", "preview", "music"] as const;
  type SettingsTab = (typeof TAB_IDS)[number];
  // Onglet demandé depuis ailleurs (raccourci « régler l'aperçu » de la fiche
  // voiture), consommé une fois : sans la remise à `null`, revenir plus tard
  // dans les Réglages rouvrirait toujours le même onglet.
  const requested = TAB_IDS.find((id) => id === nav.settingsTab);
  nav.settingsTab = null;
  let activeTab = $state<SettingsTab>(requested ?? "general");
  // Libellés recalculés à chaque changement de langue (`t` est réactif) — un
  // tableau `const` de clés figées les aurait laissés dans l'ancienne langue.
  const tabItems = $derived(
    TAB_IDS.map((id) => ({ id, label: t(`settings.tab${id[0].toUpperCase()}${id.slice(1)}`) })),
  );

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

  // Périphérique de contrôle (§7.4) : réglages indépendants de `config`
  // (persistés via ui_prefs.json, pas config.json) donc hors du flux
  // dirty/Enregistrer ci-dessus — ils s'appliquent tout de suite, comme les
  // autres réglages de `uiPrefs.svelte.ts`.
  //
  // Réglages ne garde que ce qui est rattrapable : le coupe-circuit global, la
  // liste des périphériques connus (débranchés compris — le label est
  // mémorisé) et de quoi revenir sur une réponse. Le tableau de diagnostic en
  // direct, lui, vit dans le panneau, replié sous « Détails techniques » : il
  // ne sert qu'au cas où la calibration échoue.
  const sourceLabels: Record<ProfileSource, string> = {
    calibrated: "controller.settings.sourceCalibrated",
    override: "controller.settings.sourceOverride",
    standard: "controller.settings.sourceStandard",
    none: "controller.settings.sourceNone",
  };

  const knownDevices = $derived.by(() => {
    const live = new Map(controllers.live.map((d) => [d.key, d]));
    return Object.values(deviceRecords()).map((r) => {
      const d = live.get(r.key);
      // `label` est le `Gamepad.id` brut : la résolution du profil marche donc
      // aussi pour un périphérique débranché (le mapping, lui, reste inconnu
      // tant qu'il n'est pas là — on ne prétend pas qu'il est standard).
      return {
        ...r,
        connected: !!d,
        source: resolveProfile({ id: r.label, mapping: d?.mapping ?? "" }, r).source,
      };
    });
  });

  // Garde de navigation (§10bis) : quitter Réglages avec des changements non
  // enregistrés propose d'enregistrer ou d'annuler (et dans ce cas, revient
  // sur l'aperçu live déjà appliqué — zoom, langue).
  setSectionGuard(async () => {
    // **Deux jeux de réglages, une seule garde.** L'onglet Aperçu ne passe pas
    // par `config` (il vit dans `ui_prefs.json`) mais il a exactement le même
    // besoin : ses curseurs s'appliquent à l'écran sans être enregistrés, donc
    // quitter sans demander perdrait — ou pire, validerait en silence — ce que
    // l'utilisateur était en train d'essayer. Une seconde garde n'est pas
    // possible, `setSectionGuard` n'a qu'un emplacement : c'est donc celle-ci
    // qui interroge les deux.
    const previewDirty = preview3dDirty();
    if (!dirty && !previewDirty) return true;
    const wantsSave = await confirm(t("settings.unsavedPrompt"), {
      title: t("settings.unsavedTitle"),
      okLabel: t("settings.save"),
      cancelLabel: t("settings.discard"),
    });
    if (wantsSave) {
      if (previewDirty) await savePreview3dPrefs();
      if (!dirty) return true;
      await save();
      return validation?.is_valid ?? false;
    }
    // Annulé : revient sur tout ce qui a été appliqué en aperçu live.
    if (previewDirty) revertPreview3dPrefs();
    applyZoomFor(savedConfig.prefs);
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
    applyZoomFor(config.prefs);
  }

  /** Live preview too, and that is the whole point: this zoom is only ever
   * judged from inside Big Picture, and until now it applied on the NEXT entry
   * - so, for anyone setting it from the mode itself, on nothing at all. */
  function onBigPictureZoomChange(value: string) {
    config.prefs.bigpicture_zoom = value ? Number(value) : null;
    applyZoomFor(config.prefs);
  }
</script>

<div class="settings" class:wide={activeTab === "preview"}>
  <header>
    <h2 class="lbl-screen">{t("settings.title")}</h2>
  </header>

  <Tabs tabs={tabItems} active={activeTab} onselect={(v) => (activeTab = v as SettingsTab)} />

  {#if activeTab === "music"}
    <MusicTab />
  {:else if activeTab === "preview"}
    <!-- Réglages appliqués tout de suite, donc pas de garde de navigation :
         celle-ci ne porte que sur AppConfig. L'onglet a son propre bouton
         Enregistrer (écriture disque différée, voir PreviewTab). -->
    <p class="sub">{t("settings.tabPreviewHint")}</p>
    <PreviewTab />
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
            onchange={(e) => onBigPictureZoomChange(e.currentTarget.value)}
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

      <section class="lang-section">
        <label class="check">
          <input
            type="checkbox"
            checked={gamepadEnabled()}
            onchange={(e) => setGamepadEnabled(e.currentTarget.checked)}
          />
          <span>{t("controller.settings.enabled")}</span>
        </label>
        <p class="hint">{t("controller.settings.enabledHint")}</p>

        <div class="lbl device-lbl">{t("controller.settings.known")}</div>
        {#if knownDevices.length}
          <div class="devices">
            {#each knownDevices as d (d.key)}
              <div class="device">
                <div class="dev-b">
                  <div class="dev-name" class:off={!d.connected}>
                    {d.label}{#if !d.connected}<span class="dim"> ({t("controller.settings.disconnected")})</span>{/if}
                  </div>
                  <div class="dev-id mono">{d.key} · {t(sourceLabels[d.source])}</div>
                </div>
                <button class="btn" type="button" onclick={() => setDeviceUse(d.key, d.label, !d.use)}>
                  {d.use ? t("controller.settings.used") : t("controller.settings.unused")}
                </button>
                <button class="btn" type="button" disabled={!d.connected} onclick={() => openControllerSetup(d.key)}>
                  {t("controller.settings.calibrate")}
                </button>
                <button class="btn" type="button" onclick={() => forgetDevice(d.key)}>
                  {t("controller.settings.forget")}
                </button>
              </div>
            {/each}
          </div>
        {:else}
          <p class="hint">{t("controller.settings.noneKnown")}</p>
        {/if}
        <button class="btn open-setup" type="button" onclick={() => openControllerSetup()}>
          {t("controller.settings.open")}
        </button>
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
  /* L'onglet Aperçu prend toute la largeur : il porte un aperçu 3D et treize
     curseurs, et le but est de voir l'effet d'un réglage **sans scroller**.
     Les autres onglets restent en colonne étroite — un formulaire large est
     plus difficile à lire, pas plus facile. */
  .settings.wide {
    max-width: none;
  }
  header {
    margin-bottom: 22px;
  }
  /* 12px comme les autres sous-titres d'écran (voir Profiles.svelte). */
  .sub {
    color: var(--muted);
    font-size: 12px;
    margin-top: 6px;
    line-height: 1.5;
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
  .device-lbl {
    margin-top: 16px;
    margin-bottom: 8px;
  }
  .devices {
    border: 1px solid var(--line);
  }
  .device {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    background: var(--panel2);
  }
  .device + .device {
    border-top: 1px solid var(--line);
  }
  .dev-b {
    flex: 1;
    min-width: 0;
  }
  .dev-name {
    font-size: 11.5px;
    color: var(--txt);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  /* Débranché : le périphérique reste listé (son label est mémorisé), grisé
     pour qu'on ne le confonde pas avec ce qui est là maintenant. */
  .dev-name.off {
    color: var(--muted);
  }
  .dim {
    color: var(--faint);
  }
  .dev-id {
    font-size: 9px;
    color: var(--faint);
    margin-top: 2px;
  }
  .open-setup {
    margin-top: 12px;
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
