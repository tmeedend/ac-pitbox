// Pont typé vers les commandes L1 (bibliothèque & import).
import { invoke, convertFileSrc } from "@tauri-apps/api/core";

export type ModKind = "Car" | "Track";

export interface ModCard {
  id_interne: string;
  kind: ModKind;
  brand: string | null;
  display_name: string | null;
  year: number | null;
  car_class: string | null;
  category: string | null;
  country: string | null;
  is_favorite: boolean;
  active_version_id: string | null;
  version_count: number;
  created_at: string;
  tags_from_mod: string[];
  tags_from_rule: string[];
  tags_manual: string[];
  drivetrain: string | null;
  engine_pos: string | null;
  aspiration: string | null;
  engine_config: string | null;
  gearbox: string | null;
  preview: string | null;
  active: boolean;
}

export interface VersionRow {
  id: string;
  mod_id: string;
  version_label: string | null;
  author: string | null;
  imported_at: string;
  library_path: string;
  source_archive: string | null;
  content_signature: string | null;
  csp_features: string[];
  skins: string[];
  layouts: string[];
  tags_from_mod: string[];
}

export interface HistoryRow {
  timestamp: string;
  event: string;
  details: string;
}

export interface NativeSpecs {
  bhp: string | null;
  torque: string | null;
  weight: string | null;
  topspeed: string | null;
  acceleration: string | null;
  pwratio: string | null;
  range: string | null;
  description: string | null;
  country: string | null;
  author: string | null;
  year: number | null;
  power_curve: [number, number][];
  torque_curve: [number, number][];
}

export interface ModDetail extends ModCard {
  versions: VersionRow[];
  history: HistoryRow[];
  specs: NativeSpecs | null;
}

export interface FuzzyConflict {
  existing_id: string;
  existing_name: string | null;
}

export interface ImportedMod {
  id_interne: string;
  kind: ModKind;
  display_name: string | null;
  outcome: "IMPORT" | "UPDATE_REPLACE";
  version_label: string | null;
  conflict: FuzzyConflict | null;
}

export interface ArchiveResult {
  archive: string;
  mods: ImportedMod[];
  error: string | null;
}

export interface ImportProgress {
  archive: string;
  phase: "extract" | "scan" | "filing" | "done";
  current: number;
  total: number;
  label: string;
}

export function importArchives(paths: string[]): Promise<ArchiveResult[]> {
  return invoke<ArchiveResult[]>("import_archives", { paths });
}

export function resolveConflict(
  newId: string,
  oldId: string,
  action: "keep_both" | "replace",
): Promise<void> {
  return invoke<void>("resolve_conflict", { newId, oldId, action });
}

export function listLibrary(): Promise<ModCard[]> {
  return invoke<ModCard[]>("list_library");
}

export function getModDetail(id: string): Promise<ModDetail | null> {
  return invoke<ModDetail | null>("get_mod_detail", { id });
}

export function activateMod(id: string, versionId?: string): Promise<void> {
  return invoke<void>("activate_mod", { id, versionId: versionId ?? null });
}

export function deactivateMod(id: string): Promise<void> {
  return invoke<void>("deactivate_mod", { id });
}

export function setFavorite(id: string, favorite: boolean): Promise<void> {
  return invoke<void>("set_favorite", { id, favorite });
}

export function setManualTags(id: string, tags: string[]): Promise<void> {
  return invoke<void>("set_manual_tags", { id, tags });
}

export function setModField(
  id: string,
  field: string,
  value: string | null,
): Promise<void> {
  return invoke<void>("set_mod_field", { id, field, value });
}

export function reapplyRules(): Promise<number> {
  return invoke<number>("reapply_rules");
}

/** Transforme un chemin de fichier local en URL utilisable par <img>. */
export function previewSrc(path: string | null): string | null {
  return path ? convertFileSrc(path) : null;
}
