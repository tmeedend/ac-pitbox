<script lang="ts">
  // Curseurs de l'aperçu 3D, partagés par l'écran Réglages et le panneau posé
  // sur la fiche voiture. Un seul endroit qui connaisse les libellés, les
  // bornes et l'ordre : deux copies auraient divergé au premier ajout.
  //
  // Les réglages sont rangés en **groupes** (`PREVIEW3D_GROUPS`), et ce
  // composant en affiche un à la fois. C'est ce qui permet au panneau compact
  // de ne montrer que le cadrage — celui qu'on règle en voyant le résultat —
  // pendant que l'écran Réglages les présente tous, chacun avec son bouton de
  // remise à zéro posé à côté de ce qu'il remet à zéro.
  import Slider from "../Slider.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import {
    PREVIEW3D_GROUPS,
    PREVIEW3D_RANGES,
    preview3dPrefs,
    resetPreview3dGroup,
    setPreview3dValue,
    type Preview3dGroup,
  } from "$lib/preview3dPrefs.svelte";

  /** `compact` : version posée par-dessus l'aperçu, où la place manque et où
   * le résultat est sous les yeux — pas de texte d'aide. */
  let { compact = false, group = "framing" }: { compact?: boolean; group?: Preview3dGroup } = $props();

  const prefs = $derived(preview3dPrefs());
  const keys = $derived(PREVIEW3D_GROUPS[group]);

  // Rien n'est écrit ici, ni au démontage : les curseurs ne touchent qu'à
  // l'état affiché, et c'est l'écran qui décide d'enregistrer ou d'annuler
  // (voir `preview3dPrefs.svelte.ts`).

  /** Unité affichée à côté de la valeur. Les degrés et les pourcentages sont
   * universels et restent en dur ; le flou n'a pas d'unité. */
  const UNITS: Partial<Record<string, string>> = {
    azimuth: "°",
    elevation: "°",
    fov: "°",
    reflectionBlur: "",
  };

  /** Le flou est stocké en dixièmes — la mécanique des préférences travaille
   * sur des entiers — mais s'affiche comme le nombre qu'il est. */
  function display(key: string, value: number): string {
    if (key === "reflectionBlur") return (value / 10).toFixed(1).replace(".", ",");
    return `${value}${UNITS[key] ?? "%"}`;
  }
</script>

{#each keys as key (key)}
  <section class:compact>
    <Slider
      label={t(`settings.preview3dSlider.${key}`)}
      value={prefs[key]}
      min={PREVIEW3D_RANGES[key].min}
      max={PREVIEW3D_RANGES[key].max}
      step={PREVIEW3D_RANGES[key].step}
      display={display(key, prefs[key])}
      hint={t(`settings.preview3dSlider.${key}Hint`)}
      {compact}
      oninput={(v) => setPreview3dValue(key, v)}
    />
  </section>
{/each}

<footer class:compact>
  <button class="btn" type="button" onclick={() => resetPreview3dGroup(group)}>
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
