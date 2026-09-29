<script lang="ts">
  // The tech sheet of a car, read mode (FICHE§7.1): the key figures in a strip,
  // the mechanics in short sentences, the electronics as chips.
  //
  // Everything shown comes from the base (`techsheet::effective`), never from a
  // file — which is what keeps the sheet whole for a mod whose files are gone.
  // Two signs, and no colour, say where a value comes from (R5): "≈" before
  // what the tags deduced, "✎" after what the user corrected. Each sign names
  // its meaning on hover rather than in a legend line, which cost a row of
  // height on every sheet that carried one. A native `title`, not `Tooltip`:
  // the key figures clip their overflow (ellipsis), which would cut the bubble.
  import { countryLabel, flagFor, loadFlags } from "$lib/flags.svelte";
  import { i18n, t } from "$lib/i18n/index.svelte";
  import {
    aidChips,
    fieldLabel,
    isDerived,
    isEdited,
    keyFigures,
    mechanicsRows,
    type TechSheet,
    type TechSource,
  } from "$lib/detail/techSheet";
  import AidChip from "./AidChip.svelte";
  import FigureGrid from "./FigureGrid.svelte";
  import SheetRow from "./SheetRow.svelte";

  let { sheet }: { sheet: TechSheet } = $props();

  // Idempotent: the filter bar may already have loaded the flags.
  $effect(() => {
    void loadFlags();
  });

  const figures = $derived(keyFigures(sheet, i18n.locale));
  const rows = $derived(mechanicsRows(sheet, { locale: i18n.locale, countryLabel, flagFor }));
  const chips = $derived(aidChips(sheet));
</script>

{#snippet marked(source: TechSource, text: string)}
  {#if isDerived(source)}<span class="sign" title={t("techsheet.source.rules")}>≈</span>{/if}{text}{#if isEdited(source)}<span
      class="sign after"
      title={t("techsheet.source.user")}>✎</span
    >{/if}
{/snippet}

{#if figures.length}
  <FigureGrid>
    {#each figures as f (f.field)}
      <div>
        <div class="lbl-key cap">{fieldLabel(f.field)}</div>
        <div class="fig-v">
          {#if f.text}
            <span class="fig-text">{@render marked(f.source, f.text)}</span>
          {:else}
            {@render marked(f.source, f.num ?? "")}{#if f.unit}<small>{f.unit}</small>{/if}
          {/if}
        </div>
      </div>
    {/each}
  </FigureGrid>
{/if}

{#if rows.length}
  <div class="section">
    <div class="blk-sub">{t("techsheet.mechanics")}</div>
    {#each rows as r (r.key)}
      <SheetRow label={t(`techsheet.row.${r.key}`)}>
        <div class="parts">
          {#each r.parts as p, i (i)}
            <span class="part">{#if p.flag}<img class="flag" src={p.flag} alt="" />{/if}{@render marked(p.source, p.text)}</span>
          {/each}
        </div>
      </SheetRow>
    {/each}
  </div>
{/if}

{#if chips.length}
  <div class="section">
    <div class="blk-sub">{t("techsheet.electronics")}</div>
    <div class="chips">
      {#each chips as c (c.field)}
        <AidChip state={c.present} sub={c.sub}>{@render marked(c.source, fieldLabel(c.field))}</AidChip>
      {/each}
    </div>
  </div>
{/if}

{#if !figures.length && !rows.length && !chips.length}
  <p class="empty">{t("techsheet.empty")}</p>
{/if}

<style>
  .cap {
    text-transform: uppercase;
    margin-bottom: 4px;
  }
  .fig-v {
    font-family: var(--mono);
    font-size: 17px;
    color: var(--txt);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .fig-v small {
    font-size: 10.5px;
    color: var(--muted);
    margin-left: 3px;
  }
  /* The author's own words, when they were not a number: smaller, so a long
     one does not fight the figures beside it. */
  .fig-text {
    font-size: 11.5px;
    color: var(--txt2);
  }

  .section {
    padding: 12px 14px 6px;
  }
  .section + .section {
    border-top: 1px solid var(--line);
  }
  .parts {
    display: flex;
    flex-wrap: wrap;
    font-size: 12px;
    color: var(--txt2);
    line-height: 1.5;
  }
  .part {
    white-space: nowrap;
  }
  /* The separator ends a part rather than starting the next: when the sentence
     wraps in a narrow sheet, a line then never opens on a "·". */
  .part:not(:last-child)::after {
    content: "·";
    color: var(--faint);
    margin: 0 7px;
  }
  /* Same flag size as the rest of the app (filters, grid nationality). */
  .flag {
    width: 16px;
    height: 12px;
    object-fit: cover;
    border: 1px solid var(--line);
    vertical-align: -1px;
    margin-right: 5px;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    padding: 2px 0 6px;
  }

  /* R5: the signs are grey, the value keeps its colour. */
  .sign {
    color: var(--muted);
    font-family: var(--mono);
    margin-right: 4px;
    cursor: help;
  }
  .sign.after {
    margin: 0 0 0 4px;
    font-size: 0.8em;
  }
  .empty {
    padding: 14px;
    font-size: 11.5px;
    color: var(--muted);
  }
</style>
