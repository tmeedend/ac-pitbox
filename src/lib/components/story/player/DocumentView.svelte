<script lang="ts">
  // A document on the scene (SPEC-story-mode, Content types, templates and
  // frames): something read in person. As a `newspaper`, a page - the
  // paper's name on its masthead, the first line as the headline, the photo,
  // the rest in columns; as a `letter`, a sheet with its lines. It offers no
  // reply: a click, Space or Enter closes it in Story Mode. The same 16:9
  // stage as a dialogue's, so that the author previews what the player sees.
  import { onMount } from "svelte";
  import { t } from "$lib/i18n/index.svelte";
  import type { PlayerStep } from "$lib/story/player/types";

  interface Props {
    layout: "newspaper" | "letter";
    /** Who it is from: the paper's name, a letter's sender. */
    from: string | null;
    /** A photo's URL, if it has one. */
    picture: string | null;
    steps: PlayerStep[];
    bare?: boolean;
    autofocus?: boolean;
    onend?: () => void;
  }
  const { layout, from, picture, steps, bare = false, autofocus = false, onend }: Props = $props();

  const headline = $derived(layout === "newspaper" ? (steps[0]?.text ?? "") : "");
  const body = $derived(layout === "newspaper" ? steps.slice(1) : steps);

  let element: HTMLDivElement;
  onMount(() => {
    if (autofocus) element.focus();
  });

  function onkeydown(e: KeyboardEvent) {
    if (e.key === " " || e.key === "Enter") {
      e.preventDefault();
      onend?.();
    }
  }
</script>

<div
  class="stage"
  class:bare
  role="button"
  tabindex="0"
  aria-label={t("story.player.next")}
  bind:this={element}
  onclick={() => onend?.()}
  {onkeydown}
>
  <article class="paper {layout}">
    {#if layout === "newspaper"}
      {#if from}<header class="masthead">{from}</header>{/if}
      <h1>{headline}</h1>
      {#if picture}<img class="photo" src={picture} alt="" />{/if}
      <div class="columns">
        {#each body as s, i (i)}<p>{s.text}</p>{/each}
      </div>
    {:else}
      {#each body as s, i (i)}<p>{s.text}</p>{/each}
      {#if picture}<img class="photo" src={picture} alt="" />{/if}
    {/if}
  </article>
</div>

<style>
  .stage {
    position: relative;
    width: 100%;
    aspect-ratio: 16 / 9;
    background: var(--panel);
    border: 1px solid var(--line);
    overflow: hidden;
    cursor: pointer;
    user-select: none;
    display: flex;
    align-items: center;
    justify-content: center;
    container-type: size;
  }
  .stage.bare {
    background: transparent;
    border: none;
  }
  .stage:focus-visible {
    outline: 1px solid var(--mat-line);
  }
  .stage.bare:focus-visible {
    outline: none;
  }
  /* The sheet: the skin's background as its paper, its text as the ink. */
  .paper {
    max-height: 92cqh;
    overflow-y: auto;
    background: var(--panel2);
    color: var(--txt);
    box-shadow: 0 8px 30px rgba(0, 0, 0, 0.45);
  }
  .newspaper {
    width: min(70cqw, 125cqh);
    padding: 2.4cqh 3cqw;
  }
  .letter {
    width: min(46cqw, 82cqh);
    padding: 5cqh 4cqw;
    font-size: max(13px, 2.4cqh);
    line-height: 1.6;
  }
  .masthead {
    text-align: center;
    font-size: max(18px, 5.5cqh);
    font-weight: 700;
    letter-spacing: 0.02em;
    padding-bottom: 1cqh;
    border-bottom: 2px solid currentColor;
    margin-bottom: 1.6cqh;
  }
  h1 {
    margin: 0 0 1.6cqh;
    font-size: max(16px, 4.2cqh);
    line-height: 1.15;
    color: var(--txt);
  }
  .photo {
    display: block;
    width: 100%;
    max-height: 34cqh;
    object-fit: cover;
    margin-bottom: 1.6cqh;
  }
  .columns {
    column-count: 2;
    column-gap: 2.4cqw;
    font-size: max(12px, 2.1cqh);
    line-height: 1.5;
  }
  p {
    margin: 0 0 1cqh;
  }
  .letter p {
    margin-bottom: 1.6cqh;
  }
</style>
