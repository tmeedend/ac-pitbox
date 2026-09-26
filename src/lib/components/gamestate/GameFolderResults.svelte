<script lang="ts">
  // The search results (DOSSIER§7.3): three groups - mods, folders, files -
  // fifty lines each with "see the N others". The mods group is what answers
  // "do I have this mod": it finds library elements whether they are laid or
  // not, and says where each one is (DOSSIER§7.4).
  import { t } from "$lib/i18n/index.svelte";
  import { highlightParts } from "$lib/gamestate/highlight";
  import StatePills from "./StatePills.svelte";
  import type { ModHit, PathHit, SearchResults } from "$lib/gamestate/gamestate";

  interface Props {
    results: SearchResults;
    query: string;
    onpickmod: (hit: ModHit) => void;
    onpickpath: (hit: PathHit) => void;
    onfilter: (hit: ModHit) => void;
    onmore: (group: "mods" | "dirs" | "files") => void;
  }
  let { results, query, onpickmod, onpickpath, onfilter, onmore }: Props = $props();

  const empty = $derived(results.mods.total + results.dirs.total + results.files.total === 0);
  const PRESENCE_TONE: Record<string, string> = { inGame: "pill-ok" };
</script>

{#snippet marked(text: string)}
  {#each highlightParts(text, query) as p, i (i)}{#if p.hit}<mark>{p.text}</mark>{:else}{p.text}{/if}{/each}
{/snippet}

{#snippet more(group: "mods" | "dirs" | "files", total: number, shown: number)}
  {#if total > shown}
    <button type="button" class="more" onclick={() => onmore(group)}>
      {t("gamefolder.seeOthers", { count: (total - shown).toLocaleString() })}
    </button>
  {/if}
{/snippet}

{#snippet paths(title: string, group: "dirs" | "files", list: { total: number; items: PathHit[] })}
  {#if list.total}
    <section class="rg">
      <h3 class="rgh lbl">{title} <b>{list.total.toLocaleString()}</b></h3>
      {#each list.items as hit (hit.row.id)}
        <button type="button" class="rr" onclick={() => onpickpath(hit)}>
          <span class="a">
            <span class="n mono">{@render marked(hit.row.name)}</span>
            <span class="s mono">{@render marked(hit.parent || "\\")}</span>
          </span>
          <span class="prov">{hit.row.provider ?? ""}</span>
          <StatePills row={hit.row} />
        </button>
      {/each}
      {@render more(group, list.total, list.items.length)}
    </section>
  {/if}
{/snippet}

<div class="results">
  {#if empty}
    <p class="none">{t("gamefolder.noResult", { query })}</p>
  {:else}
    {#if results.mods.total}
      <section class="rg">
        <h3 class="rgh lbl">{t("gamefolder.groupMods")} <b>{results.mods.total.toLocaleString()}</b></h3>
        {#each results.mods.items as hit (`${hit.owner.kind}:${hit.owner.id}`)}
          <div class="rr mod">
            <button type="button" class="a pick" onclick={() => onpickmod(hit)}>
              <span class="n">
                {@render marked(hit.owner.name)}
                <span class="kind lbl-key">{t(`gamefolder.ownerKind.${hit.owner.kind}`)}</span>
              </span>
              <span class="s mono">{@render marked(hit.owner.id)}</span>
            </button>
            <span class="pres">
              <span class="pill {PRESENCE_TONE[hit.presence] ?? ''}">{t(`gamefolder.presence.${hit.presence}`)}</span>
              {#if hit.drift}<span class="pill pill-warn"><span aria-hidden="true">≠</span>{t("gamefolder.state.drift")}</span>{/if}
            </span>
            <button type="button" class="btn btn-ghost f" onclick={() => onfilter(hit)}>{t("gamefolder.filter")}</button>
          </div>
        {/each}
        {@render more("mods", results.mods.total, results.mods.items.length)}
      </section>
    {/if}
    {@render paths(t("gamefolder.groupDirs"), "dirs", results.dirs)}
    {@render paths(t("gamefolder.groupFiles"), "files", results.files)}
  {/if}
</div>

<style>
  .results {
    height: 100%;
    overflow-y: auto;
    padding: 4px 0 20px;
  }
  .rg {
    padding: 6px 0 12px;
  }
  .rgh {
    padding: 6px 16px;
    gap: 10px;
    margin-bottom: 0;
  }
  .rgh b {
    font-family: var(--mono);
    font-weight: 400;
    color: var(--txt2);
    letter-spacing: 0;
    font-size: 11px;
  }
  .rr {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto auto;
    align-items: center;
    gap: 12px;
    width: 100%;
    padding: 6px 16px;
    background: none;
    text-align: left;
    color: var(--txt2);
  }
  .rr:hover {
    background: var(--panel);
  }
  .a {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .pick {
    background: none;
    text-align: left;
    padding: 0;
  }
  .n {
    font-size: 12.5px;
    color: var(--txt);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .n.mono {
    font-size: 12px;
  }
  .s {
    font-size: 11px;
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .kind {
    text-transform: uppercase;
    margin-left: 8px;
  }
  .prov {
    font-size: 11.5px;
    color: var(--txt2);
    max-width: 220px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .pres {
    display: inline-flex;
    gap: 4px;
  }
  .pres .pill {
    font-size: 10.5px;
    padding: 1px 6px;
  }
  .f {
    padding: 4px 10px;
    font-size: 11px;
  }
  .more {
    background: none;
    color: var(--blue);
    font-size: 11.5px;
    padding: 4px 16px;
  }
  .more:hover {
    text-decoration: underline;
  }
  .none {
    padding: 28px 16px;
    color: var(--txt2);
    font-size: 13px;
    max-width: 60ch;
  }
  /* The match, in the yellow of focus - text only: the design system has no
     yellow surface to lay under it, and a new one is not worth a highlight. */
  mark {
    background: none;
    color: var(--yellow);
  }
</style>
