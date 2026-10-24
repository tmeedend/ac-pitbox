<script lang="ts">
  // A file of the story for one place of a frame or a skin (SPEC-story-mode,
  // Skins: how a frame gets its look): picked among those already in its
  // folder, or brought in from anywhere by "Import…", which copies it into
  // the story (`importIntoStory`) and picks it. Never moved, never renamed
  // in place: a file of the same name gets a number. A file named in the
  // story but not on disk stays shown as written, and says so.
  //
  // Not dropped onto the screen: a file dropped on the window is a mod to
  // install, everywhere in Pit Box (`importState`).
  import { open } from "@tauri-apps/plugin-dialog";
  import { errorText } from "$lib/errors";
  import { t } from "$lib/i18n/index.svelte";
  import { importIntoStory } from "$lib/story/studioState.svelte";

  interface Props {
    /** The path as the story writes it; empty for none. */
    value: string;
    /** What can be picked, as the story writes them. */
    options: string[];
    /** Where an imported file goes: a folder of `media/`, or a skin's. */
    into: string;
    /** The kinds of file it takes, by extension. */
    kinds: string[];
    /** The empty choice's label. */
    emptyLabel: string;
    onpick: (path: string) => void;
  }
  const { value, options, into, kinds, emptyLabel, onpick }: Props = $props();

  const all = $derived(value && !options.includes(value) ? [value, ...options] : options);
  const fits = $derived(options.filter((o) => kinds.some((k) => o.toLowerCase().endsWith(`.${k}`))));
  let problem = $state<string | null>(null);
  let busy = $state(false);

  async function bring() {
    const picked = await open({ multiple: false, filters: [{ name: kinds.join(", "), extensions: kinds }] });
    if (!picked || typeof picked !== "string") return;
    busy = true;
    try {
      onpick(await importIntoStory(picked, into));
      problem = null;
    } catch (e) {
      problem = errorText(e);
    } finally {
      busy = false;
    }
  }
</script>

<div class="pick">
  <select class="input mono" {value} onchange={(e) => onpick((e.currentTarget as HTMLSelectElement).value)}>
    <option value="">{emptyLabel}</option>
    {#each all.filter((o) => o === value || fits.includes(o)) as o (o)}<option value={o}>{o}</option>{/each}
  </select>
  <button class="btn btn-ghost" disabled={busy} onclick={bring}>{t("story.frames.import")}</button>
  {#if value && !options.includes(value)}<span class="missing">{t("story.frames.fileMissing")}</span>{/if}
</div>
{#if problem}<p class="errbox">{problem}</p>{/if}

<style>
  .pick {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  select {
    flex: 1;
    min-width: 0;
  }
  .missing {
    color: var(--yellow);
    font-size: 12px;
  }
  .errbox {
    margin: 6px 0 0;
  }
</style>
