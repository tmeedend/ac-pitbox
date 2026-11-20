<script lang="ts">
  // Regenerated thumbnails offered to someone who came from a version without
  // them (GRILLE§5.5). A new install chooses in the setup wizard; an existing
  // one never sees that page, and without this the feature would wait,
  // unnoticed, behind a settings tab.
  //
  // Asked once: either answer is written and the offer is gone for good.
  // "Later" only hides it until the next launch. Opening the settings answers
  // nothing — the tab's own switch does, when saved.
  import Toast from "./Toast.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { errorText } from "$lib/errors";
  import { nav } from "$lib/shell/nav.svelte";
  import { answerGridThumbs, gridThumbsUnanswered } from "$lib/gridthumbs/gridThumbPrefs.svelte";

  let later = $state(false);
  let busy = $state(false);
  let error = $state("");

  async function answer(enabled: boolean) {
    busy = true;
    error = "";
    try {
      await answerGridThumbs(enabled);
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = false;
    }
  }

  function openSettings() {
    nav.settingsTab = "thumbs";
    nav.section = "settings";
  }
</script>

{#if gridThumbsUnanswered() && !later}
  <Toast
    title={t("gridThumbs.offerTitle")}
    tone="info"
    onclose={() => (later = true)}
    closeLabel={t("gridThumbs.staleLater")}
  >
    <div class="body">{t("gridThumbs.offerBody")}</div>
    <div class="acts">
      <button class="btn btn-primary" type="button" disabled={busy} onclick={() => answer(true)}
        >{t("gridThumbs.offerYes")}</button
      >
      <button class="btn" type="button" disabled={busy} onclick={() => answer(false)}>{t("gridThumbs.offerNo")}</button>
      <button class="btn" type="button" onclick={openSettings}>{t("gridThumbs.offerSettings")}</button>
    </div>
    {#if error}<div class="errbox">{error}</div>{/if}
  </Toast>
{/if}

<style>
  .body {
    font-size: 11px;
    color: var(--txt2);
  }
  .acts {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 8px;
  }
</style>
