// Pont typé vers l'ontologie de tags (§5).
import { invoke } from "@tauri-apps/api/core";

export interface SetRule {
  /** Stable id of a shipped rule (REGLES§4); absent on the user's own. It
   * rides along when the Rules screen edits the rule, and is what tells a
   * disabled or modified shipped rule from a new one when saving. */
  id?: string;
  from: string[];
  set: string;
}
export interface TagMerge {
  id?: string;
  from: string[];
  to: string[];
}
export interface BrandFix {
  id?: string;
  name_contains: string;
  set_brand: string;
}
export interface NameToTag {
  id?: string;
  name_contains: string;
  add: string[];
}
export interface ClassFix {
  id?: string;
  from: string[];
  set_class: string | null;
  add: string[];
}
export interface ExtractionSpecs {
  drivetrain: SetRule[];
  aspiration: SetRule[];
  engine_config: SetRule[];
  engine_pos: SetRule[];
  gearbox: SetRule[];
}
export interface ExtractionCountry {
  map: Record<string, string>;
}
/** Comment un pays s'écrit → le nom sous lequel on le range.
 *
 * À ne pas confondre avec `ExtractionCountry`, qui lui ressemble : celle-là
 * DEVINE un pays absent à partir d'un tag, celle-ci NORMALISE un pays déjà
 * déclaré. D'où sa place hors des deux familles — un circuit déclare un pays
 * comme une voiture. */
export interface CountryAliases {
  map: Record<string, string>;
  /** Values unknown to the game the user chose to leave as they are
   * (TAXO§7.2) - from the overlay (`TaxonomyOverlay.ignored_countries`). */
  ignored?: string[];
}
/** A family of car categories (INDEX§6.1) — see `$lib/library/families`. */
export interface CategoryFamily {
  id: string;
  /** Only on a family the user made up; the shipped ones are translated. */
  name?: string;
  icon?: string;
  tags: string[];
}
export interface CarRules {
  brand_fix: BrandFix[];
  name_to_tag: NameToTag[];
  class_fix: ClassFix[];
  tag_merge: TagMerge[];
  extraction_specs: ExtractionSpecs;
  extraction_country: ExtractionCountry;
  category_families: CategoryFamily[];
}
export interface TrackRules {
  tag_merge: TagMerge[];
  /** Catégories de circuit autorisées (§5), tags `#` par ordre de priorité. */
  category_allowlist: string[];
}
export interface Rules {
  car: CarRules;
  track: TrackRules;
  country_aliases: CountryAliases;
}

export function getRules(): Promise<Rules> {
  return invoke<Rules>("get_rules");
}

// --- List rules in two layers: the Rules screen (REGLES§8) -------------------

/** A catalogue rule the user modified: his copy, frozen, and the fingerprint
 * of the version it was made from. The screen leaves `forked_from` empty - the
 * backend fills it, it alone can fingerprint. */
export interface Fork<T> {
  rule: T;
  forked_from: string;
}

/** The user's decisions on one list of rules: shipped rules switched off,
 * shipped rules modified, rules of his own (with `own-N` ids). */
export interface ListOverlay<T> {
  disabled?: string[];
  forks?: Record<string, Fork<T>>;
  own?: T[];
}

/** The user's decisions on the ordered allowlist of track categories. */
export interface OrderOverlay {
  removed?: string[];
  added?: string[];
  order?: string[] | null;
}

export interface RulesOverlay {
  format?: number;
  /** The global switch off: the catalogue stops applying (REGLES§7). */
  catalog_off?: boolean;
  brand_fix?: ListOverlay<BrandFix>;
  name_to_tag?: ListOverlay<NameToTag>;
  class_fix?: ListOverlay<ClassFix>;
  car_tag_merge?: ListOverlay<TagMerge>;
  drivetrain?: ListOverlay<SetRule>;
  aspiration?: ListOverlay<SetRule>;
  engine_config?: ListOverlay<SetRule>;
  engine_pos?: ListOverlay<SetRule>;
  gearbox?: ListOverlay<SetRule>;
  track_tag_merge?: ListOverlay<TagMerge>;
  track_categories?: OrderOverlay;
}

/** Where a row comes from - what its badge says: `PIT BOX`, `PIT BOX ✎`, or
 * nothing. */
export type Origin = "catalog" | "fork" | "own";

export interface RuleRow<T> {
  /** The rule as it reads, always with its id. */
  rule: T;
  origin: Origin;
  disabled: boolean;
  /** A fork whose shipped rule changed since: a new version exists. */
  outdated: boolean;
  /** Mods the rule acts on, measured on the library. */
  effect: number;
}

export interface CategoryRow {
  name: string;
  shipped: boolean;
  /** `false`: a shipped category the user removed. */
  on: boolean;
  effect: number;
}

export interface RulesView {
  catalog_on: boolean;
  catalog_version: string;
  catalog_count: number;
  overlay: RulesOverlay;
  brand_fix: RuleRow<BrandFix>[];
  name_to_tag: RuleRow<NameToTag>[];
  class_fix: RuleRow<ClassFix>[];
  car_tag_merge: RuleRow<TagMerge>[];
  drivetrain: RuleRow<SetRule>[];
  aspiration: RuleRow<SetRule>[];
  engine_config: RuleRow<SetRule>[];
  engine_pos: RuleRow<SetRule>[];
  gearbox: RuleRow<SetRule>[];
  track_tag_merge: RuleRow<TagMerge>[];
  track_categories: CategoryRow[];
}

/** A READ that measures the whole library - slow on a large one, which is why
 * it is `invoke` with a loading state rather than `invokeSafe`'s 5 s cut. */
export function getRulesView(): Promise<RulesView> {
  return invoke<RulesView>("get_rules_view");
}

/** What an import did (REGLES§9): it merges, never replaces. */
export interface ImportReport {
  added: number;
  /** Entries both sides decided differently - the user's decision stayed. */
  kept_yours: number;
  already: number;
  /** Decisions naming rules this catalogue does not have. */
  unknown: number;
  catalog_version: string;
}

/** Writes the user's decisions - the overlays, never the catalogue - to a
 * file he chose. */
export function exportRules(path: string): Promise<void> {
  return invoke<void>("export_rules", { path });
}

/** Merges an export into his decisions and re-applies the library. */
export function importRules(path: string): Promise<ImportReport> {
  return invoke<ImportReport>("import_rules", { path });
}

/** Writes the anonymous survey of the library (`survey.rs`) to a file the
 * user chose - he sends it himself. Returns the cars and tracks it covers. */
export function exportSurvey(path: string): Promise<[number, number]> {
  return invoke<[number, number]>("export_survey", { path });
}

/** The same survey, of a folder of mods Pit Box does not hold (a Mod
 * Organizer `mods` folder) - read-only, nothing imported. */
export function exportFolderSurvey(root: string, path: string): Promise<[number, number]> {
  return invoke<[number, number]>("export_folder_survey", { root, path });
}

/** Writes the decisions and re-applies them to the library; the view comes
 * back with the counters of that pass. A WRITE: `invoke`, never `invokeSafe`. */
export function saveRulesOverlay(overlay: RulesOverlay): Promise<RulesView> {
  return invoke<RulesView>("save_rules_overlay", { overlay });
}

// --- Taxonomy tables in two layers (REGLES§2) --------------------------------

/** Overlay of a `key → value` table: entries set, catalogue entries removed. */
export interface MapOverlay {
  set?: Record<string, string>;
  removed?: string[];
}

export interface FamilyOverlay {
  /** Name or icon given to a SHIPPED family. */
  meta?: Record<string, { name?: string; icon?: string }>;
  /** Families the user made (their tags live in `tags`). */
  created?: CategoryFamily[];
  /** Shipped families the user deleted. */
  removed?: string[];
  /** `tag → family id` over the catalogue; `""` detaches the tag. */
  tags?: Record<string, string>;
}

export interface TaxonomyOverlay {
  families: FamilyOverlay;
  country_aliases: MapOverlay;
  country_tags: MapOverlay;
  ignored_countries?: string[];
  brand_aliases?: MapOverlay;
  /** Brand merges answered "Ignore", as `from → to` (TAXO§7.2). */
  ignored_brand_merges?: string[];
  not_brands?: SetOverlay;
}

/** Overlay of a set of names: added by the user, catalogue ones removed. */
export interface SetOverlay {
  added?: string[];
  removed?: string[];
}

export interface TaxonomyTables {
  families: CategoryFamily[];
  country_aliases: Record<string, string>;
  country_tags: Record<string, string>;
  /** Brand spelling (lowercased) → the brand it is filed under (TAXO§7). */
  brand_aliases?: Record<string, string>;
  /** Brand-field values that are no brand (a pack, a series), lowercased:
   * the brand is read from the car's name. */
  not_brands?: string[];
}

/** What the catalogue ships, what the user decided, what applies. */
export interface TaxonomyView {
  catalog: TaxonomyTables;
  overlay: TaxonomyOverlay;
  effective: TaxonomyTables;
}

export function getTaxonomy(): Promise<TaxonomyView> {
  return invoke<TaxonomyView>("get_taxonomy");
}

/** Writes the family decisions - no re-harmonisation. A WRITE: `invoke`, never
 * `invokeSafe`, whose fallback would report as saved what never reached the
 * disk. */
export function saveFamilyOverlay(families: FamilyOverlay): Promise<TaxonomyView> {
  return invoke<TaxonomyView>("save_family_overlay", { families });
}

/** Writes the brand decisions and re-applies them to the library: the brand
 * is decided at write time, like the country. */
export function saveBrandOverlay(aliases: MapOverlay, ignored: string[], notBrands: SetOverlay): Promise<TaxonomyView> {
  return invoke<TaxonomyView>("save_brand_overlay", { aliases, ignored, notBrands });
}

/** Writes the country decisions and re-applies them to the library: the
 * country is decided at write time. */
export function saveCountryOverlay(aliases: MapOverlay, tags: MapOverlay, ignored: string[]): Promise<TaxonomyView> {
  return invoke<TaxonomyView>("save_country_overlay", { aliases, tags, ignored });
}
