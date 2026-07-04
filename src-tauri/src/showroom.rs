//! Aperçu 3D natif via `acShowroom.exe`, à la racine de l'install AC —
//! distinct de Content Manager, pas de dépendance à CM (voir recherche dans
//! `docs/showroom-3d-preview-research.md`, piste 3).
//!
//! Configuration par fichier INI (`showroom_start.ini`), même principe que
//! `race.ini` : on écrit, on lance, AC lit. Le mode fenêtré (nécessaire pour
//! un futur embed dans la page) est piloté par `video.ini` — **le même
//! fichier que le vrai jeu**. Règle d'or : jamais laissé altéré durablement.
//! `video.ini` est sauvegardé avant modification et restauré dès la
//! fermeture du showroom (process tué ou fenêtré fermée par l'utilisateur),
//! avec un filet de sécurité au démarrage de l'app si une sauvegarde traîne
//! suite à un crash.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::AppConfig;

const BACKUP_NAME: &str = "video.ini.pitbox-backup";

fn resolve_ac_cfg_dir() -> Option<PathBuf> {
    Some(dirs::document_dir()?.join("Assetto Corsa").join("cfg"))
}

/// Reproduit fidèlement le fichier généré par AC lui-même (capturé pendant
/// une session native réelle) — seuls `CAR`/`SKIN` changent. Une version
/// tronquée (sans `NEAR_PLANE`/`FAR_PLANE`/les splits d'ombre) a provoqué un
/// écran noir en test : mieux vaut garder l'intégralité des clés connues que
/// deviner lesquelles sont réellement optionnelles.
fn write_showroom_ini_at(cfg_dir: &Path, car_id: &str, skin_id: Option<&str>) -> Result<(), String> {
    fs::create_dir_all(cfg_dir).map_err(|e| format!("dossier de config AC : {e}"))?;
    let skin = skin_id.unwrap_or("");
    let content = format!(
        "[SHOWROOM]\r\n\
         CAR={car_id}\r\n\
         SKIN={skin}\r\n\
         ALLOW_SELECT_SKIN=1\r\n\
         TRACK=showroom\r\n\
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

/// Force le mode fenêtré et une taille modeste dans `video.ini` : `FULLSCREEN`,
/// `WIDTH`, `HEIGHT`. Sans réduire aussi la taille, une fenêtre "sans bordure"
/// à la résolution du bureau (ex. 3840×2160) reste visuellement indiscernable
/// du plein écran. Toutes les autres clés (refresh, anti-aliasing…) restent
/// intactes.
fn force_windowed(original: &str) -> String {
    original
        .lines()
        .map(|l| {
            let key = l.trim_start().split('=').next().unwrap_or("");
            match key {
                "FULLSCREEN" => "FULLSCREEN=0",
                "WIDTH" => "WIDTH=1280",
                "HEIGHT" => "HEIGHT=720",
                _ => l,
            }
        })
        .collect::<Vec<_>>()
        .join("\r\n")
}

/// Sauvegarde `video.ini` puis le bascule en fenêtré. No-op si une
/// sauvegarde existe déjà (session précédente pas fermée proprement : on ne
/// veut surtout pas écraser l'original par une copie déjà modifiée).
fn backup_and_force_windowed_at(cfg_dir: &Path) -> Result<(), String> {
    let path = cfg_dir.join("video.ini");
    let backup = cfg_dir.join(BACKUP_NAME);
    if backup.exists() {
        return Ok(());
    }
    let original = fs::read_to_string(&path).map_err(|e| format!("lecture video.ini : {e}"))?;
    fs::write(&backup, &original).map_err(|e| format!("sauvegarde video.ini : {e}"))?;
    fs::write(&path, force_windowed(&original)).map_err(|e| format!("écriture video.ini : {e}"))?;
    Ok(())
}

/// Restaure `video.ini` depuis la sauvegarde si elle existe. No-op sinon —
/// appelable sans risque à la fermeture du showroom et au démarrage de l'app.
fn restore_video_ini_at(cfg_dir: &Path) -> Result<(), String> {
    let path = cfg_dir.join("video.ini");
    let backup = cfg_dir.join(BACKUP_NAME);
    if !backup.exists() {
        return Ok(());
    }
    let original = fs::read_to_string(&backup).map_err(|e| format!("lecture sauvegarde video.ini : {e}"))?;
    fs::write(&path, original).map_err(|e| format!("restauration video.ini : {e}"))?;
    fs::remove_file(&backup).map_err(|e| format!("suppression sauvegarde video.ini : {e}"))?;
    Ok(())
}

/// Filet de sécurité à appeler une fois au démarrage de l'app : restaure
/// `video.ini` si une sauvegarde traîne (Pit Box ou le showroom ont été tués
/// avant la restauration normale).
pub fn restore_orphaned_video_ini() {
    if let Some(dir) = resolve_ac_cfg_dir() {
        let _ = restore_video_ini_at(&dir);
    }
}

/// Lance `acShowroom.exe` ciblé sur `car_id` (+ skin optionnel). Bascule
/// `video.ini` en fenêtré le temps de la session, restauré automatiquement
/// à la fermeture (fenêtre fermée par l'utilisateur ou process tué).
pub fn open_native_showroom(cfg: &AppConfig, car_id: &str, skin_id: Option<&str>) -> Result<(), String> {
    let ac = cfg.ac_install_path.as_ref().ok_or("dossier AC non configuré")?.clone();
    let exe = ac.join("acShowroom.exe");
    if !exe.is_file() {
        return Err("acShowroom.exe introuvable dans le dossier d'installation AC".into());
    }
    let cfg_dir = resolve_ac_cfg_dir().ok_or("dossier Documents introuvable")?;

    write_showroom_ini_at(&cfg_dir, car_id, skin_id)?;
    backup_and_force_windowed_at(&cfg_dir)?;

    let mut child = Command::new(&exe)
        .current_dir(&ac)
        .spawn()
        .map_err(|e| format!("lancement d'acShowroom.exe : {e}"))?;

    // Restauration dès la fermeture du showroom, quelle que soit la cause
    // (fenêtre fermée par l'utilisateur, ou process tué).
    std::thread::spawn(move || {
        let _ = child.wait();
        if let Some(dir) = resolve_ac_cfg_dir() {
            let _ = restore_video_ini_at(&dir);
        }
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_showroom_ini_with_car_and_skin() {
        let base = std::env::temp_dir().join(format!("pitbox-showroom-{}", uuid::Uuid::new_v4()));
        write_showroom_ini_at(&base, "ks_toyota_celica_st185", Some("00_racing_3")).unwrap();
        let content = fs::read_to_string(base.join("showroom_start.ini")).unwrap();
        assert!(content.contains("CAR=ks_toyota_celica_st185"));
        assert!(content.contains("SKIN=00_racing_3"));
        assert!(content.contains("[SHOWROOM]"));
        // Clés de clipping/ombres : leur absence a provoqué un écran noir en test réel.
        assert!(content.contains("NEAR_PLANE=0.01"));
        assert!(content.contains("FAR_PLANE=200"));
        assert!(content.contains("[FADES]"));
        assert!(content.contains("[ANIMATION]"));
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn force_windowed_shrinks_size_and_leaves_other_keys() {
        let original = "[VIDEO]\r\nFULLSCREEN=1\r\nWIDTH=3840\r\nHEIGHT=2160\r\nREFRESH=144\r\n";
        let windowed = force_windowed(original);
        assert!(windowed.contains("FULLSCREEN=0"));
        assert!(windowed.contains("WIDTH=1280"));
        assert!(windowed.contains("HEIGHT=720"));
        assert!(windowed.contains("REFRESH=144"));
        assert!(!windowed.contains("FULLSCREEN=1"));
        assert!(!windowed.contains("WIDTH=3840"));
        assert!(!windowed.contains("HEIGHT=2160"));
    }

    #[test]
    fn backup_and_restore_video_ini_roundtrip() {
        let base = std::env::temp_dir().join(format!("pitbox-video-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&base).unwrap();
        let original = "[VIDEO]\r\nFULLSCREEN=1\r\nWIDTH=3840\r\nHEIGHT=2160\r\n";
        fs::write(base.join("video.ini"), original).unwrap();

        backup_and_force_windowed_at(&base).unwrap();
        let modified = fs::read_to_string(base.join("video.ini")).unwrap();
        assert!(modified.contains("FULLSCREEN=0"));
        assert!(base.join(BACKUP_NAME).is_file());

        // Un second appel (session déjà en cours) ne doit pas écraser la sauvegarde.
        fs::write(base.join("video.ini"), "FULLSCREEN=0\r\nCORRUPTED=1\r\n").unwrap();
        backup_and_force_windowed_at(&base).unwrap();
        let backup_content = fs::read_to_string(base.join(BACKUP_NAME)).unwrap();
        assert_eq!(backup_content, original);

        restore_video_ini_at(&base).unwrap();
        let restored = fs::read_to_string(base.join("video.ini")).unwrap();
        assert_eq!(restored, original);
        assert!(!base.join(BACKUP_NAME).exists());

        // Restaurer sans sauvegarde présente est un no-op sûr.
        restore_video_ini_at(&base).unwrap();

        let _ = fs::remove_dir_all(&base);
    }
}
