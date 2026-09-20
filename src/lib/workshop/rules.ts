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
}
export interface CarRules {
  brand_fix: BrandFix[];
  name_to_tag: NameToTag[];
  class_fix: ClassFix[];
  tag_merge: TagMerge[];
  extraction_specs: ExtractionSpecs;
  extraction_country: ExtractionCountry;
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
