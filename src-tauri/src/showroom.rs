//! Aperçu 3D natif via `acShowroom.exe`, à la racine de l'install AC —
//! distinct de Content Manager, pas de dépendance à CM (voir recherche dans
//! `docs/showroom-3d-preview-research.md`, piste 3).
//!
//! **Process indépendant, plein écran.** L'intégration de la fenêtre native
//! dans la page (reparentage dans une fenêtre overlay, mode fenêtré forcé via
//! `video.ini`) a été tentée puis abandonnée : trop de problèmes en pratique
//! (WebView2 compose son rendu par-dessus toute fenêtre native sœur, la
//! fenêtre d'acShowroom est détruite/recréée pendant l'init DirectX, le
//! `video.ini` du **vrai jeu** devait être altéré puis restauré…). Pit Box se
//! contente désormais de lancer le showroom avec ses propres réglages vidéo ;
//! l'utilisateur le ferme lui-même pour revenir à l'app.
//!
//! Configuration par fichier INI (`showroom_start.ini`), même principe que
//! `race.ini` : on écrit, on lance, AC lit. Aucun fichier du jeu n'est altéré
//! durablement — `video.ini` n'est plus touché du tout.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use serde::Serialize;

use crate::config::AppConfig;
use crate::uijson;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Scène de repli quand aucune n'est choisie dans les réglages : de loin la
/// plus légère (17 Ko contre 100–300 Mo pour showroom/Hangar/industrial/beach)
/// et **sans piste audio** (pas de .bank/.wav) → chargement quasi instantané et
/// aucune musique à couper.
pub const DEFAULT_SHOWROOM: &str = "studio_white";

/// Nom de la sauvegarde `video.ini` laissée par les versions de Pit Box qui
/// forçaient le mode fenêtré. Plus jamais écrite ; seulement restaurée (voir
/// `restore_orphaned_video_ini`).
const LEGACY_BACKUP_NAME: &str = "video.ini.pitbox-backup";

fn resolve_ac_cfg_dir() -> Option<PathBuf> {
    Some(dirs::document_dir()?.join("Assetto Corsa").join("cfg"))
}

/// Reproduit fidèlement le fichier généré par AC lui-même (capturé pendant
/// une session native réelle) — seuls `CAR`/`SKIN`/`TRACK` changent. Une version
/// tronquée (sans `NEAR_PLANE`/`FAR_PLANE`/les splits d'ombre) a provoqué un
/// écran noir en test : mieux vaut garder l'intégralité des clés connues que
/// deviner lesquelles sont réellement optionnelles.
fn write_showroom_ini_at(cfg_dir: &Path, car_id: &str, skin_id: Option<&str>, scene: &str) -> Result<(), String> {
    fs::create_dir_all(cfg_dir).map_err(|e| format!("dossier de config AC : {e}"))?;
    let skin = skin_id.unwrap_or("");
    let content = format!(
        "[SHOWROOM]\r\n\
         CAR={car_id}\r\n\
         SKIN={skin}\r\n\
         ALLOW_SELECT_SKIN=1\r\n\
         TRACK={scene}\r\n\
         SELECTED_SKIN=1\r\n\
         CAR_ID=0\r\n\
         \r\n\
         [FADES]\r\n\
         ENTER_EXIT_MS=0\r\n\
         \r\n\
         [PREVIEW_MODE]\r\n\
         LOOK_AT=0,0.6,0\r\n\
         CUSTOM_CAMERA_POSITION=-0.366574,0.775145,-6.12493\r\n\
         USE_CUSTOM_CAMERA=1\r\n\
         CUSTOM_CAMERA_ROLL=0\r\n\
         CUSTOM_CAMERA_EXPOSURE=94.5\r\n\
         \r\n\
         [ANIMATION]\r\n\
         MUL=0.15\r\n\
         \r\n\
         [SETTINGS]\r\n\
         ROTATION_SPEED=1.0\r\n\
         CAMERA_DISTANCE=6\r\n\
         CAMERA_HEIGHT=1.5\r\n\
         CAMERA_FOV=30\r\n\
         CAMERA_EXPOSURE=30\r\n\
         SUN_ANGLE=-50\r\n\
         SHADOW_SPLIT0=2\r\n\
         SHADOW_SPLIT1=12\r\n\
         SHADOW_SPLIT2=50\r\n\
         NEAR_PLANE=0.01\r\n\
         FAR_PLANE=200\r\n\
         MIN_EXPOSURE=0.2\r\n\
         MAX_EXPOSURE=10000\r\n"
    );
    fs::write(cfg_dir.join("showroom_start.ini"), content).map_err(|e| format!("écriture showroom_start.ini : {e}"))
}

/// Restaure `video.ini` si une sauvegarde d'une **ancienne** version de Pit Box
/// traîne encore (l'aperçu intégré forçait le mode fenêtré 1280×720 et le
/// filtre `photographic`). Appelé une fois au démarrage : sans lui, un
/// utilisateur dont la session d'aperçu s'était mal terminée resterait coincé
/// avec ces réglages dans le vrai jeu. No-op dans tous les autres cas.
pub fn restore_orphaned_video_ini() {
    let Some(dir) = resolve_ac_cfg_dir() else { return };
    let backup = dir.join(LEGACY_BACKUP_NAME);
    if !backup.exists() {
        return;
    }
    if let Ok(original) = fs::read_to_string(&backup) {
        if fs::write(dir.join("video.ini"), original).is_ok() {
            let _ = fs::remove_file(&backup);
        }
    }
}

/// Une scène de showroom installée (`content/showroom/<id>`), telle que
/// proposée dans les réglages.
#[derive(Debug, Clone, Serialize)]
pub struct ShowroomOption {
    /// Nom de dossier — c'est lui qui part dans `TRACK=` du `showroom_start.ini`.
    pub id: String,
    /// Nom lisible (`ui/ui_showroom.json`), repli sur l'id si absent.
    pub name: String,
}

/// Showrooms installés dans AC, triés par nom lisible. Liste vide si le dossier
/// AC n'est pas configuré (les réglages affichent alors le seul défaut).
pub fn list_showrooms(cfg: &AppConfig) -> Vec<ShowroomOption> {
    let Some(ac) = &cfg.ac_install_path else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(ac.join("content").join("showroom")) {
        for e in entries.flatten() {
            let path = e.path();
            if !path.is_dir() {
                continue;
            }
            let Some(id) = e.file_name().to_str().map(str::to_string) else {
                continue;
            };
            let name = uijson::read_showroom_name(&path).unwrap_or_else(|| id.clone());
            out.push(ShowroomOption { id, name });
        }
    }
    out.sort_by_key(|a| a.name.to_lowercase());
    out
}

/// Lance `acShowroom.exe` ciblé sur `car_id` (+ skin optionnel), en process
/// indépendant : il s'affiche par-dessus Pit Box avec les réglages vidéo du
/// jeu (donc plein écran si c'est ce que l'utilisateur a configuré dans AC), et
/// c'est lui qui le ferme pour revenir à l'app. Rien à suivre côté Pit Box —
/// pas de PID mémorisé, pas de fenêtre à repositionner.
pub fn open_native_showroom(cfg: &AppConfig, car_id: &str, skin_id: Option<&str>) -> Result<(), String> {
    let ac = cfg.ac_install_path.as_ref().ok_or(crate::errors::AC_NOT_CONFIGURED)?.clone();
    // Garde-fou : `acShowroom.exe` cherche `data/lods.ini` sous `content/cars/<id>`
    // et plante (fenêtre d'erreur native, hors de notre contrôle) si `car_id`
    // n'est pas une vraie voiture — ex. l'id d'un circuit passé par erreur
    // depuis le front. Mieux vaut une erreur propre ici qu'un crash natif.
    if !crate::modscan::is_car(&ac.join("content").join("cars").join(car_id)) {
        return Err(format!("« {car_id} » n'est pas une voiture valide — aperçu 3D indisponible"));
    }
    let exe = ac.join("acShowroom.exe");
    if !exe.is_file() {
        return Err(crate::errors::SHOWROOM_EXE_MISSING.into());
    }
    let cfg_dir = resolve_ac_cfg_dir().ok_or(crate::errors::DOCUMENTS_NOT_FOUND)?;

    // Scène choisie dans les réglages, si elle est toujours installée : un
    // showroom désinstallé entre-temps ferait planter acShowroom au chargement.
    let scene = cfg
        .prefs
        .showroom_scene
        .as_deref()
        .filter(|s| ac.join("content").join("showroom").join(s).is_dir())
        .unwrap_or(DEFAULT_SHOWROOM)
        .to_string();

    write_showroom_ini_at(&cfg_dir, car_id, skin_id, &scene)?;

    let mut cmd = Command::new(&exe);
    // `acShowroom.exe` résout ses chemins relativement au répertoire courant.
    cmd.current_dir(&ac);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd.spawn().map_err(|e| format!("lancement d'acShowroom.exe : {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_showroom_ini_with_car_skin_and_scene() {
        let base = crate::testutil::temp_dir("showroom");
        write_showroom_ini_at(&base, "ks_toyota_celica_st185", Some("00_racing_3"), "Hangar").unwrap();
        let content = fs::read_to_string(base.join("showroom_start.ini")).unwrap();
        assert!(content.contains("CAR=ks_toyota_celica_st185"));
        assert!(content.contains("SKIN=00_racing_3"));
        assert!(content.contains("[SHOWROOM]"));
        // Scène choisie dans les réglages, pas le défaut codé en dur.
        assert!(content.contains("TRACK=Hangar"));
        // Clés de clipping/ombres : leur absence a provoqué un écran noir en test réel.
        assert!(content.contains("NEAR_PLANE=0.01"));
        assert!(content.contains("FAR_PLANE=200"));
        assert!(content.contains("[FADES]"));
        assert!(content.contains("[ANIMATION]"));
    }

    #[test]
    fn lists_installed_showrooms_by_readable_name() {
        let base = crate::testutil::temp_dir("showlist");
        let rooms = base.join("content").join("showroom");
        for (id, name) in [("studio_white", "Showroom White"), ("Hangar", "Hangar")] {
            fs::create_dir_all(rooms.join(id).join("ui")).unwrap();
            fs::write(
                rooms.join(id).join("ui").join("ui_showroom.json"),
                format!("{{\n\t\"name\":\"{name}\"\t\n}}"),
            )
            .unwrap();
        }
        // Sans ui_showroom.json : l'id fait office de nom.
        fs::create_dir_all(rooms.join("custom_room")).unwrap();

        let cfg = AppConfig { ac_install_path: Some(base.to_path_buf()), ..Default::default() };
        let list = list_showrooms(&cfg);
        let ids: Vec<&str> = list.iter().map(|s| s.id.as_str()).collect();
        let names: Vec<&str> = list.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["custom_room", "Hangar", "Showroom White"], "tri par nom lisible");
        assert_eq!(ids, vec!["custom_room", "Hangar", "studio_white"], "l'id reste le nom de dossier");
    }
}
