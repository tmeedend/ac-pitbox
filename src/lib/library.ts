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
  /** Pack d'origine commun aux mods d'une même archive multi-voitures (§4.7). */
  source_pack: string | null;
  /** URL d'origine (rempli plus tard par l'extension, §4.7/§12ter). */
  source_url: string | null;
  /** Auteur de la version active (colonne §6.2). */
  author: string | null;
  /** Label de version de la version active (colonne §6.2). */
  active_version_label: string | null;
  /** Date de dernière mise à jour (import de la version la plus récente, §6.2). */
  updated_at: string | null;
  /** Layouts de la version active (colonne circuits §6.2). */
  layouts: string[];
  /** Extensions CSP de la version active (colonne circuits §6.2). */
  csp_features: string[];
  /** Contenu de base Kunos : lecture seule, non désactivable (§12bis.1). */
  is_stock: boolean;
  /** Date de publication estimée (dates de fichiers à l'import), remplaçable par L7 (§6.2). */
  published_at: string | null;
  preview: string | null;
  /** Tracé du circuit à superposer à la photo (circuits, §6.1). */
  outline: string | null;
  active: boolean;
  /** Distance parcourue (km) d'après CM, si connue (§6.5). */
  distance_km: number | null;
  /** « Déjà essayé » : lancé par l'app OU km CM > 0 (§6.5). */
  tried: boolean;
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
  /** Date de publication estimée depuis les dates de fichiers (§6.2). */
  published_at: string | null;
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

export interface LayoutItem {
  id: string;
  name: string;
  length: string | null;
  preview: string | null;
  outline: string | null;
}

export interface TrackDetail {
  description: string | null;
  layouts: LayoutItem[];
}

export interface ModDetail extends ModCard {
  versions: VersionRow[];
  history: HistoryRow[];
  specs: NativeSpecs | null;
  track: TrackDetail | null;
}

export interface FuzzyConflict {
  existing_id: string;
  existing_name: string | null;
}

export interface ImportedMod {
  id_interne: string;
  kind: ModKind;
  display_name: string | null;
  outcome: "IMPORT" | "UPDATE_REPLACE" | "DUPLICATE";
  version_label: string | null;
  conflict: FuzzyConflict | null;
}

/** Ressource partagée (font/driver) installée globalement (§4.8). */
export interface SharedResult {
  kind: "fonts" | "driver";
  name: string;
  /** "installed" (nouveau) | "identical" (déjà là) | "replaced" (écrasé, différent). */
  disposition: "installed" | "identical" | "replaced";
}

/** Sous-élément rattaché (skin/son) routé à l'import (§12bis.2). */
export interface SubImported {
  sub_type: "SKIN" | "SOUND";
  parent_id: string;
  name: string;
  projected: boolean;
  warning: string | null;
}

/** App Python importée (§12bis.4). */
export interface AppImported {
  name: string;
}

export interface ArchiveResult {
  archive: string;
  mods: ImportedMod[];
  error: string | null;
  shared: SharedResult[];
  subs: SubImported[];
  apps: AppImported[];
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

/** Import de dossiers déjà décompressés (§4.5). copy=true préserve la source. */
export function importFolders(paths: string[], copy: boolean): Promise<ArchiveResult[]> {
  return invoke<ArchiveResult[]>("import_folders", { paths, copy });
}

// --- Import en masse (§4.6) ---
export type BulkStatus = "new" | "update" | "duplicate" | "ambiguous";

export interface BulkMod {
  id: string;
  kind: ModKind;
  name: string | null;
  status: BulkStatus;
  existing_id: string | null;
  existing_name: string | null;
}

export interface BulkEntry {
  subfolder: string;
  path: string;
  ignored: boolean;
  mods: BulkMod[];
}

export interface BulkExecItem {
  path: string;
  skip_ids: string[];
  replace_ids: string[];
}

export function analyzeBulkImport(parent: string): Promise<BulkEntry[]> {
  return invoke<BulkEntry[]>("analyze_bulk_import", { parent });
}

export function executeBulkImport(items: BulkExecItem[], copy: boolean): Promise<ArchiveResult[]> {
  return invoke<ArchiveResult[]>("execute_bulk_import", { items, copy });
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
