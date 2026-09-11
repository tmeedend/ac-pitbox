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
  import { i18n, t } from "$lib/i18n/index.svelte";
  import { listLibrary } from "$lib/library";
  import { savePreview3dPrefs, setPreview3dEnabled, setPreview3dValue } from "$lib/preview3dPrefs.svelte";
  import { saveGridThumbPrefs, setGridThumbsEnabled } from "$lib/gridThumbPrefs.svelte";
  import { FEATURE_GRID_THUMBS } from "$lib/features";

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
      config.library_path ||= d.library_path;
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
      // Sans les vignettes régénérées, la page des profils n'a plus de question
      // à poser : ce qu'elle réglait d'autre — aperçu 3D, plafond du cache — a
      // des défauts qui **sont** ceux du profil « Normal » qu'elle
      // présélectionnait (`features.ts`).
      if (!FEATURE_GRID_THUMBS) {
        ondone();
        return;
      }
      // Le contenu de base est indexé par `save_config` : c'est seulement
      // maintenant qu'on sait combien de voitures cette installation contient,
      // et le choix de profil ne se pose **qu'avec ce chiffre** (§5.5).
      carCount = (await listLibrary().catch(() => [])).filter((c) => c.kind === "Car").length;
      step = "profile";
    } catch (e) {
      error = errorText(e);
    } finally {
      saving = false;
    }
  }

  // --- Profil de rendu (docs/SPEC-grille.md §5.5) ---------------------------
  //
  // **Deux écrans et non un**, parce que le premier ne peut pas porter le
  // second : les profils se présentent « avec les chiffres de la bibliothèque
  // réellement scannée, pas dans l'abstrait », et cette bibliothèque n'existe
  // qu'une fois les chemins enregistrés. Un « 0 voiture détectée » aurait vidé
  // l'écran de ce qui lui donne son sens.
  //
  // Quatre exigences du §5.5, toutes tenues ici : nommer par le résultat (et
  // non « optimiser l'espace disque », qui décrit un moyen), chiffrer sur sa
  // propre bibliothèque, présélectionner le milieu, et dire que c'est
  // modifiable — cette dernière phrase transforme une décision en préférence et
  // divise le poids ressenti de l'écran.
  type Step = "paths" | "profile";
  const PROFILES = ["light", "normal", "rich"] as const;
  type Profile = (typeof PROFILES)[number];

  let step = $state<Step>("paths");
  let carCount = $state(0);
  let profile = $state<Profile>("normal");

  /** Poids du magasin de vignettes, en mégaoctets : ~150 Ko par voiture (§5.2). */
  const thumbsMb = $derived(Math.max(1, Math.round((carCount * 150) / 1024)));

  function count(n: number): string {
    return n.toLocaleString(i18n.locale);
  }

  async function applyProfile() {
    saving = true;
    try {
      setPreview3dEnabled(profile !== "light");
      setPreview3dValue("cacheMb", profile === "rich" ? 4096 : 2048);
      setGridThumbsEnabled(profile === "rich");
      await Promise.all([savePreview3dPrefs(), saveGridThumbPrefs()]);
    } catch (e) {
      // Un profil non écrit n'empêche pas d'entrer dans l'app : les défauts
      // s'appliquent, et l'écran de réglages les reprendra.
      console.error("setup: profil de rendu non enregistré", e);
    } finally {
      saving = false;
    }
    ondone();
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

      {#if step === "profile"}
        <p class="intro">{t("setup.profileIntro", { count: count(carCount) })}</p>

        <div class="profiles">
          {#each PROFILES as id (id)}
            <label class="profile" class:on={profile === id}>
              <input type="radio" name="setup_profile" value={id} bind:group={profile} />
              <span class="p-body">
                <span class="p-name">{t("setup.profile_" + id)}</span>
                <span class="p-what">{t("setup.profileWhat_" + id)}</span>
                <span class="p-cost">
                  {id === "light"
                    ? t("setup.profileCost_light")
                    : id === "normal"
                      ? t("setup.profileCost_normal")
                      : t("setup.profileCost_rich", { size: count(thumbsMb) })}
                </span>
              </span>
            </label>
          {/each}
        </div>

        <p class="later">{t("setup.profileLater")}</p>

        <footer>
          <button class="btn btn-primary" type="button" onclick={applyProfile} disabled={saving}>
            {saving ? t("settings.saving") : t("setup.finish")}
          </button>
        </footer>
      {:else}
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
        <div class="errbox">{error}</div>
      {/if}

      <footer>
        <button
          class="btn btn-primary"
          type="button"
          onclick={finish}
          disabled={!validation?.is_valid || saving}
        >
          {saving ? t("settings.saving") : FEATURE_GRID_THUMBS ? t("setup.next") : t("setup.finish")}
        </button>
      </footer>
      {/if}
    </div>
  </div>
</div>

<style>
  .profiles {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin: 16px 0 4px;
  }
  .profile {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 12px 14px;
    background: var(--raised);
    border: 1px solid var(--line);
    cursor: pointer;
  }
  .profile.on {
    border-color: var(--rosso);
  }
  .p-body {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .p-name {
    font-size: 13px;
    font-weight: 600;
    color: var(--txt);
  }
  .p-what {
    font-size: 11.5px;
    color: var(--txt2);
  }
  /* Le coût en dernier et en gris : c'est ce qui départage, pas ce qui
     nomme — le §5.5 veut des profils nommés par leur résultat. */
  .p-cost {
    font-size: 11px;
    color: var(--muted);
  }
  .later {
    margin: 10px 0 0;
    font-size: 11.5px;
    color: var(--muted);
  }
  .wizard {
    /* Le document ne défile jamais (global.css) : sur un petit écran/fenêtre,
       un contenu plus haut que la fenêtre serait sinon coupé sans recours —
       c'est ce qui obligeait à agrandir la fenêtre à la main. */
    height: 100vh;
    overflow-y: auto;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 28px 16px;
  }
  .frame {
    width: 100%;
    max-width: 680px;
    margin: auto;
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
  .errbox {
    margin: 12px 0;
  }
  footer {
    margin-top: 24px;
    padding-top: 18px;
    border-top: 1px solid var(--line);
    display: flex;
    justify-content: flex-end;
  }
</style>
