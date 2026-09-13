<script lang="ts" module>
  /** One line of the list. `meta` is whatever tells the entries apart at a
   * glance — a date for a session, a head count for a grid — and `badge` a
   * short mark on its origin (`From CM`). */
  export interface NamedEntry {
    name: string;
    meta?: string;
    badge?: string;
  }
</script>

<script lang="ts">
  // Save / load / delete a list of things the user has named.
  //
  // The gesture existed twice before this file — saved sessions and driver
  // outfits — with two implementations, and the user recognised it from one
  // screen to the other. Saved grids (§5) would have made three. So it is one
  // component now, and it is worth saying exactly what varies, because that is
  // what keeps a shared brick from becoming a grab bag:
  //
  //   `mode`        saving needs a name field and an overwrite confirmation;
  //                 loading needs neither, and a click means "take this one".
  //   `searchable`  a handful of sessions are read at a glance, thirty grids
  //                 are not (§5.2 — a searchable name beats a folder tree).
  //
  // Nothing else. The list, the delete cross, the empty state and the frame are
  // the same in both, and always were.
  import { t } from "$lib/i18n/index.svelte";

  interface Props {
    title: string;
    entries: NamedEntry[];
    mode: "save" | "load";
    /** Placeholder of the name field (`save`) — never a label explaining the
     * dialog's own workings. */
    placeholder?: string;
    emptyText: string;
    searchable?: boolean;
    /** `save`: the name typed or picked, overwrite already confirmed. */
    onsave?: (name: string) => void;
    /** `load`: the entry chosen. */
    onpick?: (name: string) => void;
    ondelete: (name: string) => void;
    onclose: () => void;
  }
  let { title, entries, mode, placeholder = "", emptyText, searchable = false, onsave, onpick, ondelete, onclose }: Props =
    $props();

  let name = $state("");
  let query = $state("");
  let confirmName = $state<string | null>(null);

  const shown = $derived(
    query.trim() ? entries.filter((e) => e.name.toLowerCase().includes(query.trim().toLowerCase())) : entries,
  );

  function activate(entry: NamedEntry) {
    if (mode === "load") {
      onpick?.(entry.name);
      return;
    }
    // Picking an existing name to overwrite it is faster than retyping it.
    name = entry.name;
    confirmName = null;
  }

  function submitSave() {
    const trimmed = name.trim();
    if (!trimmed) return;
    // Overwriting is confirmed once, in place, rather than by a second dialog:
    // the list right below already shows what is about to be replaced.
    if (entries.some((e) => e.name === trimmed) && confirmName !== trimmed) {
      confirmName = trimmed;
      return;
    }
    onsave?.(trimmed);
  }

  function remove(entry: NamedEntry, e: Event) {
    e.stopPropagation();
    ondelete(entry.name);
    if (confirmName === entry.name) confirmName = null;
  }
</script>

<div class="backdrop">
  <div class="modal">
    <header>
      <h2>{title}</h2>
      <button class="btn btn-ghost" type="button" onclick={onclose}>✕</button>
    </header>

    {#if mode === "save"}
      <div class="save-row">
        <input class="input" {placeholder} bind:value={name} onkeydown={(e) => e.key === "Enter" && submitSave()} />
        <button class="btn btn-primary" type="button" onclick={submitSave} disabled={!name.trim()}>
          {t("settings.save")}
        </button>
      </div>
      {#if confirmName}
        <div class="confirm-overwrite">
          <span>{t("launch.overwriteConfirm", { name: confirmName })}</span>
          <button class="btn btn-primary" type="button" onclick={() => onsave?.(confirmName ?? "")}>
            {t("launch.overwriteConfirmBtn")}
          </button>
          <button class="btn" type="button" onclick={() => (confirmName = null)}>{t("common.cancel")}</button>
        </div>
      {/if}
    {:else if searchable}
      <div class="save-row">
        <input class="input" {placeholder} bind:value={query} />
      </div>
    {/if}

    <div class="list">
      {#if !shown.length}
        <div class="empty">{query.trim() ? t("common.noResults") : emptyText}</div>
      {:else}
        {#each shown as e (e.name)}
          <button class="item" type="button" onclick={() => activate(e)}>
            <div class="item-b">
              <div class="item-name">{e.name}</div>
              {#if e.meta}<div class="item-meta mono">{e.meta}</div>{/if}
            </div>
            {#if e.badge}<span class="badge">{e.badge}</span>{/if}
            <span
              class="item-x"
              role="button"
              tabindex="-1"
              title={t("common.remove")}
              onclick={(ev) => remove(e, ev)}
              onkeydown={(ev) => ev.key === "Enter" && remove(e, ev)}>✕</span
            >
          </button>
        {/each}
      {/if}
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgb(0 0 0 / 60%);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }
  .modal {
    width: 460px;
    max-width: 92vw;
    max-height: 80vh;
    display: flex;
    flex-direction: column;
    background: var(--panel);
    border: 1px solid var(--rosso);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 16px;
    border-bottom: 1px solid var(--line);
    flex: none;
  }
  h2 {
    font-size: 13px;
    letter-spacing: 0.5px;
    text-transform: uppercase;
    color: var(--txt2);
  }
  .save-row {
    display: flex;
    gap: 8px;
    padding: 14px 16px 0;
    flex: none;
  }
  .save-row .input {
    flex: 1;
  }
  .confirm-overwrite {
    display: flex;
    align-items: center;
    gap: 10px;
    margin: 10px 16px 0;
    padding: 10px 12px;
    background: var(--rosso-dim);
    border: 1px solid var(--rosso-border);
    color: var(--rosso-bright);
    font-size: 11.5px;
    flex: none;
  }
  .confirm-overwrite span {
    flex: 1;
  }
  .list {
    overflow-y: auto;
    padding: 14px 16px 16px;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .empty {
    color: var(--faint);
    font-size: 12px;
    padding: 12px 0;
    text-align: center;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 9px 10px;
    background: var(--panel2);
    border: 1px solid var(--line);
    text-align: left;
  }
  .item:hover {
    background: var(--raised);
  }
  .item-b {
    flex: 1;
    min-width: 0;
  }
  .item-name {
    font-size: 12px;
    font-weight: 600;
    color: var(--txt);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .item-meta {
    font-size: 10px;
    color: var(--faint);
    margin-top: 2px;
  }
  /* Origin mark, never a state: neutral, and it says a word rather than a
     colour — a grid that came from Content Manager is not better or worse. */
  .badge {
    flex: none;
    font-family: var(--mono);
    font-size: 8px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    border: 1px solid var(--line);
    color: var(--muted);
    padding: 2px 5px;
  }
  .item-x {
    flex: none;
    color: var(--muted2);
    font-size: 12px;
    padding: 2px 4px;
  }
  .item-x:hover {
    color: var(--rosso-bright);
  }
</style>
