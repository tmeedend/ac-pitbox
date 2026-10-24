// The story open in Create Your Own, and how an edit reaches its file.
//
// The screen edits a local copy of each file (chapters, `story.yaml`, the
// world files); every edit queues a save of the whole file (SPEC-story-mode,
// Create Your Own: each change is written at once). Saves run one after the
// other, each with the stamp the previous one returned: two saves in flight
// with the same stamp would refuse each other. A save the engine refuses (a
// sentence that does not parse) leaves the edit on screen, marked as not
// saved, and the next save carries it. A rename, an undo, a redo and a
// restore join the same queue, so each runs on files every earlier edit has
// reached.
import { errorText } from "$lib/errors";
import { t } from "$lib/i18n/index.svelte";
import {
  checkStory,
  openStory as fetchStory,
  redoEdit,
  removeFile,
  renameName,
  restoreBackup,
  saveFile,
  undoEdit,
  type Chapter,
  type ContentDoc,
  type Diagnostic,
  type NameKind,
  type OpenStory,
  type StoryEvent,
} from "./studio";
import { eventFile } from "./events";
import { contentFile, findContent } from "./content";

/** The studio's sections (SPEC-story-mode, Create Your Own). */
export type StudioSection = "flow" | "events" | "content" | "drivers" | "world" | "settings" | "backups";

/** The section whose files hold `file` (a diagnostic's, say, which may end
 * with `:<line>`): `story.yaml` is the Settings', `world/` the World's,
 * `drivers/` the Drivers', `events/` the Events', `content/` the Contents',
 * the rest the flow's - `scenes/` included, until it has a screen. */
export function sectionOf(file: string): StudioSection {
  if (file === "story.yaml") return "settings";
  if (file.startsWith("drivers/")) return "drivers";
  if (file.startsWith("events/")) return "events";
  if (file.startsWith("content/")) return "content";
  return file.startsWith("world/") ? "world" : "flow";
}

/** The story-wide files, by their key in `OpenStory`. */
export type StoryFileKey = "manifest" | "characters" | "variables" | "drivers";

export const studio = $state<{
  story: OpenStory | null;
  section: StudioSection;
  /** Index in `story.chapters`. */
  chapter: number;
  /** The selected node's id, in the shown chapter. */
  node: string | null;
  /** The file of the event shown in the Events section. */
  event: string | null;
  /** The id of the item shown in the Contents section. */
  content: string | null;
  diagnostics: Diagnostic[];
  /** Why the last save of a chapter file failed, by file. */
  unsaved: Record<string, string>;
  /** Shown above the flow: the story was reloaded under the author's feet. */
  notice: string | null;
  loading: boolean;
  /** Moves each time the files on disk are known to have changed - after
   * each check, on each reload: what a view reading the disk (the preview)
   * follows. */
  revision: number;
}>({
  story: null,
  section: "flow",
  chapter: 0,
  node: null,
  event: null,
  content: null,
  diagnostics: [],
  unsaved: {},
  notice: null,
  loading: false,
  revision: 0,
});

/** Opens (or reopens) a story. Keeps the selection when it still exists. */
export async function openStudio(path: string): Promise<void> {
  studio.loading = true;
  try {
    const story = await fetchStory(path);
    const sameStory = studio.story?.path === path;
    studio.story = story;
    studio.diagnostics = story.diagnostics;
    studio.revision++;
    studio.unsaved = {};
    if (!sameStory) {
      studio.notice = null;
      studio.section = "flow";
      studio.chapter = 0;
      studio.node = null;
      studio.event = null;
      studio.content = null;
    }
    if (!story.events.some((e) => e.file === studio.event)) studio.event = story.events[0]?.file ?? null;
    if (studio.content && !findContent(story.contentFiles, studio.content)) studio.content = null;
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

/** Shows event `id` in the Events section. */
export function selectEvent(id: string): void {
  const entry = studio.story?.events.find((e) => e.value?.id === id);
  if (!entry) return;
  studio.section = "events";
  studio.event = entry.file;
}

/** Shows content item `id` in the Contents section. */
export function selectContent(id: string): void {
  if (!studio.story || !findContent(studio.story.contentFiles, id)) return;
  studio.section = "content";
  studio.content = id;
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

/** Queues a save of the event in `file`, like a chapter. */
export function saveEventFile(file: string): Promise<void> {
  return enqueue(async () => {
    const entry = studio.story?.events.find((e) => e.file === file);
    if (entry?.value) await write(entry, entry.value);
  });
}

/** Queues a save of the `.story` file `file`, like a chapter: all its items. */
export function saveContentFile(file: string): Promise<void> {
  return enqueue(async () => {
    const entry = studio.story?.contentFiles.find((f) => f.file === file);
    if (entry?.value) await write(entry, entry.value);
  });
}

/** Adds an item in a file of its own (its stamp is empty: new), shows it
 * and saves it. */
export function addContent(doc: ContentDoc): Promise<void> {
  const story = studio.story;
  if (!story) return Promise.resolve();
  const file = contentFile(doc.header.id);
  story.contentFiles.push({ file, stamp: "", value: { items: [doc] }, error: null });
  story.contentFiles.sort((a, b) => a.file.localeCompare(b.file));
  studio.section = "content";
  studio.content = doc.header.id;
  return saveContentFile(file);
}

/** Queues the deletion of item `id`: its file loses it, or goes with its
 * last item. Resolves to why it was refused, or `null` once done. */
export function removeContent(id: string): Promise<string | null> {
  return enqueue(async () => {
    const story = studio.story;
    const at = story ? findContent(story.contentFiles, id) : null;
    const entry = at ? story?.contentFiles.find((f) => f.file === at.file) : undefined;
    if (!story || !at || !entry?.value) return null;
    if (entry.value.items.length > 1) {
      entry.value.items.splice(at.index, 1);
      studio.content = null;
      await write(entry, entry.value);
      return studio.unsaved[entry.file] ?? null;
    }
    // Its last item: the file goes, which reloads every file, like a rename.
    if (Object.keys(studio.unsaved).length > 0) return t("story.studio.historyUnsaved");
    try {
      await removeFile(story.path, entry.file, entry.stamp);
    } catch (e) {
      await openStudio(story.path);
      return errorText(e);
    }
    studio.content = null;
    await openStudio(story.path);
    return null;
  });
}

/** Adds an event to the open story (its stamp is empty: new), shows it
 * and saves it. */
export function addEvent(event: StoryEvent): Promise<void> {
  const story = studio.story;
  if (!story) return Promise.resolve();
  const file = eventFile(event.id);
  story.events.push({ file, stamp: "", value: event, error: null });
  story.events.sort((a, b) => a.file.localeCompare(b.file));
  studio.section = "events";
  studio.event = file;
  return saveEventFile(file);
}

/** Queues the deletion of the event in `file`, then reloads the story.
 * Resolves to why it was refused, or `null` once done. */
export function removeEvent(file: string): Promise<string | null> {
  return enqueue(async () => {
    const path = studio.story?.path;
    const entry = studio.story?.events.find((e) => e.file === file);
    if (!path || !entry) return null;
    // The deletion reloads every file, like a rename.
    if (Object.keys(studio.unsaved).length > 0) return t("story.studio.historyUnsaved");
    try {
      await removeFile(path, file, entry.stamp);
    } catch (e) {
      await openStudio(path);
      return errorText(e);
    }
    await openStudio(path);
    return null;
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
    if (Object.keys(studio.unsaved).length > 0) return t("story.studio.historyUnsaved");
    try {
      await renameName(path, kind, from, to);
    } catch (e) {
      return errorText(e);
    }
    // An event's file named after it moved with it: keep it shown.
    if (kind === "event" && studio.event === eventFile(from)) studio.event = eventFile(to);
    await openStudio(path);
    return null;
  });
}

/** Queues an undo (or a redo) of the last edit, then reloads the story and
 * shows where the edit was: its section, its chapter. */
export function stepHistory(direction: "undo" | "redo"): Promise<void> {
  return enqueue(async () => {
    const path = studio.story?.path;
    if (!path) return;
    if (Object.keys(studio.unsaved).length > 0) {
      studio.notice = t("story.studio.historyUnsaved");
      return;
    }
    let files: string[];
    try {
      files = await (direction === "undo" ? undoEdit : redoEdit)(path);
    } catch (e) {
      // A file changed outside Pit Box: what is on disk wins, and the
      // history it stood on is gone.
      await openStudio(path);
      studio.notice = errorText(e);
      return;
    }
    if (files.length === 0) return;
    await openStudio(path);
    studio.notice = null;
    // The section shown, when the edit touched its files; the first one's
    // otherwise. An event's rename also changes the flow citing it, and
    // undoing it from the Events section must not leave that section.
    const here = files.find((f) => sectionOf(f) === studio.section);
    const first = here ?? files[0];
    studio.section = sectionOf(first);
    if (studio.story?.events.some((e) => e.file === first)) studio.event = first;
    const chapter = studio.story?.chapters.findIndex((c) => c.file === first) ?? -1;
    if (chapter >= 0) studio.chapter = chapter;
  });
}

/** Queues a restore of copy `id`, then reloads the story. Resolves to why it
 * failed, or `null` once done. */
export function restoreCopy(id: string): Promise<string | null> {
  return enqueue(async () => {
    const path = studio.story?.path;
    if (!path) return null;
    if (Object.keys(studio.unsaved).length > 0) return t("story.studio.historyUnsaved");
    try {
      await restoreBackup(path, id);
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
      const { diagnostics, usages, media, history } = await checkStory(path);
      if (studio.story?.path !== path) return;
      studio.diagnostics = diagnostics;
      studio.story.usages = usages;
      studio.story.media = media;
      studio.story.history = history;
      studio.revision++;
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
