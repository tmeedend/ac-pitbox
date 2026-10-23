<script lang="ts">
  // An illustration on the scene (SPEC-story-mode, In-person dialogues): a
  // full-screen picture for the big moments, with its text, if any, in a box
  // at the bottom. A click, Space or Enter closes it in Story Mode. The same
  // 16:9 stage as a dialogue's, so that the author previews the composition
  // the player sees.
  import { onMount } from "svelte";
  import { t } from "$lib/i18n/index.svelte";
  import type { PlayerStep } from "$lib/story/player/types";

  interface Props {
    /** The picture's URL; `null` when there is none to show (the sample
     * played from inside Pit Box carries no media). */
    picture: string | null;
    steps: PlayerStep[];
    /** No background or border of its own: the scene behind shows through. */
    bare?: boolean;
    /** Takes the keyboard when it opens. */
    autofocus?: boolean;
    /** The player is done looking: Story Mode closes it. */
    onend?: () => void;
  }
  const { picture, steps, bare = false, autofocus = false, onend }: Props = $props();

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
  {#if picture}<img src={picture} alt="" />{/if}
  {#if steps.length}
    <div class="box">
      {#each steps as s, i (i)}
        <p>{s.text}</p>
      {/each}
    </div>
  {/if}
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
  }
  .stage.bare {
    border: none;
  }
  .stage:focus-visible {
    outline: 1px solid var(--mat-line);
  }
  .stage.bare:focus-visible {
    outline: none;
  }
  img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .box {
    position: absolute;
    left: 4%;
    right: 4%;
    bottom: 4%;
    padding: 10px 14px;
    display: flex;
    flex-direction: column;
    gap: 4px;
    background: color-mix(in srgb, var(--bg) 88%, transparent);
    border: 1px solid var(--mat-line);
  }
  p {
    margin: 0;
    color: var(--txt2);
    font-size: 13px;
    line-height: 1.45;
  }
</style>
