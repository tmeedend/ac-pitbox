//! Médias liés à une voiture/un circuit (§6.1) : captures d'écran et replays
//! d'Assetto Corsa, lus en direct depuis `Documents/Assetto Corsa/`, plus les
//! backgrounds officiels CSP lus depuis l'installation AC. Aucune donnée mise
//! en cache — même principe que `resources.rs` (§4.5.2).
//!
//! Rattachement par simple « le nom de fichier contient cet id » (pas de
//! découpage voiture/circuit) : les deux espaces de noms ne se recoupent
//! jamais (`content/cars/<id>` vs `content/tracks/<id>`), donc un id trouvé
//! dans le nom désigne sans ambiguïté la bonne entité. Un faux positif
//! occasionnel (id imbriqués, ex. "imola" contenu dans un mod
//! "rt_imola_historic") est accepté : ces fichiers ne sont là que pour
//! l'agrément (§6.1), pas une fonctionnalité critique — mieux vaut un média de
//! trop qu'un rattachement manqué. Vérifié sur les fichiers réels du poste
//! avant implémentation (voir SPEC §6.1) :
//! - `screens/Screenshot_<car_id>_<track_id>_<d>-<m>-<y>-<h>-<m>-<s>.jpg` et
//!   `screens/Showroom_<car_id>_<d>-<m>-<yyyy>-<h>-<m>-<s>.jpg` (pas de
//!   circuit — le showroom n'a pas de piste). Le format de l'année dans le nom
//!   n'est pas uniforme (bug `tm_year` en session) : jamais parsé, on ne lit
//!   que le mtime du fichier.
//! - `replay/AC_<ddmmyy>-<hhmmss>_<type>_<car_id>_<track_id[_layout]>_<suffixe?>.acreplay`,
//!   suffixe final de longueur variable ou absent. `replay/temp/` est ignoré
//!   (fichiers de travail, jamais des replays terminés).
//! - `<ac_install>/extension/backgrounds/<track_id>[__<layout_id>]_<variant>.jpg` —
//!   convention CSP propre, match par préfixe.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Local};
use serde::Serialize;
use walkdir::WalkDir;

/// `%USERPROFILE%/Documents/Assetto Corsa` — même repli que `launch.rs`
/// (`assists.ini`).
pub fn documents_ac_dir() -> Option<PathBuf> {
    let profile = std::env::var("USERPROFILE").ok()?;
    Some(Path::new(&profile).join("Documents").join("Assetto Corsa"))
}

#[derive(Debug, Clone, Serialize)]
pub struct ScreenshotFile {
    pub path: String,
    pub file_name: String,
    /// Horodatage de modification du fichier — fiable, contrairement à la
    /// date embarquée dans le nom (format non uniforme selon le mode de
    /// capture, voir en-tête de module).
    pub modified_at: Option<String>,
    /// Id de l'autre entité (circuit si `entity_id` est une voiture, et
    /// inversement) trouvé dans le nom de fichier, s'il y en a un.
    pub matched_counterpart: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReplayFile {
    pub path: String,
    pub file_name: String,
    pub session_type: Option<String>,
    pub recorded_at: Option<String>,
    pub matched_counterpart: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BackgroundFile {
    pub path: String,
    pub layout_id: Option<String>,
}

/// Le plus long id contenu dans le nom l'emporte : réduit (sans l'éliminer)
/// le risque qu'un id court (ex. "imola") masque un id plus spécifique qui le
/// contient aussi (ex. "rt_imola_historic") — best-effort, pas une garantie
/// (voir en-tête de module).
fn best_counterpart(file_stem: &str, counterpart_ids: &HashSet<String>) -> Option<String> {
    counterpart_ids
        .iter()
        .filter(|id| !id.is_empty() && file_stem.contains(id.as_str()))
        .max_by_key(|id| id.len())
        .cloned()
}

fn list_screenshots_in(dir: &Path, entity_id: &str, counterpart_ids: &HashSet<String>) -> Vec<ScreenshotFile> {
    if !dir.is_dir() {
        return Vec::new();
    }
    let mut out: Vec<ScreenshotFile> = WalkDir::new(dir)
        .max_depth(1)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| {
            let path = e.path();
            let is_image = path
                .extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| x.eq_ignore_ascii_case("jpg") || x.eq_ignore_ascii_case("png"));
            if !is_image {
                return None;
            }
            let stem = path.file_stem()?.to_str()?;
            if !stem.contains(entity_id) {
                return None;
            }
            let modified_at = e
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .map(|t| DateTime::<Local>::from(t).to_rfc3339());
            Some(ScreenshotFile {
                path: path.to_string_lossy().into_owned(),
                file_name: path.file_name()?.to_string_lossy().into_owned(),
                modified_at,
                matched_counterpart: best_counterpart(stem, counterpart_ids),
            })
        })
        .collect();
    out.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));
    out
}

/// Captures personnelles mettant en scène `entity_id` (§6.1). `counterpart_ids`
/// est l'ensemble des id de circuits (si `entity_id` est une voiture) ou de
/// voitures (si `entity_id` est un circuit) connus de la bibliothèque, pour
/// résoudre `matched_counterpart`.
pub fn list_screenshots(entity_id: &str, counterpart_ids: &HashSet<String>) -> Vec<ScreenshotFile> {
    let Some(dir) = documents_ac_dir() else {
        return Vec::new();
    };
    list_screenshots_in(&dir.join("screens"), entity_id, counterpart_ids)
}

/// Parse le préfixe fixe `AC_<ddmmyy>-<hhmmss>_<type>_…` d'un replay — la
/// seule partie du nom dont le format est garanti (le reste, voiture/circuit,
/// est géré par simple `contains`, voir en-tête de module).
fn parse_replay_stem(stem: &str) -> (Option<String>, Option<String>) {
    let Some(rest) = stem.strip_prefix("AC_") else {
        return (None, None);
    };
    let mut parts = rest.splitn(3, '_');
    let recorded_at = parts.next().and_then(parse_ddmmyy_hhmmss);
    let session_type = parts.next().map(str::to_string);
    (session_type, recorded_at)
}

fn parse_ddmmyy_hhmmss(s: &str) -> Option<String> {
    let (d, t) = s.split_once('-')?;
    if d.len() != 6 || t.len() != 6 || !d.bytes().all(|b| b.is_ascii_digit()) || !t.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let day: u32 = d[0..2].parse().ok()?;
    let month: u32 = d[2..4].parse().ok()?;
    let year: i32 = 2000 + d[4..6].parse::<i32>().ok()?;
    let hour: u32 = t[0..2].parse().ok()?;
    let min: u32 = t[2..4].parse().ok()?;
    let sec: u32 = t[4..6].parse().ok()?;
    let date = chrono::NaiveDate::from_ymd_opt(year, month, day)?;
    let time = chrono::NaiveTime::from_hms_opt(hour, min, sec)?;
    Some(
        chrono::NaiveDateTime::new(date, time)
            .format("%Y-%m-%dT%H:%M:%S")
            .to_string(),
    )
}

fn list_replays_in(dir: &Path, entity_id: &str, counterpart_ids: &HashSet<String>) -> Vec<ReplayFile> {
    if !dir.is_dir() {
        return Vec::new();
    }
    // `max_depth(1)` : ne descend pas dans `replay/temp/` (fichiers de travail,
    // jamais des replays terminés).
    let mut out: Vec<ReplayFile> = WalkDir::new(dir)
        .max_depth(1)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| {
            let path = e.path();
            let is_replay = path
                .extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| x.eq_ignore_ascii_case("acreplay"));
            if !is_replay {
                return None;
            }
            let stem = path.file_stem()?.to_str()?;
            if !stem.contains(entity_id) {
                return None;
            }
            let (session_type, recorded_at) = parse_replay_stem(stem);
            Some(ReplayFile {
                path: path.to_string_lossy().into_owned(),
                file_name: path.file_name()?.to_string_lossy().into_owned(),
                session_type,
                recorded_at,
                matched_counterpart: best_counterpart(stem, counterpart_ids),
            })
        })
        .collect();
    out.sort_by(|a, b| b.recorded_at.cmp(&a.recorded_at));
    out
}

/// Replays impliquant `entity_id` (§6.1) — mêmes conventions que
/// `list_screenshots`.
pub fn list_replays(entity_id: &str, counterpart_ids: &HashSet<String>) -> Vec<ReplayFile> {
    let Some(dir) = documents_ac_dir() else {
        return Vec::new();
    };
    list_replays_in(&dir.join("replay"), entity_id, counterpart_ids)
}

/// Fusionne les captures détectées automatiquement avec les rattachements
/// manuels (§6.1, repli quand `contains(entity_id)` ne trouve rien) —
/// dédupliqué par chemin, un lien manuel n'écrase jamais une détection
/// automatique déjà présente.
pub fn merge_screenshot_links(mut auto: Vec<ScreenshotFile>, manual_paths: &[String]) -> Vec<ScreenshotFile> {
    let known: HashSet<String> = auto.iter().map(|s| s.path.clone()).collect();
    for p in manual_paths {
        if known.contains(p) {
            continue;
        }
        let Some(file_name) = Path::new(p).file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let modified_at = std::fs::metadata(p)
            .ok()
            .and_then(|m| m.modified().ok())
            .map(|t| DateTime::<Local>::from(t).to_rfc3339());
        auto.push(ScreenshotFile {
            path: p.clone(),
            file_name: file_name.to_string(),
            modified_at,
            matched_counterpart: None,
        });
    }
    auto.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));
    auto
}

/// Même principe que `merge_screenshot_links`, pour les replays.
pub fn merge_replay_links(mut auto: Vec<ReplayFile>, manual_paths: &[String]) -> Vec<ReplayFile> {
    let known: HashSet<String> = auto.iter().map(|s| s.path.clone()).collect();
    for p in manual_paths {
        if known.contains(p) {
            continue;
        }
        let path = Path::new(p);
        let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let (session_type, recorded_at) = parse_replay_stem(stem);
        auto.push(ReplayFile {
            path: p.clone(),
            file_name: file_name.to_string(),
            session_type,
            recorded_at,
            matched_counterpart: None,
        });
    }
    auto.sort_by(|a, b| b.recorded_at.cmp(&a.recorded_at));
    auto
}

fn list_backgrounds_in(dir: &Path, track_id: &str, layout_id: Option<&str>) -> Vec<BackgroundFile> {
    if !dir.is_dir() {
        return Vec::new();
    }
    let mut all: Vec<BackgroundFile> = WalkDir::new(dir)
        .max_depth(1)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| {
            let path = e.path();
            let stem = path.file_stem()?.to_str()?;
            let rest = stem.strip_prefix(track_id)?;
            // Convention CSP : "<track_id>_<variant>" (base) ou
            // "<track_id>__<layout_id>_<variant>" (layout spécifique).
            let layout = if let Some(after) = rest.strip_prefix("__") {
                Some(after.rsplit_once('_').map(|(l, _)| l).unwrap_or(after).to_string())
            } else if rest.starts_with('_') {
                None
            } else {
                return None;
            };
            Some(BackgroundFile {
                path: path.to_string_lossy().into_owned(),
                layout_id: layout,
            })
        })
        .collect();
    if let Some(layout) = layout_id {
        let specific: Vec<BackgroundFile> = all
            .iter()
            .filter(|b| b.layout_id.as_deref() == Some(layout))
            .cloned()
            .collect();
        if !specific.is_empty() {
            return specific;
        }
        // Aucun background pour ce layout précis : repli sur les backgrounds
        // génériques du circuit (§6.2, étape 3 avant le fond neutre).
        all.retain(|b| b.layout_id.is_none());
        return all;
    }
    all.sort_by(|a, b| a.path.cmp(&b.path));
    all
}

/// Backgrounds officiels CSP pour `track_id` (§6.1, onglet Backgrounds — dispo
/// seulement pour un circuit). `layout_id: None` renvoie tout (usage onglet) ;
/// `Some(layout)` filtre sur ce layout avec repli sur les backgrounds
/// génériques (usage `resolve_session_background`).
pub fn list_backgrounds(ac_install: &Path, track_id: &str, layout_id: Option<&str>) -> Vec<BackgroundFile> {
    list_backgrounds_in(&ac_install.join("extension").join("backgrounds"), track_id, layout_id)
}

/// Chaîne de repli du fond photo de l'écran de réglages (§6.2/§9.3) :
/// 1. Screenshot perso du combo exact (même voiture + même circuit).
/// 2. Screenshot perso du même circuit, autre voiture.
/// 3. Background officiel du circuit.
/// 4. `None` — fond neutre côté appelant.
pub fn resolve_session_background(
    ac_install: &Path,
    car_id: &str,
    track_id: &str,
    layout_id: Option<&str>,
) -> Option<String> {
    let counterpart = HashSet::from([car_id.to_string()]);
    let shots = list_screenshots(track_id, &counterpart);
    if let Some(combo) = shots.iter().find(|s| s.matched_counterpart.as_deref() == Some(car_id)) {
        return Some(combo.path.clone());
    }
    if let Some(any) = shots.first() {
        return Some(any.path.clone());
    }
    list_backgrounds(ac_install, track_id, layout_id)
        .into_iter()
        .next()
        .map(|b| b.path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"x").unwrap();
    }

    fn ids(values: &[&str]) -> HashSet<String> {
        values.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn screenshot_matches_by_contains_and_resolves_counterpart() {
        // Noms réels observés (§6.1) : voiture et circuit collés sans
        // délimiteur, showroom sans circuit.
        let base = crate::testutil::temp_dir("media-shots");
        let dir = base.join("screens");
        write(&dir.join("Screenshot_ks_audi_sport_quattro_imola_5-6-126-16-46-48.jpg"));
        write(&dir.join("Showroom_ks_audi_tt_cup_6-6-2026-0-0-27.jpg"));

        let tracks = ids(&["imola"]);
        let by_car = list_screenshots_in(&dir, "ks_audi_sport_quattro", &tracks);
        assert_eq!(by_car.len(), 1, "seule la capture en session mentionne cette voiture");
        assert_eq!(
            by_car[0].matched_counterpart.as_deref(),
            Some("imola"),
            "le circuit doit être retrouvé dans le nom sans délimiteur explicite"
        );

        let cars = ids(&["ks_audi_sport_quattro"]);
        let by_track = list_screenshots_in(&dir, "imola", &cars);
        assert_eq!(by_track.len(), 1, "le showroom ne contient pas 'imola'");

        let showroom = list_screenshots_in(&dir, "ks_audi_tt_cup", &tracks);
        assert_eq!(showroom.len(), 1);
        assert_eq!(
            showroom[0].matched_counterpart, None,
            "le showroom n'a pas de circuit : aucun id de circuit ne doit matcher"
        );
    }

    #[test]
    fn best_counterpart_prefers_longest_matching_id() {
        // Faux positif accepté par design (voir en-tête de module) : entre
        // deux id qui matchent tous les deux, le plus long/spécifique gagne.
        let stem = "Screenshot_some_car_rt_imola_historic_1-1-2026-0-0-0";
        let counterparts = ids(&["imola", "rt_imola_historic"]);
        assert_eq!(
            best_counterpart(stem, &counterparts).as_deref(),
            Some("rt_imola_historic")
        );
    }

    #[test]
    fn replay_parses_fixed_prefix_and_matches_ids_ignoring_temp_folder() {
        let base = crate::testutil::temp_dir("media-replays");
        let dir = base.join("replay");
        write(&dir.join("AC_060524-232248_R_rss_formula_rss_3_v6_ks_barcelona_layout_gp_osrw62.acreplay"));
        write(&dir.join("AC_300124-231824_R_vrc_erc_1998_pageau_shannonville_long.acreplay"));
        // Fichier de travail : jamais un replay terminé, doit être ignoré même
        // s'il matcherait par ailleurs.
        write(
            &dir.join("temp")
                .join("AC_010101-000000_R_vrc_erc_1998_pageau_shannonville_long.acreplay"),
        );

        let found = list_replays_in(&dir, "vrc_erc_1998_pageau", &HashSet::new());
        assert_eq!(found.len(), 1, "temp/ ne doit jamais être scanné");
        assert_eq!(found[0].session_type.as_deref(), Some("R"));
        assert_eq!(found[0].recorded_at.as_deref(), Some("2024-01-30T23:18:24"));

        let barcelona = list_replays_in(&dir, "ks_barcelona_layout_gp", &HashSet::new());
        assert_eq!(
            barcelona.len(),
            1,
            "suffixe de longueur variable (osrw62) sans incidence sur le match"
        );
    }

    #[test]
    fn merge_manual_links_adds_missing_files_without_duplicating_known_ones() {
        let base = crate::testutil::temp_dir("media-manual");
        let manual = base.join("orphan_no_car_or_track_in_name.jpg");
        write(&manual);
        let known = ScreenshotFile {
            path: "already_found.jpg".to_string(),
            file_name: "already_found.jpg".to_string(),
            modified_at: Some("2026-01-01T00:00:00".to_string()),
            matched_counterpart: Some("some_track".to_string()),
        };
        let auto = vec![known.clone()];
        let manual_paths = vec![known.path.clone(), manual.to_string_lossy().into_owned()];

        let merged = merge_screenshot_links(auto, &manual_paths);
        assert_eq!(merged.len(), 2, "le chemin déjà détecté ne doit pas être dupliqué");
        assert!(
            merged.iter().any(|s| s.path == manual.to_string_lossy()),
            "le fichier rattaché manuellement doit apparaître même sans id dans son nom"
        );
    }

    #[test]
    fn background_filters_by_layout_with_fallback_to_generic() {
        // Convention CSP réelle (§6.1) : double underscore avant le layout.
        let base = crate::testutil::temp_dir("media-bg");
        let dir = base.join("extension").join("backgrounds");
        write(&dir.join("ks_brands_hatch_901.jpg"));
        write(&dir.join("ks_brands_hatch__gp_901.jpg"));
        write(&dir.join("ks_brands_hatch__gp_902.jpg"));
        write(&dir.join("ks_brands_hatch__indy_901.jpg"));

        let all = list_backgrounds_in(&dir, "ks_brands_hatch", None);
        assert_eq!(
            all.len(),
            4,
            "sans layout demandé, tout le circuit remonte (usage onglet)"
        );

        let gp = list_backgrounds_in(&dir, "ks_brands_hatch", Some("gp"));
        assert_eq!(gp.len(), 2, "les deux variantes du layout gp");
        assert!(gp.iter().all(|b| b.layout_id.as_deref() == Some("gp")));

        let unknown_layout = list_backgrounds_in(&dir, "ks_brands_hatch", Some("does_not_exist"));
        assert_eq!(
            unknown_layout.len(),
            1,
            "repli sur le background générique du circuit si le layout n'a pas le sien"
        );
        assert_eq!(unknown_layout[0].layout_id, None);
    }

    #[test]
    fn resolve_session_background_follows_fallback_chain() {
        let base = crate::testutil::temp_dir("media-fallback");
        let screens = base.join("screens");
        let ac_install = base.join("ac");
        let bg_dir = ac_install.join("extension").join("backgrounds");

        // Étape 3 seule dispo : aucun screenshot, le background officiel doit
        // être choisi.
        write(&bg_dir.join("ks_imola_901.jpg"));
        // documents_ac_dir n'est pas mockable simplement (lit %USERPROFILE%) —
        // on teste donc les fonctions `_in` séparément, déjà couvertes
        // ci-dessus, plus l'ordre de préférence ici via `list_backgrounds`
        // directement (le chaînon déjà vérifié dans les autres tests).
        let bg_only = list_backgrounds(&ac_install, "ks_imola", None);
        assert_eq!(bg_only.len(), 1);

        // Combo exact vs même circuit seul : la fonction `_in` sous-jacente
        // (list_screenshots_in) est déjà testée plus haut ; ici on vérifie que
        // le combo exact est préféré à un simple match circuit lorsque les
        // deux sont présents dans le même dossier.
        write(&screens.join("Screenshot_other_car_ks_imola_1-1-2026-10-0-0.jpg"));
        write(&screens.join("Screenshot_target_car_ks_imola_1-1-2026-11-0-0.jpg"));
        let shots = list_screenshots_in(&screens, "ks_imola", &ids(&["target_car"]));
        let combo = shots
            .iter()
            .find(|s| s.matched_counterpart.as_deref() == Some("target_car"));
        assert!(
            combo.is_some(),
            "le combo exact doit être identifiable parmi les captures du circuit"
        );
    }
}
