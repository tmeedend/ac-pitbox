<script lang="ts">
  // The engine's verdict on the whole story, re-run after every save
  // (SPEC-story-mode, Create Your Own: validation runs continuously). A
  // diagnostic about a flow node selects it; the others name their file.
  import { t } from "$lib/i18n/index.svelte";
  import { selectNode, studio } from "$lib/story/studioState.svelte";

  const errors = $derived(studio.diagnostics.filter((d) => d.severity === "error").length);
  const warnings = $derived(studio.diagnostics.length - errors);
</script>

<section class="blk checks">
  <div class="blk-h">
    <span class="blk-t">{t("story.studio.checks")}</span>
    <span class="blk-n">
      {#if studio.diagnostics.length === 0}
        {t("story.studio.allClear")}
      {:else}
        {t("story.studio.errorCount", { count: errors })} · {t("story.studio.warningCount", { count: warnings })}
      {/if}
    </span>
  </div>
  {#if studio.diagnostics.length}
    <ul>
      {#each studio.diagnostics as d, i (i)}
        <li>
          <span class="pill" class:pill-err={d.severity === "error"} class:pill-warn={d.severity === "warning"}>
            {d.severity === "error" ? t("story.studio.error") : t("story.studio.warning")}
          </span>
          {#if d.node}
            <button class="where mono" onclick={() => selectNode(d.node!)}>{d.node}</button>
          {:else}
            <span class="where mono file">{d.file}</span>
          {/if}
          <span class="msg">{d.message}</span>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .checks {
    margin-top: 26px;
    max-width: 720px;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 8px 16px 12px;
  }
  li {
    display: flex;
    align-items: baseline;
    gap: 10px;
    padding: 5px 0;
    font-size: 12px;
  }
  .pill {
    flex-shrink: 0;
    font-size: 10px;
  }
  .where {
    flex-shrink: 0;
    background: transparent;
    color: var(--txt2);
    font-size: 11.5px;
    padding: 0;
  }
  button.where:hover {
    color: var(--txt);
    text-decoration: underline;
  }
  .file {
    color: var(--muted);
  }
  .msg {
    color: var(--txt2);
    min-width: 0;
  }
</style>
