<script lang="ts">
  // A dialogue on the scene (SPEC-story-mode, In-person dialogues): a 16:9
  // stage with the scene behind (a plain background until the story's scenes
  // exist), the characters present as portraits, the line in a text box at
  // the bottom, and the replies once the last line is read. A click, Space
  // or Enter shows the next line. Who stands where with which face is
  // `stageAt`'s, so a dialogue shows the same here and in Story Mode.
  import { t } from "$lib/i18n/index.svelte";
  import { initials } from "$lib/story/player/phone";
  import { nextLine, stageAt } from "$lib/story/player/stage";
  import type { PlayerChoice, PlayerStep, Speaker } from "$lib/story/player/types";

  interface Props {
    steps: PlayerStep[];
    choices: PlayerChoice[];
    /** The characters present, in the order listed. */
    present: string[];
    /** The item's sender: who says a line without a speaker. */
    from: string | null;
    speakers: Record<string, Speaker>;
    ended: boolean;
    onchoose: (label: string) => void;
    /** The player went past the last line of an ended dialogue: Story Mode
     * closes the layer; the preview has nothing to close. */
    onend?: () => void;
  }
  const { steps, choices, present, from, speakers, ended, onchoose, onend }: Props = $props();

  /** The said line shown, as an index into `steps`. */
  let at = $state(-1);
  /** A reply was chosen: show the first line it leads to when it comes. */
  let chosen = $state(false);

  // Every dependency read first (CLAUDE.md: an effect only follows what it
  // read before leaving).
  $effect(() => {
    const [list, now, waiting] = [steps, at, chosen];
    if (waiting) {
      const next = nextLine(list, now);
      if (next !== null) {
        at = next;
        chosen = false;
      }
    } else if (now < 0 || now >= list.length || list[now]?.kind !== "said") {
      // Opened, or the text changed under the preview: from its first line.
      at = nextLine(list, -1) ?? -1;
    }
  });

  const stage = $derived(stageAt(present, from, steps, at));
  const more = $derived(nextLine(steps, at) !== null);

  function advance() {
    const next = nextLine(steps, at);
    if (next !== null) at = next;
    else if (ended) onend?.();
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === " " || e.key === "Enter") {
      e.preventDefault();
      advance();
    }
  }

  function choose(label: string) {
    chosen = true;
    onchoose(label);
  }

  const LEFT = { left: "20%", center: "50%", right: "80%" } as const;

  function picture(who: string, expression: string): string | null {
    const portraits = speakers[who]?.portraits ?? {};
    return portraits[expression] ?? portraits.neutral ?? null;
  }
</script>

<div class="stage" role="button" tabindex="0" aria-label={t("story.player.next")} onclick={advance} {onkeydown}>
  {#each stage.cast as c (c.who)}
    {@const src = picture(c.who, c.expression)}
    <div class="actor" class:speaking={c.speaking} style:left={LEFT[c.position]}>
      {#if src}
        <img {src} alt={speakers[c.who]?.name ?? c.who} />
      {:else}
        <span class="silhouette">{initials(speakers[c.who]?.name ?? c.who)}</span>
      {/if}
    </div>
  {/each}

  {#if !more && choices.length}
    <div class="choices">
      {#each choices as c (c.label)}
        <button
          class="choice"
          onclick={(e) => {
            e.stopPropagation();
            choose(c.label);
          }}>{c.text}</button
        >
      {/each}
    </div>
  {/if}

  {#if stage.line}
    {@const who = stage.line.speaker ?? from}
    <div class="box">
      {#if who}<div class="speaker">{speakers[who]?.name ?? who}</div>{/if}
      <div class="text">{stage.line.text}</div>
      {#if more}
        <span class="more">▾</span>
      {:else if ended}
        <span class="more end">{t("story.player.end")}</span>
      {/if}
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
  .stage:focus-visible {
    outline: 1px solid var(--mat-line);
  }
  .actor {
    position: absolute;
    bottom: 0;
    height: 78%;
    transform: translateX(-50%);
    display: flex;
    align-items: flex-end;
    opacity: 0.45;
    transition: opacity 0.15s;
  }
  .actor.speaking {
    opacity: 1;
  }
  .actor img {
    height: 100%;
    width: auto;
    object-fit: contain;
  }
  .silhouette {
    width: 90px;
    height: 70%;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 45px 45px 0 0;
    background: var(--mat);
    color: var(--txt2);
    font-size: 14px;
    font-weight: 600;
  }
  .box {
    position: absolute;
    left: 4%;
    right: 4%;
    bottom: 4%;
    min-height: 22%;
    padding: 10px 14px;
    background: color-mix(in srgb, var(--bg) 88%, transparent);
    border: 1px solid var(--mat-line);
  }
  .speaker {
    color: var(--txt);
    font-size: 12px;
    font-weight: 600;
    margin-bottom: 4px;
  }
  .text {
    color: var(--txt2);
    font-size: 13px;
    line-height: 1.45;
  }
  .more {
    position: absolute;
    right: 10px;
    bottom: 6px;
    color: var(--muted);
    font-size: 11px;
  }
  .choices {
    position: absolute;
    left: 50%;
    top: 18%;
    transform: translateX(-50%);
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 45%;
  }
  .choice {
    padding: 8px 14px;
    background: color-mix(in srgb, var(--bg) 88%, transparent);
    border: 1px solid var(--mat-line);
    color: var(--txt2);
    font-size: 13px;
    text-align: center;
  }
  .choice:hover {
    border-color: var(--blue-border);
    color: var(--txt);
  }
</style>
