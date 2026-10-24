<script lang="ts">
  // The phone in a game (SPEC-story-mode, Content types, templates and
  // frames): a home screen with the frame's apps, each a template showing
  // some content types - a thread, an inbox, a feed, a statement. Which apps,
  // their names and icons, and when each appears are the frame's (`apps`,
  // from the engine). The phone opens on its first messaging app, which the
  // next event heads, and goes back to it when an event comes to wait. A feed
  // or a statement opened is read as a whole.
  import { untrack, type Snippet } from "svelte";
  import { i18n, t } from "$lib/i18n/index.svelte";
  import { appItems, appName, appsShown, homeApp, pendingIn } from "$lib/story/player/apps";
  import { initials, whenLabel, type DateDisplay } from "$lib/story/player/phone";
  import type { Post, Speaker, StatementLine } from "$lib/story/player/types";
  import type { AppShown, SeenItem } from "$lib/story/play";
  import { mediaUrl } from "$lib/story/player/speakers";
  import { playPhoneSound, providePhoneDress } from "$lib/story/player/phoneDress";
  import type { PhoneLook } from "$lib/story/play";
  import FeedView from "./player/FeedView.svelte";
  import PhoneFrame from "./player/PhoneFrame.svelte";
  import StatementView from "./player/StatementView.svelte";
  import BackButton from "./phone/BackButton.svelte";
  import MailApp from "./phone/MailApp.svelte";
  import MessagesApp from "./phone/MessagesApp.svelte";

  interface Props {
    /** The frame's apps, in the home screen's order. */
    apps: AppShown[];
    /** The story's folder, for the apps' icons; `null` for the sample. */
    folder: string | null;
    /** How the phone looks: its picture, status bar, wallpaper, sounds. */
    look: PhoneLook | null;
    /** The story's sound is off. */
    muted: boolean;
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
  const { apps, folder, look, muted, items, clock, date, dateDisplay, speakers, money, onreply, onseen, next }: Props =
    $props();

  /** The screen shown: the home screen, or an app, by id. */
  let screen = $state<string>(untrack(() => homeApp(apps)?.id ?? "home"));
  const shownApps = $derived(appsShown(apps));
  /** The app open; `null` on the home screen, or for an app the frame put
   * up since no longer has. */
  const open = $derived(apps.find((a) => a.id === screen) ?? null);
  /** Another app than the one shown calls for the player. */
  const elsewhere = $derived(shownApps.some((app) => app.id !== screen && pendingIn(items, app) > 0));

  providePhoneDress(() => ({ look, skin: open?.skin ?? null, folder, home: open === null }));

  /** Opens an app, with the phone's tap. */
  function openApp(id: string) {
    screen = id;
    playPhoneSound(folder, look?.sounds.tap, muted);
  }

  /** Sends a reply, with the phone's own sound. */
  function reply(content: string, label: string) {
    playPhoneSound(folder, look?.sounds.send, muted);
    onreply(content, label);
  }

  function when(item: SeenItem): string {
    return whenLabel(item.date, item.clock, dateDisplay, t, i18n.locale);
  }

  function nameOf(from: string | null): string {
    return from ? (speakers[from]?.name ?? from) : t("story.play.unknownSender");
  }

  const posts = $derived(
    open?.template === "feed"
      ? appItems(items, open).map(
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
    open?.template === "statement"
      ? appItems(items, open).map((i): StatementLine => ({ key: String(i), when: when(items[i]), steps: items[i].steps }))
      : [],
  );

  // An event coming to wait brings the messages back: they are what it heads.
  let hadNext = false;
  $effect(() => {
    const has = next !== undefined;
    if (has && !hadNext) screen = untrack(() => homeApp(apps)?.id ?? "home");
    hadNext = has;
  });

  // A feed or a statement is read as a whole once open, what arrives in it
  // included.
  $effect(() => {
    const [app, list] = [open, items];
    if (app?.template !== "feed" && app?.template !== "statement") return;
    for (const i of appItems(list, app)) if (!list[i].seen) onseen(list[i].content);
  });
</script>

{#if open?.template === "thread"}
  <MessagesApp
    title={appName(open, t)}
    {apps}
    {items}
    {clock}
    {date}
    {speakers}
    onreply={reply}
    {onseen}
    {next}
    {elsewhere}
    onhome={() => (screen = "home")}
  />
{:else if open?.template === "inbox"}
  <MailApp
    app={open}
    {items}
    {clock}
    {date}
    {dateDisplay}
    {speakers}
    {elsewhere}
    onreply={reply}
    {onseen}
    onhome={() => (screen = "home")}
  />
{:else if open === null}
  <PhoneFrame {clock} {date} title={t("story.play.apps")} avatar={null}>
    <div class="home">
      {#each shownApps as app (app.id)}
        {@const pending = pendingIn(items, app)}
        {@const name = appName(app, t)}
        {@const icon = mediaUrl(folder, app.icon)}
        <button class="app" onclick={() => openApp(app.id)}>
          {#if icon}
            <img class="icon" src={icon} alt="" />
          {:else}
            <span class="icon">{initials(name)}</span>
          {/if}
          <span class="name">{name}</span>
          {#if pending}<span class="badge" aria-label={t("story.play.unread")}>{pending}</span>{/if}
        </button>
      {/each}
    </div>
  </PhoneFrame>
{:else}
  <PhoneFrame {clock} {date} title={appName(open, t)} avatar={null}>
    <div class="screen">
      <BackButton label={t("story.play.apps")} dot={elsewhere} onclick={() => (screen = "home")} />
      {#if open.template === "statement"}
        <StatementView {lines} balance={money} />
      {:else}
        <FeedView
          {posts}
          {speakers}
          onchoose={(key, label) => reply(items[Number(key)].content, label)}
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
  img.icon {
    object-fit: cover;
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
