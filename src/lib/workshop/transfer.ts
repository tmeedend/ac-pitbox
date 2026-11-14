// Typed bridge to the library transfer (EXPORT§), and the rules the two
// dialogs share. Pure but for `invoke`, for Vitest.
import { invoke } from "@tauri-apps/api/core";
import type { LivePreset } from "$lib/gridthumbs/gridThumbs";

/** Must stay aligned with `transfer::Part`. */
export type Part = "library" | "classification" | "sessions" | "profiles" | "preferences";
export const PARTS: Part[] = ["library", "classification", "sessions", "profiles", "preferences"];

export interface Counts {
  cars: number;
  tracks: number;
  layers: number;
  skins: number;
  sounds: number;
  apps: number;
  others: number;
  stock_with_user_data: number;
  unmanaged: number;
  profiles: number;
  sessions: number;
  grids: number;
}

export interface Manifest {
  format: number;
  app_version: string;
  exported_at: string;
  parts: Part[];
  counts: Counts;
  active_at_export: string[];
  library_bytes_at_export: number;
}

export interface Estimate {
  counts: Counts;
  /** Per part, as the zip will hold it; a part with nothing to carry is
   * absent. */
  bytes: Partial<Record<Part, number>>;
  /** What the generated thumbnails add, per preset in use (EXPORT§4.3). */
  thumb_bytes_per_preset: number;
  /** What the thumbnails the skeletons already froze add, box checked. */
  frozen_thumb_bytes: number;
}

export interface ExportReport {
  path: string;
  bytes: number;
  counts: Counts;
  library_bytes: number;
}

/** Why an export cannot be imported here: `key` is an i18n key that `count`
 * and `version` fill. */
export interface Refusal {
  key: string;
  count: number | null;
  version: string | null;
}

/** The groups of entries a merge asks about (EXPORT§7.4). Must stay aligned
 * with `transfer::Group`. */
export type Group = "names" | "classification" | "tech" | "prefs";
export const GROUPS: Group[] = ["names", "classification", "tech", "prefs"];

/** Which side wins a disagreement: this machine's value, or the export's. */
export type Side = "here" | "export";

export type Choices = Record<Group, Side>;

/** Nothing replaced unless asked (EXPORT§7.4). */
export function defaultChoices(): Choices {
  return { names: "here", classification: "here", tech: "here", prefs: "here" };
}

export interface Named {
  id: string;
  name: string;
}

/** What an import would do here, before anything is written. */
export interface Plan {
  /** This installation already holds a library: the import merges. */
  merging: boolean;
  added: number;
  kept: number;
  blocked: Named[];
  attached_skipped: number;
  filled: number;
  notes_joined: number;
  /** Disagreements by group; a group without any is absent. */
  conflicts: Partial<Record<Group, number>>;
}

export interface Inspection {
  manifest: Manifest;
  refusal: Refusal | null;
  file_bytes: number;
  plan: Plan | null;
}

export interface Decided {
  group: Group;
  side: Side;
  items: Named[];
}

export interface ImportReport {
  mods: number;
  layers: number;
  apps: number;
  others: number;
  added: Named[];
  added_addons: Named[];
  /** Already here: files, versions and rows kept as they are. */
  kept: Named[];
  /** Their folder was already in the library: not brought in. */
  blocked: Named[];
  attached_skipped: number;
  filled: Named[];
  decided: Decided[];
  notes_joined: Named[];
  stock_applied: number;
  stock_missing: string[];
  thumbs_adopted: number;
  profiles_renamed: string[];
  grids_renamed: string[];
  presets_renamed: number;
  /** Classification files this machine kept, holding its own decisions. */
  classification_kept: string[];
  active_at_export: number;
  library_bytes_at_export: number;
}

export function estimateTransfer(): Promise<Estimate> {
  return invoke<Estimate>("transfer_estimate");
}

/** `thumbPresets`: the presets whose grid thumbnails leave frozen
 * (EXPORT§4.3); empty, none leaves. */
export function exportLibrary(path: string, parts: Part[], thumbPresets: LivePreset[]): Promise<ExportReport> {
  return invoke<ExportReport>("transfer_export", { path, parts, thumbPresets });
}

export function inspectExport(path: string): Promise<Inspection> {
  return invoke<Inspection>("transfer_inspect", { path });
}

export function importLibrary(path: string, parts: Part[], choices: Choices): Promise<ImportReport> {
  return invoke<ImportReport>("transfer_import", { path, parts, choices });
}

/** The i18n key of each group's name, shared by the choice and the report. */
export const GROUP_LABEL: Record<Group, string> = {
  names: "transfer.groupNames",
  classification: "transfer.groupClassification",
  tech: "transfer.groupTech",
  prefs: "transfer.groupPrefs",
};

/** Whether the import screen says what the merge will do: a library here,
 * or entries on game content to merge — notes on a Kunos car count too. */
export function mergeToShow(plan: Plan): boolean {
  return plan.merging || plan.notes_joined > 0 || Object.values(plan.conflicts).some((n) => (n ?? 0) > 0);
}

/** The parts an import starts with checked: all those the file holds — but
 * Preferences when merging, which would replace this machine's settings
 * (EXPORT§7.3, step 3). */
export function initialImportParts(available: Part[], merging: boolean): Part[] {
  return available.filter((p) => !(merging && p === "preferences"));
}

/** Profiles names mods: without the library they have nothing to name, and
 * their box is greyed (EXPORT§3). */
export function partAllowed(part: Part, selected: Part[]): boolean {
  return part !== "profiles" || selected.includes("library");
}

/** The parts that will really travel, in their order. */
export function effectiveParts(selected: Part[]): Part[] {
  return PARTS.filter((p) => selected.includes(p) && partAllowed(p, selected));
}

/** The name the save dialog proposes: `Pit Box - 2026-10-09.pitbox`. */
export function exportFileName(date: Date): string {
  const pad = (n: number) => String(n).padStart(2, "0");
  return `Pit Box - ${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}.pitbox`;
}
