<script lang="ts">
  // "Simulation" block of the launch screen (SESSION§3): what the simulation models
  // and what it forgives, active whatever the session type. Pure presentation:
  // everything is a direct read/write of `setup` (state shared with the
  // parent) — no logic to lift up.
  //
  // **ABS and traction control have left** (L5§2.2). They are not rules of
  // the session but capabilities of the CAR — the setting only exists because
  // the car has the hardware, which is exactly what the `Factory` line said out
  // loud. They now sit in the car card of the session column, folded under
  // `PERFORMANCE` with the ballast and the restrictor, and that line follows
  // them there.
  //
  // The ideal line does **not** follow: it is not a capability of the car but a
  // display aid, so it stays here with the tyre blankets and the penalties.
  import { type RaceSetup } from "$lib/launch/launch";
  import Slider from "$lib/components/ui/Slider.svelte";
  import { t } from "$lib/i18n/index.svelte";

  let { setup }: { setup: RaceSetup } = $props();
</script>

<!-- Tyre blankets sits with the tyres rather than with the aids: it is the
     state the tyres start in, like their wear, not something that helps the
     driver drive. -->
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
    <!-- Three ticks on one line: what the simulation starts from and what it
         forgives. The ideal line joined them when the driving aids left —
         it was the only one of that block that was never about the car. -->
    <div class="ticks">
      <label class="check"
        ><input type="checkbox" bind:checked={setup.tyre_blankets} /><span>{t("launch.tyreBlankets")}</span></label
      >
      <label class="check"
        ><input type="checkbox" bind:checked={setup.penalties} /><span>{t("launch.penalties")}</span></label
      >
      <label class="check"
        ><input type="checkbox" bind:checked={setup.ideal_line} /><span>{t("launch.idealLine")}</span></label
      >
    </div>
  </div>
</section>

<style>
  /* Damage/fuel/tyres on one row (two if the width is short): no reason to let
     each slider stretch across the whole width for a 0-100/200 setting that
     reads perfectly well narrower. The slider itself comes from
     `Slider.svelte` — frame, track and thumb defined once for the whole app.

     A block may take the width it is given; its controls keep their own
     gauge and stay flush left (L5§3.3). A 900px slider for a setting one
     poses to the percent is a regression, not a gain. */
  .opt-row {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(140px, 220px));
    justify-content: start;
    gap: 10px 16px;
  }
  .ticks {
    display: flex;
    flex-wrap: wrap;
    gap: 8px 20px;
    margin-top: 10px;
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
