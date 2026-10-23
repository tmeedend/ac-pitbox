<script lang="ts">
  // The dialogue log (SPEC-story-mode, In-person dialogues): the lines of
  // the dialogues played so far, the latest at the bottom, the replies chosen
  // in the player's blue. Over the stage while a dialogue plays; Escape, the
  // gamepad's B or a click outside close it.
  import { tick } from "svelte";
  import { t } from "$lib/i18n/index.svelte";
  import type { LogLine } from "$lib/story/player/game";
  import type { Speaker } from "$lib/story/player/types";

  interface Props {
    lines: LogLine[];
    speakers: Record<string, Speaker>;
    onclose: () => void;
  }
  const { lines, speakers, onclose }: Props = $props();

  let list = $state<HTMLDivElement | undefined>(undefined);

  // Opened on the latest line: the panel scrolls itself, never the page.
  $effect(() => {
    void lines.length;
    void tick().then(() => {
      if (list) list.scrollTop = list.scrollHeight;
    });
  });

  function onkeydown(e: KeyboardEvent) {
    if (e.key !== "Escape") return;
    e.preventDefault();
    onclose();
  }
</script>

<svelte:window {onkeydown} />

<div class="backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && onclose()}>
  <div class="log" role="dialog" aria-label={t("story.player.log")}>
    <header>
      <span class="lbl-sub">{t("story.player.log")}</span>
      <button class="btn btn-ghost" onclick={onclose}>{t("story.journal.close")}</button>
    </header>
    <div class="lines" bind:this={list}>
      {#each lines as l, i (i)}
        {#if l.step.kind === "chose"}
          <p class="mine"><span class="who">{t("story.player.you")}</span>{l.step.text}</p>
        {:else}
          {@const who = l.step.speaker ?? l.from}
          <p>{#if who}<span class="who">{speakers[who]?.name ?? who}</span>{/if}{l.step.text}</p>
        {/if}
      {/each}
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 24px;
    background: color-mix(in srgb, var(--bg) 60%, transparent);
  }
  .log {
    width: min(640px, 100%);
    max-height: 100%;
    display: flex;
    flex-direction: column;
    border: 1px solid var(--line);
    background: var(--card);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 8px 8px 14px;
    border-bottom: 1px solid var(--line);
  }
  .lines {
    overflow-y: auto;
    padding: 10px 14px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  p {
    margin: 0;
    color: var(--txt2);
    font-size: 13px;
    line-height: 1.45;
  }
  .who {
    display: block;
    color: var(--txt);
    font-size: 11.5px;
    font-weight: 600;
  }
  .mine {
    padding-left: 8px;
    border-left: 2px solid var(--blue);
  }
</style>
