<script lang="ts">
  // What keeps a story file the screen edits from being saved: it does not
  // read (fixed by hand, then reopened), or its last save was refused.
  import { t } from "$lib/i18n/index.svelte";
  import { studio } from "$lib/story/studioState.svelte";

  interface Props {
    entry: { file: string; error: string | null };
  }
  const { entry }: Props = $props();
</script>

{#if entry.error}
  <p class="errbox">{t("story.world.unreadable", { file: entry.file, reason: entry.error })}</p>
{:else if studio.unsaved[entry.file]}
  <p class="errbox">{t("story.studio.notSaved", { reason: studio.unsaved[entry.file] })}</p>
{/if}

<style>
  .errbox {
    margin: 0 0 14px;
  }
</style>
