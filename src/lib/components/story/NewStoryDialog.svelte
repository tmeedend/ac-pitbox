<script lang="ts">
  // "New story": a title, the starting frame and an empty folder. Pit Box
  // writes the smallest story that validates clean there
  // (`story_engine::write::new_story`), in the app's language as the story's
  // own - the author changes it with the story settings. The frame starts
  // unchosen: the platform never picks one for the author.
  import { open } from "@tauri-apps/plugin-dialog";
  import { i18n, t } from "$lib/i18n/index.svelte";
  import { errorText } from "$lib/errors";
  import { createStory, type Frame } from "$lib/story/studio";
  import { FRAMES, frameLabel } from "$lib/story/frames";
  import Modal from "$lib/components/ui/Modal.svelte";
  import Field from "$lib/components/ui/Field.svelte";
  import Seg from "$lib/components/ui/Seg.svelte";

  interface Props {
    oncreated: (path: string) => void;
    oncancel: () => void;
  }
  const { oncreated, oncancel }: Props = $props();

  let title = $state("");
  let folder = $state<string | null>(null);
  let frame = $state<Frame | null>(null);
  const ready = $derived(!!folder && !!title.trim() && !!frame);
  let error = $state<string | null>(null);
  let busy = $state(false);

  async function pick() {
    const picked = await open({ directory: true, title: t("story.studio.pickEmptyFolder") });
    if (typeof picked === "string") folder = picked;
  }

  async function create() {
    if (!folder || !title.trim() || !frame) return;
    busy = true;
    error = null;
    try {
      await createStory(folder, title.trim(), i18n.locale, frame, t("story.studio.chapterTitle", { n: 1 }));
      oncreated(folder);
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = false;
    }
  }
</script>

<Modal title={t("story.studio.newStory")} onclose={oncancel}>
  <Field label={t("story.studio.storyTitle")}>
    <input class="input" bind:value={title} />
  </Field>
  <Field label={t("story.settings.startFrame")}>
    <Seg
      items={FRAMES.map((f) => ({ value: f, label: frameLabel(f) }))}
      value={frame ?? ""}
      onselect={(v) => (frame = v)}
    />
  </Field>
  <Field label={t("story.studio.folder")}>
    <div class="folder">
      <span class="mono path">{folder ?? "—"}</span>
      <button class="btn" onclick={pick}>{t("story.studio.choose")}</button>
    </div>
  </Field>
  {#if error}<p class="errbox">{error}</p>{/if}
  {#snippet footer()}
    <button class="btn btn-ghost" onclick={oncancel}>{t("common.cancel")}</button>
    <button class="btn" disabled={busy || !ready} onclick={create}>
      {t("story.studio.create")}
    </button>
  {/snippet}
</Modal>

<style>
  .folder {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .path {
    flex: 1;
    min-width: 0;
    color: var(--muted);
    font-size: 11.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .errbox {
    margin: 12px 0 0;
  }
</style>
