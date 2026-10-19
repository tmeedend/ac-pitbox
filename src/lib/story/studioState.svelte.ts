// The story open in Create Your Own, and how an edit reaches its file.
//
// The screen edits a local copy of each file (chapters, `story.yaml`, the
// world files); every edit queues a save of the whole file (SPEC-story-mode,
// Create Your Own: each change is written at once). Saves run one after the
// other, each with the stamp the previous one returned: two saves in flight
// with the same stamp would refuse each other. A save the engine refuses (a
// sentence that does not parse) leaves the edit on screen, marked as not
// saved, and the next save carries it. A rename joins the same queue, so it
// runs on files every earlier edit has reached.
import { errorText } from "$lib/errors";
import { t } from "$lib/i18n/index.svelte";
import {
  checkStory,
  openStory as fetchStory,
  renameName,
  saveFile,
  type Chapter,
  type Diagnostic,
  type NameKind,
  type OpenStory,
} from "./studio";

/** The studio's sections (SPEC-story-mode, Create Your Own). */
export type StudioSection = "flow" | "world" | "settings";

/** The section whose files hold `file` (a diagnostic's, say): `story.yaml`
 * is the Settings', `world/` the World's, everything else the flow's -
 * until events and content get their own sections. */
export function sectionOf(file: string): StudioSection {
  if (file === "story.yaml") return "settings";
  return file.startsWith("world/") ? "world" : "flow";
}

/** The story-wide files, by their key in `OpenStory`. */
export type StoryFileKey = "manifest" | "characters" | "variables";

export const studio = $state<{
  story: OpenStory | null;
  section: StudioSection;
  /** Index in `story.chapters`. */
  chapter: number;
  /** The selected node's id, in the shown chapter. */
  node: string | null;
  diagnostics: Diagnostic[];
  /** Why the last save of a chapter file failed, by file. */
  unsaved: Record<string, string>;
  /** Shown above the flow: the story was reloaded under the author's feet. */
  notice: string | null;
  loading: boolean;
}>({
  story: null,
  section: "flow",
  chapter: 0,
  node: null,
  diagnostics: [],
  unsaved: {},
  notice: null,
  loading: false,
});

/** Opens (or reopens) a story. Keeps the selection when it still exists. */
export async function openStudio(path: string): Promise<void> {
  studio.loading = true;
  try {
    const story = await fetchStory(path);
    const sameStory = studio.story?.path === path;
    studio.story = story;
    studio.diagnostics = story.diagnostics;
    studio.unsaved = {};
    if (!sameStory) {
      studio.notice = null;
      studio.section = "flow";
      studio.chapter = 0;
      studio.node = null;
    }
    studio.chapter = Math.min(studio.chapter, Math.max(0, story.chapters.length - 1));
    const nodes = story.chapters[studio.chapter]?.chapter?.nodes ?? [];
    if (!nodes.some((n) => n.id === studio.node)) studio.node = null;
  } finally {
    studio.loading = false;
  }
}

export function closeStudio(): void {
  studio.story = null;
  studio.node = null;
  studio.notice = null;
}

/** Shows the chapter holding node `id`, in the flow, and selects it. */
export function selectNode(id: string): void {
  const index = studio.story?.chapters.findIndex((c) => c.chapter?.nodes.some((n) => n.id === id)) ?? -1;
  if (index < 0) return;
  studio.section = "flow";
  studio.chapter = index;
  studio.node = id;
}

let queue: Promise<void> = Promise.resolve();
let recheck: ReturnType<typeof setTimeout> | null = null;

/** Runs `job` once every edit queued before it is done. */
function enqueue<T>(job: () => Promise<T>): Promise<T> {
  const run = queue.then(job);
  // A rejected save must not freeze every later one (CLAUDE.md, golden rule
  // 6): the queue keeps going whatever happened before.
  queue = run.then(
    () => undefined,
    (e) => console.warn("story studio: save failed", e),
  );
  return run;
}

/** Queues a save of the chapter at `index`, as it stands when its turn
 * comes - so a burst of edits writes the latest state, in order. */
export function saveChapterAt(index: number): Promise<void> {
  return enqueue(async () => {
    const entry = studio.story?.chapters[index];
    if (entry?.chapter) await write(entry, entry.chapter);
  });
}

/** Queues a save of `story.yaml` or a world file, like a chapter. */
export function saveStoryFile(key: StoryFileKey): Promise<void> {
  return enqueue(async () => {
    const entry = studio.story?.[key];
    if (entry?.value) await write(entry, entry.value);
  });
}

/** Queues a rename of `from` and of every sentence citing it, then reloads
 * the story, whose files it changed. Resolves to why it was refused, or
 * `null` once done. */
export function renameEverywhere(kind: NameKind, from: string, to: string): Promise<string | null> {
  return enqueue(async () => {
    const path = studio.story?.path;
    if (!path) return null;
    // The rename reloads every file: an edit kept on screen because the
    // engine refused it would vanish with the reload.
    if (Object.keys(studio.unsaved).length > 0) return t("story.world.renameUnsaved");
    try {
      await renameName(path, kind, from, to);
    } catch (e) {
      return errorText(e);
    }
    await openStudio(path);
    return null;
  });
}

async function write(entry: { file: string; stamp: string }, value: unknown): Promise<void> {
  const story = studio.story;
  if (!story) return;
  try {
    const expected = entry.stamp === "" ? null : entry.stamp;
    entry.stamp = await saveFile(story.path, entry.file, expected, $state.snapshot(value));
    delete studio.unsaved[entry.file];
    // The reload it announced is behind the author now: a save went through.
    studio.notice = null;
    scheduleCheck();
  } catch (e) {
    const key = String(e);
    if (key === "errors.storyFileChanged") {
      // Changed in a text editor meanwhile: what is on disk wins, and the
      // author is told their last edit did not go through.
      await openStudio(story.path);
      studio.notice = errorText(e);
    } else {
      studio.unsaved[entry.file] = errorText(e);
    }
  }
}

/** Re-runs the validator once a burst of saves has settled. */
function scheduleCheck(): void {
  if (recheck) clearTimeout(recheck);
  recheck = setTimeout(async () => {
    recheck = null;
    const path = studio.story?.path;
    if (!path) return;
    try {
      const { diagnostics, usages } = await checkStory(path);
      if (studio.story?.path !== path) return;
      studio.diagnostics = diagnostics;
      studio.story.usages = usages;
    } catch (e) {
      console.warn("story studio: check failed", e);
    }
  }, 250);
}

/** Adds a chapter file to the open story (its stamp is empty: new) and
 * saves it. */
export function addChapter(file: string, chapter: Chapter): void {
  const story = studio.story;
  if (!story) return;
  story.chapters.push({ file, stamp: "", chapter, error: null });
  studio.chapter = story.chapters.length - 1;
  studio.node = chapter.nodes[0]?.id ?? null;
  void saveChapterAt(studio.chapter);
}
