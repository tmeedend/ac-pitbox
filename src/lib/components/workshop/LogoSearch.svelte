<script lang="ts">
  // A brand's logo searched online (TAXO§9.1): Wikidata's logos for the
  // brand first, then a Wikimedia Commons search, page by page. The user picks
  // one; it is downloaded and becomes his own logo file, as one he gave would
  // (TAXO§9). Nothing is fetched before he opens this.
  import { onMount } from "svelte";
  import Modal from "$lib/components/ui/Modal.svelte";
  import { errorText } from "$lib/errors";
  import { t } from "$lib/i18n/index.svelte";
  import { adoptBrandLogo, searchBrandLogos, type LogoCandidate } from "$lib/library/brandLogos.svelte";

  interface Props {
    brand: string;
    onclose: () => void;
  }
  let { brand, onclose }: Props = $props();

  // The brand's name to start with, edited at will ("Renault Sport").
  let query = $state("");
  let candidates = $state<LogoCandidate[]>([]);
  let next = $state<number | null>(null);
  let searched = $state(false);
  let loading = $state(false);
  let adopting = $state(false);
  let error = $state("");
  /** The picked candidate, by its URL: a value, not the object (CLAUDE.md,
   * `$state` proxies). */
  let picked = $state<string | null>(null);

  async function search(offset: number) {
    loading = true;
    error = "";
    try {
      const page = await searchBrandLogos(query, offset);
      candidates = offset === 0 ? page.candidates : [...candidates, ...page.candidates];
      next = page.next;
      if (offset === 0) picked = null;
    } catch (e) {
      error = errorText(e);
    } finally {
      loading = false;
      searched = true;
    }
  }

  async function adopt() {
    if (!picked) return;
    adopting = true;
    error = "";
    try {
      await adoptBrandLogo(brand, picked);
      onclose();
    } catch (e) {
      error = errorText(e);
    } finally {
      adopting = false;
    }
  }

  onMount(() => {
    query = brand;
    void search(0);
  });
</script>

<Modal title={t("logoSearch.title", { brand })} width="760px" onclose={adopting ? undefined : onclose}>
  <form
    class="bar"
    onsubmit={(e) => {
      e.preventDefault();
      void search(0);
    }}
  >
    <input class="input" bind:value={query} spellcheck="false" />
    <button type="submit" class="btn" disabled={loading || !query.trim()}>{t("logoSearch.search")}</button>
  </form>

  {#if error}<div class="errbox">{error}</div>{/if}

  {#if candidates.length}
    <div class="grid">
      {#each candidates as c (c.url)}
        <button
          type="button"
          class="cand"
          class:on={picked === c.url}
          title={c.name}
          disabled={adopting}
          onclick={() => (picked = c.url)}
          ondblclick={() => {
            picked = c.url;
            void adopt();
          }}
        >
          <span class="thumb"><img src={c.thumb} alt={c.name} loading="lazy" /></span>
          <span class="meta mono">{c.format.toUpperCase()} · {c.width}×{c.height}</span>
          {#if c.official}<span class="pill" title={t("logoSearch.officialTip")}>{t("logoSearch.official")}</span>{/if}
        </button>
      {/each}
    </div>
    {#if next !== null}
      <button type="button" class="btn more" disabled={loading} onclick={() => void search(next ?? 0)}
        >{t("logoSearch.more")}</button
      >
    {/if}
  {:else if searched && !loading && !error}
    <p class="empty">{t("logoSearch.none")}</p>
  {/if}
  <p class="source lbl-sub">{t("logoSearch.source")}</p>

  {#snippet footer()}
    <button type="button" class="btn" disabled={adopting} onclick={onclose}>{t("common.cancel")}</button>
    <button type="button" class="btn btn-primary" disabled={!picked || adopting} onclick={() => void adopt()}
      >{t("logoSearch.use")}</button
    >
  {/snippet}
</Modal>

<style>
  .bar {
    display: flex;
    gap: 8px;
  }
  .bar .input {
    flex: 1;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(130px, 1fr));
    gap: 8px;
  }
  .cand {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    padding: 8px;
    background: var(--panel2);
    border: 1px solid var(--line);
  }
  .cand.on {
    border-color: var(--rosso-bright);
    background: var(--rosso-dim);
  }
  /* A light ground: most logos are drawn for one, and a black one would
     vanish on the dark interface. The plate of a baked logo (TAXO§5), in the
     same token as `Emblem`'s. */
  .thumb {
    width: 100%;
    height: 90px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--txt);
  }
  .thumb img {
    max-width: 90%;
    max-height: 80px;
    object-fit: contain;
  }
  .meta {
    font-size: 10px;
    color: var(--muted);
  }
  .pill {
    position: absolute;
    top: 4px;
    left: 4px;
  }
  .more {
    align-self: center;
  }
  .empty {
    color: var(--muted);
    font-size: 12px;
  }
  .source {
    margin-top: 2px;
  }
</style>
