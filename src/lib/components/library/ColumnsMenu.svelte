<script lang="ts">
  // Which columns a table shows — the button and its tick list.
  //
  // It lived inline in the car/track library, and the opponent grid needs the
  // very same gesture (§4.2). Scoped CSS being what it is, a second copy would
  // have drifted at the first column added on one side only, with nothing to
  // flag it — the exact mechanism the shared-components chantier is about.
  //
  // **One axis of variation, and it has a reason**: `size` says where the
  // trigger lives, not how big it is. A library toolbar and a grid header do
  // not run at the same scale, and naming the prop after the place rather than
  // after a pixel value is what keeps it from becoming a grab bag.
  //
  // Behaviour is deliberately identical to the inline version it replaces,
  // including closing only through its own button: the library screen has to
  // behave exactly as before this extraction.
  import { t } from "$lib/i18n/index.svelte";

  export interface ColumnItem {
    key: string;
    label: string;
    /** Always shown, always ticked, never clickable — the column that IS the
     * row (a car's name, an opponent's car). */
    fixed?: boolean;
  }

  interface Props {
    items: ColumnItem[];
    /** Keys currently shown. `fixed` items count as shown whatever this says. */
    visible: string[];
    ontoggle: (key: string) => void;
    size?: "bar" | "header";
    /** One line under the list, when the available width limits what fits —
     * the grid at 600 px says how many more columns it can take (§4.3). */
    note?: string;
  }
  let { items, visible, ontoggle, size = "bar", note }: Props = $props();

  let open = $state(false);
</script>

<div class="wrap">
  <button
    class:btn={size === "bar"}
    class:hbtn={size === "header"}
    class:on={open}
    type="button"
    aria-expanded={open}
    onclick={() => (open = !open)}>{t("library.columns")}</button
  >
  {#if open}
    <div class="menu">
      {#each items as col (col.key)}
        <label class:fixed={col.fixed}>
          <input
            type="checkbox"
            checked={col.fixed || visible.includes(col.key)}
            disabled={col.fixed}
            onchange={() => ontoggle(col.key)}
          />
          <span>{col.label}</span>
        </label>
      {/each}
      {#if note}<p class="note">{note}</p>{/if}
    </div>
  {/if}
</div>

<style>
  .wrap {
    position: relative;
  }
  /* `.btn` is global and carries the toolbar trigger; only the header variant
     is local, because only the grid header needs it. Same shape as the
     `Regenerate` next to it — they are siblings, not a control and its label. */
  .hbtn {
    background: transparent;
    border: 1px solid var(--line);
    color: var(--muted);
    font-size: 8.5px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    padding: 3px 8px;
  }
  .hbtn:hover {
    background: var(--panel2);
    color: var(--txt2);
  }
  /* Level 2 of the red scale while open (§7.2ter): the menu is a surface one
     has deliberately opened, and the trigger says which one. */
  .on {
    border-color: var(--rosso-border);
    background: var(--rosso-dim);
    color: var(--rosso-bright);
  }
  .menu {
    position: absolute;
    top: calc(100% + 4px);
    right: 0;
    z-index: 20;
    background: var(--panel);
    border: 1px solid var(--line);
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 180px;
    box-shadow: 0 6px 18px rgb(0 0 0 / 40%);
  }
  .menu label {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: var(--txt2);
    padding: 3px 4px;
    cursor: pointer;
  }
  .menu label:hover {
    background: var(--raised);
  }
  .menu label.fixed {
    color: var(--muted);
    cursor: default;
  }
  .note {
    border-top: 1px solid var(--line);
    margin-top: 5px;
    padding: 7px 4px 1px;
    font-size: 11px;
    line-height: 1.45;
    color: var(--muted);
  }
</style>
