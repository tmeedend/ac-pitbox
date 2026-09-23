<script lang="ts">
  // Categories tab of the Workshop (TAXO§6): the family table the car index
  // and the Family filter read (INDEX§6.1).
  //
  // One row per family, unfolded in place. What a family is made of is a list
  // of TAGS — a relation of parenthood, not of identity (TAXO§7.3): attaching
  // `lmp1` to Prototype does not hide the tag, it stays filterable on its own.
  //
  // **Every change is written at once**, through `saveCategoryFamilies`, which
  // stores the table and nothing else: no re-harmonisation of the library, a
  // family being an index over tags and not a rule. Writes go through a queue
  // whose failures are shown and logged — a write that fails in silence is the
  // bug rule 6 of CLAUDE.md was written for.
  import { onMount } from "svelte";
  import { errorText } from "$lib/errors";
  import { t } from "$lib/i18n/index.svelte";
  import { familiesOfTags, familyLookup, familyTag, type CategoryFamily } from "$lib/library/families";
  import { FAMILY_ICONS, NEUTRAL_ICON, familyIcon } from "$lib/library/familyIcons";
  import { setFamilies } from "$lib/library/familyTable.svelte";
  import { listLibrary } from "$lib/library/library";
  import { modTags } from "$lib/library/cardSearch";
  import {
    addFamily,
    deleteFamily,
    isCurated,
    moveTag,
    patchFamily,
    removeTag,
    restoreFamily,
    tagCounts,
  } from "$lib/workshop/familyEdit";
  import { defaultCategoryFamilies, getRules, saveCategoryFamilies } from "$lib/workshop/rules";

  let families = $state<CategoryFamily[]>([]);
  let shipped = $state<CategoryFamily[]>([]);
  /** Tags of every car, all origins merged — what the index reads. */
  let cars = $state<string[][]>([]);
  let loading = $state(true);
  let error = $state("");
  let open = $state<string | null>(null);
  let tagQuery = $state("");
  let newName = $state("");
  /** Two-step destructive buttons: the first click arms, the second acts. */
  let armed = $state<string | null>(null);

  onMount(async () => {
    try {
      const [rules, defaults, cards] = await Promise.all([getRules(), defaultCategoryFamilies(), listLibrary()]);
      families = rules.car.category_families ?? [];
      shipped = defaults;
      cars = cards.filter((c) => c.kind === "Car").map(modTags);
    } catch (e) {
      error = errorText(e);
    } finally {
      loading = false;
    }
  });

  let queue: Promise<void> = Promise.resolve();
  /** Shows the new table at once, then writes it. The stored table comes back
   * normalised and replaces the local one, so the screen says what the file
   * says. The `catch` is not optional: a rejected promise in the chain would
   * freeze every later write until the next start. */
  function commit(next: CategoryFamily[]) {
    families = next;
    armed = null;
    queue = queue
      .then(async () => {
        const stored = await saveCategoryFamilies(next);
        families = stored;
        setFamilies(stored);
        error = "";
      })
      .catch((e) => {
        console.error("save_category_families", e);
        error = errorText(e);
      });
  }

  const lookup = $derived(familyLookup(families));
  const counts = $derived.by(() => {
    const m = new Map<string, number>();
    let none = 0;
    for (const tags of cars) {
      const ids = familiesOfTags(tags, lookup, families);
      if (!ids.length) none++;
      for (const id of ids) m.set(id, (m.get(id) ?? 0) + 1);
    }
    return { byFamily: m, none };
  });
  const perTag = $derived(tagCounts(cars));
  /** Largest first, as in every list of terms (TAXO§6.1): the ones that
   * matter are the ones seen most. */
  const rows = $derived(
    [...families].sort((a, b) => (counts.byFamily.get(b.id) ?? 0) - (counts.byFamily.get(a.id) ?? 0)),
  );
  const shippedOf = (id: string) => shipped.find((s) => s.id === id);

  function nameOf(f: CategoryFamily): string {
    return f.name ?? t(`families.${f.id}`);
  }

  /** Tags offered in the add field: those the library carries, matching what
   * is typed, largest first — a tag no car carries attaches nothing. Each
   * says which family holds it now, since attaching it here moves it. */
  function suggestions(f: CategoryFamily): { tag: string; count: number; owner: string | null }[] {
    const q = familyTag(tagQuery);
    const mine = new Set(f.tags.map(familyTag));
    return [...perTag.entries()]
      .filter(([tag]) => !mine.has(tag) && (!q || tag.includes(q)))
      .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
      .slice(0, 8)
      .map(([tag, count]) => {
        const owner = lookup.get(tag);
        const of = owner ? families.find((x) => x.id === owner) : undefined;
        return { tag, count, owner: of ? nameOf(of) : null };
      });
  }

  function attach(f: CategoryFamily, tag: string) {
    if (!familyTag(tag)) return;
    commit(moveTag(families, tag, f.id));
    tagQuery = "";
  }

  function toggle(id: string) {
    open = open === id ? null : id;
    tagQuery = "";
    armed = null;
  }

  function create() {
    const name = newName.trim();
    if (!name) return;
    const { families: next, id } = addFamily(families, name);
    commit(next);
    newName = "";
    open = id;
  }

  function arm(key: string, act: () => void) {
    if (armed === key) act();
    else armed = key;
  }
</script>

<div class="categories">
  <!-- Workshop tab: the screen title belongs to the Workshop. -->
  <header>
    <p class="lbl-sub">{t("categories.subtitle")}</p>
    {#if !loading}
      <p class="none-count">{t("categories.unclassified", { count: counts.none })}</p>
    {/if}
  </header>

  {#if error}<div class="errbox">{error}</div>{/if}

  {#if !loading}
    <ul class="list">
      {#each rows as f (f.id)}
        {@const n = counts.byFamily.get(f.id) ?? 0}
        {@const base = shippedOf(f.id)}
        {@const curated = isCurated(f, base)}
        <li class:open={open === f.id}>
          <button type="button" class="row" aria-expanded={open === f.id} onclick={() => toggle(f.id)}>
            <svg class="ico" viewBox="0 0 120 52" aria-hidden="true">{@html familyIcon(f.icon)}</svg>
            <span class="nm">{nameOf(f)}</span>
            <span class="num">{t("index.cars", { count: n })}</span>
            <span class="num">{t("categories.tagCount", { count: f.tags.length })}</span>
            <span class="flag" class:on={curated} title={curated ? t("categories.curated") : undefined} aria-hidden={!curated}
              >⚑</span
            >
            <span class="chev" aria-hidden="true">{open === f.id ? "▾" : "›"}</span>
          </button>

          {#if open === f.id}
            <div class="detail">
              <label class="field">
                <span class="lbl-key">{t("categories.name")}</span>
                <input
                  class="input"
                  value={f.name ?? ""}
                  placeholder={base ? t(`families.${f.id}`) : ""}
                  onchange={(e) => commit(patchFamily(families, f.id, { name: e.currentTarget.value }))}
                />
              </label>

              <div class="field">
                <span class="lbl-key">{t("categories.icon")}</span>
                <div class="icons" role="radiogroup" aria-label={t("categories.icon")}>
                  {#each [...Object.keys(FAMILY_ICONS), ""] as icon (icon)}
                    <button
                      type="button"
                      class="pick"
                      role="radio"
                      aria-checked={(f.icon ?? "") === icon}
                      aria-label={icon || t("categories.neutralIcon")}
                      onclick={() => commit(patchFamily(families, f.id, { icon: icon || undefined }))}
                    >
                      <svg viewBox="0 0 120 52" aria-hidden="true">{@html icon ? FAMILY_ICONS[icon] : NEUTRAL_ICON}</svg>
                    </button>
                  {/each}
                </div>
              </div>

              <div class="field">
                <span class="lbl-key">{t("categories.tags")}</span>
                <div class="tags">
                  {#each f.tags as tag (tag)}
                    <span class="tok">
                      {tag}<span class="c">{perTag.get(familyTag(tag)) ?? 0}</span>
                      <button
                        type="button"
                        class="x"
                        title={t("categories.removeTag")}
                        onclick={() => commit(removeTag(families, f.id, tag))}>×</button
                      >
                    </span>
                  {:else}
                    <span class="empty">{t("categories.noTag")}</span>
                  {/each}
                </div>
                <input
                  class="input add"
                  placeholder={t("categories.addTag")}
                  bind:value={tagQuery}
                  onkeydown={(e) => {
                    if (e.key !== "Enter") return;
                    e.preventDefault();
                    attach(f, suggestions(f)[0]?.tag ?? tagQuery);
                  }}
                />
                <ul class="sugg">
                  {#each suggestions(f) as s (s.tag)}
                    <li>
                      <button type="button" onclick={() => attach(f, s.tag)}>
                        <span class="st">{s.tag}</span>
                        <span class="c">{s.count}</span>
                        {#if s.owner}<span class="owner">{t("categories.inFamily", { name: s.owner })}</span>{/if}
                      </button>
                    </li>
                  {/each}
                </ul>
              </div>

              <div class="actions">
                {#if base && curated}
                  <button type="button" class="btn" onclick={() => commit(restoreFamily(families, shipped, f.id))}>
                    {t("categories.restore")}
                  </button>
                {/if}
                <button
                  type="button"
                  class="btn"
                  onclick={() =>
                    arm(`del:${f.id}`, () => {
                      commit(deleteFamily(families, f.id));
                      open = null;
                    })}
                >
                  {armed === `del:${f.id}` ? t("categories.confirmDelete") : t("common.delete")}
                </button>
              </div>
            </div>
          {/if}
        </li>
      {/each}
    </ul>

    <div class="foot">
      <input
        class="input"
        placeholder={t("categories.newPlaceholder")}
        bind:value={newName}
        onkeydown={(e) => e.key === "Enter" && create()}
      />
      <button type="button" class="btn btn-primary" disabled={!newName.trim()} onclick={create}>
        {t("categories.create")}
      </button>
      <button
        type="button"
        class="btn reset"
        onclick={() =>
          arm("reset", () => {
            commit(shipped.map((f) => ({ ...f, tags: [...f.tags] })));
            open = null;
          })}
      >
        {armed === "reset" ? t("categories.confirmResetAll") : t("categories.resetAll")}
      </button>
    </div>
  {/if}
</div>

<style>
  .categories {
    max-width: 820px;
  }
  header {
    margin-bottom: 14px;
  }
  .none-count {
    margin: 4px 0 0;
    font-size: 11.5px;
    color: var(--muted2);
  }
  .errbox {
    margin-bottom: 12px;
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    border-top: 1px solid var(--line);
  }
  .list > li {
    border-bottom: 1px solid var(--line);
  }
  .row {
    width: 100%;
    display: grid;
    grid-template-columns: 46px 1fr 110px 80px 18px 16px;
    align-items: center;
    gap: 12px;
    padding: 8px 10px;
    background: none;
    color: var(--txt);
    font: inherit;
    font-size: 12.5px;
    text-align: left;
  }
  .row:hover {
    background: var(--raised);
  }
  .row:focus-visible,
  .pick:focus-visible,
  .sugg button:focus-visible {
    outline: 2px solid var(--rosso);
    outline-offset: -2px;
  }
  /* 20 px in the list (TAXO§6.1). The holes are painted in the background of
     the row, as on the index tiles. */
  .ico {
    height: 20px;
    width: auto;
    fill: var(--txt2);
  }
  .ico :global(.cut),
  .pick :global(.cut) {
    fill: var(--panel);
  }
  .ico :global(.smoke),
  .pick :global(.smoke) {
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    opacity: 0.5;
  }
  .num {
    color: var(--muted);
    font-size: 11.5px;
  }
  /* The curation flag: the dimmed red of the accent scale, a state and not an
     action (TAXO§11). Invisible but kept in place when off, so the columns do
     not dance from one row to the next. */
  .flag {
    color: var(--rosso-border);
    visibility: hidden;
  }
  .flag.on {
    visibility: visible;
  }
  .chev {
    color: var(--muted2);
  }

  .detail {
    padding: 6px 10px 16px 68px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .field .input {
    max-width: 320px;
  }
  .icons {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .pick {
    width: 58px;
    height: 34px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--panel);
    border: 1px solid var(--line);
    color: var(--txt2);
  }
  .pick svg {
    height: 20px;
    width: auto;
    fill: var(--txt2);
  }
  .pick:hover {
    border-color: var(--faint2);
  }
  .pick[aria-checked="true"] {
    border-color: var(--rosso-border);
    background: var(--rosso-dim);
  }
  .pick[aria-checked="true"] :global(.cut) {
    fill: var(--rosso-dim);
  }
  .tags {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .tok {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 24px;
    padding: 0 2px 0 8px;
    border: 1px solid var(--line);
    background: var(--panel2);
    font-size: 12px;
  }
  .c {
    color: var(--muted2);
    font-size: 10.5px;
  }
  .x {
    width: 18px;
    height: 18px;
    background: none;
    color: var(--muted2);
  }
  .x:hover {
    color: var(--txt);
    background: var(--raised);
  }
  .empty {
    color: var(--muted2);
    font-size: 12px;
  }
  .sugg {
    list-style: none;
    margin: 0;
    padding: 0;
    max-width: 320px;
  }
  .sugg button {
    width: 100%;
    display: flex;
    align-items: baseline;
    gap: 8px;
    padding: 4px 8px;
    background: none;
    color: var(--txt2);
    font: inherit;
    font-size: 12px;
    text-align: left;
  }
  .sugg button:hover {
    background: var(--raised);
    color: var(--txt);
  }
  .owner {
    margin-left: auto;
    color: var(--muted2);
    font-size: 11px;
  }
  .actions {
    display: flex;
    gap: 8px;
  }
  .foot {
    display: flex;
    gap: 8px;
    margin-top: 16px;
  }
  .foot .input {
    width: 260px;
  }
  .reset {
    margin-left: auto;
  }
</style>
