<script lang="ts">
  // What a visual novel always offers while a dialogue plays
  // (SPEC-story-mode, In-person dialogues): the log, fast-forward through
  // what was read, and auto-advance at the speed the player picks - kept in
  // `ui_prefs.json`, a preference of the player, not of a story.
  import { t } from "$lib/i18n/index.svelte";
  import { AUTO_SPEEDS, type AutoSpeed } from "$lib/story/player/stage";
  import { autoSpeed, setAutoSpeed } from "$lib/story/playPrefs";
  import Seg from "$lib/components/ui/Seg.svelte";

  interface Props {
    skip: boolean;
    auto: boolean;
    onlog: () => void;
  }
  let { skip = $bindable(), auto = $bindable(), onlog }: Props = $props();

  const LABEL: Record<AutoSpeed, string> = {
    slow: "story.player.speedSlow",
    normal: "story.player.speedNormal",
    fast: "story.player.speedFast",
  };
</script>

<!-- A press never takes the focus from the stage: Space and Enter go on
     reading there, they would press the button again here. -->
<div class="controls" role="toolbar" tabindex="-1" onmousedown={(e) => e.preventDefault()}>
  <button class="btn btn-ghost" onclick={onlog}>{t("story.player.log")}</button>
  <button class="btn btn-ghost" class:on={skip} aria-pressed={skip} onclick={() => (skip = !skip)}
    >{t("story.player.skip")}</button
  >
  <button class="btn btn-ghost" class:on={auto} aria-pressed={auto} onclick={() => (auto = !auto)}
    >{t("story.player.auto")}</button
  >
  <Seg
    items={AUTO_SPEEDS.map((s) => ({ value: s, label: t(LABEL[s]) }))}
    value={autoSpeed()}
    onselect={(v) => setAutoSpeed(v as AutoSpeed)}
    tone="neutral"
    size="mini"
  />
</div>

<style>
  .controls {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .on {
    background: var(--raised);
    color: var(--txt);
  }
</style>
