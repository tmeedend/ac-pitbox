// Pont typé vers l'ontologie de tags (§5).
import { invoke } from "@tauri-apps/api/core";

export interface SetRule {
  from: string[];
  set: string;
}
export interface TagMerge {
  from: string[];
  to: string[];
}
export interface BrandFix {
  name_contains: string;
  set_brand: string;
}
export interface NameToTag {
  name_contains: string;
  add: string[];
}
export interface ClassFix {
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
   * (TAXO§7.2). */
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

/** Enregistre et réapplique ; renvoie le nombre de mods retraités. */
export function saveRules(rules: Rules): Promise<number> {
  return invoke<number>("save_rules", { rules });
}

/** Aperçu d'impact : nombre de mods affectés par les règles candidates. */
export function rulesImpact(rules: Rules): Promise<number> {
  return invoke<number>("rules_impact", { rules });
}

/** Writes the family table only - no re-harmonisation (TAXO§6). A WRITE, hence
 * `invoke` and not `invokeSafe`: a fallback would report as saved a table that
 * never reached the disk. Returns the table as stored. */
export function saveCategoryFamilies(families: CategoryFamily[]): Promise<CategoryFamily[]> {
  return invoke<CategoryFamily[]>("save_category_families", { families });
}

/** The family table as the app ships it, for "Restore". */
export function defaultCategoryFamilies(): Promise<CategoryFamily[]> {
  return invoke<CategoryFamily[]>("default_category_families");
}

/** Writes the country aliases and re-applies them to the library - the
 * country is decided at write time, so an alias changes stored values. A WRITE:
 * `invoke`, never `invokeSafe`. Returns the number of mods processed. */
export function saveCountryAliases(aliases: CountryAliases): Promise<number> {
  return invoke<number>("save_country_aliases", { aliases });
}

/** The aliases as the app ships them, for "Restore". */
export function defaultCountryAliases(): Promise<CountryAliases> {
  return invoke<CountryAliases>("default_country_aliases");
}
