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
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(default_rules)
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
    /// Tag `#` principal = catégorie (§5bis).
    pub category: Option<String>,
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
pub fn apply_car(
    rules: &Rules,
    raw_tags: &[String],
    name: &str,
    class: &str,
    country_empty: bool,
) -> Harmonized {
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

/// Harmonise un circuit (tag_merge + remove ; pas de classe/specs).
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
    Harmonized {
        category: pick_category(&out),
        tags_from_rule: out.into_iter().collect(),
        ..Default::default()
    }
}

/// Catégorie = premier tag `#` (convention CM, §5bis).
fn pick_category(tags: &BTreeSet<String>) -> Option<String> {
    tags.iter().find(|t| t.starts_with('#')).cloned()
}
