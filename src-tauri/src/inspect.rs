//! Inspection d'un dossier de mod (lecture seule) : features CSP, skins, layouts.
//! Porté de la détection CSP de `tracks.py` (sections d'`ext_config.ini`).

use std::path::{Path, PathBuf};

use chrono::{DateTime, Local};
use walkdir::WalkDir;

use crate::modscan::ModKind;

/// Détecte les sections CSP pertinentes dans un texte de config déjà lu
/// (`ext_config.ini` du mod, ou config CSP "chargée" séparément — voir
/// `csp_features_loaded`). Facteur commun des deux sources.
fn parse_csp_features(text: &str) -> Vec<String> {
    let mut feats = Vec::new();
    let upper = text.to_uppercase();
    if upper.contains("[GRASS_FX") {
        feats.push("grassfx".to_string());
    }
    if upper.contains("[RAIN_FX") {
        feats.push("rainfx".to_string());
    }
    if upper.contains("[LIGHT_SERIES_1") {
        feats.push("lightingfx".to_string());
    }
    // Ajustements saisonniers (arbres/herbe qui changent de couleur, §6.4bis) :
    // le circuit doit avoir un bloc dédié référençant les conditions
    // SEASON_*_NORTH (cf. extension/config/tracks/common/conditions.ini) —
    // sans ça, choisir une saison dans l'app n'a aucun effet visuel.
    if upper.contains("SEASON_WINTER") || upper.contains("SEASON_AUTUMN") || upper.contains("SEASON_SUMMER") {
        feats.push("season".to_string());
    }
    feats
}

/// Détecte les features CSP en lisant le `ext_config.ini` embarqué du mod.
/// (À l'import, le mod n'est pas encore installé : on lit sa propre config.)
pub fn csp_features(mod_dir: &Path) -> Vec<String> {
    let mut feats = Vec::new();
    let candidates = [
        mod_dir.join("extension").join("ext_config.ini"),
        mod_dir.join("ext_config.ini"),
    ];
    for cfg in candidates {
        if let Ok(text) = std::fs::read_to_string(&cfg) {
            feats.extend(parse_csp_features(&text));
        }
    }
    feats.sort();
    feats.dedup();
    feats
}

/// Complète la détection avec la config CSP "chargée" séparément par CSP pour
/// ce contenu — `extension/config/{cars,tracks}/loaded/<id>.ini`, un dossier
/// PARTAGÉ hors du mod. C'est là que vivent les configs CSP du contenu de
/// base Kunos (téléchargées par CSP depuis son dépôt communautaire
/// `acc-extension-config`, pas fournies par Kunos) — sans ce dossier, la
/// détection CSP du contenu de base est systématiquement vide même quand CSP
/// gère bien pluie/saisons pour ce circuit. Peut aussi s'appliquer à des mods
/// tiers déjà répertoriés par ce même dépôt communautaire.
pub fn csp_features_loaded(ac_install_path: &Path, kind: ModKind, id: &str) -> Vec<String> {
    let sub = match kind {
        ModKind::Car => "cars",
        ModKind::Track => "tracks",
    };
    let path = ac_install_path
        .join("extension")
        .join("config")
        .join(sub)
        .join("loaded")
        .join(format!("{id}.ini"));
    std::fs::read_to_string(&path)
        .map(|t| parse_csp_features(&t))
        .unwrap_or_default()
}

/// Skins d'une voiture : sous-dossiers de `skins/`.
pub fn car_skins(car_dir: &Path) -> Vec<String> {
    list_subdirs(&car_dir.join("skins"))
}

/// Layouts d'un circuit : sous-dossiers de `ui/` contenant un `ui_track.json`.
/// Renvoie un layout par défaut si le circuit est mono-layout.
pub fn track_layouts(track_dir: &Path) -> Vec<String> {
    let ui = track_dir.join("ui");
    let mut layouts = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&ui) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() && p.join("ui_track.json").is_file() {
                if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                    layouts.push(name.to_string());
                }
            }
        }
    }
    layouts.sort();
    if layouts.is_empty() && ui.join("ui_track.json").is_file() {
        // Mono-layout : chaîne vide, PAS "(default)" — cette valeur finit dans
        // CONFIG_TRACK du race.ini (voir launch::build_race_ini) ; un texte
        // littéral y casse le lancement (CM cherche un dossier de layout
        // nommé "(default)", qui n'existe pas). Même convention que l'id vide
        // utilisé par uijson::read_track_detail pour ce cas.
        layouts.push(String::new());
    }
    layouts
}

/// Date de publication estimée (§6.2) : date de modification la plus récente
/// parmi les fichiers du mod, lue **avant** rangement en bibliothèque. Pour une
/// archive, `dir` est le dossier d'extraction temporaire — 7-Zip restitue sur
/// les fichiers extraits les dates internes de l'archive, donc la valeur
/// obtenue reflète ces dates internes, pas l'instant de l'extraction.
pub fn estimate_published_at(dir: &Path) -> Option<String> {
    let newest = WalkDir::new(dir)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.metadata().ok()?.modified().ok())
        .max()?;
    let dt: DateTime<Local> = newest.into();
    Some(dt.to_rfc3339())
}

fn list_subdirs(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            if e.path().is_dir() {
                if let Some(name) = e.file_name().to_str() {
                    out.push(name.to_string());
                }
            }
        }
    }
    out.sort();
    out
}

/// Vignettes de preview AC : presque toujours `.jpg`, parfois `.png`.
const PREVIEW_NAMES: [&str; 2] = ["preview.jpg", "preview.png"];
const OUTLINE_NAMES: [&str; 4] = ["outline.png", "outline.jpg", "preview.png", "preview.jpg"];

fn first_existing(dir: &Path, names: &[&str]) -> Option<String> {
    for n in names {
        let p = dir.join(n);
        if p.is_file() {
            return Some(p.to_string_lossy().into_owned());
        }
    }
    None
}

/// Cherche une image de circuit (`names`) dans `ui/`, puis dans le premier
/// sous-dossier de layout qui en contient une.
fn track_find(mod_dir: &Path, names: &[&str]) -> Option<String> {
    let ui = mod_dir.join("ui");
    if let Some(p) = first_existing(&ui, names) {
        return Some(p);
    }
    std::fs::read_dir(&ui)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .find_map(|p| first_existing(&p, names))
}

/// Image illustratrice d'un circuit (`preview.png`) — le fond de la vignette.
pub fn track_preview(mod_dir: &Path) -> Option<String> {
    track_find(mod_dir, &["preview.png", "preview.jpg"])
}

/// Tracé d'un circuit (`outline.png`/`map.png`) — superposé à la photo (§6.1).
pub fn track_outline(mod_dir: &Path) -> Option<String> {
    track_find(mod_dir, &["outline.png", "outline.jpg", "map.png"])
}

/// Badge/logo de la marque (`ui/badge.png`), presque toujours livré avec le mod.
pub fn brand_badge(car_dir: &Path) -> Option<String> {
    first_existing(&car_dir.join("ui"), &["badge.png", "badge.jpg"])
}

/// Chemin absolu d'une vignette de preview pour la galerie (§6.1), si trouvée.
/// Voiture : preview du premier skin qui en a une. Circuit : outline/preview.
pub fn preview_path(kind: ModKind, mod_dir: &Path) -> Option<String> {
    match kind {
        ModKind::Car => {
            let mut skins: Vec<PathBuf> = std::fs::read_dir(mod_dir.join("skins"))
                .ok()?
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect();
            skins.sort();
            skins.iter().find_map(|s| first_existing(s, &PREVIEW_NAMES))
        }
        ModKind::Track => {
            let ui = mod_dir.join("ui");
            if let Some(p) = first_existing(&ui, &OUTLINE_NAMES) {
                return Some(p);
            }
            // layouts : premier sous-dossier ui/ avec un outline.
            let entries = std::fs::read_dir(&ui).ok()?;
            entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .find_map(|p| first_existing(&p, &OUTLINE_NAMES))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mono_layout_track_reports_empty_id_not_placeholder_text() {
        // Un circuit mono-layout (ui/ui_track.json à la racine, pas de
        // sous-dossier) doit renvoyer une chaîne vide, pas un texte littéral
        // du genre "(default)" : cette valeur finit telle quelle dans
        // CONFIG_TRACK du race.ini (launch::build_race_ini) — un texte non
        // vide y fait chercher à Content Manager un dossier de layout qui
        // n'existe pas (cas réel : circuit Deutschlandring de Fat-Alfie).
        let dir = std::env::temp_dir().join(format!("pitbox-inspect-mono-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("ui")).unwrap();
        std::fs::write(dir.join("ui").join("ui_track.json"), b"{\"name\":\"Test\"}").unwrap();

        let layouts = track_layouts(&dir);
        assert_eq!(layouts, vec![String::new()]);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn multi_layout_track_reports_subfolder_names() {
        let dir = std::env::temp_dir().join(format!("pitbox-inspect-multi-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("ui").join("layout_a")).unwrap();
        std::fs::create_dir_all(dir.join("ui").join("layout_b")).unwrap();
        std::fs::write(dir.join("ui").join("layout_a").join("ui_track.json"), b"{}").unwrap();
        std::fs::write(dir.join("ui").join("layout_b").join("ui_track.json"), b"{}").unwrap();

        let layouts = track_layouts(&dir);
        assert_eq!(layouts, vec!["layout_a".to_string(), "layout_b".to_string()]);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn loaded_config_detects_rain_and_season_outside_mod_folder() {
        // Le contenu de base Kunos n'a pas ces sections dans son propre
        // dossier — elles vivent dans la config CSP "chargée" séparément
        // (§6.4bis), ex. extension/config/tracks/loaded/<id>.ini. Reproduit
        // un extrait réel (magione.ini) : RainFX + ajustements saisonniers.
        let ac = std::env::temp_dir().join(format!("pitbox-inspect-loaded-{}", uuid::Uuid::new_v4()));
        let loaded_dir = ac.join("extension").join("config").join("tracks").join("loaded");
        std::fs::create_dir_all(&loaded_dir).unwrap();
        std::fs::write(
            loaded_dir.join("magione.ini"),
            "[GRASS_FX]\nGRASS_MATERIALS=grass-shad\n\n[MATERIAL_ADJUSTMENT_6]\nCONDITION = SEASON_WINTER_NORTH\n",
        )
        .unwrap();

        let feats = csp_features_loaded(&ac, ModKind::Track, "magione");
        assert!(feats.contains(&"grassfx".to_string()));
        assert!(feats.contains(&"season".to_string()));

        // Un circuit sans config "chargée" (fichier absent) ne renvoie rien,
        // sans planter.
        assert!(csp_features_loaded(&ac, ModKind::Track, "unknown_track").is_empty());

        let _ = std::fs::remove_dir_all(&ac);
    }
}
