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

/** Enregistre et réapplique ; renvoie le nombre de mods retraités. */
export function saveRules(rules: Rules): Promise<number> {
  return invoke<number>("save_rules", { rules });
}

/** Aperçu d'impact : nombre de mods affectés par les règles candidates. */
export function rulesImpact(rules: Rules): Promise<number> {
  return invoke<number>("rules_impact", { rules });
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
}

export interface TaxonomyTables {
  families: CategoryFamily[];
  country_aliases: Record<string, string>;
  country_tags: Record<string, string>;
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

/** Writes the country decisions and re-applies them to the library: the
 * country is decided at write time. */
export function saveCountryOverlay(aliases: MapOverlay, tags: MapOverlay, ignored: string[]): Promise<TaxonomyView> {
  return invoke<TaxonomyView>("save_country_overlay", { aliases, tags, ignored });
}
