<script lang="ts">
  // Picks a car or a track of the player's library for an event
  // (SPEC-story-mode, Create Your Own: Events): the event names what the
  // author has, never an id typed by hand. The same search as the library's
  // own (`matchesQuery`): name, brand, id, tags.
  import { onMount } from "svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { matchesQuery } from "$lib/library/cardSearch";
  import { previewSrc, type ModCard, type ModKind } from "$lib/library/library";
  import { loadLibrary, studioLibrary } from "$lib/story/library.svelte";
  import Modal from "$lib/components/ui/Modal.svelte";

  interface Props {
    kind: ModKind;
    /** The id the event has now, marked in the list. */
    current?: string;
    onpick: (card: ModCard) => void;
    onclose: () => void;
  }
  const { kind, current, onpick, onclose }: Props = $props();

  let query = $state("");
  let field = $state<HTMLInputElement | null>(null);

  const cards = $derived(
    (studioLibrary.cards ?? [])
      .filter((c) => c.kind === kind && matchesQuery(c, query))
      .sort((a, b) => (a.display_name ?? a.id_interne).localeCompare(b.display_name ?? b.id_interne)),
  );

  onMount(() => {
    void loadLibrary();
    field?.focus();
  });
</script>

<Modal title={t(kind === "Car" ? "story.events.pickCar" : "story.events.pickTrack")} {onclose} width="560px">
  <input
    class="input search"
    bind:this={field}
    bind:value={query}
    placeholder={t("story.events.search")}
    aria-label={t("story.events.search")}
  />
  {#if studioLibrary.cards === null}
    <p class="empty">{t("story.events.libraryLoading")}</p>
  {:else if cards.length === 0}
    <p class="empty">{t("story.events.noMatch")}</p>
  {:else}
    <ul class="list">
      {#each cards as c (c.id_interne)}
        <li>
          <button class="item" class:on={c.id_interne === current} onclick={() => onpick(c)}>
            <span class="thumb">
              {#if c.preview}<img src={previewSrc(c.preview)} alt="" loading="lazy" />{/if}
            </span>
            <span class="what">
              <span class="name">{c.display_name ?? c.id_interne}</span>
              <span class="id mono">{c.id_interne}</span>
            </span>
            {#if c.brand}<span class="brand">{c.brand}</span>{/if}
          </button>
        </li>
      {/each}
    </ul>
  {/if}
  {#snippet footer()}
    <button class="btn" onclick={onclose}>{t("story.events.cancel")}</button>
  {/snippet}
</Modal>

<style>
  .search {
    width: 100%;
  }
  .empty {
    color: var(--muted);
    font-size: 12.5px;
    margin: 14px 0 4px;
  }
  .list {
    list-style: none;
    margin: 10px 0 0;
    padding: 0;
    max-height: calc(55 * var(--vh));
    overflow: auto;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    padding: 6px 8px;
    text-align: left;
    background: transparent;
    color: var(--txt2);
  }
  .item:hover {
    background: var(--raised);
    color: var(--txt);
  }
  .item.on {
    background: var(--raised);
    box-shadow: inset 2px 0 0 var(--rosso);
  }
  .thumb {
    width: 64px;
    height: 36px;
    flex-shrink: 0;
    background: var(--raised);
    overflow: hidden;
  }
  .thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .what {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
  }
  .name {
    font-size: 13px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .id {
    font-size: 10.5px;
    color: var(--muted);
  }
  .brand {
    font-size: 11.5px;
    color: var(--muted);
    flex-shrink: 0;
  }
</style>
