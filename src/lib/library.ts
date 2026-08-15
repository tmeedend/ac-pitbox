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
  /** Catégories de circuit (§5bis.2), multi-valué, ordonnées par priorité.
   * Vide pour une voiture (qui utilise `category`). */
  categories: string[];
  country: string | null;
  is_favorite: boolean;
  active_version_id: string | null;
  version_count: number;
  /** `null` pour le contenu de base (§4, `is_stock`) : pas de vraie date d'ajout disponible. */
  created_at: string | null;
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
  /** Taille sur disque cumulée de toutes les versions, octets (§9.4). `null`
   * tant qu'aucune n'a été calculée (mod importé avant cette fonctionnalité). */
  size_bytes: number | null;
  preview: string | null;
  /** Tracé du circuit à superposer à la photo (circuits, §6.1). */
  outline: string | null;
  active: boolean;
  /** Distance parcourue (km) d'après CM, si connue (§6.5). */
  distance_km: number | null;
  /** « Déjà essayé » : lancé par l'app OU km CM > 0 (§6.5). */
  tried: boolean;
  /** Poids natif (voitures), lu à la volée dans ui_car.json (§6.2). */
  weight: string | null;
  /** Badge/logo de la marque (ui/badge.png, voitures), à la place des initiales. */
  badge: string | null;
  /** Mod cassé (fichiers de la version active manquants/invalides, §6.4) —
   * même détection que l'écran Maintenance, signalée ici sur la carte. */
  broken: boolean;
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
  /** Taille sur disque de cette version, octets (§9.4). */
  size_bytes: number | null;
  /** Archive/dossier source conservé en bibliothèque (§10/§11), si le réglage
   * était activé à l'import. `null` = non conservé, pas de réinstallation possible. */
  kept_archive_path: string | null;
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
  /** Nom du DLC Kunos d'origine (contenu de base uniquement) — `null` pour le
   * jeu de base ou un mod importé. */
  stock_pack: string | null;
}

export interface FuzzyConflict {
  existing_id: string;
  existing_name: string | null;
}

export interface ImportedMod {
  id_interne: string;
  kind: ModKind;
  display_name: string | null;
  /** EXTENSION : rangé comme couche à part (§4.4). AMBIGUOUS : rien écrit, à
   * trancher par l'utilisateur (mise à jour ou extension). */
  outcome: "IMPORT" | "UPDATE_REPLACE" | "DUPLICATE" | "EXTENSION" | "AMBIGUOUS";
  version_label: string | null;
  conflict: FuzzyConflict | null;
  /** Décompte de comparaison (§4.4), présent pour EXTENSION/AMBIGUOUS. */
  added_count?: number;
  overwritten_count?: number;
  existing_total?: number;
  /** Fichiers annexes redirigés vers le dossier ressources du mod (§4.6). */
  resources_extracted: number;
}

/** Décision de reprise pour un import ambigu (§4.4). */
export interface ImportDecision {
  id: string;
  decision: "update" | "extension";
}

/** Couche/extension rattachée à une base (§4.4). */
export interface LayerRow {
  id: string;
  parent_id: string;
  parent_kind: ModKind;
  name: string;
  library_path: string;
  source_archive: string | null;
  added_count: number;
  overwritten_count: number;
  /** Appliquée à la composition en jeu (§4.4). */
  is_active: boolean;
  /** Ordre de priorité : la plus haute gagne à la superposition. */
  priority: number;
  imported_at: string;
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
  /** Fichiers annexes redirigés vers le dossier ressources (§4.6). */
  resources_extracted: number;
}

/** App Python importée (§12bis.4). */
export interface AppImported {
  name: string;
  resources_extracted: number;
}

/** Mod « autre » importé — type non reconnu, jamais perdu (§6.1bis). */
export interface OtherImported {
  id: string;
  resources_extracted: number;
}

export interface ArchiveResult {
  archive: string;
  mods: ImportedMod[];
  error: string | null;
  shared: SharedResult[];
  subs: SubImported[];
  apps: AppImported[];
  others: OtherImported[];
  /** Fichiers livrés à côté du mod et rattachés à lui (§4.6ter). */
  satellites?: number;
}

export interface ImportProgress {
  archive: string;
  /** "queued" : posé côté frontend dès le drop/la sélection, avant même le
   * premier événement backend — retour immédiat le temps que la commande
   * (async, §4.6bis) démarre réellement le traitement. */
  phase: "queued" | "extract" | "scan" | "filing" | "done";
  current: number;
  total: number;
  label: string;
}

export function importArchives(
  paths: string[],
  decisions: ImportDecision[] = [],
): Promise<ArchiveResult[]> {
  return invoke<ArchiveResult[]>("import_archives", { paths, decisions });
}

/** Import de dossiers déjà décompressés (§4.5). copy=true préserve la source. */
export function importFolders(
  paths: string[],
  copy: boolean,
  decisions: ImportDecision[] = [],
): Promise<ArchiveResult[]> {
  return invoke<ArchiveResult[]>("import_folders", { paths, copy, decisions });
}

/** Couches/extensions rattachées à une base (§4.4). */
export function listLayers(parentId: string): Promise<LayerRow[]> {
  return invoke<LayerRow[]>("list_layers", { parentId });
}

/** Toutes les couches d'un type (vue transversale add-ons, §4.4). */
export function listLayersByKind(kind: ModKind): Promise<LayerRow[]> {
  return invoke<LayerRow[]>("list_layers_by_kind", { kind });
}

/** Supprime une couche/extension (fichiers + overlay + recompose, §4.4). */
export function deleteLayer(id: string): Promise<void> {
  return invoke<void>("delete_layer", { id });
}

/** Active/désactive une couche puis recompose le contenu en jeu (§4.4). */
export function setLayerActive(id: string, active: boolean): Promise<void> {
  return invoke<void>("set_layer_active", { id, active });
}

/** Réordonne une couche (up = plus prioritaire) puis recompose (§4.4). */
export function reorderLayer(id: string, direction: "up" | "down"): Promise<void> {
  return invoke<void>("reorder_layer", { id, direction });
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

/** Ouvre le dossier réel du mod (voiture/circuit, géré ou contenu de base)
 * dans l'explorateur — résolu et ouvert côté backend (voir open_mod_folder,
 * contourne le scope ACL du plugin opener). */
export function openModFolder(id: string): Promise<void> {
  return invoke<void>("open_mod_folder", { id });
}

/** Fichier annexe du mod, listé dans le bloc Ressources (§4.6). */
export interface ResourceFile {
  name: string;
  /** Chemin relatif au dossier ressources (sous-dossiers éventuels). */
  rel_path: string;
  size_bytes: number;
}

/** Fichiers annexes du mod, lus en direct sur disque (§4.6) — pas de cache,
 * un dépôt manuel dans le dossier ressources apparaît sans réimport. */
export function listModResources(id: string): Promise<ResourceFile[]> {
  return invoke<ResourceFile[]>("list_mod_resources", { id });
}

/** Ouvre un fichier annexe avec l'application par défaut de l'OS (§4.6). */
export function openModResource(id: string, relPath: string): Promise<void> {
  return invoke<void>("open_mod_resource", { id, relPath });
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
