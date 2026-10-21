<script lang="ts">
  // One row of a phone app's list - a thread, a mail: who, their picture or
  // initials, the last line, when, and the unread dot.
  import { t } from "$lib/i18n/index.svelte";
  import { initials } from "$lib/story/player/phone";

  interface Props {
    name: string;
    /** Their picture, if they have one. */
    avatar: string | null;
    preview: string;
    time: string;
    unread: boolean;
    onclick: () => void;
  }
  const { name, avatar, preview, time, unread, onclick }: Props = $props();
</script>

<button class="row" {onclick}>
  {#if avatar}
    <img class="avatar" src={avatar} alt="" />
  {:else}
    <span class="avatar initials">{initials(name)}</span>
  {/if}
  <span class="text">
    <span class="who">{name}</span>
    <span class="last">{preview}</span>
  </span>
  <span class="meta">
    <span class="time">{time}</span>
    {#if unread}<span class="dot" aria-label={t("story.play.unread")}></span>{/if}
  </span>
</button>

<style>
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
     (§7.2ter), and an unread item is not on it. */
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--blue);
  }
</style>
