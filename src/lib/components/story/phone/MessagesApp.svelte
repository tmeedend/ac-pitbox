<script lang="ts">
  // The phone's messaging app in a game (SPEC-story-mode, Content types,
  // templates and frames): one thread per sender, the latest first and
  // the unread ones marked; a thread opened shows its bubbles and the replies
  // its open message offers. Opening a thread marks its messages seen. What
  // the phone has no template for yet is listed by name (L3.5). The next
  // event, when the story waits for one, heads the list (`next`).
  import type { Snippet } from "svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { CONTENT_TYPE_LABEL } from "$lib/story/labels";
  import { phoneThreads, withoutTemplate, type Thread } from "$lib/story/player/game";
  import type { Speaker } from "$lib/story/player/types";
  import type { SeenItem } from "$lib/story/play";
  import PhoneFrame from "$lib/components/story/player/PhoneFrame.svelte";
  import ThreadView from "$lib/components/story/player/ThreadView.svelte";
  import ListRow from "./ListRow.svelte";

  interface Props {
    items: SeenItem[];
    clock: string;
    /** The date as the story shows it. */
    date: string;
    speakers: Record<string, Speaker>;
    onreply: (content: string, label: string) => void;
    onseen: (content: string) => void;
    /** The event the story waits for, shown above the threads. */
    next?: Snippet;
  }
  const { items, clock, date, speakers, onreply, onseen, next }: Props = $props();

  const threads = $derived(phoneThreads(items));
  const others = $derived(withoutTemplate(items));
  const names = $derived(Object.fromEntries(Object.entries(speakers).map(([k, s]) => [k, s.name])));
  /** The thread open, by its sender; `undefined`: the app's list. */
  let open = $state<string | null | undefined>(undefined);
  const thread = $derived(open === undefined ? undefined : threads.find((x) => x.from === open));

  function nameOf(from: string | null): string {
    return from ? (names[from] ?? from) : t("story.play.unknownSender");
  }

  function lastLine(x: Thread): string {
    const last = x.steps[x.steps.length - 1];
    return last ? last.text : "";
  }

  // An event coming to wait goes back to the list it heads: from a thread,
  // nothing would say the story waits for it.
  let hadNext = false;
  $effect(() => {
    const has = next !== undefined;
    if (has && !hadNext) open = undefined;
    hadNext = has;
  });

  // A message arriving in the open thread is read as it comes.
  $effect(() => {
    const [shown, unseen] = [thread, thread?.unseen ?? []];
    if (shown) for (const content of unseen) onseen(content);
  });
</script>

{#if thread}
  <PhoneFrame
    {clock}
    {date}
    title={nameOf(thread.from)}
    avatar={thread.from ? (speakers[thread.from]?.portraits.neutral ?? null) : null}
  >
    <div class="thread">
      <button class="back" onclick={() => (open = undefined)}>‹ {t("story.play.messages")}</button>
      <ThreadView
        steps={thread.steps}
        choices={thread.choices}
        from={thread.from}
        {names}
        onchoose={(label) => thread.replyTo && onreply(thread.replyTo, label)}
      />
    </div>
  </PhoneFrame>
{:else}
  <PhoneFrame {clock} {date} title={t("story.play.messages")} avatar={null}>
    <ul class="list">
      {#if next}
        <li class="next">
          <div class="lbl-key">{t("story.play.nextEvent")}</div>
          {@render next()}
        </li>
      {/if}
      {#each threads as x (x.from ?? "")}
        <li>
          <ListRow
            name={nameOf(x.from)}
            avatar={x.from ? (speakers[x.from]?.portraits.neutral ?? null) : null}
            preview={lastLine(x)}
            time={x.clock}
            unread={x.unseen.length > 0 || x.choices.length > 0}
            onclick={() => (open = x.from)}
          />
        </li>
      {/each}
      {#each others as o, i (o.content + ":" + i)}
        <li class="other">{t("story.play.noTemplate", { kind: t(CONTENT_TYPE_LABEL[o.kind]), from: nameOf(o.from) })}</li>
      {/each}
      {#if threads.length === 0 && others.length === 0}
        <li class="none">{t("story.play.noMessage")}</li>
      {/if}
    </ul>
  </PhoneFrame>
{/if}

<style>
  .thread {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }
  .back {
    align-self: flex-start;
    background: transparent;
    color: var(--txt2);
    font-size: 11.5px;
    padding: 4px 8px;
  }
  .back:hover {
    color: var(--txt);
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    overflow-y: auto;
    height: 100%;
  }
  .next {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px 12px 12px;
    border-bottom: 1px solid var(--line);
  }
  .other,
  .none {
    padding: 9px 10px;
    font-size: 11.5px;
    color: var(--muted);
  }
</style>
