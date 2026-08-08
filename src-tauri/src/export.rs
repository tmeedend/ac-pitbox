//! Export d'archive autonome (§9.1). Repackage un mod en `.7z` complet, en
//! embarquant pour une voiture ses **dépendances éparpillées** : pilotes 3D
//! (`content/driver/*.kn5`), polices (`content/fonts/*`), crews
//! (`content/texture/crew_*`), config d'extension. Porté de `drivers.py`/
//! `fonts.py`/`cars.py::modFiles`.
//!
//! Les dépendances se résolvent via `data/driver3d.ini` et
//! `data/digital_instruments.ini`. Si la voiture n'a que `data.acd` (packé), on
//! l'extrait via QuickBMS + `acd.bms` **seulement si configurés** (§9.2, hors
//! chemin critique) ; sinon on prévient que certaines dépendances peuvent
//! manquer. Le contenu Kunos est exclu (§9.1, [[kunos]]).

use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use rusqlite::Connection;
use serde::Serialize;

use crate::config::AppConfig;
use crate::modscan::ModKind;
use crate::{archive, kunos, overlay};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone, Serialize)]
pub struct ExportReport {
    /// Chemin de l'archive créée.
    pub archive: String,
    /// Chemins relatifs embarqués (mod + dépendances).
    pub included: Vec<String>,
    /// Avertissements non bloquants (ex. dépendances non résolues).
    pub warnings: Vec<String>,
}

fn kind_of(s: &str) -> ModKind {
    if s == "Track" {
        ModKind::Track
    } else {
        ModKind::Car
    }
}

fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| if r#"\/:*?"<>|"#.contains(c) { '_' } else { c })
        .collect::<String>()
        .trim()
        .to_string()
}

/// Exporte la version active d'un mod en archive autonome dans `dest_dir` (§9.1).
pub fn export_mod(conn: &Connection, cfg: &AppConfig, mod_id: &str, dest_dir: &Path) -> Result<ExportReport, String> {
    let sevenzip = cfg
        .sevenzip_exe
        .as_ref()
        .ok_or(crate::errors::SEVENZIP_NOT_CONFIGURED)?;
    let m = overlay::get_mod(conn, mod_id)
        .map_err(|e| e.to_string())?
        .ok_or(crate::errors::MOD_NOT_FOUND)?;
    let kind = kind_of(&m.kind);
    let vid = m
        .active_version_id
        .clone()
        .ok_or(crate::errors::NO_ACTIVE_VERSION_TO_EXPORT)?;
    let stored = overlay::get_version_path(conn, &vid)
        .map_err(|e| e.to_string())?
        .ok_or(crate::errors::VERSION_NOT_FOUND)?;
    let lib =
        crate::libpath::resolve(cfg.library_path.as_deref(), &stored).ok_or(crate::errors::LIBRARY_NOT_CONFIGURED)?;
    if !lib.is_dir() {
        return Err(crate::errors::VERSION_FILES_MISSING.into());
    }

    let mut included = Vec::new();
    let mut warnings = Vec::new();

    // Dossier de mise en scène : on y reconstruit l'arborescence AC à archiver.
    let staging = std::env::temp_dir().join(format!("pitbox-export-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&staging).map_err(|e| e.to_string())?;

    let seg = kind.content_folder();
    let dest_mod = staging.join("content").join(seg).join(mod_id);
    archive::copy_dir(&lib, &dest_mod).map_err(|e| format!("copie du mod : {e}"))?;
    included.push(format!("content/{seg}/{mod_id}"));

    if matches!(kind, ModKind::Car) {
        resolve_car_deps(cfg, &lib, mod_id, &staging, &mut included, &mut warnings);
    }

    // Nom d'archive lisible : nom affiché (assaini) + version si dispo.
    let base = sanitize(m.display_name.as_deref().unwrap_or(mod_id));
    let base = if base.is_empty() { mod_id.to_string() } else { base };
    let archive_path = unique_archive(dest_dir, &base);

    let res = archive::create_7z(sevenzip, &staging, &archive_path);
    let _ = std::fs::remove_dir_all(&staging);
    res?;

    Ok(ExportReport {
        archive: archive_path.to_string_lossy().into_owned(),
        included,
        warnings,
    })
}

fn unique_archive(dir: &Path, base: &str) -> PathBuf {
    let p = dir.join(format!("{base}.7z"));
    if !p.exists() {
        return p;
    }
    dir.join(format!("{base}-{}.7z", &uuid::Uuid::new_v4().to_string()[..8]))
}

/// Résout et met en scène les dépendances d'une voiture (pilotes, polices,
/// crews, config d'extension). Tout est best-effort : une dépendance absente
/// produit un avertissement, pas une erreur.
fn resolve_car_deps(
    cfg: &AppConfig,
    lib_car: &Path,
    car_id: &str,
    staging: &Path,
    included: &mut Vec<String>,
    warnings: &mut Vec<String>,
) {
    let Some(ac) = &cfg.ac_install_path else {
        warnings.push("Dossier AC non configuré : dépendances (pilotes/polices) non embarquées.".into());
        return;
    };

    // Sources des .ini : d'abord le `data/` décompressé du mod ; sinon `data.acd`.
    let data_dir = lib_car.join("data");
    let acd_workdir = if data_dir.join("driver3d.ini").is_file() || data_dir.join("digital_instruments.ini").is_file() {
        Some(data_dir.clone())
    } else if lib_car.join("data.acd").is_file() {
        match extract_acd(cfg, &lib_car.join("data.acd")) {
            Some(dir) => Some(dir),
            None => {
                warnings.push(
                    "data.acd packé non extrait (QuickBMS / acd.bms non configurés) — pilotes et polices custom peut-être manquants.".into(),
                );
                None
            }
        }
    } else {
        None
    };

    // --- Pilotes 3D (driver3d.ini [MODEL] NAME + skins [DRIVER3D_MODEL] NAME) ---
    let mut drivers: Vec<String> = Vec::new();
    if let Some(dir) = &acd_workdir {
        if let Some(ini) = read_ini(&dir.join("driver3d.ini")) {
            if let Some(name) = ini.get("MODEL", "NAME") {
                drivers.push(name.to_string());
            }
        }
    }
    // Skins : ext_config.ini de chaque skin peut imposer un pilote.
    if let Ok(entries) = std::fs::read_dir(lib_car.join("skins")) {
        for e in entries.flatten() {
            let skin = e.path();
            if !skin.is_dir() {
                continue;
            }
            if let Some(ini) = read_ini(&skin.join("ext_config.ini")) {
                if let Some(name) = ini.get("DRIVER3D_MODEL", "NAME") {
                    drivers.push(name.to_string());
                }
            }
            // Crews (combinaisons/casques/marques) référencés par skin.ini.
            collect_crews(ac, &skin, staging, included, warnings);
        }
    }
    for name in dedup(drivers) {
        if kunos::is_kunos_driver(&name) {
            continue;
        }
        let rel = format!("content/driver/{name}.kn5");
        if stage_file(ac, staging, &rel) {
            included.push(rel);
        } else {
            warnings.push(format!("Pilote 3D introuvable : {name}.kn5"));
        }
    }

    // --- Polices (digital_instruments.ini ITEM_n FONT) ---
    let mut fonts: Vec<String> = Vec::new();
    if let Some(dir) = &acd_workdir {
        if let Some(ini) = read_ini(&dir.join("digital_instruments.ini")) {
            for sec in ini.section_names() {
                if sec.to_ascii_uppercase().starts_with("ITEM_") {
                    if let Some(name) = ini.get(&sec, "FONT") {
                        fonts.push(name.to_string());
                    }
                }
            }
        }
    }
    for name in dedup(fonts) {
        if kunos::is_kunos_font(&name) {
            continue;
        }
        for ext in ["png", "txt", "ttf"] {
            let rel = format!("content/fonts/{name}.{ext}");
            if stage_file(ac, staging, &rel) {
                included.push(rel);
            }
        }
    }

    // --- Config d'extension (CSP) de la voiture ---
    for rel in [
        format!("extension/config/cars/{car_id}.ini"),
        format!("extension/config/cars/loaded/{car_id}.ini"),
        format!("extension/config/cars/{car_id}.ini.blm"),
    ] {
        if stage_file(ac, staging, &rel) {
            included.push(rel);
        }
    }
}

/// Crews référencés par `skin.ini` [CREW] SUIT/HELMET/BRAND → dossiers texture.
fn collect_crews(ac: &Path, skin: &Path, staging: &Path, included: &mut Vec<String>, warnings: &mut Vec<String>) {
    let Some(ini) = read_ini(&skin.join("skin.ini")) else {
        return;
    };
    for crew_type in ["SUIT", "HELMET", "BRAND"] {
        let Some(name) = ini.get("CREW", crew_type) else {
            continue;
        };
        if kunos::is_kunos_crew(name, crew_type) {
            continue;
        }
        // content/texture/crew_<type><name> (name commence par un backslash).
        let rel = format!("content/texture/crew_{}{}", crew_type.to_lowercase(), name).replace('\\', "/");
        if stage_dir(ac, staging, &rel) {
            included.push(rel);
        } else {
            warnings.push(format!("Crew introuvable : {rel}"));
        }
    }
}

/// Copie `ac/rel` → `staging/rel` (fichier). Vrai si la source existait.
fn stage_file(ac: &Path, staging: &Path, rel: &str) -> bool {
    let src = ac.join(rel);
    if !src.is_file() {
        return false;
    }
    let dst = staging.join(rel);
    if let Some(parent) = dst.parent() {
        if std::fs::create_dir_all(parent).is_err() {
            return false;
        }
    }
    std::fs::copy(&src, &dst).is_ok()
}

/// Copie `ac/rel` → `staging/rel` (dossier). Vrai si la source existait.
fn stage_dir(ac: &Path, staging: &Path, rel: &str) -> bool {
    let src = ac.join(rel);
    if !src.is_dir() {
        return false;
    }
    archive::copy_dir(&src, &staging.join(rel)).is_ok()
}

/// Extrait `data.acd` via QuickBMS + `acd.bms` si configurés (§9.2). La clé de
/// déchiffrement dérive du nom de dossier : l'`.acd` est lu dans son emplacement.
fn extract_acd(cfg: &AppConfig, acd: &Path) -> Option<PathBuf> {
    let quickbms = cfg.quickbms_exe.as_ref()?;
    let script = cfg.acd_bms_script.as_ref()?;
    let dest = std::env::temp_dir().join(format!("pitbox-acd-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dest).ok()?;

    let mut cmd = Command::new(quickbms);
    cmd.arg(script).arg(acd).arg(&dest);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let ok = cmd.output().map(|o| o.status.success()).unwrap_or(false);
    if ok {
        Some(dest)
    } else {
        let _ = std::fs::remove_dir_all(&dest);
        None
    }
}

fn dedup(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v.dedup();
    v
}

// --- Mini-lecteur INI (sections + clés, insensible à la casse) --------------

struct Ini {
    sections: Vec<(String, Vec<(String, String)>)>,
}

impl Ini {
    fn parse(text: &str) -> Ini {
        let mut sections: Vec<(String, Vec<(String, String)>)> = Vec::new();
        let mut current = String::new();
        sections.push((current.clone(), Vec::new()));
        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
                continue;
            }
            if let Some(name) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                current = name.trim().to_string();
                sections.push((current.clone(), Vec::new()));
            } else if let Some((k, v)) = line.split_once('=') {
                // Coupe un éventuel commentaire en fin de valeur.
                let v = v.split(';').next().unwrap_or(v);
                if let Some(last) = sections.last_mut() {
                    last.1.push((k.trim().to_string(), v.trim().to_string()));
                }
            }
        }
        Ini { sections }
    }

    fn get(&self, section: &str, key: &str) -> Option<&str> {
        self.sections
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(section))
            .and_then(|(_, kvs)| {
                kvs.iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case(key))
                    .map(|(_, v)| v.as_str())
            })
    }

    fn section_names(&self) -> Vec<String> {
        self.sections.iter().map(|(n, _)| n.clone()).collect()
    }
}

fn read_ini(path: &Path) -> Option<Ini> {
    let text = std::fs::read_to_string(path).ok()?;
    Some(Ini::parse(text.trim_start_matches('\u{feff}')))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ini_parse_and_lookup() {
        let ini = Ini::parse("[MODEL]\nNAME=custom_driver ; comment\n\n[ITEM_0]\nFONT=myfont\n[ITEM_1]\nFONT=arial\n");
        assert_eq!(ini.get("model", "name"), Some("custom_driver"));
        assert_eq!(ini.get("ITEM_0", "FONT"), Some("myfont"));
        let items: Vec<String> = ini
            .section_names()
            .into_iter()
            .filter(|s| s.to_ascii_uppercase().starts_with("ITEM_"))
            .collect();
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn export_car_embeds_custom_driver() {
        let Some(sevenzip) = crate::detect::find_7zip() else {
            return; // 7-Zip requis
        };
        let base = crate::testutil::temp_dir("exp");
        let library = base.join("library");
        let ac = base.join("ac");
        let dest = base.join("out");
        std::fs::create_dir_all(&dest).unwrap();

        // Voiture en bibliothèque avec data/driver3d.ini → pilote custom.
        let car = library.join("cars").join("mycar").join("v1");
        std::fs::create_dir_all(car.join("ui")).unwrap();
        std::fs::write(car.join("ui").join("ui_car.json"), r#"{"name":"My Car"}"#).unwrap();
        std::fs::create_dir_all(car.join("data")).unwrap();
        std::fs::write(car.join("data").join("driver3d.ini"), "[MODEL]\nNAME=custom_driver\n").unwrap();

        // Pilote custom installé globalement + un pilote Kunos (à NE PAS embarquer).
        std::fs::create_dir_all(ac.join("content").join("driver")).unwrap();
        std::fs::write(ac.join("content").join("driver").join("custom_driver.kn5"), b"KN5").unwrap();

        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let now = chrono::Local::now().to_rfc3339();
        overlay::upsert_mod(&conn, "mycar", "Car", Some("B"), Some("My Car"), "h", None, &now).unwrap();
        overlay::insert_version(
            &conn,
            "v1",
            "mycar",
            Some("1.0"),
            None,
            &now,
            &car.to_string_lossy(),
            None,
            "sig",
            &[],
            &[],
            &[],
            &[],
            None,
        )
        .unwrap();
        overlay::set_active_version(&conn, "mycar", "v1").unwrap();

        let cfg = AppConfig {
            sevenzip_exe: Some(sevenzip.clone()),
            library_path: Some(library.clone()),
            ac_install_path: Some(ac.clone()),
            ..Default::default()
        };

        let report = export_mod(&conn, &cfg, "mycar", &dest).unwrap();
        assert!(Path::new(&report.archive).is_file(), "archive créée");
        assert!(
            report.included.iter().any(|p| p.contains("custom_driver.kn5")),
            "pilote embarqué: {:?}",
            report.included
        );

        // Vérifie le contenu réel de l'archive.
        let check = base.join("check");
        std::fs::create_dir_all(&check).unwrap();
        archive::extract(&sevenzip, Path::new(&report.archive), &check).unwrap();
        assert!(check
            .join("content")
            .join("cars")
            .join("mycar")
            .join("ui")
            .join("ui_car.json")
            .is_file());
        assert!(check.join("content").join("driver").join("custom_driver.kn5").is_file());
    }
}
