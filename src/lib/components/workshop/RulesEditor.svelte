<script lang="ts">
  // Atelier › Règles (REGLES§8): the catalogue's rules and the user's in ONE
  // list per section, in execution order - his first, then the catalogue's,
  // a fork in place of the rule it forks - because that order is the only one
  // that explains a result.
  //
  // Every gesture is saved at once and re-applied to the library: the switch
  // is immediate and without confirmation (REGLES§5), the counters come back
  // measured from the same pass (REGLES§8.3). No "save" bar any more - there
  // is nothing pending to save. Modifying a SHIPPED rule is the one gesture
  // that asks first, because it freezes the rule (REGLES§8.2).
  import { onMount } from "svelte";
  import Tabs from "$lib/components/ui/Tabs.svelte";
  import ContextMenu from "$lib/components/ui/ContextMenu.svelte";
  import { getRulesView, saveRulesOverlay, type RuleRow, type RulesOverlay, type RulesView } from "$lib/workshop/rules";
  import {
    CAR_SECTIONS,
    addCategory,
    addRule,
    describe,
    editRule,
    fromForm,
    moveCategory,
    removeCategory,
    removeRule,
    restoreDefault,
    setCatalogOn,
    setCategoryOn,
    setEnabled,
    toForm,
    type AnyRule,
    type RuleForm,
    type Section,
  } from "$lib/workshop/rulesEdit";
  import { bumpLibraryVersion } from "$lib/library/libraryVersion.svelte";
  import { errorText } from "$lib/errors";
  import { t } from "$lib/i18n/index.svelte";

  let view = $state<RulesView | null>(null);
  let tab = $state<"car" | "track">("car");
  let mineOnly = $state(false);
  let busy = $state(false);
  let error = $state("");

  /** The row being edited - `id: null` for a new rule of the section. */
  let editing = $state<{ section: Section; id: string | null; form: RuleForm } | null>(null);
  /** A shipped rule the user asked to modify: the question of REGLES§8.2. */
  let confirmFork = $state<{ section: Section; row: RuleRow<AnyRule> } | null>(null);
  let menu = $state<{ x: number; y: number; items: { label: string; onclick: () => void; danger?: boolean }[] } | null>(
    null,
  );
  let catInput = $state("");

  onMount(async () => {
    try {
      view = await getRulesView();
    } catch (e) {
      console.error("get_rules_view", e);
      error = errorText(e);
    }
  });

  /** Writes a decision and takes back the view the backend measured after
   * re-applying it. A WRITE: the failure is shown, never swallowed. */
  async function commit(next: RulesOverlay) {
    busy = true;
    error = "";
    try {
      view = await saveRulesOverlay(next);
      bumpLibraryVersion();
    } catch (e) {
      console.error("save_rules_overlay", e);
      error = errorText(e);
    } finally {
      busy = false;
    }
  }

  const rowsOf = (s: Section) => (view ? (view[s] as RuleRow<AnyRule>[]) : []);
  const shownRows = (s: Section) => rowsOf(s).filter((r) => !mineOnly || r.origin !== "catalog");
  const idOf = (r: RuleRow<AnyRule>) => r.rule.id ?? "";

  function toggle(s: Section, row: RuleRow<AnyRule>) {
    if (!view || busy) return;
    const on = row.disabled;
    // Shown at once; the counters follow when the pass is done.
    row.disabled = !on;
    void commit(setEnabled(view.overlay, s, idOf(row), on));
  }

  function startEdit(s: Section, row: RuleRow<AnyRule>) {
    if (row.origin === "catalog") {
      confirmFork = { section: s, row };
      return;
    }
    editing = { section: s, id: idOf(row), form: toForm(s, row.rule) };
  }

  function startAdd(s: Section) {
    editing = { section: s, id: null, form: { a: "", b: "", c: "" } };
  }

  function saveEdit() {
    if (!view || !editing) return;
    const rule = fromForm(editing.section, editing.form);
    if (!rule) return;
    const { section, id } = editing;
    editing = null;
    void commit(id === null ? addRule(view.overlay, section, rule) : editRule(view.overlay, section, id, rule));
  }

  function openMenu(e: MouseEvent, s: Section, row: RuleRow<AnyRule>) {
    // Same reason as FicheHeader: without it this click reaches `document`
    // after the menu mounts, and its own close listener shuts it at once.
    e.stopPropagation();
    if (!view) return;
    const o = view.overlay;
    const id = idOf(row);
    const items: { label: string; onclick: () => void; danger?: boolean }[] = [
      { label: t("rules.edit"), onclick: () => startEdit(s, row) },
      { label: t("rules.duplicate"), onclick: () => void commit(addRule(o, s, row.rule)) },
    ];
    if (row.origin === "fork") {
      items.push({ label: t("rules.restoreDefault"), onclick: () => void commit(restoreDefault(o, s, id)) });
    }
    if (row.origin === "own") {
      items.push({ label: t("common.delete"), onclick: () => void commit(removeRule(o, s, id)), danger: true });
    }
    // Anchored under the button; `ContextMenu` takes real window pixels and
    // divides by the zoom itself (`zoom.svelte.ts`).
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    menu = { x: rect.left - 150, y: rect.bottom + 4, items };
  }

  function forkAnyway() {
    if (!confirmFork) return;
    const { section, row } = confirmFork;
    confirmFork = null;
    editing = { section, id: idOf(row), form: toForm(section, row.rule) };
  }

  function disableInstead() {
    if (!confirmFork || !view) return;
    const { section, row } = confirmFork;
    confirmFork = null;
    if (!row.disabled) toggle(section, row);
  }

  const effectText = (n: number) => (n === 0 ? "—" : n === 1 ? t("rules.effectOne") : t("rules.effect", { count: n }));

  const SECTION_TITLE: Record<Section, string> = {
    brand_fix: "rules.brandFixTitle",
    name_to_tag: "rules.nameToTagTitle",
    class_fix: "rules.classFixTitle",
    car_tag_merge: "rules.tagMerge",
    drivetrain: "rules.fieldDrivetrain",
    aspiration: "rules.fieldAspiration",
    engine_config: "rules.fieldEngineConfig",
    engine_pos: "rules.fieldEnginePos",
    gearbox: "rules.fieldGearbox",
    track_tag_merge: "rules.tagMerge",
  };
  const SECTION_HINT: Partial<Record<Section, string>> = {
    brand_fix: "rules.brandFixHint",
    name_to_tag: "rules.nameToTagHint",
    class_fix: "rules.classFixHint",
    car_tag_merge: "rules.tagMergeHint",
    track_tag_merge: "rules.tagMergeHint",
  };
  const SPEC_SECTIONS = new Set<Section>(["drivetrain", "aspiration", "engine_config", "engine_pos", "gearbox"]);
  const PLACEHOLDER: Record<string, [string, string]> = {
    brand_fix: ["bayro", "BMW"],
    name_to_tag: ["police", "police"],
    class_fix: ["rally, hillclimb", "#rally"],
    tags: ["hothatch, hot hatchback", "hatchback"],
    spec: ["rwd, propulsion", "RWD"],
  };
  const placeholder = (s: Section) =>
    PLACEHOLDER[s] ?? (SPEC_SECTIONS.has(s) ? PLACEHOLDER.spec : PLACEHOLDER.tags);

  const carTop = CAR_SECTIONS.filter((s) => !SPEC_SECTIONS.has(s));
  const carSpecs = CAR_SECTIONS.filter((s) => SPEC_SECTIONS.has(s));

  const shippedCategories = $derived(view?.track_categories.filter((c) => c.shipped).map((c) => c.name) ?? []);
  const effectiveCategories = $derived(view?.track_categories.filter((c) => c.on).map((c) => c.name) ?? []);
  const shownCategories = $derived(view?.track_categories.filter((c) => !mineOnly || !c.shipped) ?? []);
</script>

<svelte:window
  onkeydown={(e) => {
    if (e.key !== "Escape") return;
    if (confirmFork) confirmFork = null;
    else if (editing) editing = null;
  }}
/>

{#snippet editor(s: Section)}
  {#if editing}
    {@const [pa, pb] = placeholder(s)}
    <form
      class="edit"
      onsubmit={(e) => {
        e.preventDefault();
        saveEdit();
      }}
    >
      <!-- svelte-ignore a11y_autofocus -->
      <input class="input mono" bind:value={editing.form.a} placeholder={pa} autofocus />
      <span class="arrow">→</span>
      {#if s === "class_fix"}
        <select class="input sel" bind:value={editing.form.b}>
          <option value="">{t("rules.noneOption")}</option>
          <option value="race">race</option>
          <option value="street">street</option>
        </select>
        <input class="input mono" bind:value={editing.form.c} placeholder={pb} />
      {:else}
        <input class="input mono" bind:value={editing.form.b} placeholder={pb} />
      {/if}
      <button class="btn btn-primary" type="submit" disabled={!fromForm(s, editing.form) || busy}>
        {t("rules.save")}
      </button>
      <button class="btn" type="button" onclick={() => (editing = null)}>{t("common.cancel")}</button>
    </form>
  {/if}
{/snippet}

{#snippet section(s: Section, sub: boolean)}
  {@const rows = shownRows(s)}
  <section class:sub>
    <div class="s-head">
      <svelte:element this={sub ? "h4" : "h3"}>
        {t(SECTION_TITLE[s])} <span class="cnt">{rows.length}</span>
      </svelte:element>
      <button class="btn-ghost add" type="button" disabled={busy} onclick={() => startAdd(s)}>+ {t("rules.addRule")}</button>
    </div>
    {#if SECTION_HINT[s]}<p class="hint">{t(SECTION_HINT[s]!)}</p>{/if}
    {#if editing?.section === s && editing.id === null}{@render editor(s)}{/if}
    <div class="rows">
      {#each rows as row (idOf(row))}
        {#if editing?.section === s && editing.id === idOf(row)}
          {@render editor(s)}
        {:else}
          {@const d = describe(s, row.rule)}
          <div class="row" class:off={row.disabled}>
            <button
              type="button"
              class="switch"
              role="switch"
              aria-checked={!row.disabled}
              aria-label={t("rules.enabled")}
              disabled={busy}
              onclick={() => toggle(s, row)}
            ><span></span></button>
            <span class="badge-cell">
              {#if row.origin !== "own"}<span class="badge">{t("rules.badgeCatalog")}</span>{/if}
              {#if row.origin === "fork"}<span class="forked" title={t("rules.forkedTitle")}>✎</span>{/if}
            </span>
            <button type="button" class="what" ondblclick={() => startEdit(s, row)}>
              <span class="mono from">{d.from}</span>
              <span class="arrow">→</span>
              <span class="mono to">{d.to}</span>
            </button>
            {#if row.outdated}
              <span class="flag" title={t("rules.newVersionTitle")}>⚑ {t("rules.newVersion")}</span>
            {/if}
            <span class="effect" class:none={row.effect === 0}>{effectText(row.effect)}</span>
            <button type="button" class="more" title={t("rules.actions")} onclick={(e) => openMenu(e, s, row)}>⋮</button>
          </div>
        {/if}
      {:else}
        {#if mineOnly && !(editing?.section === s && editing.id === null)}
          <p class="empty-mine">{t("rules.noneOfYours")}</p>
        {/if}
      {/each}
    </div>
  </section>
{/snippet}

<!-- Écran à défilement interne (`noPad` dans AppShell). -->
<div class="screen">
  <div class="scroll">
    <div class="rules">
      <header class="r-header">
        <p class="lbl-sub">{t("rules.subtitle")}</p>
      </header>

      {#if error}<div class="errbox">{error}</div>{/if}

      {#if !view}
        {#if !error}<div class="empty">{t("rules.loading")}</div>{/if}
      {:else}
        <!-- REGLES§7: the global switch belongs to the screen it governs. -->
        <div class="catalog">
          <button
            type="button"
            class="switch"
            role="switch"
            aria-checked={view.catalog_on}
            aria-labelledby="catalog-switch-label"
            disabled={busy}
            onclick={() => view && void commit(setCatalogOn(view.overlay, !view.catalog_on))}
          ><span></span></button>
          <span id="catalog-switch-label" class="c-label">{t("rules.catalogSwitch")}</span>
          <span class="c-meta mono">
            {t("rules.catalogMeta", { version: view.catalog_version, count: view.catalog_count })}
          </span>
        </div>

        <div class="toolbar">
          <Tabs
            tabs={[
              { id: "car", label: t("rules.tabCars") },
              { id: "track", label: t("rules.tabTracks") },
            ]}
            active={tab}
            onselect={(v) => {
              tab = v as "car" | "track";
              editing = null;
            }}
          />
          <label class="mine">
            <input type="checkbox" bind:checked={mineOnly} />
            <span>{t("rules.mineOnly")}</span>
          </label>
          {#if busy}<span class="busy">{t("rules.applying")}</span>{/if}
        </div>

        {#if tab === "car"}
          {#each carTop as s (s)}{@render section(s, false)}{/each}
          <section>
            <div class="s-head"><h3>{t("rules.specsExtractionTitle")}</h3></div>
            <p class="hint">{t("rules.specsExtractionHint")}</p>
            {#each carSpecs as s (s)}{@render section(s, true)}{/each}
          </section>
        {:else}
          {@render section("track_tag_merge", false)}

          <!-- The allowlist is an ordered list of names, not rules with ids:
               a shipped category is switched off (removed), his own deleted. -->
          <section>
            <div class="s-head">
              <h3>{t("rules.trackCategoriesTitle")} <span class="cnt">{shownCategories.length}</span></h3>
            </div>
            <p class="hint">{t("rules.trackCategoriesHint")}</p>
            <input
              class="input add-input"
              placeholder={t("rules.addCategoryPlaceholder")}
              bind:value={catInput}
              disabled={busy}
              onkeydown={(e) => {
                if (e.key === "Enter" && view) {
                  void commit(addCategory(view.overlay, catInput, shippedCategories));
                  catInput = "";
                }
              }}
            />
            <ol class="cat-list">
              {#each shownCategories as c (c.name)}
                {@const i = effectiveCategories.indexOf(c.name)}
                <li class:off={!c.on}>
                  {#if c.shipped}
                    <button
                      type="button"
                      class="switch"
                      role="switch"
                      aria-checked={c.on}
                      aria-label={t("rules.enabled")}
                      disabled={busy}
                      onclick={() => view && void commit(setCategoryOn(view.overlay, c.name, !c.on))}
                    ><span></span></button>
                  {:else}
                    <span class="switch-gap"></span>
                  {/if}
                  <span class="cat-rank mono">{c.on ? i + 1 : ""}</span>
                  <span class="badge-cell">
                    {#if c.shipped}<span class="badge">{t("rules.badgeCatalog")}</span>{/if}
                  </span>
                  <span class="cat-name mono">{c.name}</span>
                  <span class="effect" class:none={c.effect === 0}>{effectText(c.effect)}</span>
                  {#if c.on}
                    <button class="btn-ghost" type="button" disabled={busy || i === 0} title={t("rules.moveUp")}
                      onclick={() => view && void commit(moveCategory(view.overlay, effectiveCategories, i, -1))}>↑</button>
                    <button class="btn-ghost" type="button" disabled={busy || i === effectiveCategories.length - 1}
                      title={t("rules.moveDown")}
                      onclick={() => view && void commit(moveCategory(view.overlay, effectiveCategories, i, 1))}>↓</button>
                  {/if}
                  {#if !c.shipped}
                    <button class="btn-ghost" type="button" disabled={busy} title={t("common.delete")}
                      onclick={() => view && void commit(removeCategory(view.overlay, c.name))}>✕</button>
                  {/if}
                </li>
              {/each}
            </ol>
          </section>
        {/if}
      {/if}
    </div>
  </div>
</div>

{#if menu}
  <ContextMenu x={menu.x} y={menu.y} items={menu.items} onclose={() => (menu = null)} />
{/if}

{#if confirmFork}
  <div class="backdrop">
    <div class="modal" role="dialog" aria-modal="true" aria-labelledby="fork-title">
      <p id="fork-title" class="m-title">{t("rules.forkTitle")}</p>
      <p class="m-text">{t("rules.forkText")}</p>
      <div class="m-acts">
        <button class="btn btn-primary" type="button" onclick={forkAnyway}>{t("rules.forkAnyway")}</button>
        <button class="btn" type="button" onclick={disableInstead}>{t("rules.disableInstead")}</button>
        <button class="btn" type="button" onclick={() => (confirmFork = null)}>{t("common.cancel")}</button>
      </div>
    </div>
  </div>
{/if}

<style>
  /* No red accent on this screen outside the keyboard focus: nothing in it
     belongs to the session (REGLES§12, SPEC §7.2ter). */
  .screen {
    height: 100%;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .scroll {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 0 32px 28px;
  }
  .rules {
    max-width: 860px;
  }
  .r-header {
    margin-bottom: 14px;
  }
  .catalog {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px;
    border: 1px solid var(--line);
    background: var(--panel2);
    margin-bottom: 16px;
  }
  .c-label {
    font-size: 12.5px;
    color: var(--txt);
  }
  .c-meta {
    margin-left: auto;
    font-size: 11px;
    color: var(--muted);
  }
  .toolbar {
    display: flex;
    align-items: center;
    gap: 16px;
    margin-bottom: 14px;
  }
  .mine {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: var(--muted);
    margin-left: auto;
  }
  .busy {
    font-size: 11.5px;
    color: var(--muted);
  }
  section {
    margin-bottom: 24px;
  }
  section.sub {
    margin: 12px 0 14px;
    padding-left: 10px;
    border-left: 2px solid var(--line);
  }
  .s-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 6px;
  }
  h3 {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 1px;
    color: var(--txt2);
  }
  h4 {
    font-size: 11px;
    font-weight: 400;
    color: var(--muted);
  }
  .cnt {
    color: var(--faint);
    font-family: var(--mono);
    margin-left: 6px;
  }
  .hint {
    color: var(--muted);
    font-size: 11.5px;
    margin-bottom: 8px;
    line-height: 1.5;
  }
  .add {
    padding: 3px 8px;
    font-size: 11.5px;
  }
  .rows {
    display: flex;
    flex-direction: column;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 30px;
    padding: 2px 0;
    border-bottom: 1px solid var(--line);
  }
  .row.off,
  .cat-list li.off {
    opacity: 0.55;
  }
  /* The switch: a plain track and knob, grey off, light on - no red here. */
  .switch {
    flex: none;
    width: 26px;
    height: 14px;
    padding: 0;
    border: 1px solid var(--faint);
    border-radius: 8px;
    background: var(--panel2);
    position: relative;
    cursor: pointer;
  }
  .switch span {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--muted2);
    transition: left 0.12s;
  }
  .switch[aria-checked="true"] {
    border-color: var(--muted);
    background: var(--mat);
  }
  .switch[aria-checked="true"] span {
    left: 14px;
    background: var(--txt);
  }
  .switch:disabled {
    cursor: default;
  }
  .switch-gap {
    flex: none;
    width: 26px;
  }
  .badge-cell {
    flex: none;
    width: 66px;
    display: flex;
    align-items: center;
    gap: 4px;
  }
  /* REGLES§12: 9 px, .12em, the faint text, a hairline, radius 2. */
  .badge {
    font-size: 9px;
    letter-spacing: 0.12em;
    color: var(--muted);
    border: 1px solid var(--line);
    border-radius: 2px;
    padding: 1px 4px;
    white-space: nowrap;
  }
  .forked {
    font-size: 11px;
    color: var(--txt2);
  }
  .what {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: baseline;
    gap: 8px;
    background: none;
    border: none;
    padding: 0;
    text-align: left;
    cursor: default;
    color: inherit;
  }
  .from,
  .to {
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .from {
    color: var(--txt2);
  }
  .to {
    color: var(--txt);
  }
  .arrow {
    color: var(--muted2);
    flex: none;
  }
  .flag {
    flex: none;
    font-size: 11px;
    color: var(--orange);
    white-space: nowrap;
  }
  .effect {
    flex: none;
    width: 64px;
    text-align: right;
    font-size: 11.5px;
    color: var(--txt2);
    font-family: var(--mono);
  }
  .effect.none {
    color: var(--faint);
  }
  .more {
    flex: none;
    width: 22px;
    height: 22px;
    background: none;
    border: none;
    color: var(--muted);
    cursor: pointer;
  }
  .more:hover {
    color: var(--txt);
  }
  .edit {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 0;
    border-bottom: 1px solid var(--line);
  }
  .edit .input {
    flex: 1;
    min-width: 0;
  }
  .edit .sel {
    flex: 0 0 110px;
  }
  .empty-mine {
    font-size: 11.5px;
    color: var(--faint);
    padding: 4px 0;
  }
  .empty {
    color: var(--muted);
    padding: 40px 0;
  }
  .add-input {
    margin-bottom: 10px;
    max-width: 420px;
  }
  .cat-list {
    list-style: none;
    display: flex;
    flex-direction: column;
    max-width: 620px;
  }
  .cat-list li {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 30px;
    border-bottom: 1px solid var(--line);
  }
  .cat-rank {
    color: var(--faint);
    font-size: 10px;
    width: 18px;
    text-align: right;
    flex: none;
  }
  .cat-name {
    flex: 1;
    color: var(--txt);
    font-size: 12px;
  }
  .cat-list .btn-ghost {
    padding: 2px 7px;
    font-size: 12px;
  }
  .cat-list .btn-ghost:disabled {
    opacity: 0.3;
  }
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
    width: 480px;
    max-width: 92vw;
    background: var(--panel);
    border: 1px solid var(--line);
    padding: 18px 20px;
  }
  .m-title {
    font-size: 13px;
    color: var(--txt);
    margin-bottom: 8px;
  }
  .m-text {
    font-size: 12px;
    color: var(--txt2);
    line-height: 1.5;
    margin-bottom: 16px;
  }
  .m-acts {
    display: flex;
    gap: 8px;
  }
</style>
