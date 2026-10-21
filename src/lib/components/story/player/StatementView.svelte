<script lang="ts">
  // The statement template (SPEC-story-mode, Content types, templates and
  // frames): the account's lines, the latest first, under the balance when
  // the story has money. A line is the item's text: the amounts are the
  // author's words, the money itself moves by the story's effects. Shared by
  // the preview and Story Mode.
  import { t } from "$lib/i18n/index.svelte";
  import type { PlayerStep, StatementLine } from "$lib/story/player/types";

  interface Props {
    lines: StatementLine[];
    /** The money the player has; `null` when the story has no economy. */
    balance: number | null;
  }
  const { lines, balance }: Props = $props();

  function text(steps: PlayerStep[]): string {
    return steps.map((s) => s.text).join(" ");
  }
</script>

<div class="statement">
  {#if balance !== null}
    <div class="balance">
      <span class="lbl-key">{t("story.play.balance")}</span>
      <span class="amount">{balance}</span>
    </div>
  {/if}
  <ul>
    {#each lines as l (l.key)}
      <li>
        <span class="text">{text(l.steps)}</span>
        <span class="when">{l.when}</span>
      </li>
    {/each}
  </ul>
</div>

<style>
  .statement {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }
  .balance {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 14px 12px;
    border-bottom: 1px solid var(--line);
  }
  .amount {
    color: var(--txt);
    font-size: 20px;
    font-weight: 600;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  li {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 9px 12px;
    border-bottom: 1px solid var(--line);
  }
  .text {
    color: var(--txt);
    font-size: 12.5px;
    overflow-wrap: anywhere;
  }
  .when {
    color: var(--muted);
    font-size: 10.5px;
  }
</style>
