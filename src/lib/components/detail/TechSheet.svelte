<script lang="ts">
  // The one tech sheet of a car. Only the full detail page
  // (`DetailPage.svelte`) draws it now, since the compact side panel was
  // removed — but the rows stay here rather than inlined there, because of how
  // they came to be shared in the first place.
  //
  // The two used to each build their own list, and they had drifted: the panel
  // showed the whole native sheet (power, torque, weight, top speed, 0-100,
  // power/weight, range, country) plus the five harmonized fields, while the
  // page showed six rows and left out everything the engine actually says about
  // itself. Same screen name, same title, half the content - reported by the
  // user, and exactly what the "shared components" chantier is about: two
  // copies of one thing drift, and nothing flags it.
  //
  // The frame stays the host's business: what is shared is the content and the
  // row itself.
  import { t } from "$lib/i18n/index.svelte";
  import type { ModDetail } from "$lib/library";

  interface Props {
    detail: ModDetail;
    /** Narrowest a column may get before the grid drops one. The sheet fits
     * itself to whatever width the host gives it: two columns in the side
     * panel, three on the page, without either of them saying so. */
    minColumn?: number;
    /** Cell background — the host's own card surface, so the sheet does not
     * look pasted onto it. */
    surface?: "panel" | "panel2";
    /** Outer 1px rule. Off when the host already frames it (a `.blk` card). */
    framed?: boolean;
  }
  let { detail, minColumn = 128, surface = "panel", framed = true }: Props = $props();

  interface Row {
    label: string;
    value: string;
    /** Deduced by the rule engine (§5bis.1) rather than read in the mod's own
     * file — shown in the rule colour, with the tooltip that says so. */
    derived?: boolean;
  }

  /** Engine position, abbreviated: the sheet is a grid of narrow cells, and
   * "REAR" spelled out pushes a column wider than the value deserves. */
  function posLabel(pos: string): string {
    if (pos === "FRONT") return t("detail.posFront");
    if (pos === "MID") return t("detail.posMid");
    if (pos === "REAR") return t("detail.posRear");
    return pos;
  }

  /** Distance driven (§6.5). Always present, unlike every other row: an empty
   * odometer is itself the answer, and "never driven" is worth reading. */
  function odometer(d: ModDetail): string {
    if (d.distance_km != null) return `${d.distance_km.toFixed(1)} km`;
    return d.tried ? t("detail.triedYes") : t("detail.triedNo");
  }

  const rows = $derived.by(() => {
    const d = detail;
    const out: Row[] = [];
    const add = (label: string, value: string | null | undefined, derived = false) => {
      if (value) out.push({ label, value, derived });
    };
    const s = d.specs;
    // Read as-is in the mod's `ui_car.json`, empty rows skipped: a sheet of
    // dashes says nothing that its absence would not.
    add(t("modpanel.specPower"), s?.bhp);
    add(t("modpanel.specTorque"), s?.torque);
    add(t("columns.weight"), s?.weight);
    add(t("modpanel.specTopSpeed"), s?.topspeed);
    add(t("modpanel.specAccel"), s?.acceleration);
    add(t("modpanel.specPwRatio"), s?.pwratio);
    add(t("modpanel.specRange"), s?.range);
    add(t("columns.country"), s?.country ?? d.country);
    // Deduced by the rules, hence the separate look.
    add(t("columns.drivetrain"), d.drivetrain, true);
    add(t("columns.aspiration"), d.aspiration, true);
    add(t("columns.engineConfig"), d.engine_config, true);
    add(t("columns.enginePos"), d.engine_pos ? posLabel(d.engine_pos) : null, true);
    add(t("columns.gearbox"), d.gearbox, true);
    out.push({ label: t("detail.odometer"), value: odometer(d) });
    return out;
  });
</script>

<div class="ts {surface}" class:framed style:--ts-min="{minColumn}px">
  {#each rows as r (r.label)}
    <div class="cell" class:derived={r.derived} title={r.derived ? t("modpanel.derivedTooltip") : undefined}>
      <div class="lbl-key k">{r.label}</div>
      <div class="v">{r.value}</div>
    </div>
  {/each}
</div>

<style>
  /* Les lignes du quadrillage sont le fond qui transparaît dans l'interstice
     de 1px entre les cellules — pas des bordures, qui doubleraient d'épaisseur
     entre deux voisines. `auto-fit` : c'est la largeur reçue qui décide du
     nombre de colonnes, pas l'appelant. */
  .ts {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(var(--ts-min, 128px), 1fr));
    gap: 1px;
    background: var(--line);
  }
  /* Le trait extérieur, quand l'hôte n'encadre pas déjà la fiche lui-même. */
  .ts.framed {
    padding: 1px;
  }
  .cell {
    background: var(--panel);
    padding: 6px 9px;
    min-width: 0;
  }
  .ts.panel2 .cell {
    background: var(--panel2);
  }
  .k {
    text-transform: uppercase;
    margin-bottom: 3px;
  }
  .v {
    font-size: 11.5px;
    color: var(--txt2);
    font-family: var(--mono);
    overflow: hidden;
    text-overflow: ellipsis;
  }
  /* Vert = déduit par une règle, même code couleur que partout ailleurs. */
  .cell.derived .v {
    color: var(--green);
  }
</style>
