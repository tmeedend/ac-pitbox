//! Orchestration de l'harmonisation (§5/§5) : applique l'ontologie sur un
//! mod et persiste le résultat dans l'overlay. Utilisé à l'import et lors d'une
//! réapplication globale après édition des règles.

use rusqlite::Connection;

use crate::config::AppConfig;
use crate::modscan::ModKind;
use crate::overlay::{self, ModRow};
use crate::rules::{self, Harmonized, Rules};
use crate::uijson;

fn is_empty(c: Option<&str>) -> bool {
    c.is_none_or(|s| s.trim().is_empty())
}

/// Computes the harmonisation from the mod's raw values.
///
/// A car's brand is decided here, whole: a `brand_fix` rule's if one matched,
/// else the file's - then the user's merges and the case/accent folding
/// (`brands::canonical`, TAXO§7). So `Harmonized::brand` is the brand the car
/// is filed under, not only a rule's correction.
pub fn compute(
    rules: &Rules,
    kind: ModKind,
    raw_tags: &[String],
    name: &str,
    class: &str,
    native_country: Option<&str>,
    native_brand: Option<&str>,
) -> Harmonized {
    match kind {
        ModKind::Car => {
            let mut h = rules::apply_car(rules, raw_tags, name, class, is_empty(native_country));
            h.brand = h
                .brand
                .take()
                .or_else(|| native_brand.map(str::to_string))
                .and_then(|b| crate::brands::canonical(&b, &rules.brand_aliases));
            h
        }
        ModKind::Track => rules::apply_track(rules, raw_tags),
    }
}

/// Persiste l'harmonisation.
///
/// **Le seul endroit où le pays final est décidé**, et c'est ce qui rend la
/// normalisation fiable : les deux sources se rejoignent ici — le champ natif
/// du `ui_*.json` s'il est renseigné, sinon celui qu'une règle a déduit d'un
/// tag (`extraction_country`). Normaliser plus haut, à la lecture du `ui_json`,
/// aurait laissé passer le second sans y toucher.
pub fn store(
    conn: &Connection,
    id: &str,
    h: &Harmonized,
    native_country: Option<&str>,
    rules: &Rules,
) -> rusqlite::Result<()> {
    let country = final_country(rules, h, native_country);
    overlay::update_harmonization(
        conn,
        id,
        h.brand.as_deref(),
        h.car_class.as_deref(),
        h.category.as_deref(),
        &h.categories,
        country.as_deref(),
        &h.tags_from_rule,
        h.drivetrain.as_deref(),
        h.engine_pos.as_deref(),
        h.aspiration.as_deref(),
        h.engine_config.as_deref(),
        h.gearbox.as_deref(),
    )
}

/// Le pays retenu : le natif s'il est renseigné, sinon celui qu'un tag a donné,
/// puis **normalisé dans les deux cas** (`rules::canonical_country`). Extrait de
/// `store` pour être testable sans base.
pub fn final_country(rules: &Rules, h: &Harmonized, native_country: Option<&str>) -> Option<String> {
    native_country
        .filter(|c| !c.trim().is_empty())
        .map(|s| s.to_string())
        .or_else(|| h.country.clone())
        .and_then(|c| rules::canonical_country(&c, &rules.country_aliases))
}

/// Réapplique l'ontologie à tous les mods (après édition des règles).
/// Renvoie le nombre de mods retraités.
pub fn harmonize_all(conn: &Connection, cfg: &AppConfig, rules: &Rules) -> rusqlite::Result<usize> {
    harmonize_all_counting(conn, cfg, rules).map(|(n, _)| n)
}

/// `harmonize_all`, which also counts, per rule id, the mods each rule acted
/// on (`Harmonized::fired`) - the effect counters of the Rules screen
/// (REGLES§8.3), measured on the pass that stores, not on a second one.
pub fn harmonize_all_counting(conn: &Connection, cfg: &AppConfig, rules: &Rules) -> rusqlite::Result<(usize, Effects)> {
    let mods = overlay::list_mods(conn)?;
    let mut n = 0;
    let mut effects = Effects::new();
    // One transaction for the whole pass: the Rules screen re-applies on every
    // switch now, and a commit per mod is a disk sync per mod.
    let tx = conn.unchecked_transaction()?;
    for m in &mods {
        let Some((h, native_country)) = recompute_for(conn, cfg, rules, m) else {
            continue;
        };
        count(&mut effects, &h);
        match store(conn, &m.id_interne, &h, native_country.as_deref(), rules) {
            Ok(()) => n += 1,
            Err(e) => log::warn!("harmonisation of {} not stored: {e}", m.id_interne),
        }
    }
    tx.commit()?;
    Ok((n, effects))
}

/// Rule id → number of mods it acts on.
pub type Effects = std::collections::BTreeMap<String, usize>;

fn count(effects: &mut Effects, h: &Harmonized) {
    for id in &h.fired {
        *effects.entry(id.clone()).or_default() += 1;
    }
}

/// The effect counters without storing anything: what the Rules screen shows
/// when it opens.
pub fn effects(conn: &Connection, cfg: &AppConfig, rules: &Rules) -> rusqlite::Result<Effects> {
    let mut effects = Effects::new();
    for m in &overlay::list_mods(conn)? {
        if let Some((h, _)) = recompute_for(conn, cfg, rules, m) {
            count(&mut effects, &h);
        }
    }
    Ok(effects)
}

/// Recalcule l'harmonisation d'un mod en relisant sa version active (lecture
/// seule), **et rend le pays natif qu'il vient d'y lire**.
///
/// Les deux repartent ensemble parce qu'ils viennent du même fichier, et c'est
/// une correction : un second lecteur, à côté, appelait `read_car` quel que
/// soit le type. Un circuit n'ayant pas d'`ui_car.json`, son pays revenait
/// vide à chaque réharmonisation — et comme `apply_track` n'extrait aucun pays
/// d'un tag, la valeur posée à l'import était **effacée**, en silence, dès
/// qu'on touchait aux règles. Le bug était dans la duplication, pas dans le
/// `read_car` : un seul lecteur ne peut pas se tromper de type.
fn recompute_for(
    conn: &Connection,
    cfg: &AppConfig,
    rules: &Rules,
    m: &ModRow,
) -> Option<(Harmonized, Option<String>)> {
    read_and_compute(conn, cfg, rules, m).map(|(ui, h)| (h, ui.country))
}

/// Reads a mod's active version (read-only) and classifies it: what its file
/// says, and what the rules make of it. The survey needs both side by side.
pub fn read_and_compute(
    conn: &Connection,
    cfg: &AppConfig,
    rules: &Rules,
    m: &ModRow,
) -> Option<(uijson::UiInfo, Harmonized)> {
    let kind = if m.kind == "Track" {
        ModKind::Track
    } else {
        ModKind::Car
    };
    let vid = m.active_version_id.as_ref()?;
    let stored = overlay::get_version_path(conn, vid).ok().flatten()?;
    let lib = crate::libpath::resolve(cfg.library_path.as_deref(), &stored)?;
    let ui = match kind {
        ModKind::Car => uijson::read_car(&lib),
        ModKind::Track => uijson::read_track(&lib),
    }
    .unwrap_or_default();
    let class = ui.class.clone().unwrap_or_default();
    let name = ui.name.clone().unwrap_or_else(|| m.id_interne.clone());
    let h = compute(
        rules,
        kind,
        &ui.tags,
        &name,
        &class,
        ui.country.as_deref(),
        ui.brand.as_deref(),
    );
    Some((ui, h))
}

/// What the harmonisation would store for every mod under `rules`, computed
/// and NEVER stored: `mod id → classification` (rule tags, category, class,
/// final country, spec fields), serialised so two runs compare as strings.
///
/// **Plus a car's category families**, which are not stored at all - the
/// library derives them from the tags when it displays them (`families.ts`) -
/// but which the user sees change in the index. Left out, a catalogue update
/// that only moved tags between families reported "0 mods reclassified" while
/// the index had changed: measured on the dev install with `tuned` added to
/// Street.
///
/// The "diff nul" bench of REGLES§13.5: restructuring how rules are stored
/// must classify the library exactly as before, and the only proof is to
/// classify it twice and compare. A mod whose files cannot be read is left
/// out on both sides.
// The benches compare classifications; the application also needs the rules
// that acted, and calls `snapshot_fired`.
#[cfg(test)]
pub fn snapshot(
    conn: &Connection,
    cfg: &AppConfig,
    rules: &Rules,
) -> rusqlite::Result<std::collections::BTreeMap<String, String>> {
    snapshot_fired(conn, cfg, rules).map(|(lines, _)| lines)
}

/// Mod id → ids of the rules that acted on it (`Harmonized::fired`).
pub type Fired = std::collections::BTreeMap<String, Vec<String>>;

/// `snapshot`, plus which rules acted on each mod - what lets a catalogue
/// update say which rule reclassified what (REGLES§6.3), from the same pass.
pub fn snapshot_fired(
    conn: &Connection,
    cfg: &AppConfig,
    rules: &Rules,
) -> rusqlite::Result<(std::collections::BTreeMap<String, String>, Fired)> {
    let mut out = std::collections::BTreeMap::new();
    let mut fired = Fired::new();
    let owner = pitbox_catalog::taxonomy::lookup(&rules.car.category_families);
    for m in overlay::list_mods(conn)? {
        let Some((mut h, native)) = recompute_for(conn, cfg, rules, &m) else {
            continue;
        };
        let country = final_country(rules, &h, native.as_deref());
        h.tags_from_rule.sort();
        // The three tag origins the library merges (`modTags`), as it does.
        let families: std::collections::BTreeSet<&String> = if m.kind == "Car" {
            m.tags_from_mod
                .iter()
                .chain(&h.tags_from_rule)
                .chain(&m.tags_manual)
                .filter_map(|t| owner.get(&pitbox_catalog::taxonomy::family_tag(t)))
                .collect()
        } else {
            Default::default()
        };
        let line = serde_json::to_string(&(&h, country, families)).unwrap_or_default();
        fired.insert(m.id_interne.clone(), std::mem::take(&mut h.fired));
        out.insert(m.id_interne, line);
    }
    Ok((out, fired))
}

/// The mods two snapshots classify differently.
pub fn snapshot_diff(
    a: &std::collections::BTreeMap<String, String>,
    b: &std::collections::BTreeMap<String, String>,
) -> Vec<String> {
    a.keys()
        .chain(b.keys().filter(|k| !a.contains_key(*k)))
        .filter(|k| a.get(*k) != b.get(*k))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A country reaches the overlay by one of TWO paths, and they must not
    /// disagree: the `country` field of the mod's `ui_*.json`, or, when that
    /// field is empty, a tag the rules recognise (`extraction_country`). This
    /// is why the normalisation lives in `store` and nowhere else — done at
    /// `ui_json` read time, it would have left the second path untouched, and
    /// a car tagged `usa` would have been filed apart from a car declaring
    /// `U.S.A.` while both mean the same country.
    #[test]
    fn both_paths_to_a_country_land_on_the_same_spelling() {
        let rules = rules::default_rules();

        // 1. Déclaré dans le fichier, orthographe non canonique.
        let declared = compute(&rules, ModKind::Car, &[], "Any Car", "street", Some("U.S.A."), None);
        let from_file = super::final_country(&rules, &declared, Some("U.S.A."));

        // 2. Champ natif vide : c'est le tag qui parle.
        let tagged = compute(&rules, ModKind::Car, &["usa".into()], "Any Car", "street", None, None);
        let from_tag = super::final_country(&rules, &tagged, None);

        assert_eq!(from_file.as_deref(), Some("United States"), "declared in the file");
        assert_eq!(from_tag.as_deref(), Some("United States"), "deduced from a tag");
        assert_eq!(from_file, from_tag, "the two paths file the car under one country");
    }

    /// A declared country always wins over a tag: the author took the trouble
    /// to write it. The alias table normalises it, it never overrides it.
    #[test]
    fn a_declared_country_is_normalised_not_replaced_by_a_tag() {
        let rules = rules::default_rules();
        let h = compute(
            &rules,
            ModKind::Car,
            &["germany".into()],
            "Any Car",
            "street",
            Some("U.S.A."),
            None,
        );
        assert_eq!(
            super::final_country(&rules, &h, Some("U.S.A.")).as_deref(),
            Some("United States"),
            "the file said the United States, the tag does not get to say Germany"
        );
    }

    /// **A track keeps its country when the rules are re-applied.**
    ///
    /// Real bug, and a silent one: re-harmonising read the native country
    /// through `read_car` whatever the kind. A track has no `ui_car.json`, so
    /// its country came back empty — and `apply_track` extracts none from a
    /// tag — which blanked in the overlay what the import had read correctly.
    /// Nothing said so: the country simply left the sheet and the filter, the
    /// next time the rules were touched.
    ///
    /// The test goes through the whole path (library on disk, overlay, active
    /// version) because that is where the bug was: the two readers were each
    /// right on their own, and only disagreed once assembled.
    /// The bench itself: a change of rules that reclassifies a mod is seen,
    /// one that changes nothing is not.
    #[test]
    fn a_snapshot_sees_a_reclassification_and_nothing_else() {
        let base = crate::testutil::temp_dir("harmo-snapshot");
        let lib = base.join("lib");
        let dir = lib.join("cars").join("rss_car").join("v");
        std::fs::create_dir_all(dir.join("ui")).unwrap();
        std::fs::write(
            dir.join("ui").join("ui_car.json"),
            r#"{"name":"RSS Formula","class":"race","tags":["singleseater","rwd"]}"#,
        )
        .unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let now = chrono::Local::now().to_rfc3339();
        overlay::upsert_mod(&conn, "rss_car", "Car", None, Some("RSS Formula"), "h", None, &now).unwrap();
        overlay::insert_version(
            &conn,
            "rss_car_v",
            "rss_car",
            Some("1.0"),
            None,
            &now,
            &dir.to_string_lossy(),
            None,
            "sig",
            &[],
            &[],
            &[],
            &[],
            None,
        )
        .unwrap();
        overlay::set_active_version(&conn, "rss_car", "rss_car_v").unwrap();
        let cfg = AppConfig {
            library_path: Some(lib.clone()),
            ..Default::default()
        };
        let rules = rules::default_rules();
        let before = snapshot(&conn, &cfg, &rules).unwrap();
        assert_eq!(before.len(), 1, "the mod is classified");
        assert!(
            snapshot_diff(&before, &snapshot(&conn, &cfg, &rules.clone()).unwrap()).is_empty(),
            "same rules, same classification"
        );
        let mut changed = rules.clone();
        changed.car.extraction_specs.drivetrain.clear();
        assert_eq!(
            snapshot_diff(&before, &snapshot(&conn, &cfg, &changed).unwrap()),
            vec!["rss_car".to_string()],
            "losing the drivetrain extraction reclassifies the car"
        );
        // A family is derived, never stored - and still a reclassification.
        let mut refamilied = rules.clone();
        for f in &mut refamilied.car.category_families {
            f.tags.retain(|t| t != "singleseater");
        }
        assert_eq!(
            snapshot_diff(&before, &snapshot(&conn, &cfg, &refamilied).unwrap()),
            vec!["rss_car".to_string()],
            "leaving Open-wheel is a change the index shows"
        );
    }

    /// REGLES§13.5 on a REAL install: the rules as the legacy `tag-rules.json`
    /// held them, against the rules after the migration to catalogue +
    /// overlays - the library must be classified exactly the same. Works on
    /// COPIES of the config files; the install itself is only read.
    ///
    /// ```text
    /// cargo test --lib harmonize::tests::real_install_diff_nul -- --ignored --nocapture
    /// ```
    /// `PITBOX_CONFIG_DIR` overrides `%APPDATA%\com.pitbox.app`. Without a
    /// legacy file there (already migrated), the retired copy is used.
    /// What the Rules screen costs on a real library (REGLES§8.3): the
    /// counters when it opens, then a re-application as every switch does -
    /// on a copy, the real base is never written. Prints the timings and the
    /// busiest rules; `--nocapture` to read them.
    #[test]
    #[ignore = "reads the Pit Box configuration and library of this machine"]
    fn real_install_effect_counters() {
        let src = std::env::var_os("PITBOX_CONFIG_DIR")
            .map(std::path::PathBuf::from)
            .or_else(|| std::env::var_os("APPDATA").map(|d| std::path::Path::new(&d).join("com.pitbox.app")))
            .expect("PITBOX_CONFIG_DIR or APPDATA");
        let work = crate::testutil::temp_dir("real-effects");
        for f in ["config.json", "overlay.sqlite", "taxonomy.json", "rules-overlay.json"] {
            if src.join(f).is_file() {
                std::fs::copy(src.join(f), work.join(f)).unwrap();
            }
        }
        let cfg: AppConfig = serde_json::from_str(&std::fs::read_to_string(work.join("config.json")).unwrap()).unwrap();
        let conn = overlay::open(&work.join("overlay.sqlite")).unwrap();
        let rules = rules::load_from_dir(&work);

        let t = std::time::Instant::now();
        let fx = effects(&conn, &cfg, &rules).unwrap();
        println!("counters: {} rules acting, {:?}", fx.len(), t.elapsed());
        let t = std::time::Instant::now();
        let (n, fx2) = harmonize_all_counting(&conn, &cfg, &rules).unwrap();
        println!("re-application: {n} mods, {:?}", t.elapsed());
        assert_eq!(fx, fx2, "the stored pass counts what the read-only one does");
        let mut top: Vec<_> = fx.iter().collect();
        top.sort_by(|a, b| b.1.cmp(a.1));
        for (id, n) in top.iter().take(10) {
            println!("  {n:>4}  {id}");
        }
    }

    #[test]
    #[ignore = "reads the Pit Box configuration and library of this machine"]
    fn real_install_diff_nul() {
        let src = std::env::var_os("PITBOX_CONFIG_DIR")
            .map(std::path::PathBuf::from)
            .or_else(|| std::env::var_os("APPDATA").map(|d| std::path::Path::new(&d).join("com.pitbox.app")))
            .expect("PITBOX_CONFIG_DIR or APPDATA");
        let work = crate::testutil::temp_dir("real-diff-nul");
        for f in ["config.json", "overlay.sqlite", "taxonomy.json"] {
            if src.join(f).is_file() {
                std::fs::copy(src.join(f), work.join(f)).unwrap();
            }
        }
        let legacy = ["tag-rules.json", "tag-rules.pre-overlay.json"]
            .iter()
            .map(|f| src.join(f))
            .find(|p| p.is_file())
            .expect("a legacy rules file to compare against");
        std::fs::copy(&legacy, work.join("tag-rules.json")).unwrap();

        let cfg: AppConfig = serde_json::from_str(&std::fs::read_to_string(work.join("config.json")).unwrap()).unwrap();
        let conn = overlay::open(&work.join("overlay.sqlite")).unwrap();

        // Before: the legacy file as the pre-layer load used it - its lists
        // as written, the allowlist refilled when missing, the taxonomy
        // tables from the (already migrated) taxonomy overlay.
        let mut before: Rules =
            serde_json::from_str(&std::fs::read_to_string(work.join("tag-rules.json")).unwrap()).unwrap();
        let tax = crate::taxonomy::load_or_migrate(&work.join("taxonomy.json"), &before.clone());
        crate::taxonomy::apply(&mut before, &crate::taxonomy::catalog(), &tax);
        if before.track.category_allowlist.is_empty() {
            before.track.category_allowlist = rules::default_rules().track.category_allowlist;
        }

        let after = rules::load_from_dir(&work);
        let migrated: pitbox_catalog::rules::RulesOverlay =
            serde_json::from_str(&std::fs::read_to_string(work.join("rules-overlay.json")).unwrap()).unwrap();
        println!("migrated overlay: {}", serde_json::to_string(&migrated).unwrap());
        assert!(
            work.join("tag-rules.pre-overlay.json").is_file(),
            "legacy file set aside"
        );

        let a = snapshot(&conn, &cfg, &before).unwrap();
        let b = snapshot(&conn, &cfg, &after).unwrap();
        let diff = snapshot_diff(&a, &b);
        println!("{} mods classified, {} classified differently", a.len(), diff.len());
        for id in diff.iter().take(20) {
            println!("  {id}");
        }
        assert!(diff.is_empty(), "diff nul: the migration must not reclassify anything");
    }

    #[test]
    fn re_applying_the_rules_leaves_a_track_its_country() {
        let base = crate::testutil::temp_dir("harmo-track-country");
        let lib = base.join("lib");
        let dir = lib.join("tracks").join("le_lancone").join("v");
        std::fs::create_dir_all(dir.join("ui")).unwrap();
        // Verbatim du mod réel : l'auteur a voulu écrire deux entrées.
        std::fs::write(
            dir.join("ui").join("ui_track.json"),
            r#"{"name":"Le Lancone","country":"France\", \"Corsica","tags":["rally"]}"#,
        )
        .unwrap();

        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let now = chrono::Local::now().to_rfc3339();
        overlay::upsert_mod(&conn, "le_lancone", "Track", None, Some("Le Lancone"), "h", None, &now).unwrap();
        overlay::insert_version(
            &conn,
            "le_lancone_v",
            "le_lancone",
            Some("1.0"),
            None,
            &now,
            &dir.to_string_lossy(),
            None,
            "sig",
            &[],
            &[],
            &[],
            &[],
            None,
        )
        .unwrap();
        overlay::set_active_version(&conn, "le_lancone", "le_lancone_v").unwrap();

        let cfg = AppConfig {
            library_path: Some(lib.clone()),
            ..Default::default()
        };
        let rules = rules::default_rules();
        harmonize_all(&conn, &cfg, &rules).unwrap();

        let row = overlay::list_mods(&conn)
            .unwrap()
            .into_iter()
            .find(|m| m.id_interne == "le_lancone")
            .expect("the track is listed");
        assert_eq!(
            row.country.as_deref(),
            Some("France"),
            "a track keeps the country of its own ui_track.json, read up to the quote"
        );
    }

    /// A track declares a country like a car does, and writes it just as
    /// freely — hence a table shared by both families rather than one filed
    /// under `car`.
    #[test]
    fn a_track_country_goes_through_the_same_table() {
        let rules = rules::default_rules();
        let h = compute(
            &rules,
            ModKind::Track,
            &[],
            "Any Track",
            "",
            Some("Great Britain"),
            None,
        );
        assert_eq!(
            super::final_country(&rules, &h, Some("Great Britain")).as_deref(),
            Some("United Kingdom")
        );
    }
}
