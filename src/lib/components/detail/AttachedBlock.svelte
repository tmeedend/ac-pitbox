<script lang="ts">
  // Ce qui est posé sur ce mod (§4.3). Sans ce bloc, une notice
  // livrée avec la voiture n'était atteignable que par l'inventaire :
  // ses fichiers appartiennent au mod qui la porte, pas à la voiture,
  // donc l'onglet Médias ne la montrera jamais.
  // C'est aussi ce qui rend le rattachement déduit sans danger : une
  // déduction ratée coûte un raccourci manquant ici, jamais un mod
  // introuvable — il reste dans l'inventaire quoi qu'il arrive.
  //
  // Only "other" mods have a sheet of their own; the other rows are listed,
  // not opened.
  import type { InventoryRow } from "$lib/inventory/inventory";
  import { t } from "$lib/i18n/index.svelte";

  interface Props {
    rows: InventoryRow[];
    onopen: (row: InventoryRow) => void;
  }
  let { rows, onopen }: Props = $props();
</script>

{#if rows.length}
  <section class="blk">
    <header class="blk-h">
      <span class="blk-t">{t("detail.attachedTitle")}</span>
      <span class="blk-n">{rows.length}</span>
    </header>
    <div class="blk-b">
      <ul class="attached">
        {#each rows as a (a.uid)}
          <li>
            <button
              class="a-row"
              type="button"
              disabled={!a.uid.startsWith("OTHER:")}
              title={a.uid.startsWith("OTHER:") ? t("inventory.openFiche") : t("inventory.noFiche")}
              onclick={() => onopen(a)}
            >
              <span class="a-name">{a.name}</span>
              <span class="a-type mono">{t(`inventory.type${a.kind}`)}</span>
              {#if a.active === false}<span class="a-off mono">{t("common.inactive")}</span>{/if}
            </button>
          </li>
        {/each}
      </ul>
    </div>
  </section>
{/if}

<style>
  .attached {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .a-row {
    width: 100%;
    display: flex;
    align-items: baseline;
    gap: 10px;
    background: transparent;
    padding: 4px 2px;
    text-align: left;
  }
  .a-row:hover:not(:disabled) {
    background: var(--raised);
  }
  .a-row:disabled {
    cursor: default;
  }
  .a-name {
    flex: 1;
    min-width: 0;
    color: var(--txt2);
    font-size: 11.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .a-type {
    flex: none;
    color: var(--muted2);
    font-size: 9px;
    letter-spacing: 1px;
    text-transform: uppercase;
  }
  .a-off {
    flex: none;
    color: var(--orange);
    font-size: 9px;
  }
</style>
