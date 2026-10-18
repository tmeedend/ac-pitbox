// The story open in Create Your Own, and how an edit reaches its file.
//
// The screen edits a local copy of each chapter; every edit queues a save of
// the whole chapter (SPEC-story-mode, Create Your Own: each change is written
// at once). Saves run one after the other, each with the stamp the previous
// one returned: two saves in flight with the same stamp would refuse each
// other. A save the engine refuses (a sentence that does not parse) leaves
// the edit on screen, marked as not saved, and the next save carries it.
import { errorText } from "$lib/errors";
import {
  checkStory,
  openStory as fetchStory,
  saveChapter,
  type Chapter,
  type Diagnostic,
  type OpenStory,
} from "./studio";

export const studio = $state<{
  story: OpenStory | null;
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

/** Shows the chapter holding node `id` and selects it. */
export function selectNode(id: string): void {
  const index = studio.story?.chapters.findIndex((c) => c.chapter?.nodes.some((n) => n.id === id)) ?? -1;
  if (index < 0) return;
  studio.chapter = index;
  studio.node = id;
}

let queue: Promise<void> = Promise.resolve();
let recheck: ReturnType<typeof setTimeout> | null = null;

/** Queues a save of the chapter at `index`, as it stands when its turn
 * comes - so a burst of edits writes the latest state, in order. */
export function saveChapterAt(index: number): Promise<void> {
  const run = queue.then(() => writeChapter(index));
  // A rejected save must not freeze every later one (CLAUDE.md, golden rule
  // 6): the queue keeps going whatever happened before.
  queue = run.catch((e) => console.warn("story studio: save failed", e));
  return run;
}

async function writeChapter(index: number): Promise<void> {
  const story = studio.story;
  const entry = story?.chapters[index];
  if (!story || !entry?.chapter) return;
  try {
    const expected = entry.stamp === "" ? null : entry.stamp;
    entry.stamp = await saveChapter(story.path, entry.file, expected, $state.snapshot(entry.chapter) as Chapter);
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
      const diagnostics = await checkStory(path);
      if (studio.story?.path === path) studio.diagnostics = diagnostics;
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
