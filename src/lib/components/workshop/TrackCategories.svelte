<script lang="ts">
  // The track half of Classement › Families (TAXO§6.4): the closed vocabulary
  // of track categories, out of the Rules tab - it is a list of terms, not a
  // heuristic, the same reason brands and countries have their own tab. The
  // Rules tab keeps the merge rules that PRODUCE these categories; each row
  // here shows the ones that produce it.
  //
  // A category is its `#tag`, fixed once written. Three kinds of decision, each
  // saved where it lives (`trackCategoryEdit.ts`): order and on/off in the
  // rules overlay, re-applied to the library because they change the
  // category a track stores; name and icon in the taxonomy overlay, display
  // only; the Online page's words in the same overlay. Every gesture is
  // written at once, through one queue whose failures are shown and logged.
  import { onMount } from "svelte";
  import { errorText } from "$lib/errors";
  import { writeQueue } from "$lib/workshop/writeQueue";
  import { t } from "$lib/i18n/index.svelte";
  import Switch from "$lib/components/ui/Switch.svelte";
  import TrackCategoryIcon from "$lib/components/library/TrackCategoryIcon.svelte";
  import { TRACK_CATEGORY_ICONS } from "$lib/library/trackCategoryIcons";
  import { categoryIconId, categoryName, categoryTag, tagForName } from "$lib/library/trackCategoryNames";
  import { setTrackCategoryLooks } from "$lib/library/trackCategoryTable.svelte";
  import { bumpLibraryVersion } from "$lib/library/libraryVersion.svelte";
  import {
    getRulesView,
    getTaxonomy,
    saveRulesOverlay,
    saveServerKeywords,
    saveTrackCategoryLooks,
    type CategoryLook,
    type CategoryRow,
    type MapOverlay,
    type RulesOverlay,
    type RulesView,
    type TaxonomyView,
  } from "$lib/workshop/rules";
  import { addCategory, moveCategory, removeCategory, setCategoryOn, setEnabled } from "$lib/workshop/rulesEdit";
  import { showRule } from "$lib/workshop/rulesFocus.svelte";
  import {
    addKeyword,
    goneKeywords,
    isCurated,
    keywordsOf,
    producers,
    removeKeyword,
    restoreKeyword,
    setLook,
    withoutLook,
  } from "$lib/workshop/trackCategoryEdit";

  let rules = $state<RulesView | null>(null);
  let taxonomy = $state<TaxonomyView | null>(null);
  let error = $state("");
  let busy = $state(false);
  let open = $state<string | null>(null);
  let newName = $state("");
  let newWord = $state("");
  /** Two-step destructive buttons: the first click arms, the second acts. */
  let armed = $state<string | null>(null);

  onMount(async () => {
    try {
      [rules, taxonomy] = await Promise.all([getRulesView(), getTaxonomy()]);
    } catch (e) {
      console.error("track categories", e);
      error = errorText(e);
    }
  });

  const looks = $derived<Record<string, CategoryLook>>(taxonomy?.overlay.track_category_looks ?? {});
  const rows = $derived<CategoryRow[]>(rules?.track_categories ?? []);
  /** The categories on, in their order: rank 1 is a track's main category. */
  const effective = $derived(rows.filter((r) => r.on).map((r) => r.name));
  const shipped = $derived(rows.filter((r) => r.shipped).map((r) => r.name));
  const keywords = $derived<Record<string, string>>(taxonomy?.effective.server_keywords ?? {});
  const keywordCatalog = $derived<Record<string, string>>(taxonomy?.catalog.server_keywords ?? {});

  const nameOf = (tag: string) => categoryName(tag, looks, t);

  const queue = writeQueue({ busy: (b) => (busy = b), error: (text) => (error = text) });

  /** One write at a time, in the order of the gestures; a gesture disarms the
   * two-step buttons. */
  function enqueue(label: string, write: () => Promise<void>) {
    armed = null;
    void queue(label, write);
  }

  /** Order, on/off, a category added or removed: re-applied to the library. */
  function commitRules(next: RulesOverlay) {
    enqueue("save_rules_overlay", async () => {
      rules = await saveRulesOverlay(next);
      bumpLibraryVersion();
    });
  }

  /** A name or an icon: display only, nothing re-applied. */
  function commitLooks(next: Record<string, CategoryLook>) {
    enqueue("save_track_category_looks", async () => {
      taxonomy = await saveTrackCategoryLooks(next);
      setTrackCategoryLooks(taxonomy.overlay.track_category_looks ?? {});
    });
  }

  function commitKeywords(next: MapOverlay) {
    enqueue("save_server_keywords", async () => {
      taxonomy = await saveServerKeywords(next);
    });
  }

  const keywordOverlay = () => taxonomy?.overlay.server_keywords ?? {};

  function toggle(tag: string) {
    open = open === tag ? null : tag;
    newWord = "";
    armed = null;
  }

  function create() {
    const name = newName.trim();
    if (!name || !rules) return;
    const tag = tagForName(name, rows.map((r) => r.name));
    commitRules(addCategory(rules.overlay, tag, shipped));
    commitLooks(setLook(looks, tag, { name }));
    newName = "";
    open = tag;
  }

  /** A shipped category as Pit Box ships it, but for its place in the list:
   * the order is one decision over the whole list (`Restore all`). */
  function restore(row: CategoryRow) {
    if (!rules) return;
    if (looks[row.name]) commitLooks(withoutLook(looks, row.name));
    if (!row.on) commitRules(setCategoryOn(rules.overlay, row.name, true));
  }

  function restoreAll() {
    if (!rules) return;
    commitRules({ ...rules.overlay, track_categories: {} });
    commitLooks({});
    open = null;
  }

  function arm(key: string, act: () => void) {
    if (armed === key) act();
    else armed = key;
  }

  const tracksText = (n: number) => t(n === 1 ? "index.trackOne" : "index.tracks", { count: n });
</script>

<div class="cats">
  {#if error}<div class="errbox">{error}</div>{/if}

  {#if !rules || !taxonomy}
    {#if !error}<p class="empty">{t("common.loading")}</p>{/if}
  {:else}
    <ul class="list">
      {#each rows as row (row.name)}
        {@const i = effective.indexOf(row.name)}
        {@const curated = isCurated(row, rows, looks)}
        <li class:open={open === row.name} class:off={!row.on}>
          <div class="row">
            <button type="button" class="head" aria-expanded={open === row.name} onclick={() => toggle(row.name)}>
              <span class="ico"><TrackCategoryIcon tag={row.name} size={20} /></span>
              <span class="nm"
                >{nameOf(row.name)}{#if row.shipped}<span class="badge" title={t("taxonomyOrigin.shippedTitle")}
                    >PIT BOX</span
                  >{/if}</span
              >
              <span class="num"
                >{row.on ? tracksText(row.effect) : t("common.inactive")}{#if row.on && row.masked}<span
                    class="masked"
                    title={t("rules.maskedTitle", { count: row.masked })}> · ⚑ {row.masked}</span
                  >{/if}</span
              >
              <span class="flag" class:on={curated} title={curated ? t("categories.curated") : undefined} aria-hidden={!curated}
                >⚑</span
              >
            </button>
            <span class="rank mono" title={i === 0 ? t("trackCats.main") : undefined}>{row.on ? i + 1 : ""}</span>
            <button
              type="button"
              class="arrow"
              title={t("rules.moveUp")}
              aria-label={t("rules.moveUp")}
              disabled={busy || !row.on || i === 0}
              onclick={() => rules && commitRules(moveCategory(rules.overlay, effective, i, -1))}>↑</button
            >
            <button
              type="button"
              class="arrow"
              title={t("rules.moveDown")}
              aria-label={t("rules.moveDown")}
              disabled={busy || !row.on || i === effective.length - 1}
              onclick={() => rules && commitRules(moveCategory(rules.overlay, effective, i, 1))}>↓</button
            >
            <button type="button" class="chev" aria-hidden="true" tabindex="-1" onclick={() => toggle(row.name)}
              >{open === row.name ? "▾" : "›"}</button
            >
          </div>

          {#if open === row.name}
            {@const made = producers(rules.track_tag_merge, row.name)}
            {@const words = keywordsOf(keywords, row.name)}
            {@const gone = goneKeywords(keywordCatalog, keywords, row.name)}
            <div class="detail">
              <div class="line">
                <label class="field">
                  <span class="lbl-key">{t("categories.name")}</span>
                  <input
                    class="input"
                    value={looks[row.name]?.name ?? ""}
                    placeholder={categoryName(row.name, {}, t)}
                    onchange={(e) => commitLooks(setLook(looks, row.name, { name: e.currentTarget.value }))}
                  />
                </label>
                <span class="tag mono" title={t("trackCats.tagTitle")}>{row.name}</span>
                {#if row.shipped}
                  <span class="onoff">
                    <Switch
                      on={row.on}
                      label={t("rules.enabled")}
                      disabled={busy}
                      onclick={() => rules && commitRules(setCategoryOn(rules.overlay, row.name, !row.on))}
                    />
                    <span>{row.on ? t("common.active") : t("common.inactive")}</span>
                  </span>
                {/if}
              </div>

              <div class="field">
                <span class="lbl-key">{t("categories.icon")}</span>
                <div class="icons" role="radiogroup" aria-label={t("categories.icon")}>
                  {#each Object.keys(TRACK_CATEGORY_ICONS) as icon (icon)}
                    <!-- Picking the category's own icon stores nothing: it
                         stays the app's, and follows a better one. -->
                    <button
                      type="button"
                      class="pick"
                      role="radio"
                      aria-checked={categoryIconId(row.name, looks) === icon}
                      aria-label={categoryName(`#${icon}`, {}, t)}
                      title={categoryName(`#${icon}`, {}, t)}
                      onclick={() =>
                        commitLooks(
                          setLook(looks, row.name, { icon: icon === categoryIconId(row.name, {}) ? undefined : icon }),
                        )}
                    >
                      <TrackCategoryIcon {icon} size={20} />
                    </button>
                  {/each}
                </div>
              </div>

              <div class="field">
                <span class="lbl-key">{t("trackCats.producedBy")}</span>
                {#each made as r (r.rule.id)}
                  <div class="rule" class:off={r.disabled}>
                    <Switch
                      on={!r.disabled}
                      label={t("rules.enabled")}
                      disabled={busy}
                      onclick={() =>
                        rules && commitRules(setEnabled(rules.overlay, "track_tag_merge", r.rule.id ?? "", r.disabled))}
                    />
                    <span class="mono from">{r.rule.from.join(", ")}</span>
                    <span class="arrow-to">→</span>
                    <span class="mono">{r.rule.to.join(", ")}</span>
                    {#if r.origin !== "own"}<span class="badge">{t("rules.badgeCatalog")}</span>{/if}
                    <span class="c">{r.effect ? tracksText(r.effect) : "—"}</span>
                    <button type="button" class="link" onclick={() => void showRule("track_tag_merge", r.rule.id ?? "")}
                      >{t("trackCats.openInRules")}</button
                    >
                  </div>
                {:else}
                  <span class="empty">{t("trackCats.noRule")}</span>
                {/each}
              </div>

              <div class="field">
                <span class="lbl-key">{t("trackCats.keywords")}</span>
                <div class="tags">
                  {#each words as w (w)}
                    {@const mine = categoryTag(keywordCatalog[w] ?? "") !== row.name}
                    <span class="tok" class:mine title={mine ? t("taxonomyOrigin.added") : undefined}>
                      {#if mine}<span class="mark" aria-hidden="true">✎</span>{/if}{w}
                      <button
                        type="button"
                        class="x"
                        title={t("common.delete")}
                        onclick={() => commitKeywords(removeKeyword(keywordOverlay(), keywordCatalog, w))}>×</button
                      >
                    </span>
                  {:else}
                    <span class="empty">{t("trackCats.noKeyword")}</span>
                  {/each}
                  {#each gone as w (w)}
                    <span class="tok gone" title={t("taxonomyOrigin.removed")}>
                      <s>{w}</s>
                      <button
                        type="button"
                        class="x"
                        title={t("taxonomyOrigin.putBack")}
                        onclick={() => commitKeywords(restoreKeyword(keywordOverlay(), w))}>↺</button
                      >
                    </span>
                  {/each}
                </div>
                <input
                  class="input add"
                  placeholder={t("trackCats.addKeyword")}
                  bind:value={newWord}
                  onkeydown={(e) => {
                    if (e.key !== "Enter" || !newWord.trim()) return;
                    e.preventDefault();
                    commitKeywords(addKeyword(keywordOverlay(), keywordCatalog, newWord, row.name));
                    newWord = "";
                  }}
                />
              </div>

              <div class="actions">
                {#if row.shipped && (looks[row.name] || !row.on)}
                  <button type="button" class="btn" onclick={() => restore(row)}>{t("categories.restore")}</button>
                {/if}
                {#if !row.shipped}
                  <button
                    type="button"
                    class="btn"
                    onclick={() =>
                      arm(`del:${row.name}`, () => {
                        if (rules) commitRules(removeCategory(rules.overlay, row.name));
                        commitLooks(withoutLook(looks, row.name));
                        open = null;
                      })}
                  >
                    {armed === `del:${row.name}` ? t("categories.confirmDelete") : t("common.delete")}
                  </button>
                {/if}
              </div>
            </div>
          {/if}
        </li>
      {/each}
    </ul>

    <div class="foot">
      <input
        class="input"
        placeholder={t("trackCats.newPlaceholder")}
        bind:value={newName}
        onkeydown={(e) => e.key === "Enter" && create()}
      />
      <button type="button" class="btn btn-primary" disabled={!newName.trim() || busy} onclick={create}>
        {t("categories.create")}
      </button>
      <button type="button" class="btn reset" disabled={busy} onclick={() => arm("reset", restoreAll)}>
        {armed === "reset" ? t("categories.confirmResetAll") : t("categories.resetAll")}
      </button>
    </div>
  {/if}
</div>

<style>
  .cats {
    max-width: 820px;
    --tile-bg: var(--panel);
  }
  .errbox {
    margin-bottom: 12px;
  }
  .empty {
    color: var(--muted2);
    font-size: 12px;
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
  .list > li.off .head {
    opacity: 0.55;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 4px;
    padding-right: 6px;
  }
  .row:hover {
    background: var(--raised);
  }
  .head {
    flex: 1;
    min-width: 0;
    display: grid;
    grid-template-columns: 46px 1fr 140px 18px;
    align-items: center;
    gap: 12px;
    padding: 8px 10px;
    background: none;
    color: var(--txt);
    font: inherit;
    font-size: 12.5px;
    text-align: left;
  }
  .head:focus-visible,
  .pick:focus-visible,
  .arrow:focus-visible {
    outline: 2px solid var(--rosso);
    outline-offset: -2px;
  }
  .ico {
    color: var(--txt2);
    display: flex;
  }
  .num {
    color: var(--muted);
    font-size: 11.5px;
    white-space: nowrap;
  }
  /* The curation flag: the dimmed red of the accent scale, a state and not an
     action (TAXO§11). Kept in place when off, so the columns do not dance. */
  .flag {
    color: var(--rosso-border);
    visibility: hidden;
  }
  .flag.on {
    visibility: visible;
  }
  .rank {
    width: 18px;
    text-align: right;
    color: var(--faint);
    font-size: 10px;
  }
  .arrow,
  .chev {
    width: 22px;
    height: 22px;
    background: none;
    color: var(--muted2);
    font: inherit;
    font-size: 12px;
  }
  .arrow:hover:not(:disabled) {
    color: var(--txt);
    background: var(--mat);
  }
  .arrow:disabled {
    opacity: 0.3;
  }
  .badge {
    font-size: 9px;
    letter-spacing: 0.12em;
    color: var(--muted2);
    border: 1px solid var(--line);
    border-radius: 2px;
    padding: 0 4px;
    margin-left: 8px;
    vertical-align: 1px;
  }

  .detail {
    padding: 6px 10px 16px 68px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .line {
    display: flex;
    align-items: flex-end;
    gap: 14px;
    flex-wrap: wrap;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .field .input {
    max-width: 320px;
  }
  .tag {
    color: var(--muted2);
    font-size: 11.5px;
    padding-bottom: 7px;
  }
  .onoff {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    color: var(--muted);
    font-size: 12px;
    padding-bottom: 7px;
  }
  .icons {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .pick {
    --tile-bg: var(--panel);
    width: 58px;
    height: 34px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--tile-bg);
    border: 1px solid var(--line);
    color: var(--txt2);
  }
  .pick:hover {
    border-color: var(--faint2);
  }
  .pick[aria-checked="true"] {
    --tile-bg: var(--rosso-dim);
    border-color: var(--rosso-border);
  }
  .rule {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 26px;
    font-size: 12px;
  }
  .rule.off .from,
  .rule.off .mono {
    opacity: 0.55;
  }
  .from {
    color: var(--txt2);
  }
  .arrow-to,
  .c {
    color: var(--muted2);
  }
  .c {
    font-size: 11px;
  }
  .link {
    margin-left: auto;
    background: none;
    border: 0;
    padding: 0;
    color: var(--blue);
    font: inherit;
    font-size: 11.5px;
    cursor: pointer;
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
  .tok.mine {
    border-style: dashed;
    border-color: var(--faint2);
  }
  .tok .mark {
    color: var(--txt2);
  }
  .tok.gone {
    color: var(--muted2);
    background: none;
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
  .add {
    max-width: 320px;
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
