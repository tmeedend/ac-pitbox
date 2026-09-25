//! Tag ontology (§5) — **data, not code**. Loads the rule set (embedded seed
//! `default-tag-rules.json`, then an editable copy in the config directory) and
//! applies it **non-destructively**: the output feeds the overlay, never the
//! mod's own file.
//!
//! The vocabulary is a **closed allowlist**, as the spec always described it: a
//! tag no rule is able to produce is not promoted to a rule tag at all — it
//! simply stays the mod's raw file tag, displayed as such and hidden with the
//! rest of them. The engine used to insert unknown tags verbatim instead, so
//! the green "rule" badge meant "survived the pipeline", not "recognised"; that
//! leak is why the seed carried a 314-entry `remove` blacklist to patch it back
//! shut, and why one `#` tag invented by a single mod author became a category
//! in the library filter. Recognition is now the rule, the blacklist is gone,
//! and adding a merge rule is what extends the vocabulary.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

/// Jeu de règles par défaut, embarqué à la compilation (seed).
pub const DEFAULT_RULES: &str = include_str!("../rules/default-tag-rules.json");

/// Harmonisation engine version. Bumped whenever the same rules would now
/// yield a different result — the overlay then holds a stale computation, and
/// the startup catch-up in `lib.rs` recomputes it. Same need, and same remedy,
/// as `preview::CONVERTER_VERSION`: a cached result has to be told when the
/// code that produced it has moved on.
///
/// 2 — closed vocabulary: unknown tags are no longer promoted (§5).
/// 3 — country aliases: a declared country is normalised on the way in (§5).
/// 4 — a track keeps its country when the rules are re-applied: the second
///     reader used `read_car` whatever the kind, so every re-harmonisation
///     silently blanked it. The bump is what gives those tracks their country
///     back, from their own `ui_track.json`.
/// 5 — countries fold case, accents and ISO codes onto the game's spelling
///     (TAXO§7.1).
/// 6 — brands too: the user's merges, then case, spaces and accents onto the
///     library's most used spelling (TAXO§7, `brands.rs`).
pub const ENGINE_VERSION: u32 = 6;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Rules {
    #[serde(default)]
    pub car: CarRules,
    #[serde(default)]
    pub track: TrackRules,
    /// Hors des deux familles, et à dessein : un circuit déclare un pays comme
    /// une voiture, et il l'écrit tout aussi librement.
    #[serde(default)]
    pub country_aliases: CountryAliases,
    /// Brand spelling (lowercased) → the brand it is filed under (TAXO§7,
    /// `brands.rs`). From the taxonomy catalogue and overlay, like the
    /// country aliases.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub brand_aliases: BTreeMap<String, String>,
    /// Brand-field values that are no brand (a pack, a series, a modder),
    /// lowercased: the brand is read from the car's name (`brands.rs`).
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub not_brands: BTreeSet<String>,
}

/// Comment un pays s'écrit, et le nom sous lequel on le range.
///
/// **À ne pas confondre avec `extraction_country`**, qui lui ressemble et ne
/// répond pas à la même question. L'extraction DEVINE un pays absent à partir
/// d'un tag (« cette voiture porte le tag `germany`, donc elle est allemande »)
/// et ne s'applique que si le champ natif est vide. Les alias, eux,
/// NORMALISENT un pays déjà déclaré (« ce mod dit `U.S.A.`, c'est-à-dire
/// United States ») et s'appliquent toujours. Les fusionner laisserait un tag
/// réécrire un pays que l'auteur a pris la peine de déclarer.
///
/// Clés en minuscules, valeurs telles qu'on veut les lire à l'écran. Le jeu de
/// départ est **mesuré** — voir `rules/default-tag-rules.json`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CountryAliases {
    #[serde(default)]
    pub map: BTreeMap<String, String>,
    /// Values unknown to the game that the user chose to leave as they are
    /// (TAXO§7.2, "Ignore"): the Countries tab stops offering to attach them.
    /// Remembered, and reversible from the tab.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ignored: Vec<String>,
}

/// Le pays tel qu'on le range, à partir de ce que le mod a écrit.
///
/// Deux passes, et une seule table — celle des règles, donc éditable :
///
///  1. **la valeur cassée se coupe au premier guillemet.** Relevé sur
///     `le_lancone`, dont le `ui_track.json` porte
///     `"country": "France\", \"Corsica"` : l'auteur a voulu écrire deux
///     entrées et en a produit une seule, guillemets compris. Un nom de pays
///     n'en contient jamais, donc ce qui précède le premier est le nom.
///     **Jamais à la virgule**, qui paraîtrait faire pareil : la table du jeu
///     en contient (`Tanzania, {United Republic of}`) et les couper les
///     rendrait introuvables ;
///  2. **l'alias**, s'il y en a un pour cette orthographe.
///
/// Sans alias, la valeur ressort telle que l'auteur l'a écrite, aux espaces de
/// bord près : on ne corrige que ce qu'on sait corriger.
pub fn canonical_country(raw: &str, aliases: &CountryAliases) -> Option<String> {
    crate::nationalities::with_known(|known| canonical_country_in(raw, aliases, known))
}

/// Case, spaces and accents folded away: `JAPAN`, ` japan ` and `Japán` are
/// the same string, and merging them is not a decision (TAXO§7.1).
pub(crate) fn fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.to_lowercase().chars() {
        let base = match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' => 'a',
            'ç' | 'ć' | 'č' => 'c',
            'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ě' => 'e',
            'ì' | 'í' | 'î' | 'ï' | 'ī' => 'i',
            'ñ' | 'ń' | 'ň' => 'n',
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' => 'o',
            'ù' | 'ú' | 'û' | 'ü' | 'ū' | 'ů' => 'u',
            'ý' | 'ÿ' => 'y',
            'š' | 'ś' => 's',
            'ž' | 'ź' | 'ż' => 'z',
            'ř' => 'r',
            other => other,
        };
        out.push(base);
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `canonical_country` against an explicit game table — the testable half.
///
/// After the aliases (a decision the user can see and edit), two merges that
/// are NOT decisions (TAXO§7.1): a name equal to one of the game's once case,
/// spaces and accents are folded takes the game's spelling, and a valid ISO
/// code (`JP`, `JPN`) takes the country's name — a normalised table, not a
/// judgement. Anything else is kept as written: guessing a closer country is
/// what the Countries tab proposes, never what the harmonisation decides.
pub fn canonical_country_in(
    raw: &str,
    aliases: &CountryAliases,
    known: &[crate::nationalities::Nationality],
) -> Option<String> {
    let cleaned = cleaned_country(raw)?;
    if let Some(canonical) = aliases.map.get(&cleaned.to_lowercase()) {
        return Some(canonical.clone());
    }
    let folded = fold(&cleaned);
    let upper = cleaned.to_uppercase();
    let hit = known.iter().find(|n| {
        fold(&n.name) == folded
            || (upper.len() == 3 && n.code == upper)
            || (upper.len() == 2 && n.iso2.as_deref() == Some(upper.as_str()))
    });
    Some(hit.map(|n| n.name.clone()).unwrap_or(cleaned))
}

/// The value cut at its first quote and trimmed, `None` when nothing is left.
fn cleaned_country(raw: &str) -> Option<String> {
    let quote = raw.find('"');
    let cleaned = match quote {
        // `> 0` : un nom qui COMMENCE par un guillemet ne laisse rien à lire.
        Some(i) if i > 0 => &raw[..i],
        Some(_) => "",
        None => raw,
    }
    .trim();
    (!cleaned.is_empty()).then(|| cleaned.to_string())
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
    pub extraction_specs: ExtractionSpecs,
    #[serde(default)]
    pub extraction_country: ExtractionCountry,
    /// Category families (INDEX§6.1, TAXO§7.3): which tags make a
    /// car a "Race" or a "Classic". **Read by the library, never by this
    /// engine** — a family is an index over tags, not a rule, so it has no
    /// output to write into the overlay and no reason to bump `ENGINE_VERSION`.
    /// It lives here because this is the one table the user can already edit,
    /// and the one the Categories tab will edit.
    #[serde(default)]
    pub category_families: Vec<CategoryFamily>,
}

/// One family of car categories — defined with the merge of the layers in
/// the `pitbox-catalog` crate, shared with `rules-tool`.
pub use pitbox_catalog::taxonomy::CategoryFamily;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TrackRules {
    #[serde(default)]
    pub tag_merge: Vec<TagMerge>,
    /// Catégories de circuit autorisées (§5), tags `#` par ordre de
    /// priorité décroissante. Un circuit peut en porter plusieurs (celles de
    /// ses tags présentes ici) ; la première de la liste qu'il possède est sa
    /// catégorie principale. Éditable ; rempli au chargement depuis le seed
    /// embarqué si absent (config d'avant cette fonctionnalité).
    #[serde(default)]
    pub category_allowlist: Vec<String>,
}

/// The list-rule types, with their stable id (REGLES§4) — defined in the
/// `pitbox-catalog` crate, shared with `rules-tool`.
pub use pitbox_catalog::rules::{BrandFix, ClassFix, NameToTag, RulesOverlay, SetRule, TagMerge};

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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExtractionCountry {
    #[serde(default)]
    pub map: BTreeMap<String, String>,
}

// --- Chargement / sauvegarde (fichier éditable, §12) ------------------------

/// The file pre-layer versions copied every rule into. Read once, to migrate
/// the user's decisions out of it, then set aside (`load_from_dir`).
const LEGACY_FILE: &str = "tag-rules.json";
/// Where the legacy file goes once migrated - kept, never deleted.
const RETIRED_FILE: &str = "tag-rules.pre-overlay.json";

pub fn config_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().app_config_dir().map_err(|e| e.to_string())
}

/// The legacy rules file, when there is one and it reads. One that exists but
/// does not parse is logged and left where it is: it is not migrated, so it
/// must not be retired either.
pub(crate) fn read_legacy(dir: &std::path::Path) -> Option<Rules> {
    let text = std::fs::read_to_string(dir.join(LEGACY_FILE)).ok()?;
    match serde_json::from_str(&text) {
        Ok(r) => Some(r),
        Err(e) => {
            log::warn!("{LEGACY_FILE} unreadable, not migrated: {e}");
            None
        }
    }
}

/// The overlay on the taxonomy tables as stored — migrated the first time.
pub fn load_taxonomy(app: &AppHandle) -> crate::taxonomy::TaxonomyOverlay {
    match config_dir(app) {
        Ok(dir) => crate::taxonomy::load_or_migrate(&dir.join("taxonomy.json"), &read_legacy(&dir).unwrap_or_default()),
        Err(e) => {
            log::warn!("taxonomy overlay path: {e}");
            crate::taxonomy::TaxonomyOverlay::default()
        }
    }
}

/// Writes the taxonomy overlay (normalised against the catalogue) and returns
/// the rules as they now apply.
pub fn save_taxonomy(
    app: &AppHandle,
    overlay: crate::taxonomy::TaxonomyOverlay,
) -> Result<(Rules, crate::taxonomy::TaxonomyOverlay), String> {
    let overlay = overlay.normalized(&crate::taxonomy::catalog());
    crate::taxonomy::save(&config_dir(app)?.join("taxonomy.json"), &overlay)?;
    Ok((load(app), overlay))
}

/// The shipped rules — the catalogue: `default-tag-rules.json` for the list
/// rules, plus the taxonomy catalogue that lives in its own file (`taxonomy.rs`
/// says why).
///
/// The catalogue IN FORCE: the embedded one, or the previous one when the user
/// went back to it (`catalog_update`, REGLES§6.4).
pub fn default_rules() -> Rules {
    catalog_from(&crate::catalog_update::active())
        .or_else(|| catalog_from(&crate::catalog_update::embedded()))
        .expect("the embedded rules must be valid")
}

/// A catalogue from its two texts, `None` when they do not parse.
pub fn catalog_from(texts: &crate::catalog_update::CatalogTexts) -> Option<Rules> {
    let mut r: Rules = serde_json::from_str(&texts.rules).ok()?;
    let t: crate::taxonomy::TaxonomyTables = serde_json::from_str(&texts.taxonomy).ok()?;
    r.car.category_families = t.families;
    r.country_aliases.map = t.country_aliases;
    r.car.extraction_country.map = t.country_tags;
    r.brand_aliases = t.brand_aliases;
    r.not_brands = t.not_brands;
    Some(r)
}

/// The rules as they apply: the catalogue, with the user's decisions on top.
pub fn load(app: &AppHandle) -> Rules {
    match config_dir(app) {
        Ok(dir) => load_from_dir(&dir),
        Err(e) => {
            log::warn!("config dir unavailable, embedded rules used: {e}");
            default_rules()
        }
    }
}

/// `load`, from an explicit configuration directory - what the tests and the
/// diff-nul bench (`harmonize::snapshot`) use to load a real user's rules
/// without a running application.
///
/// **Nothing of the catalogue is copied to the user any more** (REGLES§2): the
/// rules start from the embedded catalogue and receive two overlays, the
/// taxonomy tables' (`taxonomy.json`) and the list rules' (`rules-overlay.json`).
/// Both are migrated once from the legacy `tag-rules.json`, which is then set
/// aside as `tag-rules.pre-overlay.json` - kept, never deleted, and no longer
/// read. It is retired only once BOTH overlays are on disk: a migration that
/// failed to write runs again at the next load, from the same file.
pub fn load_from_dir(dir: &std::path::Path) -> Rules {
    load_from_dir_on(dir, &default_rules())
}

/// `load_from_dir` on an explicit catalogue - how a catalogue update classifies
/// the library under the previous one to measure what changed.
pub fn load_from_dir_on(dir: &std::path::Path, catalog: &Rules) -> Rules {
    let legacy = read_legacy(dir);
    let tax_path = dir.join("taxonomy.json");
    let lists_path = dir.join("rules-overlay.json");
    if legacy.is_some() && tax_path.is_file() && lists_path.is_file() {
        // Both overlays exist: this file is not read, it came back AFTER the
        // migration. Seen on the dev machine (2026-09-23): an older copy of
        // `tag-rules.json` reappeared twice, from nothing this application
        // writes. Said in the log so the next occurrence can be dated.
        log::warn!("{LEGACY_FILE} reappeared after migration; set aside, not read");
    }
    let mut rules = catalog.clone();

    let tax = crate::taxonomy::load_or_migrate(&tax_path, legacy.as_ref().unwrap_or(&Rules::default()));
    crate::taxonomy::apply(&mut rules, &crate::taxonomy::tables_of(catalog), &tax);

    let lists = crate::rule_overlay::load_or_migrate(&lists_path, legacy.as_ref());
    let lists = crate::rule_overlay::normalized(lists, catalog);
    crate::rule_overlay::apply(&mut rules, catalog, &lists);

    if legacy.is_some() && tax_path.is_file() && lists_path.is_file() {
        if let Err(e) = std::fs::rename(dir.join(LEGACY_FILE), retired_path(dir)) {
            log::warn!("{LEGACY_FILE} migrated but not set aside, it will be read again: {e}");
        }
    }
    rules
}

/// Where to set the legacy file aside. Never onto an earlier one: on Windows a
/// rename replaces its target, and "kept, never deleted" would quietly become
/// "the last one kept".
fn retired_path(dir: &std::path::Path) -> PathBuf {
    let first = dir.join(RETIRED_FILE);
    if !first.exists() {
        return first;
    }
    (2..)
        .map(|n| dir.join(format!("tag-rules.pre-overlay-{n}.json")))
        .find(|p| !p.exists())
        .expect("an unused name")
}

/// The list-rules overlay as the Rules screen edits it: the user's decisions
/// only, normalised against the catalogue in force.
pub fn load_rules_overlay(app: &AppHandle) -> Result<RulesOverlay, String> {
    let dir = config_dir(app)?;
    let o = crate::rule_overlay::load_or_migrate(&dir.join("rules-overlay.json"), read_legacy(&dir).as_ref());
    Ok(crate::rule_overlay::normalized(o, &default_rules()))
}

/// Writes what the Rules screen decided - decisions on the catalogue, never a
/// copy of it (REGLES§2) - and returns it normalised.
pub fn save_rules_overlay(app: &AppHandle, o: RulesOverlay) -> Result<RulesOverlay, String> {
    let o = crate::rule_overlay::normalized(o, &default_rules());
    crate::rule_overlay::save(&config_dir(app)?.join("rules-overlay.json"), &o)?;
    Ok(o)
}

// --- Application (moteur) ---------------------------------------------------

/// Résultat de l'harmonisation d'un mod (overlay, non destructif).
#[derive(Debug, Clone, Default, Serialize)]
pub struct Harmonized {
    /// Ids of the rules that acted on this mod (a first-match rule that
    /// matched, every name → tag that matched, a track category found) - what
    /// the effect counters of the Rules screen count (REGLES§8.3). Never
    /// stored, never compared: it says who produced the result, it is not
    /// part of it.
    #[serde(skip)]
    pub fired: Vec<String>,
    /// Raw tags no rule recognised - dropped from the closed vocabulary (§5).
    /// Never stored: what the survey counts, to tell which tags the catalogue
    /// should learn next (`survey.rs`).
    #[serde(skip)]
    pub unrecognized: Vec<String>,
    pub tags_from_rule: Vec<String>,
    /// Tag `#` principal = catégorie (§5). Pour un circuit : la 1ʳᵉ de
    /// `categories` (la plus prioritaire).
    pub category: Option<String>,
    /// Catégories de circuit (§5), multi-valué, par ordre de priorité.
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
fn extract<'a>(rules: &'a [SetRule], tag: &str) -> Option<&'a SetRule> {
    rules.iter().find(|r| r.from.iter().any(|f| norm_tag(f) == tag))
}

fn merge_lookup<'a>(rules: &'a [TagMerge], tag: &str) -> Option<&'a TagMerge> {
    rules.iter().find(|r| r.from.iter().any(|f| norm_tag(f) == tag))
}

/// Notes that a rule acted (`Harmonized::fired`).
fn fired(h: &mut Harmonized, id: &Option<String>) {
    if let Some(id) = id {
        if !h.fired.contains(id) {
            h.fired.push(id.clone());
        }
    }
}

/// Closed vocabulary for cars: every tag some rule is able to produce.
///
/// Deriving it from the rule outputs rather than keeping a separate list is
/// what makes it self-maintaining — writing a rule *is* declaring its
/// vocabulary, so the two can never drift apart. An incoming tag is kept only
/// if it belongs here, which also covers the case no merge rule can: a file
/// already spelling the canonical form (`#gt3`), whose left-hand side no rule
/// lists because there is nothing to correct.
fn known_car_tags(c: &CarRules) -> BTreeSet<String> {
    let mut known: BTreeSet<String> = BTreeSet::new();
    for r in &c.tag_merge {
        known.extend(r.to.iter().map(|t| norm_tag(t)));
    }
    for r in &c.name_to_tag {
        known.extend(r.add.iter().map(|t| norm_tag(t)));
    }
    for r in &c.class_fix {
        known.extend(r.add.iter().map(|t| norm_tag(t)));
    }
    known
}

/// Same for tracks, plus the category allowlist — under **both** spellings.
///
/// Six seeded categories (`#oval`, `#drag`, `#karting`, `#rallycross`, `#test`,
/// `#touge`) are produced by no merge rule whatsoever: they only ever worked
/// because unknown tags used to pass straight through, the bare `oval` reaching
/// `track_categories` untouched. Accepting the `#` form alone would have
/// silently stripped those tracks of their category.
fn known_track_tags(t: &TrackRules) -> BTreeSet<String> {
    let mut known: BTreeSet<String> = BTreeSet::new();
    for r in &t.tag_merge {
        known.extend(r.to.iter().map(|s| norm_tag(s)));
    }
    for c in &t.category_allowlist {
        known.insert(strip_hash(c));
        known.insert(norm_cat(c));
    }
    known
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
            fired(&mut h, &r.id);
            break;
        }
    }

    // name_to_tag.
    for r in &c.name_to_tag {
        if name_l.contains(&r.name_contains.to_lowercase()) {
            out.extend(r.add.iter().cloned());
            fired(&mut h, &r.id);
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
                fired(&mut h, &r.id);
                break;
            }
        }
    }

    // Par tag : extraction specs → pays → fusion/déduction.
    let known = known_car_tags(c);
    for raw in raw_tags {
        let tag = norm_tag(raw);
        if tag.is_empty() {
            continue;
        }
        // Extraction technique (consomme le tag).
        let s = &c.extraction_specs;
        if let Some(r) = extract(&s.drivetrain, &tag) {
            h.drivetrain = Some(r.set.clone());
            fired(&mut h, &r.id);
            continue;
        }
        if let Some(r) = extract(&s.aspiration, &tag) {
            h.aspiration = Some(r.set.clone());
            fired(&mut h, &r.id);
            continue;
        }
        if let Some(r) = extract(&s.engine_config, &tag) {
            h.engine_config = Some(r.set.clone());
            fired(&mut h, &r.id);
            continue;
        }
        if let Some(r) = extract(&s.engine_pos, &tag) {
            h.engine_pos = Some(r.set.clone());
            fired(&mut h, &r.id);
            continue;
        }
        if let Some(r) = extract(&s.gearbox, &tag) {
            h.gearbox = Some(r.set.clone());
            fired(&mut h, &r.id);
            continue;
        }
        // Extraction pays (si natif vide), consomme le tag.
        if country_empty && h.country.is_none() {
            if let Some(country) = c.extraction_country.map.get(&tag) {
                h.country = Some(country.clone());
                continue;
            }
        }
        // Merge / deduction. A tag outside the vocabulary is deliberately
        // dropped here rather than kept: it is still the mod's raw file tag,
        // so nothing is lost — it is only denied the rule badge it never
        // earned, and denied becoming a category nobody declared.
        if let Some(r) = merge_lookup(&c.tag_merge, &tag) {
            out.extend(r.to.iter().cloned());
            fired(&mut h, &r.id);
        } else if known.contains(&tag) {
            out.insert(tag);
        } else {
            h.unrecognized.push(tag);
        }
    }

    h.category = pick_category(&out);
    h.tags_from_rule = out.into_iter().collect();
    h
}

/// Harmonises a track (tag_merge only; no class, no specs). Categories
/// (§5): the subset of its tags found in the allowlist, ordered by
/// priority — multi-valued.
pub fn apply_track(rules: &Rules, raw_tags: &[String]) -> Harmonized {
    let t = &rules.track;
    let known = known_track_tags(t);
    let mut out: BTreeSet<String> = BTreeSet::new();
    let mut h = Harmonized::default();
    for raw in raw_tags {
        let tag = norm_tag(raw);
        if tag.is_empty() {
            continue;
        }
        if let Some(r) = merge_lookup(&t.tag_merge, &tag) {
            out.extend(r.to.iter().cloned());
            fired(&mut h, &r.id);
        } else if known.contains(&tag) {
            out.insert(tag);
        } else {
            h.unrecognized.push(tag);
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
        // An allowlist entry has no id: its name is its key.
        fired(&mut h, &Some(format!("track-category:{cat}")));
    }
    Harmonized {
        category: categories.first().cloned(),
        categories,
        tags_from_rule: out.into_iter().collect(),
        ..h
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

/// Catégorie = premier tag `#` (convention CM, §5). Utilisé pour les voitures.
fn pick_category(tags: &BTreeSet<String>) -> Option<String> {
    tags.iter().find(|t| t.starts_with('#')).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn merge(from: &[&str], to: &[&str]) -> TagMerge {
        TagMerge {
            id: None,
            from: from.iter().map(|s| s.to_string()).collect(),
            to: to.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn track_rules(allowlist: &[&str]) -> Rules {
        Rules {
            track: TrackRules {
                category_allowlist: allowlist.iter().map(|s| s.to_string()).collect(),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn car_rules(tag_merge: Vec<TagMerge>) -> Rules {
        Rules {
            car: CarRules {
                tag_merge,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn tags(raw: &[&str]) -> Vec<String> {
        raw.iter().map(|s| s.to_string()).collect()
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
        assert!(!h.tags_from_rule.contains(&"drift".to_string()));
    }

    // §5 — Closed vocabulary: a tag no rule can produce is not promoted to a
    // rule tag. It is not lost, it stays the mod's raw file tag.
    #[test]
    fn track_tag_outside_vocabulary_is_not_promoted() {
        let rules = track_rules(&["#circuit"]);
        let h = apply_track(&rules, &tags(&["circuit", "gt"]));
        assert!(
            h.tags_from_rule.contains(&"#circuit".to_string()),
            "known category kept"
        );
        assert!(
            !h.tags_from_rule.contains(&"gt".to_string()),
            "unknown tag not promoted"
        );
    }

    // §5 — Six seeded categories (#oval, #drag, #karting, #rallycross,
    // #test, #touge) are the output of no merge rule at all: the allowlist is
    // their only declaration. Recognising the bare spelling is what keeps them
    // working now that unknown tags no longer pass through.
    #[test]
    fn track_allowlist_category_survives_without_merge_rule() {
        let rules = track_rules(&["#oval"]);
        let h = apply_track(&rules, &tags(&["oval"]));
        assert_eq!(
            h.categories,
            vec!["#oval".to_string()],
            "bare allowlist tag still promoted"
        );
        assert_eq!(h.category.as_deref(), Some("#oval"));
    }

    // §5 — Same rule on the car side.
    #[test]
    fn car_tag_outside_vocabulary_is_not_promoted() {
        let rules = car_rules(vec![merge(&["gt3"], &["#gt3"])]);
        let h = apply_car(&rules, &tags(&["gt3", "wobbly"]), "Some Car", "", false);
        assert!(h.tags_from_rule.contains(&"#gt3".to_string()), "merged tag kept");
        assert!(
            !h.tags_from_rule.contains(&"wobbly".to_string()),
            "unknown tag not promoted"
        );
    }

    // §5 — An unknown `#` tag used to become the car's category, which is
    // what filled the library filter with one-off categories invented by a
    // single mod author. It must now leave the car without one.
    #[test]
    fn car_unknown_hash_tag_is_not_a_category() {
        let rules = car_rules(vec![merge(&["gt3"], &["#gt3"])]);
        let h = apply_car(&rules, &tags(&["#homemade"]), "Some Car", "", false);
        assert_eq!(h.category, None, "undeclared category refused");
        assert!(h.tags_from_rule.is_empty(), "nothing promoted");
    }

    // §5 — A file already spelling the canonical form matches no merge rule
    // (there is nothing to correct), so only the vocabulary can vouch for it.
    #[test]
    fn car_canonical_tag_survives_without_merge_rule() {
        let rules = car_rules(vec![merge(&["gt3"], &["#gt3"])]);
        let h = apply_car(&rules, &tags(&["#gt3"]), "Some Car", "", false);
        assert_eq!(
            h.category.as_deref(),
            Some("#gt3"),
            "canonical tag recognised as itself"
        );
    }

    #[test]
    fn track_without_allowed_tag_has_no_category() {
        let rules = track_rules(&["#rally", "#circuit"]);
        let h = apply_track(&rules, &["gt".into(), "fun".into()]);
        assert!(h.categories.is_empty());
        assert_eq!(h.category, None);
    }

    /// INDEX§8: without a family table there is no category index. The
    /// backfill in `load` copies it from here, so an empty seed would leave
    /// every existing install without one.
    #[test]
    fn embedded_seed_has_category_families() {
        let fams = default_rules().car.category_families;
        assert!(fams.iter().any(|f| f.id == "race"), "shipped families present");
        assert!(fams.iter().all(|f| !f.tags.is_empty()), "no family without a tag");
    }

    fn nat(code: &str, iso2: Option<&str>, name: &str) -> crate::nationalities::Nationality {
        crate::nationalities::Nationality {
            code: code.into(),
            name: name.into(),
            flag: None,
            iso2: iso2.map(str::to_string),
        }
    }

    /// TAXO§7.1: case, spaces, accents and a valid ISO code merge on their own;
    /// nothing else does.
    #[test]
    fn a_country_folds_onto_the_game_spelling_and_nothing_further() {
        let known = [
            nat("JPN", Some("JP"), "Japan"),
            nat("CIV", Some("CI"), "Côte d'Ivoire"),
            nat("SCT", None, "Scotland"),
        ];
        let a = CountryAliases::default();
        for written in ["JAPAN", "  japan ", "JP", "jpn"] {
            assert_eq!(
                canonical_country_in(written, &a, &known).as_deref(),
                Some("Japan"),
                "{written}"
            );
        }
        assert_eq!(
            canonical_country_in("cote d'ivoire", &a, &known).as_deref(),
            Some("Côte d'Ivoire")
        );
        assert_eq!(
            canonical_country_in("Nippon", &a, &known).as_deref(),
            Some("Nippon"),
            "a closer country is proposed by the tab, never decided here"
        );
        assert_eq!(
            canonical_country_in("SC", &a, &known).as_deref(),
            Some("SC"),
            "no ISO code, no code merge"
        );
    }

    #[test]
    fn embedded_seed_has_track_category_allowlist() {
        // Le seed embarqué doit fournir la liste (sinon le backfill est vide).
        let r = default_rules();
        assert!(r.track.category_allowlist.contains(&"#rally".to_string()));
        assert!(r.track.category_allowlist.contains(&"#circuit".to_string()));
    }

    /// The seeded aliases are the ones the survey found, and they are what
    /// makes 80 of 395 mods land on a flag instead of none.
    #[test]
    fn the_seeded_country_aliases_cover_what_the_survey_found() {
        let a = default_rules().country_aliases;
        for written in ["U.S.A.", "USA", "United States of America"] {
            assert_eq!(
                canonical_country(written, &a).as_deref(),
                Some("United States"),
                "{written} is how mods spell the United States"
            );
        }
        assert_eq!(
            canonical_country("Great Britain", &a).as_deref(),
            Some("United Kingdom")
        );
    }

    /// Assetto Corsa gives Scotland, England, Wales and Northern Ireland a flag
    /// of their own: folding them into the United Kingdom would replace one
    /// correct flag with another, and lose what the mod's author wrote.
    #[test]
    fn the_british_nations_keep_their_own_name() {
        let a = default_rules().country_aliases;
        for kept in ["Scotland", "England", "Wales", "Northern Ireland"] {
            assert_eq!(
                canonical_country(kept, &a).as_deref(),
                Some(kept),
                "{kept} is a country here"
            );
        }
    }

    /// Real file, `le_lancone`: its `ui_track.json` carries
    /// `"country": "France\", \"Corsica"` - the author meant two entries and
    /// wrote one, quotes included.
    #[test]
    fn a_country_broken_by_its_author_is_read_up_to_the_quote() {
        let a = default_rules().country_aliases;
        assert_eq!(canonical_country("France\", \"Corsica", &a).as_deref(), Some("France"));
        assert_eq!(canonical_country("\"", &a), None, "nothing to read before the quote");
    }

    /// **The rule is the quote, never the comma** - and this is the whole point
    /// of the test. Cutting at the comma would look like it does the same job,
    /// and would make unreachable the two names the game's own table spells
    /// that way.
    #[test]
    fn a_country_is_never_cut_at_a_comma() {
        let a = default_rules().country_aliases;
        for kept in ["Tanzania, {United Republic of}", "Micronesia, {Federated States of}"] {
            assert_eq!(
                canonical_country(kept, &a).as_deref(),
                Some(kept),
                "the game writes it so"
            );
        }
    }

    /// An unknown spelling comes back as the author wrote it: we correct what we
    /// know how to correct, and invent nothing.
    #[test]
    fn an_unknown_country_keeps_the_spelling_of_its_author() {
        let a = default_rules().country_aliases;
        assert_eq!(canonical_country("  Freedonia ", &a).as_deref(), Some("Freedonia"));
        assert_eq!(canonical_country("   ", &a), None);
    }

    /// REGLES§8.3: the engine says which rules acted on a mod - the ids the
    /// effect counters count - without that changing what it classifies.
    #[test]
    fn the_engine_names_the_rules_that_acted() {
        let rules = default_rules();
        let h = apply_car(&rules, &["rwd".into()], "Bayro M3 E30", "street", false);
        assert!(h.fired.iter().any(|id| id == "pitbox.brand.bayro"), "{:?}", h.fired);
        assert!(
            h.fired.iter().any(|id| id.starts_with("pitbox.spec-drivetrain.")),
            "{:?}",
            h.fired
        );
        let t = apply_track(&rules, &["hillclimb".into()]);
        assert!(
            t.fired.contains(&"track-category:#hillclimb".to_string()),
            "{:?}",
            t.fired
        );
    }
}
