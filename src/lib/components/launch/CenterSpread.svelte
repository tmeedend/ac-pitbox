
<script lang="ts">
  // Un réglage « centre ± écart » : une poignée sur le curseur, un champ pour
  // l'écart, et le libellé qui dit ce que les deux donnent ensemble.
  //
  // **Pourquoi pas deux poignées.** Le geste fréquent est de monter tout le
  // plateau de quelques points sans en changer la dispersion : avec un
  // min et un max, il demande deux manipulations, et rater l'une des deux
  // resserre la fourchette sans qu'on s'en aperçoive. Déplacer le centre
  // conserve l'écart par construction.
  //
  // **Pourquoi l'écart est un champ et non un second curseur.** `± 0` — tout le
  // plateau à la même force — est une valeur qu'on veut poser exactement, et
  // qu'un pointeur sur une piste de 130 px n'atteint qu'à la bagarre.
  //
  // Deux instances : la difficulté et l'agressivité. Elles ne diffèrent que par
  // leurs bornes, ce qui est exactement ce qu'un composant partagé doit porter
  // en paramètre.
  import { band } from "$lib/launch/aiBand";
  import { t } from "$lib/i18n/index.svelte";
  import NumberStepper from "$lib/components/ui/NumberStepper.svelte";

  interface Props {
    label: string;
    center: number;
    spread: number;
    min: number;
    max: number;
    /** Ce que le réglage ajoute derrière ses nombres ("%"), jamais une unité
     * inventée : l'écart s'exprime dans la même que le centre. */
    unit?: string;
    onchange: (center: number, spread: number) => void;
    /** Explication permanente, rendue à droite du libellé. */
    info?: import("svelte").Snippet;
  }
  let { label, center, spread, min, max, unit = "%", onchange, info }: Props = $props();

  const bounds = $derived(band(center, spread, min, max));
  /** Where the band starts and ends, as a fraction of the thumb's travel.
   * The band is the spread made visible: widening the spread widens it, and
   * at ± 0 it is gone (SESSION§3.1). */
  const pct = (v: number) => (max > min ? (v - min) / (max - min) : 0);
  const bandFrom = $derived(pct(bounds[0]));
  const bandTo = $derived(pct(bounds[1]));
</script>

<div class="cs">
  <span class="fk lbl-key">{label}{@render info?.()}</span>

  <!-- `95% ± 3 (92–98)` : la plage affichée est la plage **bornée**, jamais la
       plage théorique — sinon l'écran annonce ce que le jeu ne recevra pas. -->
  <span class="val mono"
    >{center}{unit} ± {spread} <span class="range">({bounds[0]}–{bounds[1]})</span></span
  >

  <div class="row">
    <input
      class="slider"
      type="range"
      {min}
      {max}
      value={center}
      style:--lo={bandFrom}
      style:--hi={bandTo}
      aria-label={t("launch.centerLabel", { label })}
      oninput={(e) => onchange(Number(e.currentTarget.value), spread)}
    />
    <!-- The same "±" as the line above: the field reads as the second number
         of "95% ± 3", which is what it sets. -->
    <span class="pm mono" aria-hidden="true">±</span>
    <NumberStepper
      width={56}
      title={t("launch.spreadLabel", { label })}
      min={0}
      max={max - min}
      value={spread}
      onchange={(v) => onchange(center, v)}
    />
  </div>
</div>

<style>
  .cs {
    display: flex;
    flex-direction: column;
    gap: 4px;
    width: 190px;
  }
  .fk {
    text-transform: uppercase;
  }
  .val {
    font-size: 9.5px;
    color: var(--txt2);
  }
  /* La plage entre parenthèses est une conséquence, pas un réglage : elle se
     lit après les deux nombres qu'on manipule, donc plus éteinte. */
  .range {
    color: var(--muted);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .slider {
    flex: 1;
    min-width: 0;
    height: 20px;
    margin: 0;
    appearance: none;
    background: transparent;
    cursor: pointer;
  }
  /* The band from the lowest to the highest value the grid will get, lighter
     than the rest of the track. **Placed on the thumb's travel, not on the
     track**: the browser keeps the whole thumb inside the track, so its centre
     only runs from half a thumb in to half a thumb short of the end. Measured
     against the full length, the band sat off the thumb (reported). */
  .slider {
    --thumb: 10px;
    --lo-x: calc(var(--thumb) / 2 + var(--lo) * (100% - var(--thumb)));
    --hi-x: calc(var(--thumb) / 2 + var(--hi) * (100% - var(--thumb)));
  }
  .slider::-webkit-slider-runnable-track {
    height: 3px;
    background: linear-gradient(
      to right,
      var(--line) var(--lo-x),
      var(--muted2) var(--lo-x),
      var(--muted2) var(--hi-x),
      var(--line) var(--hi-x)
    );
  }
  .pm {
    font-size: 12px;
    color: var(--muted);
    /* Tucked against its field: it belongs to the number, not to the track. */
    margin-right: -4px;
  }
  .slider::-webkit-slider-thumb {
    appearance: none;
    width: var(--thumb);
    height: 18px;
    border-radius: 2px;
    background: var(--rosso);
    border: 2px solid var(--panel);
    margin-top: -8px;
  }
</style>
