<script lang="ts">
  // Bloc « Simulation » de l'écran Lancement (§8.6) : dégâts/carburant/pneus
  // + aides à la conduite, actifs quel que soit le type de session. Vue de
  // présentation pure : tout est une lecture/écriture directe de `setup`
  // (état partagé avec le parent) — aucune logique à faire remonter.
  import type { RaceSetup } from "$lib/launch";
  import { t } from "$lib/i18n/index.svelte";

  let { setup }: { setup: RaceSetup } = $props();
</script>

<!-- Simulation, aides à la conduite comprises : actif quel que soit
     le type de session (§8.6). -->
<section class="blk">
  <header class="blk-h"><span class="blk-t">{t("launch.simulationLabel")}</span></header>
  <div class="blk-b">
  <div class="opt-row">
    <div class="opt">
      <div class="opt-head"><span class="opt-name lbl-key">{t("launch.damageLabel")}</span><span class="opt-val mono">{setup.damage}%</span></div>
      <input type="range" min="0" max="100" bind:value={setup.damage} style="--f:{setup.damage}%" />
    </div>
    <div class="opt">
      <div class="opt-head"><span class="opt-name lbl-key">{t("launch.fuelLabel")}</span><span class="opt-val mono">{setup.fuel_rate}%</span></div>
      <input type="range" min="0" max="200" bind:value={setup.fuel_rate} style="--f:{setup.fuel_rate / 2}%" />
    </div>
    <div class="opt">
      <div class="opt-head"><span class="opt-name lbl-key">{t("launch.tyreLabel")}</span><span class="opt-val mono">{setup.tyre_wear}%</span></div>
      <input type="range" min="0" max="200" bind:value={setup.tyre_wear} style="--f:{setup.tyre_wear / 2}%" />
    </div>
  </div>

  <div class="lbl section">{t("launch.assistsLabel")}</div>
  <div class="checks">
    <label class="check"><input type="checkbox" bind:checked={setup.abs_auto} /><span>{t("launch.absAuto")}</span></label>
    <label class="check"><input type="checkbox" bind:checked={setup.traction_control_auto} /><span>{t("launch.tractionAuto")}</span></label>
    <label class="check"><input type="checkbox" bind:checked={setup.ideal_line} /><span>{t("launch.idealLine")}</span></label>
    <label class="check"><input type="checkbox" bind:checked={setup.tyre_blankets} /><span>{t("launch.tyreBlankets")}</span></label>
  </div>
  </div>
</section>

<style>
  /* Dégâts/carburant/pneus groupés sur une même ligne (2 si l'espace manque) :
     inutile de laisser chaque curseur s'étirer sur toute la largeur pour un
     réglage 0-100/200 qui se lit très bien en plus étroit. */
  .opt-row {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(140px, 1fr));
    gap: 10px 16px;
  }
  .opt {
    margin-bottom: 14px;
  }
  .opt-row .opt {
    margin-bottom: 0;
  }
  .opt-head {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 4px;
  }
  /* Couleur/taille/interlettrage viennent de `.lbl-key` (global, harmonisation
     §chantier libellés) : ne reste ici que ce que `.lbl-key` ne couvre pas. */
  .opt-name {
    text-transform: uppercase;
  }
  .opt-val {
    margin-left: auto;
    font-size: 11px;
    color: var(--txt);
  }
  /* Curseurs simples (dégâts/carburant/pneus) : le thumb natif du
     navigateur est rond, incohérent avec le thème rectangulaire de l'app —
     resimulé en carré comme les curseurs doubles ci-dessus. */
  .opt input[type="range"] {
    width: 100%;
    height: 20px;
    margin: 0;
    appearance: none;
    background: transparent;
  }
  .opt input[type="range"]::-webkit-slider-runnable-track {
    height: 3px;
    background: linear-gradient(to right, var(--rosso) 0%, var(--rosso) var(--f, 0%), var(--line) var(--f, 0%), var(--line) 100%);
  }
  .opt input[type="range"]::-webkit-slider-thumb {
    appearance: none;
    width: 10px;
    height: 20px;
    border-radius: 2px;
    background: var(--rosso);
    border: 2px solid var(--panel);
    cursor: pointer;
    margin-top: -8.5px;
  }
  .checks {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(120px, 1fr));
    gap: 8px;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 8px;
    border: 1px solid var(--line);
    background: var(--panel2);
    padding: 8px 10px;
    cursor: pointer;
    font-size: 10px;
    color: var(--txt2);
  }
</style>
