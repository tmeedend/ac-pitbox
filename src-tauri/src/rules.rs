//! Ontologie de tags (§5.4) — **données, pas code**. Charge le jeu de règles
//! (seed embarqué `default-tag-rules.json`, puis copie éditable dans le dossier
//! de config), et applique les 6 familles + extraction de façon **non
//! destructive** : la sortie alimente l'overlay, jamais le fichier du mod.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

/// Jeu de règles par défaut, embarqué à la compilation (seed).
const DEFAULT_RULES: &str = include_str!("../rules/default-tag-rules.json");

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Rules {
    #[serde(default)]
    pub car: CarRules,
    #[serde(default)]
    pub track: TrackRules,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CarRules {
    #[serde(default)]
    pub brand_fix: Vec<BrandFix>,
    #[serde(default)]
    pub name_to_tag: Vec<NameToTag>,
    #[serde(default)]
    pub class_fix: Vec<ClassFix>,
    #[serde(default)]
    pub tag_merge: Vec<TagMerge>,
    #[serde(default)]
    pub remove: Vec<String>,
    #[serde(default)]
    pub extraction_specs: ExtractionSpecs,
    #[serde(default)]
    pub extraction_country: ExtractionCountry,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TrackRules {
    #[serde(default)]
    pub tag_merge: Vec<TagMerge>,
    #[serde(default)]
    pub remove: Vec<String>,
    /// Catégories de circuit autorisées (§5bis.2), tags `#` par ordre de
    /// priorité décroissante. Un circuit peut en porter plusieurs (celles de
    /// ses tags présentes ici) ; la première de la liste qu'il possède est sa
    /// catégorie principale. Éditable ; rempli au chargement depuis le seed
    /// embarqué si absent (config d'avant cette fonctionnalité).
    #[serde(default)]
    pub category_allowlist: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrandFix {
    pub name_contains: String,
    pub set_brand: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NameToTag {
    pub name_contains: String,
    pub add: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassFix {
    pub from: Vec<String>,
    pub set_class: Option<String>,
    #[serde(default)]
    pub add: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagMerge {
    pub from: Vec<String>,
    pub to: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExtractionSpecs {
    #[serde(default)]
    pub drivetrain: Vec<SetRule>,
    #[serde(default)]
    pub aspiration: Vec<SetRule>,
    #[serde(default)]
    pub engine_config: Vec<SetRule>,
    #[serde(default)]
    pub engine_pos: Vec<SetRule>,
    #[serde(default)]
    pub gearbox: Vec<SetRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetRule {
    pub from: Vec<String>,
    pub set: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExtractionCountry {
    #[serde(default)]
    pub map: BTreeMap<String, String>,
}

// --- Chargement / sauvegarde (fichier éditable, §12) ------------------------

fn rules_file(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    Ok(dir.join("tag-rules.json"))
}

pub fn default_rules() -> Rules {
    serde_json::from_str(DEFAULT_RULES).expect("le jeu de règles embarqué doit être valide")
}

/// Charge les règles depuis le fichier éditable ; au premier accès, sème le
/// fichier avec le jeu par défaut embarqué.
pub fn load(app: &AppHandle) -> Rules {
    let Ok(path) = rules_file(app) else {
        return default_rules();
    };
    if !path.exists() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&path, DEFAULT_RULES);
    }
    let mut rules: Rules = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(default_rules);
    // Backfill : une config antérieure à la liste blanche des catégories de
    // circuit (§5bis.2) n'a pas la clé → on la remplit depuis le seed embarqué,
    // sans réécrire le fichier (l'utilisateur peut ensuite l'éditer et sauver).
    if rules.track.category_allowlist.is_empty() {
        rules.track.category_allowlist = default_rules().track.category_allowlist;
    }
    rules
}

pub fn save(app: &AppHandle, rules: &Rules) -> Result<(), String> {
    let path = rules_file(app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(rules).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())
}

// --- Application (moteur) ---------------------------------------------------

/// Résultat de l'harmonisation d'un mod (overlay, non destructif).
#[derive(Debug, Clone, Default, Serialize)]
pub struct Harmonized {
    pub tags_from_rule: Vec<String>,
    /// Tag `#` principal = catégorie (§5bis). Pour un circuit : la 1ʳᵉ de
    /// `categories` (la plus prioritaire).
    pub category: Option<String>,
    /// Catégories de circuit (§5bis.2), multi-valué, par ordre de priorité.
    /// Vide pour une voiture (qui n'a qu'une catégorie unique via `category`).
    pub categories: Vec<String>,
    pub car_class: Option<String>,
    /// Marque corrigée (brand_fix), si applicable.
    pub brand: Option<String>,
    /// Pays extrait, uniquement si le champ natif était vide.
    pub country: Option<String>,
    pub drivetrain: Option<String>,
    pub engine_pos: Option<String>,
    pub aspiration: Option<String>,
    pub engine_config: Option<String>,
    pub gearbox: Option<String>,
}

fn norm_tag(s: &str) -> String {
    s.trim().to_lowercase()
}

/// Cherche une valeur d'extraction pour un tag dans une liste de SetRule.
fn extract(rules: &[SetRule], tag: &str) -> Option<String> {
    rules
        .iter()
        .find(|r| r.from.iter().any(|f| norm_tag(f) == tag))
        .map(|r| r.set.clone())
}

fn merge_lookup<'a>(rules: &'a [TagMerge], tag: &str) -> Option<&'a [String]> {
    rules
        .iter()
        .find(|r| r.from.iter().any(|f| norm_tag(f) == tag))
        .map(|r| r.to.as_slice())
}

/// Harmonise une voiture. `country_empty` indique si le champ natif `country`
/// est vide (auquel cas l'extraction de pays peut le remplir).
pub fn apply_car(rules: &Rules, raw_tags: &[String], name: &str, class: &str, country_empty: bool) -> Harmonized {
    let c = &rules.car;
    let name_l = name.to_lowercase();
    let mut out: BTreeSet<String> = BTreeSet::new();
    let mut h = Harmonized::default();

    // brand_fix (premier match gagne, comme l'ancien code).
    for r in &c.brand_fix {
        if name_l.contains(&r.name_contains.to_lowercase()) {
            h.brand = Some(r.set_brand.clone());
            break;
        }
    }

    // name_to_tag.
    for r in &c.name_to_tag {
        if name_l.contains(&r.name_contains.to_lowercase()) {
            out.extend(r.add.iter().cloned());
        }
    }

    // class_fix : la valeur de `class` du ui pilote classe + tags déduits.
    let class_l = norm_tag(class);
    if class_l == "race" || class_l == "street" {
        h.car_class = Some(class_l.clone());
    } else {
        for r in &c.class_fix {
            if r.from.iter().any(|f| norm_tag(f) == class_l) {
                if let Some(sc) = &r.set_class {
                    h.car_class = Some(sc.clone());
                }
                out.extend(r.add.iter().cloned());
                break;
            }
        }
    }

    // Par tag : extraction specs → pays → suppression → fusion/déduction.
    let remove: BTreeSet<String> = c.remove.iter().map(|s| norm_tag(s)).collect();
    for raw in raw_tags {
        let tag = norm_tag(raw);
        if tag.is_empty() {
            continue;
        }
        // Extraction technique (consomme le tag).
        if let Some(v) = extract(&c.extraction_specs.drivetrain, &tag) {
            h.drivetrain = Some(v);
            continue;
        }
        if let Some(v) = extract(&c.extraction_specs.aspiration, &tag) {
            h.aspiration = Some(v);
            continue;
        }
        if let Some(v) = extract(&c.extraction_specs.engine_config, &tag) {
            h.engine_config = Some(v);
            continue;
        }
        if let Some(v) = extract(&c.extraction_specs.engine_pos, &tag) {
            h.engine_pos = Some(v);
            continue;
        }
        if let Some(v) = extract(&c.extraction_specs.gearbox, &tag) {
            h.gearbox = Some(v);
            continue;
        }
        // Extraction pays (si natif vide), consomme le tag.
        if country_empty && h.country.is_none() {
            if let Some(country) = c.extraction_country.map.get(&tag) {
                h.country = Some(country.clone());
                continue;
            }
        }
        // Suppression du bruit.
        if remove.contains(&tag) {
            continue;
        }
        // Fusion / déduction.
        if let Some(to) = merge_lookup(&c.tag_merge, &tag) {
            out.extend(to.iter().cloned());
        } else {
            out.insert(tag);
        }
    }

    h.category = pick_category(&out);
    h.tags_from_rule = out.into_iter().collect();
    h
}

/// Harmonise un circuit (tag_merge + remove ; pas de classe/specs). Catégories
/// (§5bis.2) : sous-ensemble des tags présents dans la liste blanche, ordonné
/// par priorité — multi-valué.
pub fn apply_track(rules: &Rules, raw_tags: &[String]) -> Harmonized {
    let t = &rules.track;
    let remove: BTreeSet<String> = t.remove.iter().map(|s| norm_tag(s)).collect();
    let mut out: BTreeSet<String> = BTreeSet::new();
    for raw in raw_tags {
        let tag = norm_tag(raw);
        if tag.is_empty() || remove.contains(&tag) {
            continue;
        }
        if let Some(to) = merge_lookup(&t.tag_merge, &tag) {
            out.extend(to.iter().cloned());
        } else {
            out.insert(tag);
        }
    }
    // Catégories = tags ∩ liste blanche, dans l'ordre de priorité de la liste.
    // Le tag « nu » (ex. "drift") est promu en tag `#` (ex. "#drift") dans
    // tags_from_rule pour l'afficher comme catégorie (color-codée) et rester
    // cohérent avec la convention `#`.
    let categories = track_categories(&t.category_allowlist, &out);
    for cat in &categories {
        out.remove(&strip_hash(cat));
        out.insert(cat.clone());
    }
    Harmonized {
        category: categories.first().cloned(),
        categories,
        tags_from_rule: out.into_iter().collect(),
        ..Default::default()
    }
}

/// Normalise une catégorie en `#minuscule` (avec `#` de tête garanti).
fn norm_cat(s: &str) -> String {
    format!("#{}", strip_hash(s))
}

/// Retire un éventuel `#` de tête et normalise (minuscule, trim).
fn strip_hash(s: &str) -> String {
    s.trim().to_lowercase().trim_start_matches('#').to_string()
}

/// Catégories de la liste blanche présentes dans `tags` (nu ou `#`), dans
/// l'ordre de priorité de la liste blanche.
fn track_categories(allowlist: &[String], tags: &BTreeSet<String>) -> Vec<String> {
    allowlist
        .iter()
        .filter_map(|c| {
            let cat = norm_cat(c);
            if tags.contains(&cat) || tags.contains(&strip_hash(c)) {
                Some(cat)
            } else {
                None
            }
        })
        .collect()
}

/// Catégorie = premier tag `#` (convention CM, §5bis). Utilisé pour les voitures.
fn pick_category(tags: &BTreeSet<String>) -> Option<String> {
    tags.iter().find(|t| t.starts_with('#')).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track_rules(allowlist: &[&str]) -> Rules {
        Rules {
            track: TrackRules {
                category_allowlist: allowlist.iter().map(|s| s.to_string()).collect(),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn track_categories_multi_ordered_by_priority() {
        let rules = track_rules(&["#rally", "#drift", "#circuit"]);
        // Tags en désordre, casse variée, avec/sans # : ressortent ordonnés
        // selon la liste blanche (priorité), pas selon l'ordre des tags.
        let h = apply_track(&rules, &["circuit".into(), "Drift".into(), "gt".into()]);
        assert_eq!(h.categories, vec!["#drift".to_string(), "#circuit".to_string()]);
        assert_eq!(h.category.as_deref(), Some("#drift")); // principale = la + prioritaire
                                                           // Les catégories sont promues en tags `#` ; le tag hors liste est conservé.
        assert!(h.tags_from_rule.contains(&"#drift".to_string()));
        assert!(h.tags_from_rule.contains(&"#circuit".to_string()));
        assert!(h.tags_from_rule.contains(&"gt".to_string()));
        assert!(!h.tags_from_rule.contains(&"drift".to_string()));
    }

    #[test]
    fn track_without_allowed_tag_has_no_category() {
        let rules = track_rules(&["#rally", "#circuit"]);
        let h = apply_track(&rules, &["gt".into(), "fun".into()]);
        assert!(h.categories.is_empty());
        assert_eq!(h.category, None);
    }

    #[test]
    fn embedded_seed_has_track_category_allowlist() {
        // Le seed embarqué doit fournir la liste (sinon le backfill est vide).
        let r = default_rules();
        assert!(r.track.category_allowlist.contains(&"#rally".to_string()));
        assert!(r.track.category_allowlist.contains(&"#circuit".to_string()));
    }
}
