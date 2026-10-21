<script lang="ts">
  // The phone in a game (SPEC-story-mode, Content types, templates and
  // frames): a home screen with its apps, each showing the content types it
  // has a template for - the messages (threads), the mail (inbox), the news
  // and the social network (feeds), the bank (statement). An app appears with
  // its first item, the messages always. The phone opens on the messages,
  // which the next event heads, and goes back to them when an event comes to
  // wait. A feed or a statement opened is read as a whole.
  import type { Snippet } from "svelte";
  import { i18n, t } from "$lib/i18n/index.svelte";
  import { APP_LABEL, appItems, appsShown, pendingIn, type PhoneApp } from "$lib/story/player/apps";
  import { initials, whenLabel, type DateDisplay } from "$lib/story/player/phone";
  import type { Post, Speaker, StatementLine } from "$lib/story/player/types";
  import type { SeenItem } from "$lib/story/play";
  import FeedView from "./player/FeedView.svelte";
  import PhoneFrame from "./player/PhoneFrame.svelte";
  import StatementView from "./player/StatementView.svelte";
  import BackButton from "./phone/BackButton.svelte";
  import MailApp from "./phone/MailApp.svelte";
  import MessagesApp from "./phone/MessagesApp.svelte";

  interface Props {
    items: SeenItem[];
    clock: string;
    /** The date as the story shows it. */
    date: string;
    dateDisplay: DateDisplay;
    speakers: Record<string, Speaker>;
    /** The money the player has, when the story has an economy. */
    money: number | null;
    onreply: (content: string, label: string) => void;
    onseen: (content: string) => void;
    /** The event the story waits for, heading the messages. */
    next?: Snippet;
  }
  const { items, clock, date, dateDisplay, speakers, money, onreply, onseen, next }: Props = $props();

  /** The screen shown: the home screen, or an app. */
  let screen = $state<PhoneApp | "home">("messages");
  const apps = $derived(appsShown(items));
  /** Another app than the one shown calls for the player. */
  const elsewhere = $derived(apps.some((app) => app !== screen && pendingIn(items, app) > 0));

  function when(item: SeenItem): string {
    return whenLabel(item.date, item.clock, dateDisplay, t, i18n.locale);
  }

  function nameOf(from: string | null): string {
    return from ? (speakers[from]?.name ?? from) : t("story.play.unknownSender");
  }

  const posts = $derived(
    screen === "news" || screen === "social"
      ? appItems(items, screen).map(
          (i): Post => ({
            key: String(i),
            from: items[i].from,
            author: nameOf(items[i].from),
            when: when(items[i]),
            steps: items[i].steps,
            choices: items[i].choices,
          }),
        )
      : [],
  );
  const lines = $derived(
    screen === "bank"
      ? appItems(items, "bank").map((i): StatementLine => ({ key: String(i), when: when(items[i]), steps: items[i].steps }))
      : [],
  );

  // An event coming to wait brings the messages back: they are what it heads.
  let hadNext = false;
  $effect(() => {
    const has = next !== undefined;
    if (has && !hadNext) screen = "messages";
    hadNext = has;
  });

  // A feed or a statement is read as a whole once open, what arrives in it
  // included.
  $effect(() => {
    const [shown, list] = [screen, items];
    if (shown !== "news" && shown !== "social" && shown !== "bank") return;
    for (const i of appItems(list, shown)) if (!list[i].seen) onseen(list[i].content);
  });
</script>

{#if screen === "messages"}
  <MessagesApp {items} {clock} {date} {speakers} {onreply} {onseen} {next} {elsewhere} onhome={() => (screen = "home")} />
{:else if screen === "mail"}
  <MailApp
    {items}
    {clock}
    {date}
    {dateDisplay}
    {speakers}
    {elsewhere}
    {onreply}
    {onseen}
    onhome={() => (screen = "home")}
  />
{:else if screen === "home"}
  <PhoneFrame {clock} {date} title={t("story.play.apps")} avatar={null}>
    <div class="home">
      {#each apps as app (app)}
        {@const pending = pendingIn(items, app)}
        <button class="app" onclick={() => (screen = app)}>
          <span class="icon">{initials(t(APP_LABEL[app]))}</span>
          <span class="name">{t(APP_LABEL[app])}</span>
          {#if pending}<span class="badge" aria-label={t("story.play.unread")}>{pending}</span>{/if}
        </button>
      {/each}
    </div>
  </PhoneFrame>
{:else}
  <PhoneFrame {clock} {date} title={t(APP_LABEL[screen])} avatar={null}>
    <div class="screen">
      <BackButton label={t("story.play.apps")} dot={elsewhere} onclick={() => (screen = "home")} />
      {#if screen === "bank"}
        <StatementView {lines} balance={money} />
      {:else}
        <FeedView
          {posts}
          {speakers}
          onchoose={(key, label) => onreply(items[Number(key)].content, label)}
        />
      {/if}
    </div>
  </PhoneFrame>
{/if}

<style>
  .home {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 14px 8px;
    padding: 18px 12px;
    align-content: start;
  }
  .app {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    background: transparent;
    padding: 4px;
  }
  .icon {
    width: 46px;
    height: 46px;
    border-radius: 12px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--mat);
    color: var(--txt);
    font-size: 14px;
    font-weight: 600;
  }
  .app:hover .icon {
    background: var(--mat-hi);
  }
  .name {
    color: var(--txt2);
    font-size: 11px;
  }
  /* The phone's blue, like its unread threads. */
  .badge {
    position: absolute;
    top: 0;
    right: 14px;
    min-width: 16px;
    height: 16px;
    padding: 0 4px;
    border-radius: 8px;
    background: var(--blue);
    color: var(--bg);
    font-size: 10px;
    font-weight: 600;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .screen {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }
</style>
