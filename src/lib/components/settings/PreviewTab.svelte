<script lang="ts">
  // Réglages de l'aperçu 3D des voitures (docs/SPEC-preview-3d-kn5.md §15).
  //
  // Comme MusicTab, cet onglet ne passe pas par AppConfig : ses réglages vivent
  // dans `ui_prefs.json` et s'appliquent à l'instant où on les bouge — d'où
  // l'absence de bouton Enregistrer et de garde de navigation. Une fiche
  // ouverte derrière suit le curseur sans être rechargée.
  import { t } from "$lib/i18n/index.svelte";
  import {
    preview3dPrefs,
    PREVIEW3D_RANGES,
    setPreview3dEnabled,
    setPreview3dValue,
    resetPreview3dCamera,
  } from "$lib/preview3dPrefs.svelte";

  const prefs = $derived(preview3dPrefs());

  const sliders = [
    { key: "zoom", label: "settings.preview3dZoom", hint: "settings.preview3dZoomHint", unit: "%" },
    { key: "azimuth", label: "settings.preview3dAzimuth", hint: "settings.preview3dAzimuthHint", unit: "°" },
    {
      key: "elevation",
      label: "settings.preview3dElevation",
      hint: "settings.preview3dElevationHint",
      unit: "°",
    },
    { key: "spin", label: "settings.preview3dSpin", hint: "settings.preview3dSpinHint", unit: "%" },
  ] as const;
</script>

<section class="lang-section">
  <label class="check">
    <input
      type="checkbox"
      checked={prefs.enabled}
      onchange={(e) => setPreview3dEnabled(e.currentTarget.checked)}
    />
    <span>{t("settings.preview3dEnabled")}</span>
  </label>
  <p class="hint">{t("settings.preview3dEnabledHint")}</p>
</section>

{#each sliders as s (s.key)}
  <section class="lang-section">
    <label>
      <span class="head">
        {t(s.label)}
        <span class="value mono">{prefs[s.key]}{s.unit}</span>
      </span>
      <input
        type="range"
        min={PREVIEW3D_RANGES[s.key].min}
        max={PREVIEW3D_RANGES[s.key].max}
        step={PREVIEW3D_RANGES[s.key].step}
        value={prefs[s.key]}
        oninput={(e) => setPreview3dValue(s.key, Number(e.currentTarget.value))}
      />
    </label>
    <p class="hint">{t(s.hint)}</p>
  </section>
{/each}

<footer>
  <button class="btn" type="button" onclick={resetPreview3dCamera}>
    {t("settings.preview3dReset")}
  </button>
</footer>

<style>
  /* Repris de Settings.svelte : le CSS Svelte est scopé par composant, ces
     classes ne traversent pas depuis l'écran parent (§ conventions projet). */
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
    max-width: 340px;
  }
  .lang-section label.check {
    flex-direction: row;
    align-items: center;
    gap: 8px;
    max-width: none;
    cursor: pointer;
  }
  .head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 12px;
  }
  .value {
    color: var(--txt);
    font-size: 12px;
  }
  input[type="range"] {
    width: 100%;
    accent-color: var(--rosso-bright);
  }
  .hint {
    margin-top: 8px;
    font-size: 11px;
    color: var(--faint);
  }
  footer {
    display: flex;
    justify-content: flex-end;
  }
</style>
