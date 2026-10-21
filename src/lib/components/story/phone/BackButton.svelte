<script lang="ts">
  // Back, at the top of a phone app's screen: to the app's list, or to the
  // home screen - with the unread dot when another app calls for the player.
  interface Props {
    label: string;
    /** Something elsewhere waits for the player. */
    dot?: boolean;
    onclick: () => void;
  }
  const { label, dot = false, onclick }: Props = $props();
</script>

<button class="back" {onclick}>‹ {label}{#if dot}<span class="dot"></span>{/if}</button>

<style>
  .back {
    align-self: flex-start;
    display: inline-flex;
    align-items: center;
    gap: 5px;
    background: transparent;
    color: var(--txt2);
    font-size: 11.5px;
    padding: 4px 8px;
  }
  .back:hover {
    color: var(--txt);
  }
  /* The phone's blue, like its unread threads. */
  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--blue);
  }
</style>
