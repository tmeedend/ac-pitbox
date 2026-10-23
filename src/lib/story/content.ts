// The Contents screen's logic, pure (SPEC-story-mode, Content types; Dialogue
// text): where each item is, how its placement is written, and the items the
// editor creates. Whether an item is right is the engine's validator, run
// after every save; these functions only keep an edit from writing a header
// the engine would read as something else.
import type { ContentDoc, ContentFile, ContentHeader, ContentType, EditedFile } from "./studio";

/** In the order the type control offers them: the spec's list. */
export const CONTENT_TYPES: ContentType[] = [
  "message",
  "call",
  "mail",
  "article",
  "social_post",
  "video",
  "listing",
  "statement",
  "calendar_entry",
  "dialogue",
  "illustration",
];

/** Played on the scene, outside any frame (`ContentType::on_scene`). */
export function onScene(kind: ContentType): boolean {
  return kind === "dialogue" || kind === "illustration";
}

export type Placement = "flow" | "reactive" | "pool";

/** Where an item is placed, told by which header keys are present - the
 * engine's own rule: `pool` → a pool variant, `when` → reactive, neither →
 * sent by the flow. */
export function placementOf(h: ContentHeader): Placement {
  if (h.pool !== undefined) return "pool";
  return h.when !== undefined ? "reactive" : "flow";
}

/** `h` placed as `p`: the keys of the other placements go, so the header
 * never says two things. Reactive keeps its conditions, and a pool variant
 * its own; a new pool is named, empty, for the checks to point at. */
export function setPlacement(h: ContentHeader, p: Placement): void {
  const when = h.when;
  delete h.pool;
  delete h.when;
  delete h.delay;
  delete h.priority;
  delete h.repeat;
  if (p === "reactive") h.when = when ?? [];
  if (p === "pool") {
    h.pool = "";
    if (when?.length) h.when = when;
  }
}

/** `h` as type `kind`: what only a dialogue has (the characters present,
 * the scene), or only an illustration (its picture), goes with the type. */
export function setType(h: ContentHeader, kind: ContentType): void {
  h.type = kind;
  if (kind !== "dialogue") {
    delete h.characters;
    delete h.scene;
  }
  if (kind !== "illustration") delete h.picture;
}

/** The file of a new item: named after its id, at the top of `content/`;
 * the author files it elsewhere by hand if they like. */
export function contentFile(id: string): string {
  return `content/${id}.story`;
}

/** A new item, sent by the flow, with no text yet. */
export function newContent(id: string, kind: ContentType = "message"): ContentDoc {
  return { header: { id, type: kind }, body: "" };
}

/** Where item `id` is: its file and its index there. */
export function findContent(
  files: EditedFile<ContentFile>[],
  id: string,
): { file: string; index: number } | null {
  for (const f of files) {
    const index = f.value?.items.findIndex((d) => d.header.id === id) ?? -1;
    if (index >= 0) return { file: f.file, index };
  }
  return null;
}

/** Every item's id, in every file that reads. */
export function contentIds(files: EditedFile<ContentFile>[]): string[] {
  return files.flatMap((f) => f.value?.items.map((d) => d.header.id) ?? []);
}

/** A file's name as the list titles it: its path inside `content/`. */
export function fileTitle(file: string): string {
  return file.replace(/^content\//, "");
}
