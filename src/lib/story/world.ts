// The World and Settings screens' logic, pure (SPEC-story-mode, Variables
// and Story settings): the names an author gives, and where they are cited.
// Whether the story is right is still the engine's validator, run after
// every save; these functions only keep an edit from writing a name the
// engine would read as something else.
import { fold } from "$lib/text";
import type { NameKind, NameUsages, Usage, Variables } from "./studio";

/** A name the editor gives: a letter, then letters, digits and `_`. The
 * engine's own rule (`story_engine::refs::is_name`), checked here to say so
 * at once: on the right of a comparison, `2fast` would read as a number and
 * `true` as a yes. */
export function isName(s: string): boolean {
  return /^[A-Za-z][A-Za-z0-9_]*$/.test(s) && s !== "true" && s !== "false";
}

/** Why `name` cannot be given, as an i18n key, or `null` when it can. */
export function nameProblem(name: string, taken: Iterable<string>): string | null {
  if (!isName(name)) return "story.world.nameInvalid";
  if (new Set(taken).has(name)) return "story.world.nameTaken";
  return null;
}

/** A name built from what the author typed - "Un ami" gives `un_ami` - made
 * unique among `taken`. `fallback` stands in when the label gives no letter
 * to start with. */
export function nameFrom(label: string, taken: Iterable<string>, fallback: string): string {
  let base = fold(label)
    .replace(/[^a-z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "");
  if (!isName(base)) base = base ? `${fallback}_${base}` : fallback;
  const used = new Set(taken);
  let name = base;
  for (let n = 2; used.has(name); n++) name = `${base}_${n}`;
  return name;
}

/** Flags and counters share one set of names: `flag.x` and `counter.x` read
 * apart, but one name for both is what the validator warns about. */
export function variableNames(v: Variables): string[] {
  return [...(v.flags ?? []), ...Object.keys(v.counters ?? {})];
}

export function usagesOf(all: NameUsages[], kind: NameKind, id: string): Usage[] {
  return all.find((n) => n.kind === kind && n.id === id)?.usages ?? [];
}

/** How many files the lines of `usages` are in. */
export function fileCount(usages: Usage[]): number {
  return new Set(usages.map((u) => u.file)).size;
}

/** A list typed as text, one comma between items (the authors). */
export function parseList(text: string): string[] {
  return text
    .split(",")
    .map((s) => s.trim())
    .filter((s) => s.length > 0);
}

/** The pictures of `media/` a portrait can be: the image files, by path. */
export function portraitImages(media: string[]): string[] {
  return media.filter((p) => /\.(png|jpe?g|webp)$/i.test(p));
}

/** A character's expressions in the order the editor lists them: `neutral`
 * first, the one every dialogue falls back on, then by name. */
export function expressionsInOrder(portraits: Record<string, string> | undefined): string[] {
  return Object.keys(portraits ?? {}).sort((a, b) =>
    a === "neutral" ? -1 : b === "neutral" ? 1 : a.localeCompare(b),
  );
}
