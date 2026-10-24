<script lang="ts">
  // A video no app shows, on the scene as a TV report (SPEC-story-mode,
  // Content types, templates and frames): its still in a television set,
  // the channel and the line said in a strip across the bottom (the lower
  // third). A click, Space or Enter shows the next line, and after the last
  // closes it in Story Mode. The same 16:9 stage as a dialogue's.
  import { onMount } from "svelte";
  import { t } from "$lib/i18n/index.svelte";
  import type { PlayerStep } from "$lib/story/player/types";

  interface Props {
    /** The channel's name: who it is from. */
    channel: string | null;
    /** The still's URL, if it has one. */
    picture: string | null;
    steps: PlayerStep[];
    bare?: boolean;
    autofocus?: boolean;
    onend?: () => void;
  }
  const { channel, picture, steps, bare = false, autofocus = false, onend }: Props = $props();

  let at = $state(0);
  const line = $derived(steps[at]?.text ?? "");

  let element: HTMLDivElement;
  onMount(() => {
    if (autofocus) element.focus();
  });

  function next() {
    if (at < steps.length - 1) at++;
    else onend?.();
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === " " || e.key === "Enter") {
      e.preventDefault();
      next();
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
  onclick={next}
  {onkeydown}
>
  <div class="tv-set">
    <div class="picture">
      {#if picture}<img src={picture} alt="" />{/if}
      {#if channel || line}
        <div class="lower-third">
          {#if channel}<span class="channel">{channel}</span>{/if}
          <span class="line">{line}</span>
        </div>
      {/if}
    </div>
  </div>
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
  /* The set: a dark bezel around a 16:9 screen. */
  .tv-set {
    width: 72cqw;
    padding: 1.6cqw;
    border-radius: 1.4cqw;
    background: var(--mat);
    box-shadow: 0 10px 40px rgba(0, 0, 0, 0.5);
  }
  .picture {
    position: relative;
    aspect-ratio: 16 / 9;
    background: var(--bg);
    overflow: hidden;
  }
  img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .lower-third {
    position: absolute;
    left: 4%;
    right: 4%;
    bottom: 6%;
    display: flex;
    flex-direction: column;
    gap: 0.4cqh;
    padding: 1cqh 1.4cqw;
    background: color-mix(in srgb, var(--bg) 85%, transparent);
    border-left: 4px solid var(--blue);
  }
  .channel {
    color: var(--blue);
    font-size: max(10px, 1.8cqh);
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }
  .line {
    color: var(--txt);
    font-size: max(12px, 2.6cqh);
  }
</style>
