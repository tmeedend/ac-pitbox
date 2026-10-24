<script lang="ts">
  // The phone's call app in a game (SPEC-story-mode, Content types, templates
  // and frames): the calls received, the latest first, the missed ones
  // marked; a call opened is the call screen - who calls, then the
  // conversation as it is said, with the replies it offers - and is marked
  // seen. A call is written like a dialogue, in Ink with labelled replies.
  import { i18n, t } from "$lib/i18n/index.svelte";
  import { appItems, appName } from "$lib/story/player/apps";
  import { whenLabel, type DateDisplay } from "$lib/story/player/phone";
  import type { Speaker } from "$lib/story/player/types";
  import type { AppShown, SeenItem } from "$lib/story/play";
  import PhoneFrame from "$lib/components/story/player/PhoneFrame.svelte";
  import ThreadView from "$lib/components/story/player/ThreadView.svelte";
  import BackButton from "./BackButton.svelte";
  import ListRow from "./ListRow.svelte";

  interface Props {
    /** The app shown: a call screen. */
    app: AppShown;
    items: SeenItem[];
    clock: string;
    date: string;
    dateDisplay: DateDisplay;
    speakers: Record<string, Speaker>;
    /** Another app calls for the player. */
    elsewhere: boolean;
    onreply: (content: string, label: string) => void;
    onseen: (content: string) => void;
    onhome: () => void;
  }
  const { app, items, clock, date, dateDisplay, speakers, elsewhere, onreply, onseen, onhome }: Props = $props();

  const calls = $derived(appItems(items, app));
  const names = $derived(Object.fromEntries(Object.entries(speakers).map(([k, s]) => [k, s.name])));
  /** The call open, as an index into `items`. */
  let open = $state<number | null>(null);
  const call = $derived(open === null ? null : items[open]);

  function nameOf(from: string | null): string {
    return from ? (names[from] ?? from) : t("story.play.unknownSender");
  }

  function avatarOf(from: string | null): string | null {
    return from ? (speakers[from]?.portraits.neutral ?? null) : null;
  }

  // A call opened is answered.
  $effect(() => {
    const shown = call;
    if (shown && !shown.seen) onseen(shown.content);
  });
</script>

{#if call}
  <PhoneFrame {clock} {date} title={nameOf(call.from)} avatar={avatarOf(call.from)}>
    <div class="screen">
      <BackButton label={appName(app, t)} onclick={() => (open = null)} />
      <div class="on-call">{t("story.play.onCall")}</div>
      <ThreadView
        steps={call.steps}
        choices={call.choices}
        from={call.from}
        {names}
        onchoose={(label) => onreply(call.content, label)}
      />
    </div>
  </PhoneFrame>
{:else}
  <PhoneFrame {clock} {date} title={appName(app, t)} avatar={null}>
    <div class="screen">
      <BackButton label={t("story.play.apps")} dot={elsewhere} onclick={onhome} />
      <ul class="list">
        {#each calls as index (index)}
          {@const item = items[index]}
          <li>
            <ListRow
              name={nameOf(item.from)}
              avatar={avatarOf(item.from)}
              preview={whenLabel(item.date, item.clock, dateDisplay, t, i18n.locale)}
              time={item.clock}
              unread={!item.seen || item.open}
              onclick={() => (open = index)}
            />
          </li>
        {/each}
      </ul>
    </div>
  </PhoneFrame>
{/if}

<style>
  .screen {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    overflow-y: auto;
    flex: 1;
    min-height: 0;
  }
  /* The call under way: a quiet line in the phone's accent. */
  .on-call {
    padding: 6px 12px;
    color: var(--blue);
    font-size: 11px;
    text-align: center;
  }
</style>
