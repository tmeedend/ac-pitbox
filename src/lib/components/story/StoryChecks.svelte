<script lang="ts">
  // The engine's verdict on the story, re-run after every save
  // (SPEC-story-mode, Create Your Own: validation runs continuously). A
  // diagnostic about a flow node selects it; the others name their file.
  // Each section lists the ones its own files have, or those of one file
  // when a screen shows a single one (an event's sheet), or a verdict of
  // their own (the Package section's register, which only blocks the build).
  import { t } from "$lib/i18n/index.svelte";
  import type { Diagnostic } from "$lib/story/studio";
  import { sectionOf, selectNode, studio, type StudioSection } from "$lib/story/studioState.svelte";

  interface Props {
    section: StudioSection;
    /** Only this file's (a diagnostic's file may end with `:<line>`). */
    file?: string;
    /** These instead of the story's checks. */
    list?: Diagnostic[];
  }
  const { section, file, list }: Props = $props();

  const diagnostics = $derived(
    list ??
      studio.diagnostics.filter((d) =>
        file ? d.file === file || d.file.startsWith(`${file}:`) : sectionOf(d.file) === section,
      ),
  );
  const errors = $derived(diagnostics.filter((d) => d.severity === "error").length);
  const warnings = $derived(diagnostics.length - errors);
</script>

<section class="blk checks">
  <div class="blk-h">
    <span class="blk-t">{t("story.studio.checks")}</span>
    <span class="blk-n">
      {#if diagnostics.length === 0}
        {t("story.studio.allClear")}
      {:else}
        {t("story.studio.errorCount", { count: errors })} · {t("story.studio.warningCount", { count: warnings })}
      {/if}
    </span>
  </div>
  {#if diagnostics.length}
    <ul>
      {#each diagnostics as d, i (i)}
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
