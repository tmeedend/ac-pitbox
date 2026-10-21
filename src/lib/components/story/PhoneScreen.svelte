<script lang="ts">
  // The phone in a game (SPEC-story-mode, Content types, templates and
  // frames): its apps, each showing the content types it has a template for.
  import type { Snippet } from "svelte";
  import type { Speaker } from "$lib/story/player/types";
  import type { SeenItem } from "$lib/story/play";
  import MessagesApp from "./phone/MessagesApp.svelte";

  interface Props {
    items: SeenItem[];
    clock: string;
    /** The date as the story shows it. */
    date: string;
    speakers: Record<string, Speaker>;
    onreply: (content: string, label: string) => void;
    onseen: (content: string) => void;
    /** The event the story waits for, heading the messages. */
    next?: Snippet;
  }
  const { items, clock, date, speakers, onreply, onseen, next }: Props = $props();
</script>

<MessagesApp {items} {clock} {date} {speakers} {onreply} {onseen} {next} />
