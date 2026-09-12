<script lang="ts">
  // "Simulation" block of the launch screen (§8.6): damage/fuel/tyres plus the
  // driving aids, all active whatever the session type. Pure presentation:
  // everything is a direct read/write of `setup` (state shared with the
  // parent) — no logic to lift up.
  import { carFactoryAssists, type AssistLevel, type FactoryAssists, type RaceSetup } from "$lib/launch";
  import Seg from "../Seg.svelte";
  import Slider from "../Slider.svelte";
  import Tooltip from "../Tooltip.svelte";
  import { t } from "$lib/i18n/index.svelte";

  let { setup, carName = null }: { setup: RaceSetup; carName?: string | null } = $props();

  const levels = $derived([
    { value: "off", label: t("launch.assistOff") },
    { value: "factory", label: t("launch.assistFactory") },
    { value: "on", label: t("launch.assistOn") },
  ]);

  // What `Factory` is worth for the car in session — read from its own
  // `electronics.ini` (§9.3). Content Manager shows three opaque values here;
  // this is the one thing the app can say that it cannot.
  let factory: FactoryAssists | null = $state(null);
  $effect(() => {
    const carId = setup.car_id;
    if (!carId) {
      factory = null;
      return;
    }
    let current = true;
    carFactoryAssists(carId)
      .then((found) => {
        if (current) factory = found;
      })
      // Never an error on screen: the line simply does not appear. A car whose
      // acd refuses to open is not a failure of the session settings.
      .catch(() => {
        if (current) factory = null;
      });
    return () => {
      current = false;
    };
  });

  // No hedged sentence and no "unknown": a car that does not say gets no line
  // at all (§9.3). Ten cars of the reference install are in that case.
  const factoryLine = $derived.by(() => {
    if (!factory || !carName) return null;
    const key = factory.abs
      ? factory.tractionControl
        ? "launch.factoryBoth"
        : "launch.factoryAbsOnly"
      : factory.tractionControl
        ? "launch.factoryTcOnly"
        : "launch.factoryNeither";
    return t(key, { car: carName });
  });
</script>

<!-- Simulation, driving aids included: active whatever the session type
     (§8.6). Tyre blankets sits with the tyres rather than with the aids: it is
     the state the tyres start in, like their wear, not something that helps
     the driver drive. -->
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
    <label class="check blankets"
      ><input type="checkbox" bind:checked={setup.tyre_blankets} /><span>{t("launch.tyreBlankets")}</span></label
    >

    <div class="lbl section">{t("launch.assistsLabel")}</div>
    <div class="aids">
      <!-- Three states, not two: a tick could not tell "whatever the real car
           had" from "forced on", and the middle one is the default. The order
           reads as a progression, which is why Factory sits between the two. -->
      <div>
        <span class="fk lbl-key"
          >{t("launch.absLabel")}<Tooltip text={t("launch.absTooltip")}
            ><button type="button" class="info-i">ⓘ</button></Tooltip
          ></span
        >
        <Seg value={setup.abs} onselect={(v) => (setup.abs = v as AssistLevel)} items={levels} />
      </div>
      <div>
        <span class="fk lbl-key"
          >{t("launch.tractionLabel")}<Tooltip text={t("launch.tractionTooltip")}
            ><button type="button" class="info-i">ⓘ</button></Tooltip
          ></span
        >
        <Seg
          value={setup.traction_control}
          onselect={(v) => (setup.traction_control = v as AssistLevel)}
          items={levels}
        />
      </div>
      <label class="check"><input type="checkbox" bind:checked={setup.ideal_line} /><span>{t("launch.idealLine")}</span></label>
    </div>
    {#if factoryLine}
      <p class="factory-note"><span class="fw">{t("launch.assistFactory")}</span> — {factoryLine}</p>
    {/if}
  </div>
</section>

<style>
  /* Damage/fuel/tyres on one row (two if the width is short): no reason to let
     each slider stretch across the whole width for a 0-100/200 setting that
     reads perfectly well narrower. The slider itself comes from
     `Slider.svelte` — frame, track and thumb defined once for the whole app. */
  .opt-row {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(140px, 1fr));
    gap: 10px 16px;
  }
  .blankets {
    margin-top: 10px;
  }
  .aids {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    gap: 14px 16px;
  }
  .aids > div {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  /* Colour/size/letter-spacing come from `.lbl-key` (global, §labels). */
  .fk {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    text-transform: uppercase;
  }
  .info-i {
    background: transparent;
    border: none;
    padding: 0;
    color: var(--muted2);
    font-size: 10px;
    line-height: 1;
  }
  .info-i:hover {
    background: transparent;
    color: var(--txt2);
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
  /* Same register as the weather block's implicit note: something the app
     knows and the driver would otherwise have to find out on track. */
  .factory-note {
    color: var(--muted);
    font-size: 10px;
    margin-top: 10px;
    margin-bottom: 0;
  }
  /* Blue is information in the red scale (§7.2ter) — not an alert, nothing is
     wrong here, and the word repeats a segment the reader has just seen. */
  .factory-note .fw {
    color: var(--blue);
  }
</style>
