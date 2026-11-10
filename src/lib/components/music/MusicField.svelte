<script lang="ts">
  // The MUSIC line of a term unfolded in Classement (§16.8): a brand, a car
  // family, a track category, a country. Written at once, like everything
  // else on these tabs. Absent while "Use my own music folders" is off
  // (§16.4): the assignments are kept, nothing offers to make new ones.
  import { onDestroy } from "svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { errorText } from "$lib/errors";
  import { assignMusic, loadMusicConfig, musicConfig, ownMusicOn } from "$lib/music/context.svelte";
  import { stopPreview } from "$lib/music/preview.svelte";
  import type { Taxonomy } from "$lib/music/resolve";
  import MusicSourcePicker from "./MusicSourcePicker.svelte";

  let { taxonomy, id }: { taxonomy: Taxonomy; id: string } = $props();

  void loadMusicConfig();
  onDestroy(() => void stopPreview());

  const value = $derived(musicConfig()?.assignments[taxonomy][id] ?? null);
  let error = $state("");

  function pick(source: string | null) {
    error = "";
    assignMusic(taxonomy, id, source).catch((e) => (error = errorText(e)));
  }
</script>

{#if ownMusicOn()}
  <div class="field">
    <span class="lbl-key">{t("music.label")}</span>
    <MusicSourcePicker {value} defaultLabel={t("music.followsScreen")} label={t("music.label")} onchange={pick} />
    {#if error}<div class="errbox">{error}</div>{/if}
  </div>
{/if}

<style>
  /* The `.field` of the tabs it sits in, which is scoped to them. */
  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
</style>
