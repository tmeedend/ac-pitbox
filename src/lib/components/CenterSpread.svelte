
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
  import { band } from "$lib/aiBand";
  import { t } from "$lib/i18n/index.svelte";
  import NumberStepper from "./NumberStepper.svelte";

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
      aria-label={t("launch.centerLabel", { label })}
      oninput={(e) => onchange(Number(e.currentTarget.value), spread)}
    />
    <NumberStepper
      width={56}
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
  .slider::-webkit-slider-runnable-track {
    height: 3px;
    background: var(--line);
  }
  .slider::-webkit-slider-thumb {
    appearance: none;
    width: 10px;
    height: 18px;
    border-radius: 2px;
    background: var(--rosso);
    border: 2px solid var(--panel);
    margin-top: -8px;
  }
</style>
