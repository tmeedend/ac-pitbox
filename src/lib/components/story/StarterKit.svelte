<script lang="ts">
  // The starter kit, offered where the Frames screen's images are made
  // (SPEC-story-mode, Skins: how a frame gets its look): what it is and what
  // saving it does are said before the folder is asked for, and once saved,
  // where the files went, with the way to open that folder.
  import { open } from "@tauri-apps/plugin-dialog";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import { errorText } from "$lib/errors";
  import { t } from "$lib/i18n/index.svelte";
  import { saveKit } from "$lib/story/kit";
  import { studio } from "$lib/story/studioState.svelte";

  let saving = $state(false);
  /** Where the kit went, once saved. */
  let saved = $state<{ dir: string; count: number } | null>(null);
  let problem = $state<string | null>(null);

  async function save() {
    const dir = await open({ directory: true, title: t("story.frames.kitPick") });
    if (!dir || typeof dir !== "string") return;
    saving = true;
    problem = null;
    try {
      saved = { dir, count: await saveKit(dir) };
    } catch (e) {
      problem = errorText(e);
    } finally {
      saving = false;
    }
  }

  async function show() {
    if (!saved) return;
    try {
      await revealItemInDir(saved.dir);
    } catch (e) {
      problem = errorText(e);
    }
  }
</script>

<section class="blk kit">
  <div class="blk-h"><span class="blk-t">{t("story.frames.kitTitle")}</span></div>
  <div class="blk-b">
    <p>{t("story.frames.kitWhat")}</p>
    <p class="muted">{t("story.frames.kitHow", { count: studio.story?.templates.length ?? 0 })}</p>
    <div class="actions">
      <button class="btn" disabled={saving} onclick={save}>{t("story.frames.kitSave")}</button>
      {#if saved}
        <span class="done">{t("story.frames.kitSaved", { count: saved.count, dir: saved.dir })}</span>
        <button class="btn btn-ghost" onclick={show}>{t("story.frames.kitShow")}</button>
      {/if}
    </div>
    {#if problem}<p class="errbox">{problem}</p>{/if}
  </div>
</section>

<style>
  .kit {
    margin-bottom: 18px;
  }
  p {
    margin: 0 0 6px;
    font-size: 13px;
    color: var(--txt2);
    max-width: 760px;
  }
  .muted {
    color: var(--muted);
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
    margin-top: 8px;
  }
  .done {
    font-size: 12px;
    color: var(--txt2);
  }
  .errbox {
    margin: 8px 0 0;
  }
</style>
