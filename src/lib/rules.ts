// Pont typé vers l'ontologie de tags (§5.4).
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
export interface CarRules {
  brand_fix: BrandFix[];
  name_to_tag: NameToTag[];
  class_fix: ClassFix[];
  tag_merge: TagMerge[];
  remove: string[];
  extraction_specs: ExtractionSpecs;
  extraction_country: ExtractionCountry;
}
export interface TrackRules {
  tag_merge: TagMerge[];
  remove: string[];
}
export interface Rules {
  car: CarRules;
  track: TrackRules;
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
