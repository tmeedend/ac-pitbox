<script lang="ts">
  // The journal (SPEC-story-mode, Story Mode: player side): the dialogues
  // played so far, latest first, each read again on the scene as it was
  // played - the replies chosen included, nothing to answer. Rebuilt from the
  // save's history like the phone's threads, so a game resumed keeps it.
  // Escape (the gamepad's B too) goes back to the list, then closes.
  import { i18n, t } from "$lib/i18n/index.svelte";
  import { pastDialogues } from "$lib/story/player/game";
  import { dateLabel, type DateDisplay } from "$lib/story/player/phone";
  import type { Speaker } from "$lib/story/player/types";
  import type { SeenItem } from "$lib/story/play";
  import DialogueStage from "./player/DialogueStage.svelte";

  interface Props {
    items: SeenItem[];
    dateDisplay: DateDisplay;
    speakers: Record<string, Speaker>;
    onclose: () => void;
  }
  const { items, dateDisplay, speakers, onclose }: Props = $props();

  const past = $derived(pastDialogues(items));
  /** The dialogue read again, as an index into `items`. */
  let open = $state<number | null>(null);
  const shown = $derived(open === null ? null : items[open]);

  function when(item: SeenItem): string {
    const date = dateLabel(item.date, dateDisplay, t, i18n.locale);
    return date ? `${date} · ${item.clock}` : item.clock;
  }

  function who(item: SeenItem): string {
    const ids = item.characters.length ? item.characters : item.from ? [item.from] : [];
    return ids.map((id) => speakers[id]?.name ?? id).join(", ");
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key !== "Escape") return;
    e.preventDefault();
    if (open !== null) open = null;
    else onclose();
  }
</script>

<svelte:window {onkeydown} />

{#if shown}
  <div class="replay">
    <div class="replay-bar">
      <button class="btn btn-ghost" onclick={() => (open = null)}>‹ {t("story.journal.title")}</button>
      <span class="when">{when(shown)}</span>
    </div>
    <div class="stage-box">
      <div class="fit">
        {#key open}
          <DialogueStage
            steps={shown.steps}
            choices={[]}
            present={shown.characters}
            from={shown.from}
            {speakers}
            ended
            replay
            bare
            autofocus
            onchoose={() => {}}
            onend={() => (open = null)}
          />
        {/key}
      </div>
    </div>
  </div>
{:else}
  <section class="list">
    <header>
      <span class="lbl-sub">{t("story.journal.title")}</span>
      <button class="btn btn-ghost" onclick={onclose}>{t("story.journal.close")}</button>
    </header>
    <ul>
      {#each past as index (index)}
        {@const item = items[index]}
        <li>
          <button class="row" onclick={() => (open = index)}>
            <span class="who">{who(item)}</span>
            <span class="when">{when(item)}</span>
          </button>
        </li>
      {:else}
        <li class="none">{t("story.journal.empty")}</li>
      {/each}
    </ul>
  </section>
{/if}

<style>
  .list {
    width: min(380px, 100%);
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
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    overflow-y: auto;
  }
  .row {
    display: flex;
    flex-direction: column;
    gap: 2px;
    width: 100%;
    padding: 9px 14px;
    background: transparent;
    border-bottom: 1px solid var(--line);
    text-align: left;
  }
  .row:hover {
    background: var(--raised);
  }
  .who {
    font-size: 12.5px;
    color: var(--txt);
  }
  .when {
    font-size: 11.5px;
    color: var(--muted);
  }
  .none {
    padding: 12px 14px;
    font-size: 12px;
    color: var(--muted);
  }
  .replay {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .replay-bar {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  /* The stage as large as the scene allows, in 16:9 like the preview's. */
  .stage-box {
    flex: 1;
    min-height: 0;
    container-type: size;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .fit {
    width: min(100cqw, calc(100cqh * 16 / 9));
  }
</style>
