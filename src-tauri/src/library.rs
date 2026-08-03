//! Vue bibliothèque (§6) : assemble les lignes overlay avec la vignette de
//! preview et l'état actif/inactif pour la galerie et le tableau.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde::Serialize;

use crate::cm_stats::{self, CmUsage};
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
    /// Voiture : skin ; circuit : photo illustratrice (fond).
    pub preview: Option<String>,
    /// Tracé du circuit à superposer à la photo (circuits uniquement, §6.1).
    pub outline: Option<String>,
    /// Junction présente dans content/ (détection fine = L3).
    pub active: bool,
    /// Distance parcourue (km) d'après CM, si connue (§6.5).
    pub distance_km: Option<f64>,
    /// « Déjà essayé » : lancé par l'app OU km CM > 0 (§6.5).
    pub tried: bool,
    /// Poids natif (voitures), lu à la volée dans ui_car.json — colonne §6.2.
    pub weight: Option<String>,
    /// Badge/logo de la marque (`ui/badge.png`, voitures), à la place des initiales.
    pub badge: Option<String>,
    /// Mod cassé (fichiers de la version active manquants/invalides, §6.4) —
    /// même détection que l'écran Maintenance (§9.3), remontée ici comme
    /// signalement visuel sur la carte bibliothèque.
    pub broken: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModDetail {
    #[serde(flatten)]
    pub card: ModCard,
    pub versions: Vec<VersionRow>,
    pub history: Vec<HistoryRow>,
    /// Fiche technique native (voitures uniquement), lue de ui_car.json.
    pub specs: Option<NativeSpecs>,
    /// Détail circuit (description + layouts illustrés), circuits uniquement.
    pub track: Option<uijson::TrackDetail>,
}

fn kind_of(s: &str) -> ModKind {
    if s == "Track" {
        ModKind::Track
    } else {
        ModKind::Car
    }
}

fn is_active(cfg: &AppConfig, m: &ModRow) -> bool {
    // Contenu de base Kunos : toujours un vrai dossier (jamais de déploiement),
    // chargé par AC en permanence — donc toujours « actif ».
    if m.is_stock {
        return true;
    }
    if cfg.ac_install_path.is_none() {
        return false;
    }
    // « Actif » = déploiement géré par l'app présent (symlink hérité ou
    // hardlinks, §2) — pas un vrai dossier installé hors app.
    crate::activation::is_mod_active(cfg, kind_of(&m.kind), &m.id_interne)
}

fn preview_for(conn: &Connection, cfg: &AppConfig, m: &ModRow) -> Option<String> {
    // Version active en bibliothèque, sinon content/ (contenu de base Kunos) —
    // c'est ce qui fait apparaître la vignette du stock, comme l'écran de session.
    let dir = entity_dir(conn, cfg, m)?;
    match kind_of(&m.kind) {
        ModKind::Car => inspect::preview_path(ModKind::Car, &dir),
        // Circuit : la photo illustratrice (fond), repli sur le tracé si absente.
        ModKind::Track => inspect::track_preview(&dir).or_else(|| inspect::track_outline(&dir)),
    }
}

/// Tracé d'un circuit à superposer à la photo (None pour une voiture).
fn outline_for(conn: &Connection, cfg: &AppConfig, m: &ModRow) -> Option<String> {
    if m.kind != "Track" {
        return None;
    }
    let dir = entity_dir(conn, cfg, m)?;
    inspect::track_outline(&dir)
}

/// Poids natif (voitures uniquement), lu à la volée dans ui_car.json —
/// donnée « native », jamais harmonisée par le moteur de règles (§5bis.1).
fn weight_for(conn: &Connection, cfg: &AppConfig, m: &ModRow) -> Option<String> {
    if m.kind != "Car" {
        return None;
    }
    let dir = entity_dir(conn, cfg, m)?;
    uijson::read_car_specs(&dir)?.weight
}

/// Badge/logo de la marque (voitures uniquement), lu à la volée dans `ui/badge.png`.
fn badge_for(conn: &Connection, cfg: &AppConfig, m: &ModRow) -> Option<String> {
    if m.kind != "Car" {
        return None;
    }
    let dir = entity_dir(conn, cfg, m)?;
    inspect::brand_badge(&dir)
}

fn to_card(conn: &Connection, cfg: &AppConfig, m: ModRow) -> ModCard {
    let preview = preview_for(conn, cfg, &m);
    let outline = outline_for(conn, cfg, &m);
    let active = is_active(cfg, &m);
    let weight = weight_for(conn, cfg, &m);
    let badge = badge_for(conn, cfg, &m);
    let broken = crate::maintenance::broken_reason(conn, &m).is_some();
    ModCard {
        base: m,
        preview,
        outline,
        active,
        distance_km: None,
        tried: false,
        weight,
        badge,
        broken,
    }
}

/// Renseigne la distance CM et le marqueur « essayé » (§6.5) sur une carte.
fn fill_usage(card: &mut ModCard, cm: &CmUsage, launched: &HashSet<String>) {
    let id = &card.base.id_interne;
    card.distance_km = cm.km(id);
    card.tried = launched.contains(id) || card.distance_km.is_some_and(|k| k > 0.0);
}

pub fn list_cards(conn: &Connection, cfg: &AppConfig) -> rusqlite::Result<Vec<ModCard>> {
    let cm = cm_stats::read();
    let launched = overlay::launched_ids(conn)?;
    Ok(overlay::list_mods(conn)?
        .into_iter()
        .map(|m| {
            let mut card = to_card(conn, cfg, m);
            fill_usage(&mut card, &cm, &launched);
            card
        })
        .collect())
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
    out.sort_by_key(|a| a.id.to_lowercase());
    out
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

/// Dossier de l'entité contenant `ui/`, tel que le jeu le voit :
/// - version active en bibliothèque pour un mod géré (intacte, jamais composée) ;
/// - sinon `content/<type>s/<id>` (contenu de base Kunos, ou mod géré/contenu
///   de base **composé** avec ses couches actives, §4.3/§4.4 : depuis la
///   bascule hardlinks, `content/<id>` EST directement le résultat composé —
///   plus de dossier `<lib>/composed/<type>s/<id>` intermédiaire à consulter,
///   contrairement à l'ancien mécanisme par junction).
fn entity_dir(conn: &Connection, cfg: &AppConfig, m: &ModRow) -> Option<PathBuf> {
    if !m.is_stock {
        if let Some(vid) = &m.active_version_id {
            if let Ok(Some(p)) = overlay::get_version_path(conn, vid) {
                return Some(PathBuf::from(p));
            }
        }
    }
    cfg.ac_install_path.as_ref().map(|ac| {
        ac.join("content")
            .join(kind_of(&m.kind).content_folder())
            .join(&m.id_interne)
    })
}

/// Dossier réel d'un mod (voiture/circuit, géré ou contenu de base), pour
/// « Ouvrir le dossier » dans l'explorateur — même résolution que la fiche
/// détail (`entity_dir`), exposée publiquement pour la commande dédiée.
pub fn folder_path(conn: &Connection, cfg: &AppConfig, id: &str) -> Result<PathBuf, String> {
    let m = overlay::get_mod(conn, id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("mod introuvable : {id}"))?;
    entity_dir(conn, cfg, &m).ok_or_else(|| format!("dossier introuvable pour « {id} »"))
}

/// Fonctionnalités CSP effectivement détectées pour un mod (§6.4bis) : config
/// propre au mod + config CSP "chargée" séparément par CSP (hors du mod, cf.
/// `inspect::csp_features_loaded` — c'est notamment ce qui manquait pour le
/// contenu de base). Calculé à la demande (pas mis en cache) : sert à griser
/// les réglages météo/saison non supportés sur l'écran de session.
pub fn mod_csp_features(conn: &Connection, cfg: &AppConfig, id: &str) -> Result<Vec<String>, String> {
    let m = overlay::get_mod(conn, id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("mod introuvable : {id}"))?;
    let kind = kind_of(&m.kind);
    let dir = entity_dir(conn, cfg, &m).ok_or_else(|| format!("dossier introuvable pour « {id} »"))?;
    let mut feats = inspect::csp_features(&dir);
    if let Some(ac) = &cfg.ac_install_path {
        feats.extend(inspect::csp_features_loaded(ac, kind, id));
    }
    feats.sort();
    feats.dedup();
    Ok(feats)
}

pub fn detail(conn: &Connection, cfg: &AppConfig, id: &str) -> rusqlite::Result<Option<ModDetail>> {
    let Some(m) = overlay::get_mod(conn, id)? else {
        return Ok(None);
    };
    let versions = overlay::get_versions(conn, id)?;
    let history = overlay::get_history(conn, id)?;
    // Dossier de l'entité : version active en bibliothèque, sinon content/ (stock).
    let entity_dir = entity_dir(conn, cfg, &m);
    // Fiche technique native lue à la demande (voitures).
    let specs = if m.kind == "Car" {
        entity_dir.as_deref().and_then(uijson::read_car_specs)
    } else {
        None
    };
    // Détail circuit (description + layouts illustrés).
    let track = if m.kind == "Track" {
        entity_dir.as_deref().map(uijson::read_track_detail)
    } else {
        None
    };
    let mut card = to_card(conn, cfg, m);
    fill_usage(&mut card, &cm_stats::read(), &overlay::launched_ids(conn)?);
    Ok(Some(ModDetail {
        card,
        versions,
        history,
        specs,
        track,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stock_mod_always_active() {
        // Le contenu de base Kunos est un vrai dossier (jamais une junction) :
        // il doit être considéré actif même sans dossier AC configuré.
        let base = crate::testutil::temp_dir("active");
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let now = chrono::Local::now().to_rfc3339();
        overlay::upsert_stock_mod(&conn, "ks_test_track", "Track", None, Some("Test"), &now).unwrap();
        let m = overlay::get_mod(&conn, "ks_test_track").unwrap().unwrap();
        assert!(is_active(&AppConfig::default(), &m));
    }

    #[test]
    fn managed_mod_active_when_deployed_via_hardlinks() {
        // §2 : is_active doit reconnaître le nouveau mécanisme de déploiement
        // (hardlinks), pas seulement l'ancien symlink.
        let base = crate::testutil::temp_dir("active-hl");
        let ac = base.join("ac");
        let lib = base.join("library");
        std::fs::create_dir_all(ac.join("content").join("cars")).unwrap();
        let carv = lib.join("cars").join("hl_car").join("v1");
        std::fs::create_dir_all(&carv).unwrap();
        std::fs::write(carv.join("f.txt"), "x").unwrap();

        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let now = chrono::Local::now().to_rfc3339();
        overlay::upsert_mod(&conn, "hl_car", "Car", Some("B"), Some("Test"), "h", None, &now).unwrap();
        overlay::insert_version(
            &conn,
            "v1",
            "hl_car",
            Some("1.0"),
            None,
            &now,
            &carv.to_string_lossy(),
            None,
            "sig",
            &[],
            &[],
            &[],
            &[],
            None,
        )
        .unwrap();
        overlay::set_active_version(&conn, "hl_car", "v1").unwrap();
        let cfg = AppConfig {
            ac_install_path: Some(ac),
            library_path: Some(lib),
            ..Default::default()
        };

        let m = overlay::get_mod(&conn, "hl_car").unwrap().unwrap();
        assert!(!is_active(&cfg, &m), "pas encore activé");

        crate::activation::activate(&conn, &cfg, "hl_car", None).unwrap();
        let m = overlay::get_mod(&conn, "hl_car").unwrap().unwrap();
        assert!(is_active(&cfg, &m), "déployé par hardlinks = actif");
    }

    #[test]
    fn entity_dir_ignores_stale_pre_hardlink_composed_leftover() {
        // Bug réel : `<lib>/composed/<type>s/<id>` est un reliquat de l'ancien
        // mécanisme par junction (avant la bascule hardlinks, §4.3) — plus
        // jamais écrit ni lu. Sur une bibliothèque utilisée avant la bascule,
        // ce dossier peut encore traîner sur le disque avec un contenu périmé
        // (ex. un layout apporté par une couche depuis désactivée) ; il ne
        // doit plus jamais être préféré au vrai contenu déployé dans content/.
        let base = crate::testutil::temp_dir("stale-composed");
        let ac = base.join("ac");
        let lib = base.join("library");
        let link = ac.join("content").join("tracks").join("spa");
        std::fs::create_dir_all(link.join("ui")).unwrap();
        std::fs::write(link.join("ui").join("ui_track.json"), br#"{"name":"Spa restored"}"#).unwrap();

        // Reliquat périmé : contient encore le layout "2022" d'une couche
        // pourtant désactivée depuis.
        let stale = lib.join("composed").join("tracks").join("spa");
        std::fs::create_dir_all(stale.join("ui").join("2022")).unwrap();
        std::fs::write(stale.join("ui").join("2022").join("ui_track.json"), "{}").unwrap();

        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        overlay::upsert_stock_mod(&conn, "spa", "Track", Some("Kunos"), Some("Spa"), "now").unwrap();
        let cfg = AppConfig {
            ac_install_path: Some(ac),
            library_path: Some(lib),
            ..Default::default()
        };
        let m = overlay::get_mod(&conn, "spa").unwrap().unwrap();

        let dir = entity_dir(&conn, &cfg, &m).unwrap();
        assert_eq!(
            dir, link,
            "doit résoudre vers content/, jamais vers l'ancien dossier composé périmé"
        );
        assert!(
            !dir.join("ui").join("2022").is_dir(),
            "le layout périmé du reliquat ne doit pas apparaître"
        );
    }

    #[test]
    fn stock_car_skins_read_from_content() {
        let base = crate::testutil::temp_dir("lib");
        let ac = base.join("ac");
        // Voiture de base avec un skin installé dans content/.
        let skin = ac
            .join("content")
            .join("cars")
            .join("ks_ferrari")
            .join("skins")
            .join("rosso");
        std::fs::create_dir_all(&skin).unwrap();
        std::fs::write(skin.join("preview.jpg"), b"IMG").unwrap();

        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let now = chrono::Local::now().to_rfc3339();
        overlay::upsert_stock_mod(&conn, "ks_ferrari", "Car", Some("Ferrari"), Some("488"), &now).unwrap();

        let cfg = AppConfig {
            ac_install_path: Some(ac.clone()),
            ..Default::default()
        };
        let skins = list_mod_skins(&conn, &cfg, "ks_ferrari");
        assert_eq!(skins.len(), 1, "skin de la voiture de base lu dans content/");
        assert_eq!(skins[0].id, "rosso");
    }

    #[test]
    fn broken_mod_flagged_on_card() {
        // Mod dont la version active pointe vers un dossier bibliothèque
        // disparu (§6.4) : list_cards doit remonter broken=true, la même
        // détection que l'écran Maintenance (§9.3).
        let base = crate::testutil::temp_dir("broken-card");
        std::fs::create_dir_all(&base).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let now = chrono::Local::now().to_rfc3339();

        overlay::upsert_mod(&conn, "ghost", "Car", Some("B"), Some("Ghost"), "h", None, &now).unwrap();
        overlay::insert_version(
            &conn,
            "v1",
            "ghost",
            Some("1.0"),
            None,
            &now,
            &base.join("nope").to_string_lossy(),
            None,
            "sig",
            &[],
            &[],
            &[],
            &[],
            None,
        )
        .unwrap();
        overlay::set_active_version(&conn, "ghost", "v1").unwrap();

        let cards = list_cards(&conn, &AppConfig::default()).unwrap();
        let ghost = cards.iter().find(|c| c.base.id_interne == "ghost").unwrap();
        assert!(ghost.broken);
    }

    #[test]
    fn stock_mod_never_flagged_broken() {
        // Le contenu de base n'a pas de version bibliothèque à proprement
        // parler (lecture directe dans content/) — ne doit jamais être signalé
        // cassé, même sans dossier AC configuré.
        let base = crate::testutil::temp_dir("broken-stock");
        std::fs::create_dir_all(&base).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let now = chrono::Local::now().to_rfc3339();
        overlay::upsert_stock_mod(&conn, "ks_test_track", "Track", None, Some("Test"), &now).unwrap();

        let cards = list_cards(&conn, &AppConfig::default()).unwrap();
        let stock = cards.iter().find(|c| c.base.id_interne == "ks_test_track").unwrap();
        assert!(!stock.broken);
    }
}
