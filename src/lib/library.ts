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
  /** Pack d'origine commun aux mods d'une même archive multi-voitures (§4.4). */
  source_pack: string | null;
  /** URL d'origine (rempli plus tard par l'extension, §4.4/§12ter). */
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
  /** Fichiers annexes redirigés vers le dossier ressources du mod (§4.5.2). */
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

/** Sous-élément rattaché (skin/son) routé à l'import (§12bis.2). */
export interface SubImported {
  sub_type: "SKIN" | "TRACK_SKIN" | "SOUND";
  parent_id: string;
  name: string;
  projected: boolean;
  warning: string | null;
  /** Fichiers annexes redirigés vers le dossier ressources (§4.5.2). */
  resources_extracted: number;
}

/** App Python importée (§12bis.4). */
export interface AppImported {
  name: string;
  resources_extracted: number;
}

/** Mod « autre » importé — type non reconnu, jamais perdu (§7.3). */
export interface OtherImported {
  id: string;
  resources_extracted: number;
  /** Composant optionnel (§4.6bis) : livré dans une archive à part **et**
   * modifiant le jeu de base. Importé, mais laissé inactif — à l'utilisateur
   * de trancher. */
  optional?: boolean;
  /** Nombre de fichiers du jeu de base qu'il remplacerait. */
  game_files_replaced?: number;
}

export interface ArchiveResult {
  archive: string;
  mods: ImportedMod[];
  error: string | null;
  subs: SubImported[];
  apps: AppImported[];
  others: OtherImported[];
  /** Fichiers livrés à côté du mod et rattachés à lui (§4.5.3). */
  extras?: number;
}

/** Miroir de `import_progress::Progress` (§4.2bis) — les deux changent ensemble. */
export interface ImportProgress {
  /** Rang de l'item en cours de rangement, 1-basé. 0 pendant le pesage du lot. */
  item_index: number;
  item_count: number;
  /** Lot entier, dans [0,1], pondéré en secondes estimées. */
  overall_ratio: number;
  /** Item courant, dans [0,1]. */
  item_ratio: number;
  /** Secondes restantes estimées, `null` tant que le lot n'en dit rien. */
  eta_secs: number | null;
  archive: string;
  /** "queued" : posé côté frontend dès le drop/la sélection, avant même le
   * premier événement backend — retour immédiat le temps que la commande
   * (async, §4.2) démarre réellement le traitement. */
  phase: "queued" | "sizing" | "extract" | "scan" | "filing" | "done" | "cancelled";
  /** Rang du mod dans l'item courant, quand il en contient plusieurs. */
  sub_current: number;
  sub_total: number;
  label: string;
}

/** Demande l'arrêt du lot d'import en cours (§4.2bis). */
export function cancelImport(): Promise<void> {
  return invoke<void>("cancel_import");
}

/** Tri d'un glisser-déposer : archives d'un côté, dossiers de l'autre (§4.2).
 * Seul le backend peut le faire — un chemin sans extension peut être un
 * dossier de mod comme un fichier quelconque. */
export function splitDroppedPaths(
  paths: string[],
): Promise<{ archives: string[]; folders: string[] }> {
  return invoke<{ archives: string[]; folders: string[] }>("split_dropped_paths", { paths });
}

export function importArchives(
  paths: string[],
  decisions: ImportDecision[] = [],
): Promise<ArchiveResult[]> {
  return invoke<ArchiveResult[]>("import_archives", { paths, decisions });
}

/** Import de dossiers déjà décompressés (§4.2). copy=true préserve la source. */
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

// --- Import en masse (§4.2) ---
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

/** Fichier annexe du mod, listé dans le bloc Ressources (§4.5.2). */
export interface ResourceFile {
  name: string;
  /** Chemin relatif à sa racine — dossier ressources, ou dossier du mod si `in_mod`. */
  rel_path: string;
  size_bytes: number;
  /** Document resté **dans** le dossier du mod (§4.5.1) : signalé, jamais déplacé. */
  in_mod: boolean;
}

/** Fichiers annexes du mod, lus en direct sur disque (§4.5.2) — pas de cache,
 * un dépôt manuel dans le dossier ressources apparaît sans réimport. */
export function listModResources(id: string): Promise<ResourceFile[]> {
  return invoke<ResourceFile[]>("list_mod_resources", { id });
}

/** Une entrée de l'onglet « Ajouts au jeu » (§4.5.5). */
export interface ExtraFile {
  /** Chemin relatif à la racine d'Assetto Corsa — dit où le fichier atterrit. */
  rel_path: string;
  size_bytes: number;
  /** Posé dans le jeu par ce mod. Faux = un autre mod le fournit, ou mod inactif. */
  deployed: boolean;
  /** Mod qui fournit l'exemplaire posé, quand ce n'est pas celui-ci. */
  provided_by: string | null;
  /** Remplace un fichier du jeu — l'original est sauvegardé et sera restauré (§4.5.4). */
  replaces_game_file: boolean;
  /** Chemin qui n'en est pas un pour AC (dossier d'emballage de l'auteur) :
   * conservé en bibliothèque, jamais posé dans le jeu (§4.5.3). */
  off_game_path: boolean;
}

/** Ce qu'un mod installe hors de son dossier `content/` (§4.5.3). */
export function listModExtras(id: string): Promise<ExtraFile[]> {
  return invoke<ExtraFile[]>("list_mod_extras", { id });
}

/**
 * Une décision que l'**app** a prise seule au dernier import du mod (§4.6).
 * À ne pas confondre avec `ImportDecision` ci-dessus, qui est la décision de
 * l'**utilisateur** sur un cas ambigu (§4.4) : l'une rend compte, l'autre
 * arbitre.
 */
export interface ImportJournalEntry {
  /** Clé i18n courte — le libellé vit dans les locales, jamais en base. */
  kind: string;
  /** Le chemin sur lequel la décision a porté, tel qu'il était dans l'archive. */
  subject: string;
  /** Ce qui en a été fait (destination, mod de rattachement), quand ça s'impose. */
  detail: string | null;
  archive: string;
  decided_at: string;
}

/** Journal des décisions du dernier import (§4.6) — vide pour la plupart des mods. */
export function listImportDecisions(id: string): Promise<ImportJournalEntry[]> {
  return invoke<ImportJournalEntry[]>("list_import_decisions", { id });
}

/** Ouvre un fichier annexe avec l'application par défaut de l'OS (§4.5.2). */
export function openModResource(id: string, relPath: string, inMod: boolean): Promise<void> {
  return invoke<void>("open_mod_resource", { id, relPath, inMod });
}

/** URL `asset://` d'une ressource, pour l'afficher dans un `<img>` (§4.5.2). */
export async function modResourceSrc(id: string, relPath: string, inMod: boolean): Promise<string> {
  return convertFileSrc(await invoke<string>("get_mod_resource_path", { id, relPath, inMod }));
}

/**
 * Octets bruts d'une ressource (§4.5.2). Passe par l'IPC plutôt que par
 * `asset://` : un `fetch` du protocole personnalisé dépendrait de ses en-têtes
 * CORS, là où une commande Tauri renvoie l'`ArrayBuffer` sans intermédiaire.
 * Échoue au-delà du plafond de prévisualisation (`errors.resourceTooLarge`).
 */
export function readModResource(id: string, relPath: string, inMod: boolean): Promise<ArrayBuffer> {
  return invoke<ArrayBuffer>("read_mod_resource", { id, relPath, inMod });
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
