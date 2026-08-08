//! Orchestration de l'harmonisation (§5.4/§5.5) : applique l'ontologie sur un
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

/// Calcule l'harmonisation à partir des valeurs brutes du mod.
pub fn compute(
    rules: &Rules,
    kind: ModKind,
    raw_tags: &[String],
    name: &str,
    class: &str,
    native_country: Option<&str>,
) -> Harmonized {
    match kind {
        ModKind::Car => rules::apply_car(rules, raw_tags, name, class, is_empty(native_country)),
        ModKind::Track => rules::apply_track(rules, raw_tags),
    }
}

/// Persiste l'harmonisation. Le pays final = natif s'il existe, sinon extrait.
pub fn store(conn: &Connection, id: &str, h: &Harmonized, native_country: Option<&str>) -> rusqlite::Result<()> {
    let country = native_country
        .filter(|c| !c.trim().is_empty())
        .map(|s| s.to_string())
        .or_else(|| h.country.clone());
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

/// Réapplique l'ontologie à tous les mods (après édition des règles).
/// Renvoie le nombre de mods retraités.
pub fn harmonize_all(conn: &Connection, cfg: &AppConfig, rules: &Rules) -> rusqlite::Result<usize> {
    let mods = overlay::list_mods(conn)?;
    let mut n = 0;
    for m in &mods {
        if reharmonize_one(conn, cfg, rules, m).is_ok() {
            n += 1;
        }
    }
    Ok(n)
}

fn reharmonize_one(conn: &Connection, cfg: &AppConfig, rules: &Rules, m: &ModRow) -> rusqlite::Result<()> {
    let Some(h) = recompute_for(conn, cfg, rules, m) else {
        return Ok(());
    };
    let native_country = native_country(conn, cfg, m);
    store(conn, &m.id_interne, &h, native_country.as_deref())
}

/// Recalcule l'harmonisation d'un mod en relisant sa version active (lecture seule).
fn recompute_for(conn: &Connection, cfg: &AppConfig, rules: &Rules, m: &ModRow) -> Option<Harmonized> {
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
    Some(compute(rules, kind, &ui.tags, &name, &class, ui.country.as_deref()))
}

fn native_country(conn: &Connection, cfg: &AppConfig, m: &ModRow) -> Option<String> {
    let vid = m.active_version_id.as_ref()?;
    let stored = overlay::get_version_path(conn, vid).ok().flatten()?;
    let lib = crate::libpath::resolve(cfg.library_path.as_deref(), &stored)?;
    uijson::read_car(&lib).and_then(|ui| ui.country)
}

/// Aperçu d'impact (§5.4) : nombre de mods dont l'harmonisation changerait
/// avec le jeu de règles candidat (comparé aux tags règle / catégorie / classe
/// actuellement stockés). Ne modifie rien.
pub fn count_affected(conn: &Connection, cfg: &AppConfig, rules: &Rules) -> rusqlite::Result<usize> {
    use std::collections::BTreeSet;
    let mods = overlay::list_mods(conn)?;
    let mut n = 0;
    for m in &mods {
        let Some(h) = recompute_for(conn, cfg, rules, m) else {
            continue;
        };
        let cand: BTreeSet<&String> = h.tags_from_rule.iter().collect();
        let cur: BTreeSet<&String> = m.tags_from_rule.iter().collect();
        if cand != cur || h.category != m.category || h.categories != m.categories || h.car_class != m.car_class {
            n += 1;
        }
    }
    Ok(n)
}
