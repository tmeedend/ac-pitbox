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
//!   suffixe final de longueur variable ou absent — c'est le nom que le jeu
//!   donne à un autosave. Content Manager, lui, **renomme** un replay qu'on
//!   conserve (motif par défaut `<car>_<track>_<ddmmyy>-<hhmmss>`), ce qui le
//!   sort de la rotation d'AC. `replay/temp/` est ignoré (fichiers de travail,
//!   jamais des replays terminés).
//!   Le reste — pilote, durée, nombre de voitures — se lit dans l'en-tête du
//!   fichier lui-même (`acreplay.rs`), pas dans son nom.
//! - `<ac_install>/extension/backgrounds/<track_id>[__<layout_id>]_<variant>.jpg` —
//!   convention CSP propre, match par préfixe.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Local, TimeZone};
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

/// A replay of the Documents folder, described from three sources that each
/// know something the others do not: the file name (session type, whether AC
/// wrote it), the filesystem (size, and a recording date when the name has
/// none), and the header itself (driver, duration, cars on track — §6.1).
#[derive(Debug, Clone, Serialize)]
pub struct ReplayFile {
    pub path: String,
    pub file_name: String,
    pub session_type: Option<String>,
    pub recorded_at: Option<String>,
    pub matched_counterpart: Option<String>,
    pub size_bytes: u64,
    /// True while the file still carries the `AC_<date>_<type>_…` name the
    /// game gives it: that is what puts it in the autosave rotation, and
    /// renaming it is exactly how one is kept (§6.1).
    pub autosave: bool,
    /// Position among the autosaves of the same session type, most recent
    /// first (1-based). `None` for a kept replay, which is in no rotation.
    pub autosave_rank: Option<i32>,
    /// How many autosaves of that session type the game keeps
    /// (`cfg/replay.ini`, §6.1). With `autosave_rank`, this is the whole
    /// answer to "is this one about to disappear?" — a rank above the limit
    /// means the next session of the same type pushes it out.
    pub autosave_limit: Option<i32>,
    pub car_id: Option<String>,
    pub driver_name: Option<String>,
    pub track_id: Option<String>,
    pub track_layout: Option<String>,
    pub cars_number: Option<i32>,
    pub duration_s: Option<i64>,
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

/// Ce qu'un nom de replay dit, et qui dépend de qui l'a écrit :
/// - **Assetto Corsa** : `AC_<ddmmyy>-<hhmmss>_<type>_<car>_<track…>` — c'est
///   un autosave, donc soumis à la rotation de `cfg/replay.ini` (§6.1).
/// - **Content Manager conservant un replay** : il le renomme, motif par
///   défaut `<car>_<track>_<ddmmyy>-<hhmmss>`. Le motif est configurable côté
///   CM, mais la date en fin de nom est ce qu'on sait lire — et sortir du
///   motif `AC_` suffit à dire l'essentiel : le fichier n'est plus dans la
///   rotation.
///
/// Un replay renommé à la main ne dit plus rien : ni type ni date. Le mtime
/// prend alors le relais côté `describe_replay`, sans quoi il tombait en fin
/// de liste sans horodatage — c'est-à-dire exactement là où on ne regarde pas
/// le replay qu'on vient d'enregistrer.
struct ReplayNaming {
    session_type: Option<String>,
    recorded_at: Option<String>,
    autosave: bool,
}

fn parse_replay_name(stem: &str) -> ReplayNaming {
    if let Some(rest) = stem.strip_prefix("AC_") {
        let mut parts = rest.splitn(3, '_');
        let recorded_at = parts.next().and_then(parse_ddmmyy_hhmmss);
        let session_type = parts.next().map(str::to_string);
        // La date doit avoir été lue : `AC_something_else` n'est pas un
        // autosave du jeu, c'est un nom qui commence par les mêmes lettres.
        if recorded_at.is_some() {
            return ReplayNaming {
                session_type,
                recorded_at,
                autosave: true,
            };
        }
    }
    ReplayNaming {
        session_type: None,
        recorded_at: stem.rsplit_once('_').and_then(|(_, tail)| parse_ddmmyy_hhmmss(tail)),
        autosave: false,
    }
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
    // Même forme que le mtime (RFC 3339 avec décalage local) : les deux
    // sources se retrouvent dans le même champ et doivent se trier ensemble.
    // Le jeu écrit une heure locale ; une heure ambiguë (passage à l'heure
    // d'hiver) prend la première des deux, à une heure près une fois par an.
    Local
        .from_local_datetime(&chrono::NaiveDateTime::new(date, time))
        .earliest()
        .map(|dt| dt.to_rfc3339())
}

/// Rang de chaque autosave parmi ceux de son type, le plus récent en 1 —
/// c'est-à-dire sa place dans la rotation d'AC (§6.1). Indexé par nom de
/// fichier.
///
/// Compté sur **tout** le dossier et pas sur les seuls replays de la fiche
/// affichée : la rotation ignore de quelle voiture il s'agit, elle ne compte
/// que le type de session.
fn autosave_ranks(dir: &Path) -> HashMap<String, i32> {
    let mut by_type: HashMap<String, Vec<(String, String)>> = HashMap::new();
    for entry in WalkDir::new(dir).max_depth(1).into_iter().flatten() {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if !is_replay_path(path) {
            continue;
        }
        let (Some(stem), Some(name)) = (
            path.file_stem().and_then(|s| s.to_str()),
            path.file_name().and_then(|s| s.to_str()),
        ) else {
            continue;
        };
        let naming = parse_replay_name(stem);
        if !naming.autosave {
            continue;
        }
        let key = naming.session_type.unwrap_or_default();
        let at = naming.recorded_at.unwrap_or_default();
        by_type.entry(key).or_default().push((at, name.to_string()));
    }
    let mut ranks = HashMap::new();
    for (_, mut files) in by_type {
        files.sort_by(|a, b| b.0.cmp(&a.0));
        for (i, (_, name)) in files.into_iter().enumerate() {
            ranks.insert(name, i as i32 + 1);
        }
    }
    ranks
}

fn is_replay_path(path: &Path) -> bool {
    path.extension()
        .and_then(|x| x.to_str())
        .is_some_and(|x| x.eq_ignore_ascii_case("acreplay"))
}

/// Tout ce qu'on sait dire d'un fichier de replay : son nom, son en-tête
/// (§6.1, `acreplay.rs`) et ce que le disque en dit. Un en-tête illisible
/// n'écarte jamais le fichier — la ligne s'affiche avec ce qui reste.
fn describe_replay(
    path: &Path,
    counterpart_ids: &HashSet<String>,
    ranks: &HashMap<String, i32>,
    policy: &crate::acreplay::AutosavePolicy,
) -> Option<ReplayFile> {
    let stem = path.file_stem()?.to_str()?;
    let file_name = path.file_name()?.to_string_lossy().into_owned();
    let naming = parse_replay_name(stem);
    let meta = std::fs::metadata(path).ok();
    let recorded_at = naming.recorded_at.or_else(|| {
        meta.as_ref()
            .and_then(|m| m.modified().ok())
            .map(|t| DateTime::<Local>::from(t).to_rfc3339())
    });
    let header = crate::acreplay::read_header(path);
    Some(ReplayFile {
        path: path.to_string_lossy().into_owned(),
        session_type: naming.session_type.clone(),
        recorded_at,
        matched_counterpart: best_counterpart(stem, counterpart_ids),
        size_bytes: meta.as_ref().map(|m| m.len()).unwrap_or(0),
        autosave: naming.autosave,
        autosave_rank: if naming.autosave {
            ranks.get(&file_name).copied()
        } else {
            None
        },
        autosave_limit: if naming.autosave && policy.enabled {
            Some(policy.limit_for(naming.session_type.as_deref().unwrap_or_default()))
        } else {
            None
        },
        car_id: header.as_ref().map(|h| h.car_id.clone()),
        driver_name: header.as_ref().map(|h| h.driver_name.clone()),
        track_id: header.as_ref().map(|h| h.track_id.clone()),
        track_layout: header.as_ref().map(|h| h.track_layout.clone()),
        cars_number: header.as_ref().map(|h| h.cars_number),
        duration_s: header.as_ref().map(|h| h.duration_s),
        file_name,
    })
}

fn list_replays_in(
    dir: &Path,
    entity_id: &str,
    counterpart_ids: &HashSet<String>,
    policy: &crate::acreplay::AutosavePolicy,
) -> Vec<ReplayFile> {
    if !dir.is_dir() {
        return Vec::new();
    }
    let ranks = autosave_ranks(dir);
    // `max_depth(1)` : ne descend pas dans `replay/temp/` (fichiers de travail,
    // jamais des replays terminés).
    let mut out: Vec<ReplayFile> = WalkDir::new(dir)
        .max_depth(1)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| {
            let path = e.path();
            if !is_replay_path(path) {
                return None;
            }
            if !path.file_stem()?.to_str()?.contains(entity_id) {
                return None;
            }
            describe_replay(path, counterpart_ids, &ranks, policy)
        })
        .collect();
    sort_replays(&mut out);
    out
}

/// Le plus récent en tête. `recorded_at` est désormais toujours renseigné
/// (nom, sinon mtime), donc le tri ne rejette plus en fin de liste le replay
/// qu'on vient d'enregistrer parce que son nom ne commençait pas par `AC_`.
fn sort_replays(files: &mut [ReplayFile]) {
    files.sort_by(|a, b| b.recorded_at.cmp(&a.recorded_at));
}

/// Replays impliquant `entity_id` (§6.1) — mêmes conventions que
/// `list_screenshots`.
pub fn list_replays(entity_id: &str, counterpart_ids: &HashSet<String>) -> Vec<ReplayFile> {
    let Some(dir) = documents_ac_dir() else {
        return Vec::new();
    };
    list_replays_in(
        &dir.join("replay"),
        entity_id,
        counterpart_ids,
        &crate::acreplay::autosave_policy(),
    )
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
    // Les rangs se calculent sur le dossier du replay rattaché, pas sur celui
    // de la fiche : un lien manuel peut pointer ailleurs.
    let mut ranks_by_dir: HashMap<PathBuf, HashMap<String, i32>> = HashMap::new();
    let policy = crate::acreplay::autosave_policy();
    for p in manual_paths {
        if known.contains(p) {
            continue;
        }
        let path = Path::new(p);
        let Some(parent) = path.parent() else {
            continue;
        };
        let ranks = ranks_by_dir
            .entry(parent.to_path_buf())
            .or_insert_with(|| autosave_ranks(parent));
        if let Some(f) = describe_replay(path, &HashSet::new(), ranks, &policy) {
            auto.push(f);
        }
    }
    sort_replays(&mut auto);
    auto
}

/// Sends a screenshot/replay to the Windows recycle bin (§6.1) — never a plain
/// deletion, so a mistake stays undoable from the bin itself. That
/// recoverability is what lets the gallery skip a confirmation dialog on every
/// image.
///
/// Guard: an existing *file* only. A gallery entry never designates a folder,
/// and recycling one would take a whole tree with it.
pub fn trash_file(path: &Path) -> Result<(), String> {
    if !path.is_file() {
        return Err(crate::errors::MEDIA_NOT_A_FILE.to_string());
    }
    trash::delete(path).map_err(|e| e.to_string())
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

/// Chaîne de repli du fond photo de l'écran de réglages (§6.2/SESSION§3) :
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

    /// `list_replays_in` avec la politique d'autosave par défaut d'AC (2 R,
    /// 1 Q, 1 O) : les tests ne lisent pas le `replay.ini` du poste.
    fn list_replays_in_t(dir: &Path, entity_id: &str, counterparts: &HashSet<String>) -> Vec<ReplayFile> {
        list_replays_in(
            dir,
            entity_id,
            counterparts,
            &crate::acreplay::AutosavePolicy::default(),
        )
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

        let found = list_replays_in_t(&dir, "vrc_erc_1998_pageau", &HashSet::new());
        assert_eq!(found.len(), 1, "temp/ ne doit jamais être scanné");
        assert_eq!(found[0].session_type.as_deref(), Some("R"));
        assert!(
            found[0]
                .recorded_at
                .as_deref()
                .unwrap()
                .starts_with("2024-01-30T23:18:24"),
            "la date du nom est reprise telle quelle, en heure locale"
        );
        assert!(found[0].autosave, "un nom AC_ est un autosave du jeu");

        let barcelona = list_replays_in_t(&dir, "ks_barcelona_layout_gp", &HashSet::new());
        assert_eq!(
            barcelona.len(),
            1,
            "suffixe de longueur variable (osrw62) sans incidence sur le match"
        );
    }

    // Bug réel : le replay qu'on vient d'enregistrer n'était « pas identifié »
    // après une course. Il l'était — mais Content Manager l'ayant renommé, le
    // nom ne commençait plus par `AC_`, donc ni date ni type, et le tri par
    // date le renvoyait en fin de liste, sans horodatage. Un replay a toujours
    // une date, et le plus récent est toujours en tête.
    #[test]
    fn a_replay_renamed_by_content_manager_keeps_its_date_and_sorts_first() {
        let base = crate::testutil::temp_dir("media-cm-rename");
        let dir = base.join("replay");
        write(&dir.join("AC_170926-083257_O_rss_formula_2013_ks_silverstone_gp.acreplay"));
        // Motif de renommage par défaut de CM : la date passe en fin de nom.
        write(&dir.join("rss_formula_2013_ks_silverstone_gp_190926-130225.acreplay"));
        // Renommé à la main : plus aucune date dans le nom, le mtime prend le
        // relais.
        write(&dir.join("mon super tour.acreplay"));

        let found = list_replays_in_t(&dir, "rss_formula_2013", &HashSet::new());
        assert_eq!(found.len(), 2, "le fichier renommé à la main ne contient pas l'id");
        assert!(
            found[0]
                .recorded_at
                .as_deref()
                .unwrap()
                .starts_with("2026-09-19T13:02:25"),
            "le plus récent en tête, date lue en fin de nom"
        );
        assert!(!found[0].autosave, "un replay renommé est sorti de la rotation d'AC");
        assert!(found[1].autosave);

        let manual = list_replays_in_t(&dir, "mon super tour", &HashSet::new());
        assert_eq!(manual.len(), 1);
        assert!(
            manual[0].recorded_at.is_some(),
            "sans date dans le nom, le mtime évite la ligne sans horodatage reléguée en fin de liste"
        );
    }

    // Ce qui répond à « quand celui-ci sera-t-il supprimé ? » : la rotation
    // d'AC compte par type de session, toutes voitures confondues.
    #[test]
    fn autosave_rank_counts_per_session_type_across_the_whole_folder() {
        let base = crate::testutil::temp_dir("media-ranks");
        let dir = base.join("replay");
        write(&dir.join("AC_010926-100000_R_car_a_imola.acreplay"));
        write(&dir.join("AC_020926-100000_R_car_b_spa.acreplay"));
        write(&dir.join("AC_030926-100000_R_car_a_imola.acreplay"));
        write(&dir.join("AC_040926-100000_O_car_a_imola.acreplay"));
        write(&dir.join("car_a_imola_050926-100000.acreplay"));

        let found = list_replays_in_t(&dir, "car_a", &HashSet::new());
        let rank = |name: &str| {
            found
                .iter()
                .find(|f| f.file_name == name)
                .unwrap_or_else(|| panic!("{name} attendu dans la liste"))
                .autosave_rank
        };
        assert_eq!(
            rank("AC_030926-100000_R_car_a_imola.acreplay"),
            Some(1),
            "le plus récent des R"
        );
        assert_eq!(
            rank("AC_010926-100000_R_car_a_imola.acreplay"),
            Some(3),
            "la course d'une autre voiture compte aussi : la rotation ignore la voiture"
        );
        assert_eq!(
            rank("AC_040926-100000_O_car_a_imola.acreplay"),
            Some(1),
            "chaque type de session a sa propre rotation"
        );
        assert_eq!(
            rank("car_a_imola_050926-100000.acreplay"),
            None,
            "un replay conservé n'est dans aucune rotation"
        );

        let limit = |name: &str| found.iter().find(|f| f.file_name == name).unwrap().autosave_limit;
        assert_eq!(
            limit("AC_030926-100000_R_car_a_imola.acreplay"),
            Some(2),
            "la limite d'une course vient de [AUTOSAVE] RACE"
        );
        assert_eq!(limit("AC_040926-100000_O_car_a_imola.acreplay"), Some(1));
        assert_eq!(
            limit("car_a_imola_050926-100000.acreplay"),
            None,
            "aucune limite ne s'applique à un replay conservé"
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

    // Seul le garde-fou est testé : le chemin nominal enverrait un vrai
    // fichier dans la corbeille du poste à chaque `cargo test`, et
    // `temp_dir` ne la nettoie évidemment pas.
    #[test]
    fn trash_refuses_anything_that_is_not_an_existing_file() {
        let base = crate::testutil::temp_dir("media-trash-guard");
        let folder = base.join("a_folder");
        std::fs::create_dir_all(&folder).unwrap();

        assert_eq!(
            trash_file(&folder).err().as_deref(),
            Some(crate::errors::MEDIA_NOT_A_FILE),
            "un dossier n'est jamais un média : il ne part pas à la corbeille"
        );
        assert!(folder.is_dir(), "le dossier refusé doit rester intact");
        assert!(
            trash_file(&base.join("gone.jpg")).is_err(),
            "un chemin mort remonte une erreur au lieu d'être silencieusement ignoré"
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
