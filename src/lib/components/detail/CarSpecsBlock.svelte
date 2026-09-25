<script lang="ts">
  // Tech sheet and power curve of a car's detail page, side by side (§6).
  import type { ModDetail } from "$lib/library/library";
  import TechSheet from "./TechSheet.svelte";
  import PowerCurve from "./PowerCurve.svelte";
  import { t } from "$lib/i18n/index.svelte";

  let { detail }: { detail: ModDetail } = $props();

  const hasCurve = $derived(!!detail.specs && detail.specs.power_curve.length > 1);
</script>

<div class="tech-curve" class:with-curve={hasCurve}>
  <section class="blk fiche">
    <header class="blk-h"><span class="blk-t">{t("detail.techSheet")}</span></header>
    <TechSheet {detail} surface="panel2" framed={false} />
  </section>
  {#if hasCurve && detail.specs}
    <section class="blk curve-col">
      <header class="blk-h">
        <span class="blk-t">{t("detail.curve")}</span>
        <span class="blk-n"><span class="lg-pow">— bhp</span> <span class="lg-tor">— Nm</span></span>
      </header>
      <div class="blk-b curve-box">
        <PowerCurve power={detail.specs.power_curve} torque={detail.specs.torque_curve} />
      </div>
    </section>
  {/if}
</div>

<style>
  /* Fiche technique + courbe carrée côte à côte (§6). */
  .tech-curve {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
    align-items: flex-start;
    margin-bottom: 12px;
  }
  .tech-curve .fiche {
    flex: 1 1 200px;
    min-width: 0;
    margin-bottom: 0;
  }
  /* Plus de règle de colonnes ici : la fiche technique partagée
     (`TechSheet.svelte`) déduit leur nombre de la largeur qu'on lui donne,
     donc elle se resserre d'elle-même quand la courbe occupe la moitié de la
     rangée. */
  .curve-col {
    flex: 1 1 200px;
    max-width: 260px;
    min-width: 0;
  }
  .lg-pow {
    color: var(--rosso-bright);
  }
  .lg-tor {
    color: var(--yellow);
  }
  .curve-box {
    border: 1px solid var(--line);
    padding: 8px;
    margin-bottom: 0;
  }
</style>
