//! The measurement of FICHE§10, replayable: every car of this machine's
//! library and game folder, read by the same readers as the app, dumped one
//! JSON line per car for counting. Reads only — the overlay is a copy.
//!
//! ```text
//! cargo test --lib techsheet::measure -- --ignored --nocapture
//! ```
//! `PITBOX_CONFIG_DIR` overrides `%APPDATA%\com.pitbox.app`; `PITBOX_MEASURE_OUT`
//! is the output file (default: `techsheet-measure.jsonl` in the temp folder).

use crate::config::AppConfig;
use crate::overlay;

#[test]
#[ignore = "reads the Pit Box configuration and library of this machine"]
fn real_install_physics_dump() {
    let src = std::env::var_os("PITBOX_CONFIG_DIR")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("APPDATA").map(|d| std::path::Path::new(&d).join("com.pitbox.app")))
        .expect("PITBOX_CONFIG_DIR or APPDATA");
    let work = crate::testutil::temp_dir("real-techsheet");
    for f in ["config.json", "overlay.sqlite"] {
        std::fs::copy(src.join(f), work.join(f)).unwrap();
    }
    let cfg: AppConfig = serde_json::from_str(&std::fs::read_to_string(work.join("config.json")).unwrap()).unwrap();
    let conn = overlay::open(&work.join("overlay.sqlite")).unwrap();
    let out = std::env::var_os("PITBOX_MEASURE_OUT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("techsheet-measure.jsonl"));

    let mut lines = Vec::new();
    let t = std::time::Instant::now();
    for m in overlay::list_mods(&conn)
        .unwrap()
        .into_iter()
        .filter(|m| m.kind == "Car")
    {
        let dir = if m.is_stock {
            cfg.ac_install_path
                .as_ref()
                .map(|ac| ac.join("content").join("cars").join(&m.id_interne))
        } else {
            m.active_version_id
                .as_deref()
                .and_then(|v| overlay::get_version_path(&conn, v).ok().flatten())
                .and_then(|p| crate::libpath::resolve(cfg.library_path.as_deref(), &p))
        };
        let Some(dir) = dir else { continue };
        let physics = super::physics::read(&dir, &m.id_interne);
        let ui = crate::uijson::read_car_specs(&dir);
        lines.push(
            serde_json::json!({
                "id": m.id_interne,
                "stock": m.is_stock,
                "name": m.display_name,
                "tags": m.tags_from_mod,
                "rules": {
                    "drivetrain": m.drivetrain, "aspiration": m.aspiration, "gearbox": m.gearbox,
                    "engine_config": m.engine_config, "engine_pos": m.engine_pos,
                },
                "physics": physics,
                "ui": ui.map(|u| serde_json::json!({
                    "bhp": u.bhp, "torque": u.torque, "weight": u.weight, "topspeed": u.topspeed,
                    "acceleration": u.acceleration, "pwratio": u.pwratio, "range": u.range,
                    "year": u.year, "curve": u.power_curve.len(),
                })),
            })
            .to_string(),
        );
    }
    println!("{} cars in {:?} → {}", lines.len(), t.elapsed(), out.display());
    std::fs::write(&out, lines.join("\n")).unwrap();
}

/// FICHE§9.3 — what the first start of this version changes in the library's
/// spec columns, on a copy of this machine's base: the catch-up
/// re-harmonisation, then the backfill, and the columns compared.
///
/// ```text
/// cargo test --lib techsheet::measure::real_install_backfill_report -- --ignored --nocapture
/// ```
#[test]
#[ignore = "reads the Pit Box configuration and library of this machine"]
fn real_install_backfill_report() {
    let src = std::env::var_os("PITBOX_CONFIG_DIR")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("APPDATA").map(|d| std::path::Path::new(&d).join("com.pitbox.app")))
        .expect("PITBOX_CONFIG_DIR or APPDATA");
    let work = crate::testutil::temp_dir("real-techsheet-backfill");
    for f in ["config.json", "overlay.sqlite", "taxonomy.json", "rules-overlay.json"] {
        if src.join(f).is_file() {
            std::fs::copy(src.join(f), work.join(f)).unwrap();
        }
    }
    let cfg: AppConfig = serde_json::from_str(&std::fs::read_to_string(work.join("config.json")).unwrap()).unwrap();
    let conn = overlay::open(&work.join("overlay.sqlite")).unwrap();
    if let Some(root) = cfg.ac_install_path.as_deref() {
        crate::nationalities::set_known(crate::nationalities::nationalities(root));
    }
    let cols = |m: &overlay::ModRow| {
        [
            m.drivetrain.clone(),
            m.aspiration.clone(),
            m.gearbox.clone(),
            m.engine_config.clone(),
            m.engine_pos.clone(),
        ]
    };
    let before: std::collections::BTreeMap<String, _> = overlay::list_mods(&conn)
        .unwrap()
        .into_iter()
        .filter(|m| m.kind == "Car")
        .map(|m| (m.id_interne.clone(), cols(&m)))
        .collect();

    let t = std::time::Instant::now();
    let rules = crate::rules::load_from_dir(&work);
    crate::harmonize::harmonize_all(&conn, &cfg, &rules).unwrap();
    let pending = super::pending_cars(&conn, &cfg).unwrap();
    for p in &pending {
        super::store_files(
            &conn,
            &p.mod_id,
            &p.version,
            &super::read_files(&p.dir, &p.mod_id, p.stock),
        )
        .unwrap();
    }
    println!("{} cars read in {:?}", pending.len(), t.elapsed());

    let names = ["drivetrain", "aspiration", "gearbox", "engine_config", "engine_pos"];
    let mut changes: std::collections::BTreeMap<String, usize> = Default::default();
    for m in overlay::list_mods(&conn)
        .unwrap()
        .into_iter()
        .filter(|m| m.kind == "Car")
    {
        let after = cols(&m);
        for (i, name) in names.iter().enumerate() {
            let old = before.get(&m.id_interne).and_then(|b| b[i].clone());
            if old != after[i] {
                let key = format!(
                    "{name}: {} → {}",
                    old.as_deref().unwrap_or("∅"),
                    after[i].as_deref().unwrap_or("∅")
                );
                *changes.entry(key).or_default() += 1;
            }
        }
    }
    for (k, n) in &changes {
        println!("  {n:>4}  {k}");
    }
    assert!(super::pending_cars(&conn, &cfg).unwrap().is_empty(), "idempotent");
}
