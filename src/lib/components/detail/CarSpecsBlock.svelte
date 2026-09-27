<script lang="ts">
  // Tech sheet and power curve of a car's detail page, side by side (§6,
  // FICHE§7.1): the curve drops below when the row runs out of room.
  //
  // The sheet opens into its edit mode here (FICHE§8), through the pencil of
  // its banner — the same pencil as every other value of the page one can take
  // over by hand. The curve stays out of the edit mode: it is not editable
  // (FICHE§11), and the form needs the width.
  import type { ModDetail } from "$lib/library/library";
  import { errorText } from "$lib/errors";
  import { t } from "$lib/i18n/index.svelte";
  import { saveTechSheet, type TechEdit } from "$lib/detail/techSheet";
  import Pencil from "$lib/components/ui/Pencil.svelte";
  import TechSheet from "./TechSheet.svelte";
  import TechSheetEdit from "./TechSheetEdit.svelte";
  import PowerCurve from "./PowerCurve.svelte";

  interface Props {
    detail: ModDetail;
    /** The sheet was saved: the page reloads the car, whose columns, year and
     * country may have changed with it. */
    onchanged: () => void | Promise<void>;
  }
  let { detail, onchanged }: Props = $props();

  let editing = $state(false);
  let busy = $state(false);
  let error = $state<string | null>(null);

  const sheet = $derived(detail.tech ?? { values: {}, edited: [], fallback: {} });

  /** A curve as stored in the sheet: pairs of `[rpm, value]`. */
  function curve(field: string): [number, number][] {
    const v = sheet.values[field]?.value;
    return Array.isArray(v) ? (v as [number, number][]) : [];
  }
  const power = $derived(curve("power_curve"));
  const torque = $derived(curve("torque_curve"));
  const hasCurve = $derived(!editing && power.length > 1);

  async function save(edits: TechEdit[]) {
    if (!edits.length) {
      editing = false;
      return;
    }
    busy = true;
    error = null;
    try {
      await saveTechSheet(detail.id_interne, edits);
      editing = false;
      await onchanged();
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = false;
    }
  }
</script>

<div class="tech-curve">
  <section class="blk fiche">
    <header class="blk-h">
      <span class="blk-t">{t("detail.techSheet")}</span>
      {#if editing}
        <span class="blk-n">{t("techsheet.editing")}</span>
      {:else}
        <Pencil label={t("techsheet.edit")} onclick={() => (editing = true)} />
      {/if}
    </header>
    {#if error}<div class="errbox">{error}</div>{/if}
    {#if editing}
      <TechSheetEdit {sheet} {busy} onsave={save} oncancel={() => (editing = false)} />
    {:else}
      <TechSheet {sheet} />
    {/if}
  </section>
  {#if hasCurve}
    <section class="blk curve-col">
      <header class="blk-h">
        <span class="blk-t">{t("detail.curve")}</span>
        <span class="blk-n"><span class="lg-pow">— bhp</span> <span class="lg-tor">— Nm</span></span>
      </header>
      <div class="blk-b curve-box">
        <PowerCurve {power} {torque} />
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
  /* The sheet asks for more room than the former cell grid did: its key
     figures are three wide, and its sentences read badly under 280px. Below
     that, the curve drops under it. */
  .tech-curve .fiche {
    flex: 1 1 280px;
    min-width: 0;
    margin-bottom: 0;
  }
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
  .errbox {
    margin: 10px 14px 0;
  }
</style>
