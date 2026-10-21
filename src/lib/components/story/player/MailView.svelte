<script lang="ts">
  // One mail opened, the inbox template's reading view (SPEC-story-mode,
  // Content types, templates and frames): who sent it and when, its text,
  // and the replies it offers. Shared by the preview and Story Mode.
  import type { PlayerChoice, PlayerStep } from "$lib/story/player/types";
  import ReplyChoices from "./ReplyChoices.svelte";
  import TextBody from "./TextBody.svelte";

  interface Props {
    /** The sender's name. */
    sender: string;
    when: string;
    steps: PlayerStep[];
    choices: PlayerChoice[];
    from: string | null;
    names: Record<string, string>;
    onchoose: (label: string) => void;
  }
  const { sender, when, steps, choices, from, names, onchoose }: Props = $props();
</script>

<div class="mail">
  <div class="head">
    <span class="sender">{sender}</span>
    <span class="when">{when}</span>
  </div>
  <TextBody {steps} {from} {names} />
</div>
<ReplyChoices {choices} {onchoose} />

<style>
  .mail {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px;
  }
  .head {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding-bottom: 8px;
    border-bottom: 1px solid var(--line);
  }
  .sender {
    color: var(--txt);
    font-size: 12.5px;
    font-weight: 600;
  }
  .when {
    color: var(--muted);
    font-size: 10.5px;
  }
</style>
