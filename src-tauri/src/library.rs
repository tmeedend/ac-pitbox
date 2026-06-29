//! Vue bibliothèque (§6) : assemble les lignes overlay avec la vignette de
//! preview et l'état actif/inactif pour la galerie et le tableau.

use std::path::Path;

use rusqlite::Connection;
use serde::Serialize;

use crate::config::AppConfig;
use crate::inspect;
use crate::modscan::ModKind;
use crate::overlay::{self, HistoryRow, ModRow, VersionRow};
use crate::uijson::{self, NativeSpecs};

#[derive(Debug, Clone, Serialize)]
pub struct ModCard {
    #[serde(flatten)]
    pub base: ModRow,
    /// Chemin absolu d'une preview (à passer à convertFileSrc côté front).
    pub preview: Option<String>,
    /// Junction présente dans content/ (détection fine = L3).
    pub active: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModDetail {
    #[serde(flatten)]
    pub card: ModCard,
    pub versions: Vec<VersionRow>,
    pub history: Vec<HistoryRow>,
    /// Fiche technique native (voitures uniquement), lue de ui_car.json.
    pub specs: Option<NativeSpecs>,
}

fn kind_of(s: &str) -> ModKind {
    if s == "Track" {
        ModKind::Track
    } else {
        ModKind::Car
    }
}

fn is_active(cfg: &AppConfig, m: &ModRow) -> bool {
    let Some(ac) = &cfg.ac_install_path else {
        return false;
    };
    let link = ac
        .join("content")
        .join(kind_of(&m.kind).content_folder())
        .join(&m.id_interne);
    // « Actif » = junction gérée par l'app présente (pas un vrai dossier installé hors app).
    crate::activation::is_junction(&link)
}

fn preview_for(conn: &Connection, m: &ModRow) -> Option<String> {
    let vid = m.active_version_id.as_ref()?;
    let lib = overlay::get_version_path(conn, vid).ok().flatten()?;
    inspect::preview_path(kind_of(&m.kind), Path::new(&lib))
}

fn to_card(conn: &Connection, cfg: &AppConfig, m: ModRow) -> ModCard {
    let preview = preview_for(conn, &m);
    let active = is_active(cfg, &m);
    ModCard { base: m, preview, active }
}

pub fn list_cards(conn: &Connection, cfg: &AppConfig) -> rusqlite::Result<Vec<ModCard>> {
    Ok(overlay::list_mods(conn)?
        .into_iter()
        .map(|m| to_card(conn, cfg, m))
        .collect())
}

/// Contenu réellement installé dans `content/` (Kunos + mods actifs), pour les
/// sélecteurs de l'écran de lancement. Indépendant de l'overlay.
#[derive(Debug, Clone, Serialize)]
pub struct InstalledItem {
    pub id: String,
    pub name: String,
    /// Layouts d'un circuit (vide si mono-layout ou voiture).
    pub layouts: Vec<String>,
    /// Vignette (skin voiture / outline circuit) pour les galeries du flux.
    pub preview: Option<String>,
}

pub fn list_installed(cfg: &AppConfig, kind: ModKind) -> Vec<InstalledItem> {
    let Some(ac) = &cfg.ac_install_path else {
        return Vec::new();
    };
    let dir = ac.join("content").join(kind.content_folder());
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            let p = e.path();
            if !p.is_dir() {
                continue;
            }
            let id = e.file_name().to_string_lossy().into_owned();
            let (name, layouts) = match kind {
                ModKind::Car => (inspect_name(uijson::read_car(&p), &id), Vec::new()),
                ModKind::Track => {
                    let name = inspect_name(uijson::read_track(&p), &id);
                    let mut layouts = inspect::track_layouts(&p);
                    if layouts == ["(default)"] {
                        layouts.clear();
                    }
                    (name, layouts)
                }
            };
            let preview = inspect::preview_path(kind, &p);
            out.push(InstalledItem { id, name, layouts, preview });
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

fn inspect_name(ui: Option<crate::uijson::UiInfo>, id: &str) -> String {
    ui.and_then(|u| u.name).unwrap_or_else(|| id.to_string())
}

/// Skin d'une voiture avec sa miniature (§8.6).
#[derive(Debug, Clone, Serialize)]
pub struct SkinItem {
    pub id: String,
    pub name: String,
    pub preview: Option<String>,
}

fn read_skin_name(skin_dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(skin_dir.join("ui_skin.json")).ok()?;
    let v: serde_json::Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()?;
    v.get("skinname")
        .or_else(|| v.get("name"))
        .and_then(|x| x.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.to_string())
}

/// Lit les skins d'un dossier `skins/` donné (sous-dossiers + miniature + nom).
fn read_skins_dir(skins_dir: &Path) -> Vec<SkinItem> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(skins_dir) {
        for e in entries.flatten() {
            let p = e.path();
            if !p.is_dir() {
                continue;
            }
            let id = e.file_name().to_string_lossy().into_owned();
            let preview = ["preview.jpg", "preview.png"]
                .iter()
                .map(|n| p.join(n))
                .find(|pp| pp.is_file())
                .map(|pp| pp.to_string_lossy().into_owned());
            let name = read_skin_name(&p).unwrap_or_else(|| id.clone());
            out.push(SkinItem { id, name, preview });
        }
    }
    out.sort_by(|a, b| a.id.to_lowercase().cmp(&b.id.to_lowercase()));
    out
}

/// Skins d'une voiture **installée** (`content/cars/<id>/skins`) — flux de lancement.
pub fn list_car_skins(cfg: &AppConfig, car_id: &str) -> Vec<SkinItem> {
    let Some(ac) = &cfg.ac_install_path else {
        return Vec::new();
    };
    read_skins_dir(&ac.join("content").join("cars").join(car_id).join("skins"))
}

/// Skins d'une voiture pour la fiche détail (§6.3). Pour un **mod géré**, on lit
/// la version active en bibliothèque (disponible même inactif). Pour une
/// **voiture de base Kunos** (`is_stock`, sans version bibliothèque), on lit
/// directement `content/cars/<id>/skins` — là où vivent ses skins (y compris
/// ceux projetés par junction, §12bis.2).
pub fn list_mod_skins(conn: &Connection, cfg: &AppConfig, mod_id: &str) -> Vec<SkinItem> {
    let Some(m) = overlay::get_mod(conn, mod_id).ok().flatten() else {
        return Vec::new();
    };
    if !m.is_stock {
        if let Some(lib) = m
            .active_version_id
            .as_ref()
            .and_then(|vid| overlay::get_version_path(conn, vid).ok().flatten())
        {
            return read_skins_dir(&Path::new(&lib).join("skins"));
        }
    }
    // Voiture de base (ou mod sans version) : skins installés dans content/.
    if let Some(ac) = &cfg.ac_install_path {
        return read_skins_dir(&ac.join("content").join("cars").join(mod_id).join("skins"));
    }
    Vec::new()
}

/// Dossiers météo installés (`content/weather/*`).
pub fn list_weather(cfg: &AppConfig) -> Vec<String> {
    let Some(ac) = &cfg.ac_install_path else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(ac.join("content").join("weather")) {
        for e in entries.flatten() {
            if e.path().is_dir() {
                if let Some(n) = e.file_name().to_str() {
                    out.push(n.to_string());
                }
            }
        }
    }
    out.sort();
    out
}

pub fn detail(conn: &Connection, cfg: &AppConfig, id: &str) -> rusqlite::Result<Option<ModDetail>> {
    let Some(m) = overlay::get_mod(conn, id)? else {
        return Ok(None);
    };
    let versions = overlay::get_versions(conn, id)?;
    let history = overlay::get_history(conn, id)?;
    // Fiche technique native lue à la demande dans la version active (voitures).
    let specs = if m.kind == "Car" {
        m.active_version_id
            .as_ref()
            .and_then(|vid| overlay::get_version_path(conn, vid).ok().flatten())
            .and_then(|lib| uijson::read_car_specs(Path::new(&lib)))
    } else {
        None
    };
    let card = to_card(conn, cfg, m);
    Ok(Some(ModDetail { card, versions, history, specs }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stock_car_skins_read_from_content() {
        let base = std::env::temp_dir().join(format!("pitbox-lib-{}", uuid::Uuid::new_v4()));
        let ac = base.join("ac");
        // Voiture de base avec un skin installé dans content/.
        let skin = ac.join("content").join("cars").join("ks_ferrari").join("skins").join("rosso");
        std::fs::create_dir_all(&skin).unwrap();
        std::fs::write(skin.join("preview.jpg"), b"IMG").unwrap();

        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let now = chrono::Local::now().to_rfc3339();
        overlay::upsert_stock_mod(&conn, "ks_ferrari", "Car", Some("Ferrari"), Some("488"), &now).unwrap();

        let cfg = AppConfig { ac_install_path: Some(ac.clone()), ..Default::default() };
        let skins = list_mod_skins(&conn, &cfg, "ks_ferrari");
        assert_eq!(skins.len(), 1, "skin de la voiture de base lu dans content/");
        assert_eq!(skins[0].id, "rosso");

        let _ = std::fs::remove_dir_all(&base);
    }
}
