// The Package section's logic (SPEC-story-mode, Create Your Own: Package):
// the story's registers grouped as the screen shows them. Which file needs a
// credit and which line gives it one is the engine's (`story_engine::credits`),
// never computed here.
import type { AcKind, ContentCredit, Coverage, Credit, Holding, PackageView, Permission } from "./studio";

/** What every line of a register says, whatever it names. */
export interface CreditLine {
  source?: string;
  author?: string;
  licence?: string;
  permission: Permission;
  proof?: string;
}

/** A line's free-text fields. */
export type CreditText = "source" | "author" | "licence" | "proof";

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
export function setText(line: CreditLine, key: CreditText, text: string): void {
  const value = text.trim();
  if (value) line[key] = value;
  else delete line[key];
}

/** A first line for a car or track the package carries, or for a layer of
 * one (`layer`) the story takes along: its permission waits until the
 * author says otherwise. */
export function contentLineFor(kind: AcKind, id: string, layer?: string): ContentCredit {
  return layer === undefined ? { kind, id, permission: "not_asked" } : { kind, id, layer, permission: "not_asked" };
}

/** The line of `lines` for the car or track `kind`/`id` itself (no
 * `layer`), or for its layer `layer`: its index, -1 without one. */
export function contentLineOf(lines: ContentCredit[], kind: AcKind, id: string, layer?: string): number {
  return lines.findIndex((l) => l.kind === kind && l.id === id && l.layer === layer);
}

/** What a story lacks, as a player reads it: the DLC to buy for the game's
 * own content - once each - and the folder id of anything else. */
export function missingLabels(missing: Holding[]): string[] {
  return [...new Set(missing.map((h) => (h.official && h.dlc ? h.dlc : h.id)))];
}

/** The package file a build offers to write: `<id>-<version>.pitstory`. */
export function packageName(id: string, version: string): string {
  return `${id}-${version}.pitstory`;
}
