<script lang="ts">
  // Curseurs de cadrage de l'aperçu 3D, partagés par l'écran Réglages et le
  // panneau de la fiche voiture. Un seul endroit qui connaisse les libellés,
  // les bornes et l'ordre : deux copies auraient divergé au premier ajout.
  import { onDestroy } from "svelte";
  import Slider from "../Slider.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import {
    PREVIEW3D_RANGES,
    flushPreview3dPrefs,
    preview3dPrefs,
    setPreview3dValue,
    resetPreview3dCamera,
  } from "$lib/preview3dPrefs.svelte";

  /** `compact` : version posée par-dessus l'aperçu, où la place manque et où
   * le résultat est sous les yeux — pas de texte d'aide. */
  let { compact = false }: { compact?: boolean } = $props();

  const prefs = $derived(preview3dPrefs());

  // L'écriture disque est différée (voir `preview3dPrefs.svelte.ts`) : fermer
  // le panneau juste après avoir bougé un curseur ne doit pas emporter le
  // réglage avec le délai en cours.
  onDestroy(() => void flushPreview3dPrefs());

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
    <Slider
      label={t(s.label)}
      value={prefs[s.key]}
      min={PREVIEW3D_RANGES[s.key].min}
      max={PREVIEW3D_RANGES[s.key].max}
      step={PREVIEW3D_RANGES[s.key].step}
      display="{prefs[s.key]}{s.unit}"
      hint={t(s.hint)}
      {compact}
      oninput={(v) => setPreview3dValue(s.key, v)}
    />
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
  footer {
    display: flex;
    justify-content: flex-end;
  }
  footer.compact {
    margin-top: 4px;
  }
</style>
