<script lang="ts">
  // The phone in a game (SPEC-story-mode, Content types, templates and
  // frames): the messaging app, one thread per sender, the latest first and
  // the unread ones marked; a thread opened shows its bubbles and the replies
  // its open message offers. Opening a thread marks its messages seen. What
  // the phone has no template for yet is listed by name (L3.3).
  import { t } from "$lib/i18n/index.svelte";
  import { CONTENT_TYPE_LABEL } from "$lib/story/labels";
  import { phoneThreads, withoutTemplate, type Thread } from "$lib/story/player/game";
  import { initials } from "$lib/story/player/phone";
  import type { Speaker } from "$lib/story/player/types";
  import type { SeenItem } from "$lib/story/play";
  import PhoneFrame from "./player/PhoneFrame.svelte";
  import ThreadView from "./player/ThreadView.svelte";

  interface Props {
    items: SeenItem[];
    clock: string;
    /** The date as the story shows it. */
    date: string;
    speakers: Record<string, Speaker>;
    onreply: (content: string, label: string) => void;
    onseen: (content: string) => void;
  }
  const { items, clock, date, speakers, onreply, onseen }: Props = $props();

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
      {#each threads as x (x.from ?? "")}
        {@const avatar = x.from ? speakers[x.from]?.portraits.neutral : null}
        <li>
          <button class="row" onclick={() => (open = x.from)}>
            {#if avatar}
              <img class="avatar" src={avatar} alt="" />
            {:else}
              <span class="avatar initials">{initials(nameOf(x.from))}</span>
            {/if}
            <span class="text">
              <span class="who">{nameOf(x.from)}</span>
              <span class="last">{lastLine(x)}</span>
            </span>
            <span class="meta">
              <span class="time">{x.clock}</span>
              {#if x.unseen.length || x.choices.length}<span class="dot" aria-label={t("story.play.unread")}></span>{/if}
            </span>
          </button>
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
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 9px 10px;
    background: transparent;
    border-bottom: 1px solid var(--line);
    text-align: left;
  }
  .row:hover {
    background: var(--raised);
  }
  .avatar {
    flex: none;
    width: 32px;
    height: 32px;
    border-radius: 50%;
    object-fit: cover;
  }
  .initials {
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--raised);
    color: var(--txt2);
    font-size: 11px;
  }
  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .who {
    font-size: 12.5px;
    color: var(--txt);
  }
  .last {
    font-size: 11.5px;
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .meta {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 4px;
  }
  .time {
    font-size: 10.5px;
    color: var(--muted);
  }
  /* The phone's blue, like the player's replies: the red has its scale
     (§7.2ter), and an unread thread is not on it. */
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--blue);
  }
  .other,
  .none {
    padding: 9px 10px;
    font-size: 11.5px;
    color: var(--muted);
  }
</style>
