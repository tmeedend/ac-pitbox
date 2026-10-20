<script lang="ts">
  // The phone Pit Box ships (SPEC-story-mode, Content types, templates and
  // frames): a device with its status bar and the header of the app shown,
  // around one template (`children`). Shared by the Content screen's preview
  // and Story Mode: it knows nothing of the editor. Its own theme - colours
  // of the design system, no story art - is the default every phone story
  // gets until it brings its own skin.
  import type { Snippet } from "svelte";
  import { initials } from "$lib/story/player/phone";

  interface Props {
    /** `HH:MM`. */
    clock: string;
    /** The date as the story shows it; empty for none. */
    date: string;
    /** Who the app speaks with: a name. */
    title: string;
    /** Their picture, if they have one. */
    avatar: string | null;
    children: Snippet;
  }
  const { clock, date, title, avatar, children }: Props = $props();
</script>

<div class="phone">
  <div class="status">
    <span class="clock">{clock}</span>
    {#if date}<span class="date">{date}</span>{/if}
  </div>
  <div class="head">
    {#if avatar}
      <img class="avatar" src={avatar} alt="" />
    {:else}
      <span class="avatar initials">{initials(title)}</span>
    {/if}
    <span class="title">{title}</span>
  </div>
  <div class="screen">
    {@render children()}
  </div>
</div>

<style>
  .phone {
    width: 300px;
    height: 600px;
    display: flex;
    flex-direction: column;
    border-radius: 34px;
    border: 8px solid var(--mat);
    background: var(--panel2);
    overflow: hidden;
    box-shadow: 0 0 0 1px var(--mat-line);
  }
  .status {
    display: flex;
    justify-content: space-between;
    padding: 8px 18px 4px;
    font-size: 11px;
    color: var(--txt2);
  }
  .clock {
    font-weight: 600;
  }
  .date {
    color: var(--muted);
  }
  .head {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 14px;
    border-bottom: 1px solid var(--line);
  }
  .avatar {
    width: 30px;
    height: 30px;
    border-radius: 50%;
    object-fit: cover;
    flex-shrink: 0;
  }
  .initials {
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--mat-hi);
    color: var(--txt2);
    font-size: 11px;
    font-weight: 600;
  }
  .title {
    color: var(--txt);
    font-size: 13px;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .screen {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
</style>
