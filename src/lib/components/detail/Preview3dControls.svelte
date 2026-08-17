<script lang="ts">
  // Curseurs de cadrage de l'aperçu 3D, partagés par l'écran Réglages et le
  // panneau de la fiche voiture. Un seul endroit qui connaisse les libellés,
  // les bornes et l'ordre : deux copies auraient divergé au premier ajout.
  import { t } from "$lib/i18n/index.svelte";
  import { PREVIEW3D_RANGES, preview3dPrefs, setPreview3dValue, resetPreview3dCamera } from "$lib/preview3dPrefs.svelte";

  /** `compact` : version posée par-dessus l'aperçu, où la place manque et où
   * le résultat est sous les yeux — pas de texte d'aide. */
  let { compact = false }: { compact?: boolean } = $props();

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
    { key: "height", label: "settings.preview3dHeight", hint: "settings.preview3dHeightHint", unit: "%" },
    { key: "spin", label: "settings.preview3dSpin", hint: "settings.preview3dSpinHint", unit: "%" },
  ] as const;
</script>

{#each sliders as s (s.key)}
  <section class:compact>
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
    {#if !compact}<p class="hint">{t(s.hint)}</p>{/if}
  </section>
{/each}

<footer class:compact>
  <button class="btn" type="button" onclick={resetPreview3dCamera}>
    {t("settings.preview3dReset")}
  </button>
</footer>

<style>
  /* Repris de Settings.svelte : le CSS Svelte est scopé par composant, ces
     classes ne traversent pas depuis l'écran parent (§ conventions projet). */
  section {
    margin-bottom: 22px;
    padding-bottom: 18px;
    border-bottom: 1px solid var(--line);
  }
  section.compact {
    margin-bottom: 10px;
    padding-bottom: 0;
    border-bottom: none;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 12px;
    color: var(--txt2);
    max-width: 340px;
  }
  .compact label {
    max-width: none;
    gap: 3px;
    font-size: 11px;
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
  .compact .value {
    font-size: 11px;
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
  footer.compact {
    margin-top: 4px;
  }
</style>
