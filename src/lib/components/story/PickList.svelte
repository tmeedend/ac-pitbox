<script lang="ts">
  // The list beside a section's sheet - the Events, the Contents: entries in
  // titled groups, the selected one marked, an error badge on those whose
  // file has one, and what adds an entry below (`children`).
  import type { Snippet } from "svelte";

  export interface PickEntry {
    /** What `selected` and `onselect` speak of: a file, an id. */
    key: string;
    label: string;
    /** A short word on the right: a type, a placement. */
    sub: string;
    error?: boolean;
  }

  export interface PickGroup {
    title: string;
    entries: PickEntry[];
  }

  interface Props {
    groups: PickGroup[];
    selected: string | null;
    onselect: (key: string) => void;
    children?: Snippet;
  }
  const { groups, selected, onselect, children }: Props = $props();
</script>

<nav class="list">
  {#each groups as g, i (i)}
    {#if g.entries.length}
      <div class="lbl group">{g.title}</div>
      <ul>
        {#each g.entries as e (e.key)}
          <li>
            <button class="item" class:on={selected === e.key} onclick={() => onselect(e.key)}>
              <span class="name">{e.label}</span>
              <span class="sub mono">{e.sub}</span>
              {#if e.error}<span class="pill pill-err">!</span>{/if}
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  {/each}
  {@render children?.()}
</nav>

<style>
  .list {
    width: 250px;
    flex-shrink: 0;
  }
  .list ul {
    list-style: none;
    margin: 0 0 10px;
    padding: 0;
  }
  .group {
    margin: 12px 0 4px;
  }
  .group:first-child {
    margin-top: 0;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    text-align: left;
    padding: 6px 10px;
    background: transparent;
    color: var(--txt2);
    font-size: 13px;
  }
  .item:hover {
    background: var(--raised);
  }
  .item.on {
    background: var(--raised);
    color: var(--txt);
    box-shadow: inset 2px 0 0 var(--rosso);
  }
  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sub {
    color: var(--muted);
    font-size: 10.5px;
    flex-shrink: 0;
  }
</style>
