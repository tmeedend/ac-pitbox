<script lang="ts">
  // Brands tab of the Workshop (TAXO§6): the brand as a TERM - the name a car
  // is filed under, and the spellings that lead to it.
  //
  // Case, spaces and accents merge on their own (TAXO§7.1, `brands.rs`); every
  // other merge is the user's, from a proposal or by hand, because `BMW` and
  // `BMW Motorsport`, `Nissan` and `Nismo`, may be distinctions he wants
  // (TAXO§7.2). Nothing is merged silently, and "Ignore" is remembered.
  //
  // Each brand shows its logo, elected among its cars' badges (TAXO§4,
  // `logos.rs`); the detail lets the user pick another variant, give a file of
  // his (TAXO§9), or force the light plate (TAXO§5), with the logo previewed
  // at the sizes it is actually drawn (TAXO§6.3) - one chosen on a large
  // preview turns out unreadable at 13 px one time in three.
  //
  // A brand is also where one notices that it is no brand at all - `VRC`, a
  // modder, filing 14 cars; `traffic`. Those cars say their real brand in
  // their NAME ("VRC ERC - Auriel 4" is an Audi), which a spelling merge
  // cannot read: filing all of VRC under Audi would be wrong. So the tab
  // creates the Rules screen's "name contains → brand" rules (`brand_fix`)
  // from here, and shows, on each brand, the rules that file cars into it -
  // with their switch and their count. Stored once, in the rules overlay:
  // this is a second door onto them, not a second mechanism (REGLES§11 keeps
  // a heuristic in Rules, with its counter and its update report).
  //
  // Saving re-applies to the whole library, like the Countries tab: the brand
  // is decided at write time. Same write queue, failures shown and logged
  // (CLAUDE.md rule 6).
  import { onMount, tick } from "svelte";
  import { brandFocus } from "$lib/workshop/brandFocus.svelte";
  import { scrollIntoContainer } from "$lib/shell/shellScroll";
  import { errorText } from "$lib/errors";
  import { t } from "$lib/i18n/index.svelte";
  import { listLibrary } from "$lib/library/library";
  import { bumpLibraryVersion } from "$lib/library/libraryVersion.svelte";
  import { brandProposals, mergeBrand, mergeKey } from "$lib/workshop/brandEdit";
  import { keyOrigins, keysTo, removeEntry, restoreEntries, setEntry, touches } from "$lib/workshop/countryEdit";
  import {
    getRulesView,
    getTaxonomy,
    saveBrandOverlay,
    saveRulesOverlay,
    type BrandFix,
    type MapOverlay,
    type RuleRow,
    type RulesView,
    type TaxonomyView,
  } from "$lib/workshop/rules";
  import { addRule, removeRule, setEnabled } from "$lib/workshop/rulesEdit";
  import { requestSection } from "$lib/shell/nav.svelte";
  import { open as openFile } from "@tauri-apps/plugin-dialog";
  import Emblem from "$lib/components/ui/Emblem.svelte";
  import Seg from "$lib/components/ui/Seg.svelte";
  import { previewSrc } from "$lib/library/library";
  import {
    brandLogos,
    importBrandLogo,
    loadBrandLogos,
    saveBrandLogo,
    type BrandPref,
    type LogoVariant,
  } from "$lib/library/brandLogos.svelte";

  let view = $state<TaxonomyView | null>(null);
  /** The user's decisions: what every gesture edits, ahead of the save. */
  let aliases = $state<MapOverlay>({});
  let ignored = $state<string[]>([]);
  const effAliases = $derived(view?.effective.brand_aliases ?? {});
  const catAliases = $derived(view?.catalog.brand_aliases ?? {});
  /** Stored brand of every car. */
  let brands = $state<(string | null)[]>([]);
  /** Every car with its brand and the name its file gives - what a
   * "name contains" rule reads (`rules::apply_car`). */
  let cars = $state<{ id: string; brand: string | null; name: string; label: string }[]>([]);
  /** The Rules screen's view: the `brand_fix` rows, with their counters. */
  let rulesView = $state<RulesView | null>(null);
  /** A word of the name, typed to bring the cars it catches here. */
  let nameWord = $state("");
  let loading = $state(true);
  let busy = $state(false);
  let error = $state("");
  let open = $state<string | null>(null);
  let newAlias = $state("");
  let fileUnder = $state("");
  let showEmpty = $state(false);

  async function reloadCars() {
    const cards = await listLibrary();
    const carCards = cards.filter((c) => c.kind === "Car");
    brands = carCards.map((c) => c.brand);
    cars = carCards.map((c) => ({
      id: c.id_interne,
      brand: c.brand,
      name: c.display_name_file ?? c.id_interne,
      label: c.display_name ?? c.display_name_file ?? c.id_interne,
    }));
  }

  onMount(async () => {
    try {
      const [v, rv] = await Promise.all([getTaxonomy(), getRulesView(), reloadCars(), loadBrandLogos()]);
      rulesView = rv;
      take(v);
      await focusRequested();
    } catch (e) {
      error = errorText(e);
    } finally {
      loading = false;
    }
  });

  function take(v: TaxonomyView) {
    view = v;
    aliases = v.overlay.brand_aliases ?? {};
    ignored = v.overlay.ignored_brand_merges ?? [];
  }

  let queue: Promise<void> = Promise.resolve();
  /** Takes the decisions at once, writes and re-applies them, then reads the
   * library back: the counts only mean something once the stored brands
   * moved. The `catch` keeps one failure from freezing every later write. */
  function commit(next: { aliases?: MapOverlay; ignored?: string[] }) {
    aliases = next.aliases ?? aliases;
    ignored = next.ignored ?? ignored;
    const [a, ig] = [aliases, ignored];
    queue = queue
      .then(async () => {
        busy = true;
        take(await saveBrandOverlay(a, ig));
        await reloadCars();
        bumpLibraryVersion();
        error = "";
      })
      .catch((e) => {
        console.error("save_brand_overlay", e);
        error = errorText(e);
      })
      .finally(() => (busy = false));
  }

  interface Row {
    name: string;
    cars: number;
  }

  const rows = $derived.by(() => {
    const m = new Map<string, Row>();
    const row = (name: string) => m.get(name) ?? (m.set(name, { name, cars: 0 }), m.get(name)!);
    for (const b of brands) if (b) row(b).cars++;
    // A brand only spellings lead to - its cars gone - stays, hidden unless
    // asked for (TAXO§9): the merges curated for it are kept.
    for (const to of Object.values(effAliases)) row(to);
    // By name, not by count (asked at use, 2026-09-25): the tab is where one
    // looks a brand up, and forty brands read like a directory. TAXO§6.1's
    // "by decreasing count" suits an index, which the library has.
    return [...m.values()].sort((a, b) => a.name.localeCompare(b.name, undefined, { sensitivity: "base" }));
  });
  const shownRows = $derived(rows.filter((r) => showEmpty || r.cars > 0));
  const emptyCount = $derived(rows.filter((r) => r.cars === 0).length);
  const unset = $derived(brands.filter((b) => !b).length);
  const proposals = $derived(brandProposals(rows, ignored));

  function merge(from: string, to: string) {
    commit({ aliases: mergeBrand(aliases, catAliases, effAliases, from, to) });
    if (open === from) open = to.trim();
  }

  const carsText = (n: number) =>
    n === 0 ? t("brandsTab.noCar") : n === 1 ? t("brandsTab.carOne") : t("brandsTab.cars", { count: n });

  /** A logo choice: written at once (`brand_logos.json`), the logos re-read.
   * Same failure path as the merges. */
  async function logoChoice(write: () => Promise<void>) {
    busy = true;
    try {
      await write();
      error = "";
    } catch (e) {
      console.error("brand logo", e);
      error = errorText(e);
    } finally {
      busy = false;
    }
  }

  function pickVariant(brand: string, pref: BrandPref, v: LogoVariant) {
    void logoChoice(() => saveBrandLogo(brand, { ...pref, variant: v.hash, custom: undefined }));
  }

  async function giveFile(brand: string) {
    const picked = await openFile({
      title: t("brandsTab.logoFileTitle"),
      multiple: false,
      filters: [{ name: t("brandsTab.logoFileFilter"), extensions: ["png", "svg"] }],
    });
    if (typeof picked === "string") void logoChoice(() => importBrandLogo(brand, picked));
  }

  const PREVIEW_SIZES = [13, 18, 20, 32];
  const bgKey: Record<string, string> = {
    transparent: "brandsTab.bgTransparent",
    baked: "brandsTab.bgBaked",
    opaque: "brandsTab.bgOpaque",
  };

  /** Arriving from a car sheet: that brand opened, and brought into view. */
  async function focusRequested() {
    const brand = brandFocus.brand;
    brandFocus.brand = null;
    if (!brand) return;
    open = brand;
    loading = false;
    await tick();
    const el = document.querySelector<HTMLElement>(`[data-brand="${CSS.escape(brand)}"]`);
    if (el) scrollIntoContainer(el, "center");
  }

  /** The "name contains" rules that file cars under `brand`. */
  const nameRules = (brand: string): RuleRow<BrandFix>[] =>
    (rulesView?.brand_fix ?? []).filter((r) => r.rule.set_brand === brand);

  /** Every car a word would catch - as the engine matches: the file's name,
   * lowercased, containing it - wherever it is filed now. */
  const caught = (word: string) => {
    const w = word.trim().toLowerCase();
    return w ? cars.filter((c) => c.name.toLowerCase().includes(w)) : [];
  };

  /** A rules decision from this tab: the same write as the Rules screen's,
   * then the brands read back - the rule moved cars. */
  function commitRules(next: import("$lib/workshop/rules").RulesOverlay) {
    queue = queue
      .then(async () => {
        busy = true;
        rulesView = await saveRulesOverlay(next);
        await reloadCars();
        await loadBrandLogos();
        bumpLibraryVersion();
        error = "";
      })
      .catch((e) => {
        console.error("save_rules_overlay", e);
        error = errorText(e);
      })
      .finally(() => (busy = false));
  }

  /** "name contains → this brand": a rule of his, first of his rules. */
  function fileByName(brand: string) {
    const word = nameWord.trim().toLowerCase();
    if (!rulesView || !word || !caught(word).some((c) => c.brand !== brand)) return;
    commitRules(addRule(rulesView.overlay, "brand_fix", { name_contains: word, set_brand: brand }));
    nameWord = "";
  }

  function toggle(name: string) {
    open = open === name ? null : name;
    nameWord = "";
    newAlias = "";
    fileUnder = "";
  }
</script>

<datalist id="brand-names">
  {#each rows as r (r.name)}<option value={r.name}></option>{/each}
</datalist>

<div class="brands">
  <!-- Workshop tab: the screen title belongs to the Workshop. -->
  <header>
    <p class="lbl-sub">{t("brandsTab.subtitle")}</p>
    {#if !loading}
      <p class="meta">
        {t("brandsTab.unset", { count: unset })}
        {#if busy}<span class="busy">· {t("brandsTab.applying")}</span>{/if}
      </p>
    {/if}
  </header>

  {#if error}<div class="errbox">{error}</div>{/if}

  {#if !loading && proposals.length}
    <!-- TAXO§7.2: proposed, never merged silently; "Ignore" is remembered
         and undone from the foot of the list. -->
    <div class="warnbox proposals">
      <p class="ptitle">⚑ {t("brandsTab.proposals", { count: proposals.length })}</p>
      {#each proposals as p (p.from)}
        <div class="prop">
          <span class="pname">{p.from}</span>
          <span class="arrow">→</span>
          <span class="pname">{p.to}</span>
          <span class="pcount mono">{p.fromCars} + {p.toCars}</span>
          <button type="button" class="btn" disabled={busy} onclick={() => merge(p.from, p.to)}>{t("brandsTab.merge")}</button>
          <button
            type="button"
            class="btn"
            disabled={busy}
            onclick={() => commit({ ignored: [...new Set([...ignored, mergeKey(p.from, p.to)])].sort() })}
            >{t("brandsTab.ignore")}</button
          >
        </div>
      {/each}
    </div>
  {/if}

  {#if !loading}
    <ul class="list">
      {#each shownRows as r (r.name)}
        {@const spelled = keysTo(effAliases, r.name)}
        {@const origin = keyOrigins(effAliases, catAliases, r.name)}
        {@const curated = touches(aliases, catAliases, r.name)}
        {@const logo = brandLogos.brands[r.name]}
        {@const logoSrc = logo?.path ? previewSrc(logo.path) : null}
        <li data-brand={r.name}>
          <button type="button" class="row" aria-expanded={open === r.name} onclick={() => toggle(r.name)}>
            {#if logoSrc}<Emblem src={logoSrc} plaque={logo.plaque} size={20} />{:else}<span
                class="no-logo"
                aria-hidden="true"
              ></span>{/if}
            <span class="nm">{r.name}</span>
            <span class="num">{carsText(r.cars)}</span>
            <span class="num">{t("brandsTab.aliasCount", { count: spelled.length })}</span>
            <span class="curated" class:on={curated} title={curated ? t("categories.curated") : undefined} aria-hidden={!curated}
              >⚑</span
            >
            <span class="chev" aria-hidden="true">{open === r.name ? "▾" : "›"}</span>
          </button>

          {#if open === r.name}
            {@const rulesHere = nameRules(r.name)}
            {@const target = fileUnder.trim()}
            {@const exists = rows.some((x) => x.name === target)}
            <div class="detail">
              <div class="field">
                <span class="lbl-key">{t("brandsTab.logo")}</span>
                {#if logo && logoSrc}
                  <!-- TAXO§6.3: the sizes it is drawn at, on the dark
                       interface and on the light plate. -->
                  <div class="previews">
                    {#each [false, true] as onPlate (onPlate)}
                      <div class="sizes" class:chosen={logo.plaque === onPlate}>
                        {#each PREVIEW_SIZES as size (size)}<Emblem src={logoSrc} plaque={onPlate} {size} />{/each}
                        <span class="lbl-sub">{onPlate ? t("brandsTab.renderPlaque") : t("brandsTab.renderDirect")}</span>
                      </div>
                    {/each}
                  </div>
                  <div class="render">
                    <Seg
                      items={[
                        { value: "auto", label: t("brandsTab.renderAuto") },
                        { value: "direct", label: t("brandsTab.renderDirect") },
                        { value: "plaque", label: t("brandsTab.renderPlaque") },
                      ]}
                      value={logo.pref.plaque === undefined ? "auto" : logo.pref.plaque ? "plaque" : "direct"}
                      onselect={(v) =>
                        void logoChoice(() =>
                          saveBrandLogo(r.name, { ...logo.pref, plaque: v === "auto" ? undefined : v === "plaque" }),
                        )}
                    />
                  </div>
                {:else}
                  <span class="empty">{t("brandsTab.noLogo")}</span>
                {/if}
                {#if logo?.variants.length}
                  <div class="variants">
                    {#each logo.variants as v, i (v.hash)}
                      {@const src = previewSrc(v.path)}
                      <button
                        type="button"
                        class="variant"
                        class:on={logo.choice !== "custom" && logo.path === v.path}
                        disabled={busy}
                        title={i === 0 ? t("brandsTab.elected") : undefined}
                        onclick={() => pickVariant(r.name, logo.pref, v)}
                      >
                        {#if src}<Emblem {src} plaque={v.background === "baked"} size={32} />{/if}
                        <span class="v-meta">
                          {t(bgKey[v.background])} · {v.width}×{v.height}<br />{carsText(v.cars)}
                        </span>
                      </button>
                    {/each}
                  </div>
                {/if}
                <div class="logo-acts">
                  <button type="button" class="btn" disabled={busy} onclick={() => void giveFile(r.name)}
                    >{t("brandsTab.logoFile")}</button
                  >
                  {#if logo && (logo.pref.variant || logo.pref.custom || logo.pref.plaque !== undefined)}
                    <button type="button" class="btn" disabled={busy} onclick={() => void logoChoice(() => saveBrandLogo(r.name, {}))}
                      >{t("brandsTab.logoAuto")}</button
                    >
                  {/if}
                </div>
              </div>

              <!-- The two ways a car lands in this brand, side by side and
                   worded alike, so their difference reads by itself: what
                   the FILE's brand field says (a spelling, merged), or what
                   the car's NAME contains (a Rules-screen heuristic). -->
              <div class="field">
                <span class="lbl-key">{t("brandsTab.filedHere")}</span>
                <div class="how">
                  <span class="how-lbl">{t("brandsTab.whenFileSays")}</span>
                  <div class="tags">
                    <span class="tok self">{r.name}</span>
                    {#each origin.keys as { key: alias, mine } (alias)}
                      <span class="tok" class:mine title={mine ? t("taxonomyOrigin.added") : undefined}
                        >{#if mine}<span class="mark" aria-hidden="true">✎</span>{/if}{alias}<button
                          type="button"
                          class="x"
                          title={t("brandsTab.removeAlias")}
                          disabled={busy}
                          onclick={() => commit({ aliases: removeEntry(aliases, catAliases, alias) })}>×</button
                        ></span
                      >
                    {/each}
                    {#each origin.gone as alias (alias)}
                      <span class="tok gone" title={t("taxonomyOrigin.removed")}
                        ><s>{alias}</s><button
                          type="button"
                          class="x"
                          title={t("taxonomyOrigin.putBack")}
                          disabled={busy}
                          onclick={() => commit({ aliases: setEntry(aliases, catAliases, alias, r.name, true) })}>↺</button
                        ></span
                      >
                    {/each}
                    <input
                      class="input add"
                      autocomplete="off"
                      placeholder={t("brandsTab.addAlias")}
                      bind:value={newAlias}
                      disabled={busy}
                      onkeydown={(e) => {
                        if (e.key !== "Enter" || !newAlias.trim()) return;
                        commit({ aliases: setEntry(aliases, catAliases, newAlias, r.name, true) });
                        newAlias = "";
                      }}
                    />
                  </div>
                </div>

                <div class="how">
                  <span class="how-lbl">{t("brandsTab.whenNameContains")}</span>
                  <div class="name-rules">
                    {#each rulesHere as nr (nr.rule.id)}
                      <div class="name-rule" class:off={nr.disabled}>
                        <button
                          type="button"
                          class="switch"
                          role="switch"
                          aria-checked={!nr.disabled}
                          aria-label={t("rules.enabled")}
                          disabled={busy || !rulesView}
                          onclick={() =>
                            rulesView && commitRules(setEnabled(rulesView.overlay, "brand_fix", nr.rule.id ?? "", nr.disabled))}
                          ><span></span></button
                        >
                        <span class="tok">{nr.rule.name_contains}</span>
                        {#if nr.origin !== "own"}<span class="badge">{t("rules.badgeCatalog")}</span>{/if}
                        <span class="nr-count mono">{nr.effect ? carsText(nr.effect) : "—"}</span>
                        {#if nr.origin === "own"}
                          <button
                            type="button"
                            class="x"
                            title={t("common.delete")}
                            disabled={busy || !rulesView}
                            onclick={() => rulesView && commitRules(removeRule(rulesView.overlay, "brand_fix", nr.rule.id ?? ""))}
                            >×</button
                          >
                        {:else}
                          <button type="button" class="link" onclick={() => void requestSection("rules")}
                            >{t("brandsTab.editInRules")}</button
                          >
                        {/if}
                      </div>
                    {/each}
                    <input
                      class="input add"
                      autocomplete="off"
                      placeholder={t("brandsTab.nameWord")}
                      bind:value={nameWord}
                      disabled={busy || !rulesView}
                      onkeydown={(e) => e.key === "Enter" && fileByName(r.name)}
                    />
                  </div>
                  {#if nameWord.trim()}
                    <!-- Every car the word catches, wherever it is filed now:
                         the point is to bring cars HERE. -->
                    {@const hit = caught(nameWord)}
                    {@const moving = hit.filter((c) => c.brand !== r.name)}
                    {#if hit.length}
                      <ul class="caught">
                        {#each hit.slice(0, 12) as c (c.id)}
                          <li class:here={c.brand === r.name}>
                            <span class="c-name">{c.label}</span>
                            <span class="c-brand">{c.brand === r.name ? t("brandsTab.alreadyHere") : (c.brand ?? "—")}</span>
                          </li>
                        {/each}
                        {#if hit.length > 12}<li class="more">{t("brandsTab.moreCars", { count: hit.length - 12 })}</li>{/if}
                      </ul>
                    {:else}
                      <p class="empty">{t("brandsTab.noCarCaught")}</p>
                    {/if}
                    <div>
                      <button type="button" class="btn" disabled={busy || !moving.length} onclick={() => fileByName(r.name)}>
                        {moving.length === 1 ? t("brandsTab.fileHereOne") : t("brandsTab.fileHere", { count: moving.length })}
                      </button>
                    </div>
                  {/if}
                </div>
              </div>

              <!-- Renaming and merging are one gesture; the button says which
                   one it is about to do. The list offers the existing brands
                   (a merge without a typo); the browser's typing history is
                   off - it offered names typed in unrelated fields. -->
              <div class="field">
                <span class="lbl-key">{t("brandsTab.renameOrMerge")}</span>
                <div class="attach">
                  <input
                    class="input pick"
                    list="brand-names"
                    autocomplete="off"
                    placeholder={t("brandsTab.renamePlaceholder")}
                    bind:value={fileUnder}
                    disabled={busy}
                  />
                  <button type="button" class="btn" disabled={busy || !target || target === r.name} onclick={() => merge(r.name, fileUnder)}>
                    {#if !target || target === r.name}{t("brandsTab.renameOrMerge")}{:else if exists}{t("brandsTab.mergeInto", {
                        name: target,
                      })}{:else}{t("brandsTab.renameTo", { name: target })}{/if}
                  </button>
                </div>
              </div>

              {#if curated}
                <div>
                  <button
                    type="button"
                    class="btn"
                    disabled={busy}
                    onclick={() => commit({ aliases: restoreEntries(aliases, catAliases, r.name) })}
                  >
                    {t("brandsTab.restore")}
                  </button>
                </div>
              {/if}
            </div>
          {/if}
        </li>
      {/each}
    </ul>

    <div class="foot">
      {#if emptyCount}
        <label class="toggle">
          <input type="checkbox" bind:checked={showEmpty} />
          <span>{t("brandsTab.showEmpty", { count: emptyCount })}</span>
        </label>
      {/if}
      {#if ignored.length}
        <div class="ignored">
          <span class="lbl-key">{t("brandsTab.ignored")}</span>
          {#each ignored as v (v)}
            <span class="tok"
              >{v}<button
                type="button"
                class="x"
                title={t("brandsTab.unignore")}
                disabled={busy}
                onclick={() => commit({ ignored: ignored.filter((x) => x !== v) })}>×</button
              ></span
            >
          {/each}
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .brands {
    max-width: 860px;
  }
  header {
    margin-bottom: 14px;
  }
  .meta {
    margin: 4px 0 0;
    font-size: 11.5px;
    color: var(--muted2);
  }
  .busy {
    color: var(--muted);
  }
  .errbox,
  .proposals {
    margin-bottom: 12px;
  }
  .ptitle {
    margin: 0 0 8px;
  }
  .prop {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 0;
  }
  .pname {
    min-width: 140px;
    color: var(--txt);
  }
  .pcount {
    min-width: 110px;
    color: var(--muted);
  }
  .pick {
    width: 220px;
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
    grid-template-columns: 20px 1fr 130px 120px 18px 16px;
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
  .row:focus-visible {
    outline: 2px solid var(--rosso);
    outline-offset: -2px;
  }
  /* 20 px wide, as every emblem of an Atelier list (TAXO§10). */
  .num {
    color: var(--muted);
    font-size: 11.5px;
  }
  .curated {
    color: var(--rosso-border);
    visibility: hidden;
  }
  .curated.on {
    visibility: visible;
  }
  .chev {
    color: var(--muted2);
  }
  .detail {
    padding: 6px 10px 16px 46px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .field .input.add {
    max-width: 320px;
  }
  .attach {
    display: flex;
    gap: 8px;
  }
  .tags,
  .ignored {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
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
  /* Origin of an entry (REGLES§8.1). The shipped ones are the plain case; the
     mark goes on what the USER did - fewer marks, and they point at what an
     update will not touch. */
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
  .tok.gone s {
    text-decoration-color: var(--muted2);
  }
  .empty {
    color: var(--muted2);
    font-size: 12px;
  }
  .foot {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin-top: 16px;
  }
  .toggle {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: var(--muted);
  }
  .no-logo {
    width: 20px;
    height: 20px;
    border-radius: 50%;
    background: var(--raised);
  }
  .previews {
    display: flex;
    gap: 16px;
    flex-wrap: wrap;
    margin-bottom: 8px;
  }
  .sizes {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
    background: var(--panel);
    border: 1px solid var(--line);
    opacity: 0.6;
  }
  .sizes.chosen {
    opacity: 1;
    border-color: var(--muted2);
  }
  .render {
    margin-bottom: 10px;
  }
  .variants {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-bottom: 10px;
  }
  .variant {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    background: var(--panel);
    border: 1px solid var(--line);
    color: var(--txt2);
    text-align: left;
    cursor: pointer;
  }
  .variant.on {
    border-color: var(--txt2);
  }
  .v-meta {
    font-size: 10.5px;
    color: var(--muted);
    line-height: 1.4;
  }
  .logo-acts {
    display: flex;
    gap: 8px;
  }
  .name-rules {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .name-rule {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 12px;
    color: var(--txt2);
  }
  .name-rule.off {
    opacity: 0.55;
  }
  .nr-count {
    font-size: 11.5px;
    color: var(--muted);
  }
  /* The Rules screen's switch and badge (REGLES§12), same values. */
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
  .badge {
    font-size: 9px;
    letter-spacing: 0.12em;
    color: var(--muted);
    border: 1px solid var(--line);
    border-radius: 2px;
    padding: 1px 4px;
  }
  .link {
    background: none;
    border: none;
    padding: 0;
    color: var(--muted);
    text-decoration: underline;
    text-underline-offset: 2px;
    cursor: pointer;
    font-size: 11.5px;
  }
  .link:hover {
    color: var(--txt);
  }
  .how {
    display: grid;
    grid-template-columns: 190px 1fr;
    gap: 6px 12px;
    align-items: start;
    margin-top: 6px;
  }
  .how-lbl {
    font-size: 11.5px;
    color: var(--muted);
    padding-top: 3px;
  }
  .how > :global(*):nth-child(n + 3) {
    grid-column: 2;
  }
  .tok.self {
    color: var(--muted);
  }
  .caught {
    list-style: none;
    margin: 0;
    padding: 0;
    font-size: 11.5px;
  }
  .caught li {
    display: flex;
    gap: 10px;
    padding: 1px 0;
    color: var(--txt2);
  }
  .caught li.here {
    color: var(--faint);
  }
  .c-name {
    flex: 1;
    min-width: 0;
  }
  .c-brand {
    color: var(--muted);
  }
  .caught .more {
    color: var(--faint);
  }
</style>
