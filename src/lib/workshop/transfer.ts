// Typed bridge to the library transfer (EXPORT§), and the rules the two
// dialogs share. Pure but for `invoke`, for Vitest.
import { invoke } from "@tauri-apps/api/core";

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
  /** Per part; a part with nothing to carry is absent. */
  bytes: Partial<Record<Part, number>>;
  library_bytes: number;
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

export interface Inspection {
  manifest: Manifest;
  refusal: Refusal | null;
  file_bytes: number;
}

export interface ImportReport {
  mods: number;
  layers: number;
  apps: number;
  others: number;
  stock_applied: number;
  stock_missing: string[];
  presets_renamed: number;
  active_at_export: number;
  library_bytes_at_export: number;
}

export function estimateTransfer(): Promise<Estimate> {
  return invoke<Estimate>("transfer_estimate");
}

export function exportLibrary(path: string, parts: Part[]): Promise<ExportReport> {
  return invoke<ExportReport>("transfer_export", { path, parts });
}

export function inspectExport(path: string): Promise<Inspection> {
  return invoke<Inspection>("transfer_inspect", { path });
}

export function importLibrary(path: string, parts: Part[]): Promise<ImportReport> {
  return invoke<ImportReport>("transfer_import", { path, parts });
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
