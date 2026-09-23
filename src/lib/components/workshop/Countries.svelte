<script lang="ts">
  // Countries tab of the Workshop (TAXO§6): the country as a TERM — the name it
  // is stored under, and the spellings that lead to it.
  //
  // The stored name is the game's English one (`nationalities.rs`), because
  // that is what carries a flag; the user reads it translated from its ISO
  // code (TAXO§12). Curating a country is therefore about its SPELLINGS, never
  // its image: the flag follows from the name (TAXO§3.1).
  //
  // Two tables per country, edited as the user's overlay on the catalogue
  // (REGLES§2) and merged in Rust: its SPELLINGS, which normalise a country a
  // mod declares, and its TAGS, which give it to a mod declaring none. The
  // tags lived in the Rules screen; they are an exact table too, not a
  // heuristic (REGLES§11), and one looks for them where the country is.
  //
  // **Saving re-applies them to the whole library**, unlike the
  // Categories tab: the country is decided at write time
  // (`harmonize::store`), so a new spelling changes what is stored for every
  // mod that writes it. Same write queue as Categories, failures shown and
  // logged (CLAUDE.md rule 6).
  import { onMount } from "svelte";
  import { errorText } from "$lib/errors";
  import { countryLabel, flagFor, gameCountries, gameCountry, loadFlags } from "$lib/flags.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { listLibrary } from "$lib/library/library";
  import { bumpLibraryVersion } from "$lib/library/libraryVersion.svelte";
  import {
    attachCountry,
    closestCountry,
    ignoreCountry,
    keyOrigins,
    keysTo,
    removeEntry,
    restoreEntries,
    setEntry,
    touches,
    unignoreCountry,
  } from "$lib/workshop/countryEdit";
  import { getTaxonomy, saveCountryOverlay, type MapOverlay, type TaxonomyView } from "$lib/workshop/rules";

  /** Catalogue, overlay and effective tables, as Rust last sent them. */
  let view = $state<TaxonomyView | null>(null);
  /** The user's decisions: what every gesture edits, ahead of the save. */
  let aliases = $state<MapOverlay>({});
  let tags = $state<MapOverlay>({});
  let ignored = $state<string[]>([]);
  const effAliases = $derived(view?.effective.country_aliases ?? {});
  const effTags = $derived(view?.effective.country_tags ?? {});
  const catAliases = $derived(view?.catalog.country_aliases ?? {});
  const catTags = $derived(view?.catalog.country_tags ?? {});
  /** Stored country of every mod, cars and tracks: one table for both
   * libraries (TAXO§2.2). */
  let mods = $state<{ kind: string; country: string | null }[]>([]);
  let loading = $state(true);
  let busy = $state(false);
  let error = $state("");
  let open = $state<string | null>(null);
  let newAlias = $state("");
  let newTag = $state("");
  let showEmpty = $state(false);
  /** Chosen target of each "attach to" picker, keyed by the value it moves. */
  let targets = $state<Record<string, string>>({});

  async function reloadMods() {
    const cards = await listLibrary();
    mods = cards.map((c) => ({ kind: c.kind, country: c.country }));
  }

  onMount(async () => {
    try {
      const [v] = await Promise.all([getTaxonomy(), reloadMods(), loadFlags()]);
      take(v);
    } catch (e) {
      error = errorText(e);
    } finally {
      loading = false;
    }
  });

  function take(v: TaxonomyView) {
    view = v;
    aliases = v.overlay.country_aliases;
    tags = v.overlay.country_tags;
    ignored = v.overlay.ignored_countries ?? [];
  }

  let queue: Promise<void> = Promise.resolve();
  /** Takes the new decisions at once, writes and re-applies them, then reads
   * the library back: the counts only mean something once the stored values
   * moved. The `catch` keeps one failure from freezing every later write. */
  function commit(next: { aliases?: MapOverlay; tags?: MapOverlay; ignored?: string[] }) {
    aliases = next.aliases ?? aliases;
    tags = next.tags ?? tags;
    ignored = next.ignored ?? ignored;
    const [a, tg, ig] = [aliases, tags, ignored];
    queue = queue
      .then(async () => {
        busy = true;
        take(await saveCountryOverlay(a, tg, ig));
        await reloadMods();
        bumpLibraryVersion();
        error = "";
      })
      .catch((e) => {
        console.error("save_country_aliases", e);
        error = errorText(e);
      })
      .finally(() => (busy = false));
  }

  interface Row {
    name: string;
    cars: number;
    tracks: number;
  }

  const rows = $derived.by(() => {
    const m = new Map<string, Row>();
    const row = (name: string) => m.get(name) ?? (m.set(name, { name, cars: 0, tracks: 0 }), m.get(name)!);
    for (const c of mods) {
      if (!c.country) continue;
      const r = row(c.country);
      if (c.kind === "Track") r.tracks++;
      else r.cars++;
    }
    // A country only aliases lead to - its mods gone - stays in base, hidden
    // unless asked for (TAXO§9): the spellings curated for it are kept.
    for (const to of Object.values(effAliases)) row(to);
    for (const to of Object.values(effTags)) row(to);
    return [...m.values()].sort((a, b) => b.cars + b.tracks - (a.cars + a.tracks) || a.name.localeCompare(b.name));
  });
  const shownRows = $derived(rows.filter((r) => showEmpty || r.cars + r.tracks > 0));
  const emptyCount = $derived(rows.length - rows.filter((r) => r.cars + r.tracks > 0).length);
  const unset = $derived(mods.filter((c) => !c.country).length);

  /** Every game country, in the user's language — the targets of a merge. */
  const choices = $derived(
    gameCountries()
      .map((n) => ({ name: n.name, label: countryLabel(n.name) }))
      .sort((a, b) => a.label.localeCompare(b.label)),
  );

  /** Values the game does not know, not yet ignored (TAXO§3.1): they get no
   * flag, and each is proposed for a merge, with the closest game country
   * pre-selected when there is a close one (TAXO§7.2). */
  const proposals = $derived(
    rows
      .filter((r) => r.cars + r.tracks > 0 && !gameCountry(r.name) && !ignored.includes(r.name))
      .map((r) => ({ ...r, closest: closestCountry(r.name, choices.map((c) => c.name)) })),
  );

  function targetOf(value: string, fallback: string | null): string {
    return targets[value] ?? fallback ?? "";
  }

  function toggle(name: string) {
    open = open === name ? null : name;
    newAlias = "";
    newTag = "";
  }

  function attach(from: string, to: string) {
    const out = attachCountry(
      { overlay: aliases, catalog: catAliases, effective: effAliases },
      { overlay: tags, catalog: catTags, effective: effTags },
      from,
      to,
    );
    commit({ ...out, ignored: unignoreCountry(ignored, from) });
  }

  function countText(r: Row): string {
    const parts: string[] = [];
    if (r.cars) parts.push(t(r.cars === 1 ? "index.carOne" : "index.cars", { count: r.cars }));
    if (r.tracks) parts.push(t(r.tracks === 1 ? "index.trackOne" : "index.tracks", { count: r.tracks }));
    return parts.join(" · ") || t("countriesTab.noMod");
  }
</script>

{#snippet flagOf(name: string)}
  {@const flag = flagFor(name)}
  {#if flag}<img class="flag" src={flag} alt="" />{:else}<span class="flag none" aria-hidden="true"></span>{/if}
{/snippet}

{#snippet picker(value: string, fallback: string | null)}
  <select
    class="input pick"
    aria-label={t("countriesTab.attachTo")}
    value={targetOf(value, fallback)}
    onchange={(e) => (targets = { ...targets, [value]: e.currentTarget.value })}
  >
    <option value="" disabled>{t("countriesTab.attachTo")}</option>
    {#each choices as c (c.name)}
      {#if c.name !== value}<option value={c.name}>{c.label}</option>{/if}
    {/each}
  </select>
  <button
    type="button"
    class="btn"
    disabled={busy || !targetOf(value, fallback)}
    onclick={() => {
      attach(value, targetOf(value, fallback));
      if (open === value) open = targetOf(value, fallback);
    }}>{t("countriesTab.attach")}</button
  >
{/snippet}

<div class="countries">
  <!-- Workshop tab: the screen title belongs to the Workshop. -->
  <header>
    <p class="lbl-sub">{t("countriesTab.subtitle")}</p>
    {#if !loading}
      <p class="meta">
        {t("countriesTab.unset", { count: unset })}
        {#if busy}<span class="busy">· {t("countriesTab.applying")}</span>{/if}
      </p>
    {/if}
  </header>

  {#if error}<div class="errbox">{error}</div>{/if}

  {#if !loading && proposals.length}
    <!-- Proposals (TAXO§7.2): never merged silently, and "Ignore" is
         remembered - the value stops being offered, until put back below. -->
    <div class="warnbox proposals">
      <p class="ptitle">⚑ {t("countriesTab.proposals", { count: proposals.length })}</p>
      {#each proposals as p (p.name)}
        <div class="prop">
          <span class="pname">{p.name}</span>
          <span class="pcount">{countText(p)}</span>
          {@render picker(p.name, p.closest)}
          <button type="button" class="btn" disabled={busy} onclick={() => commit({ ignored: ignoreCountry(ignored, p.name) })}
            >{t("countriesTab.ignore")}</button
          >
        </div>
      {/each}
    </div>
  {/if}

  {#if !loading}
    <ul class="list">
      {#each shownRows as r (r.name)}
        {@const label = countryLabel(r.name)}
        {@const entry = gameCountry(r.name)}
        {@const spelled = keysTo(effAliases, r.name)}
        {@const aliasOrigin = keyOrigins(effAliases, catAliases, r.name)}
        {@const tagOrigin = keyOrigins(effTags, catTags, r.name)}
        {@const curated = touches(aliases, catAliases, r.name) || touches(tags, catTags, r.name)}
        <li>
          <button type="button" class="row" aria-expanded={open === r.name} onclick={() => toggle(r.name)}>
            {@render flagOf(r.name)}
            <span class="nm">{label}{#if label !== r.name}<span class="stored mono">{r.name}</span>{/if}</span>
            <span class="num">{countText(r)}</span>
            <span class="num">{t("countriesTab.aliasCount", { count: spelled.length })}</span>
            <span class="curated" class:on={curated} title={curated ? t("categories.curated") : undefined} aria-hidden={!curated}
              >⚑</span
            >
            <span class="chev" aria-hidden="true">{open === r.name ? "▾" : "›"}</span>
          </button>

          {#if open === r.name}
            <div class="detail">
              <div class="field">
                <span class="lbl-key">{t("countriesTab.code")}</span>
                <span class="mono code">
                  {#if entry}{entry.code}{#if entry.iso2} · {entry.iso2}{/if}{:else}{t("countriesTab.unknown")}{/if}
                </span>
              </div>

              <div class="field">
                <span class="lbl-key">{t("countriesTab.aliases")}</span>
                <div class="tags">
                  {#each aliasOrigin.keys as { key: alias, mine } (alias)}
                    <span class="tok" class:mine title={mine ? t("taxonomyOrigin.added") : undefined}
                      >{#if mine}<span class="mark" aria-hidden="true">✎</span>{/if}{alias}<button
                        type="button"
                        class="x"
                        title={t("countriesTab.removeAlias")}
                        disabled={busy}
                        onclick={() => commit({ aliases: removeEntry(aliases, catAliases, alias) })}>×</button
                      ></span
                    >
                  {:else}
                    <span class="empty">{t("countriesTab.noAlias")}</span>
                  {/each}
                  {#each aliasOrigin.gone as alias (alias)}
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
                </div>
                <input
                  class="input add"
                  placeholder={t("countriesTab.addAlias")}
                  bind:value={newAlias}
                  disabled={busy}
                  onkeydown={(e) => {
                    if (e.key !== "Enter" || !newAlias.trim()) return;
                    commit({ aliases: setEntry(aliases, catAliases, newAlias, r.name, true) });
                    newAlias = "";
                  }}
                />
              </div>

              <!-- Only used when the mod declares no country: a tag never
                   rewrites what the author declared (§5). -->
              <div class="field">
                <span class="lbl-key">{t("countriesTab.tags")}</span>
                <div class="tags">
                  {#each tagOrigin.keys as { key: tag, mine } (tag)}
                    <span class="tok" class:mine title={mine ? t("taxonomyOrigin.added") : undefined}
                      >{#if mine}<span class="mark" aria-hidden="true">✎</span>{/if}{tag}<button
                        type="button"
                        class="x"
                        title={t("countriesTab.removeTag")}
                        disabled={busy}
                        onclick={() => commit({ tags: removeEntry(tags, catTags, tag) })}>×</button
                      ></span
                    >
                  {:else}
                    <span class="empty">{t("countriesTab.noAlias")}</span>
                  {/each}
                  {#each tagOrigin.gone as tag (tag)}
                    <span class="tok gone" title={t("taxonomyOrigin.removed")}
                      ><s>{tag}</s><button
                        type="button"
                        class="x"
                        title={t("taxonomyOrigin.putBack")}
                        disabled={busy}
                        onclick={() => commit({ tags: setEntry(tags, catTags, tag, r.name) })}>↺</button
                      ></span
                    >
                  {/each}
                </div>
                <input
                  class="input add"
                  placeholder={t("countriesTab.addTag")}
                  bind:value={newTag}
                  disabled={busy}
                  onkeydown={(e) => {
                    if (e.key !== "Enter" || !newTag.trim()) return;
                    commit({ tags: setEntry(tags, catTags, newTag, r.name) });
                    newTag = "";
                  }}
                />
              </div>

              <div class="field">
                <span class="lbl-key">{t("countriesTab.attachTo")}</span>
                <div class="attach">{@render picker(r.name, null)}</div>
              </div>

              {#if curated}
                <div>
                  <button type="button" class="btn" disabled={busy} onclick={() =>
                      commit({
                        aliases: restoreEntries(aliases, catAliases, r.name),
                        tags: restoreEntries(tags, catTags, r.name),
                      })}>
                    {t("countriesTab.restore")}
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
          <span>{t("countriesTab.showEmpty", { count: emptyCount })}</span>
        </label>
      {/if}
      {#if ignored.length}
        <div class="ignored">
          <span class="lbl-key">{t("countriesTab.ignored")}</span>
          {#each ignored as v (v)}
            <span class="tok"
              >{v}<button
                type="button"
                class="x"
                title={t("countriesTab.unignore")}
                disabled={busy}
                onclick={() => commit({ ignored: unignoreCountry(ignored, v) })}>×</button
              ></span
            >
          {/each}
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .countries {
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
    grid-template-columns: 24px 1fr 190px 80px 18px 16px;
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
  .flag {
    width: 20px;
    height: 14px;
    object-fit: cover;
    border: 1px solid var(--line);
  }
  .flag.none {
    display: block;
    background: repeating-linear-gradient(135deg, var(--raised), var(--raised) 3px, var(--mat) 3px, var(--mat) 6px);
  }
  .stored {
    margin-left: 8px;
    color: var(--muted2);
    font-size: 11px;
  }
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
  .code {
    font-size: 12px;
    color: var(--txt2);
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
</style>
