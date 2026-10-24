// The Package section's logic (SPEC-story-mode, Create Your Own: Package):
// the story's registers grouped as the screen shows them. Which file needs a
// credit and which line gives it one is the engine's (`story_engine::credits`),
// never computed here.
import type { Coverage, Credit, PackageView } from "./studio";

/** One register as the screen shows it: its lines, how many files each
 * names, and the files that have none yet. */
export interface RegisterGroup {
  /** `media/credits.yaml` or `frames/skins/<id>/credits.yaml`. */
  file: string;
  /** The skin it travels with; `null` for `media/`. */
  skin: string | null;
  /** `null` when the file does not read; empty when it does not exist yet. */
  lines: Credit[] | null;
  /** By line index. */
  named: number[];
  /** Paths inside the register's folder. */
  uncredited: string[];
}

const SKINS = "frames/skins/";

/** The folder a register's paths are relative to. */
export function folderOf(register: string): string {
  return register.slice(0, register.length - "credits.yaml".length);
}

/** The skin whose register this is, if it is one. */
export function skinOf(register: string): string | null {
  return register.startsWith(SKINS) ? register.slice(SKINS.length).split("/")[0] : null;
}

/** Every register the story has or needs - `media/` first, then the skins
 * by name - with what the engine said of each. */
export function registerGroups(view: PackageView): RegisterGroup[] {
  const files = new Set<string>([
    ...view.registers.map((r) => r.file),
    ...view.register.coverage.map((c) => c.register),
  ]);
  return [...files]
    .sort((a, b) => (a.startsWith("media/") ? -1 : b.startsWith("media/") ? 1 : a.localeCompare(b)))
    .map((file) => {
      const entry = view.registers.find((r) => r.file === file);
      const lines = entry ? entry.value : [];
      const covered: Coverage[] = view.register.coverage.filter((c) => c.register === file);
      const folder = folderOf(file);
      return {
        file,
        skin: skinOf(file),
        lines,
        named: (lines ?? []).map((_, i) => covered.filter((c) => c.line === i).length),
        uncredited: covered.filter((c) => c.line === null).map((c) => c.file.slice(folder.length)),
      };
    });
}

/** The files a line names, typed as a comma-separated list. */
export function splitFiles(text: string): string[] {
  return text
    .split(",")
    .map((f) => f.trim())
    .filter((f) => f.length > 0);
}

/** A first line for a file that has none: everything else is the author's
 * to fill, and its permission waits until they say otherwise. */
export function lineFor(file: string): Credit {
  return { files: [file], permission: "not_asked" };
}

/** A line's optional text field set from what was typed: an empty field is
 * left out of the file. */
export function setText(line: Credit, key: "source" | "author" | "licence" | "proof", text: string): void {
  const value = text.trim();
  if (value) line[key] = value;
  else delete line[key];
}

/** The package file a build offers to write: `<id>-<version>.pitstory`. */
export function packageName(id: string, version: string): string {
  return `${id}-${version}.pitstory`;
}
