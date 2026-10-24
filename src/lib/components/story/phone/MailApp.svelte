<script lang="ts">
  // The phone's mail app in a game (SPEC-story-mode, Content types,
  // templates and frames): the inbox, the latest first, the unread ones
  // marked; a mail opened shows its text and the replies it offers, and is
  // marked seen.
  import { i18n, t } from "$lib/i18n/index.svelte";
  import { appItems, appName } from "$lib/story/player/apps";
  import { whenLabel, type DateDisplay } from "$lib/story/player/phone";
  import type { Speaker } from "$lib/story/player/types";
  import type { AppShown, SeenItem } from "$lib/story/play";
  import MailView from "$lib/components/story/player/MailView.svelte";
  import PhoneFrame from "$lib/components/story/player/PhoneFrame.svelte";
  import BackButton from "./BackButton.svelte";
  import ListRow from "./ListRow.svelte";

  interface Props {
    /** The app shown: an inbox. */
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

  const inbox = $derived(appItems(items, app));
  const names = $derived(Object.fromEntries(Object.entries(speakers).map(([k, s]) => [k, s.name])));
  /** The mail open, as an index into `items`. */
  let open = $state<number | null>(null);
  const mail = $derived(open === null ? null : items[open]);

  function nameOf(from: string | null): string {
    return from ? (names[from] ?? from) : t("story.play.unknownSender");
  }

  function when(item: SeenItem): string {
    return whenLabel(item.date, item.clock, dateDisplay, t, i18n.locale);
  }

  function preview(item: SeenItem): string {
    return item.steps.find((s) => s.kind === "said")?.text ?? "";
  }

  // A mail opened is read.
  $effect(() => {
    const shown = mail;
    if (shown && !shown.seen) onseen(shown.content);
  });
</script>

{#if mail}
  <PhoneFrame {clock} {date} title={appName(app, t)} avatar={null}>
    <div class="screen">
      <BackButton label={appName(app, t)} onclick={() => (open = null)} />
      <MailView
        sender={nameOf(mail.from)}
        when={when(mail)}
        steps={mail.steps}
        choices={mail.choices}
        from={mail.from}
        {names}
        onchoose={(label) => onreply(mail.content, label)}
      />
    </div>
  </PhoneFrame>
{:else}
  <PhoneFrame {clock} {date} title={appName(app, t)} avatar={null}>
    <div class="screen">
      <BackButton label={t("story.play.apps")} dot={elsewhere} onclick={onhome} />
      <ul class="list">
        {#each inbox as index (index)}
          {@const item = items[index]}
          <li>
            <ListRow
              name={nameOf(item.from)}
              avatar={item.from ? (speakers[item.from]?.portraits.neutral ?? null) : null}
              preview={preview(item)}
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
</style>
