<script lang="ts">
  // Bloc « Simulation » de l'écran Lancement (§8.6) : dégâts/carburant/pneus
  // + aides à la conduite, actifs quel que soit le type de session. Vue de
  // présentation pure : tout est une lecture/écriture directe de `setup`
  // (état partagé avec le parent) — aucune logique à faire remonter.
  import type { RaceSetup } from "$lib/launch";
  import Slider from "../Slider.svelte";
  import { t } from "$lib/i18n/index.svelte";

  let { setup }: { setup: RaceSetup } = $props();
</script>

<!-- Simulation, aides à la conduite comprises : actif quel que soit
     le type de session (§8.6). -->
<section class="blk">
  <header class="blk-h"><span class="blk-t">{t("launch.simulationLabel")}</span></header>
  <div class="blk-b">
  <div class="opt-row">
    <Slider
      label={t("launch.damageLabel")}
      value={setup.damage}
      min={0}
      max={100}
      display="{setup.damage}%"
      oninput={(v) => (setup.damage = v)}
    />
    <Slider
      label={t("launch.fuelLabel")}
      value={setup.fuel_rate}
      min={0}
      max={200}
      display="{setup.fuel_rate}%"
      oninput={(v) => (setup.fuel_rate = v)}
    />
    <Slider
      label={t("launch.tyreLabel")}
      value={setup.tyre_wear}
      min={0}
      max={200}
      display="{setup.tyre_wear}%"
      oninput={(v) => (setup.tyre_wear = v)}
    />
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
     réglage 0-100/200 qui se lit très bien en plus étroit. Le curseur
     lui-même vient de `Slider.svelte` — cadre, piste et poignée y sont
     définis une seule fois pour toute l'app. */
  .opt-row {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(140px, 1fr));
    gap: 10px 16px;
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
