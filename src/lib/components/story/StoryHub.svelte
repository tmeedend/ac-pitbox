<script lang="ts">
  // The Story hub: the rail entry's two doors, Story Mode and Create Your Own
  // (`docs/story/SPEC-story-mode.md`, « Story Mode: player side »). Empty for
  // now — nothing plays or opens yet, so neither door offers an action it
  // could not carry out.
  //
  // Loaded lazily by `AppShell` and shown only while `storyUnlocked()` holds.
  import { t } from "$lib/i18n/index.svelte";
</script>

<div class="hub">
  <header>
    <div class="lbl-screen">{t("story.title")}</div>
  </header>

  <section class="door">
    <div class="lbl">{t("story.modeTitle")}</div>
    <p class="empty">{t("story.noStory")}</p>
  </section>

  <section class="door">
    <div class="lbl">{t("story.createTitle")}</div>
    <p class="empty">{t("story.noOwnStory")}</p>
  </section>
</div>

<style>
  .hub {
    display: flex;
    flex-direction: column;
    gap: 16px;
    max-width: 880px;
  }
  .door {
    background: var(--card);
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 16px;
  }
  .empty {
    color: var(--muted);
    font-size: 13px;
    margin: 10px 0 0;
  }
</style>
