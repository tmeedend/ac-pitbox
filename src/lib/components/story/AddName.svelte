<script lang="ts">
  // The field under a list of names that adds one: typed, then Enter or the
  // button. `onadd` returns why the text was refused, or `null` once added.
  import { t } from "$lib/i18n/index.svelte";

  interface Props {
    placeholder: string;
    onadd: (text: string) => string | null;
  }
  const { placeholder, onadd }: Props = $props();

  let text = $state("");
  let problem = $state<string | null>(null);

  function add() {
    if (!text.trim()) return;
    problem = onadd(text.trim());
    if (!problem) text = "";
  }
</script>

<div class="add">
  <input
    class="input"
    bind:value={text}
    {placeholder}
    aria-label={placeholder}
    onkeydown={(e) => e.key === "Enter" && add()}
  />
  <button class="btn" disabled={!text.trim()} onclick={add}>+ {t("story.world.add")}</button>
</div>
{#if problem}<p class="errbox">{problem}</p>{/if}

<style>
  .add {
    display: flex;
    gap: 8px;
    margin-top: 10px;
  }
  .add .input {
    width: 240px;
  }
  .errbox {
    margin: 6px 0 0;
  }
</style>
