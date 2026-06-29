//! Extraction d'archives (zip/rar/7z) via le 7-Zip configuré, et déplacement
//! de dossiers entre volumes — porté de la logique de `common.py`.

use std::path::Path;
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

/// Évite l'ouverture d'une fenêtre de console à chaque appel 7-Zip.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Extrait `archive` dans `dest` via `7z x`. `dest` doit exister.
pub fn extract(sevenzip: &Path, archive: &Path, dest: &Path) -> Result<(), String> {
    let mut cmd = Command::new(sevenzip);
    cmd.arg("x")
        .arg(archive)
        .arg(format!("-o{}", dest.display()))
        .arg("-y");
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = cmd
        .output()
        .map_err(|e| format!("impossible de lancer 7-Zip : {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!(
            "7-Zip a échoué (code {:?}) : {}",
            output.status.code(),
            stderr.trim()
        ))
    }
}

/// Crée une archive `.7z` à partir du **contenu** de `src_dir` (chemins relatifs
/// préservés), via `7z a`. Utilisé par l'export autonome (§9.1).
pub fn create_7z(sevenzip: &Path, src_dir: &Path, archive: &Path) -> Result<(), String> {
    if archive.exists() {
        let _ = std::fs::remove_file(archive);
    }
    let mut cmd = Command::new(sevenzip);
    cmd.current_dir(src_dir)
        .arg("a")
        .arg("-t7z")
        .arg(archive)
        .arg("*")
        .arg("-y");
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = cmd
        .output()
        .map_err(|e| format!("impossible de lancer 7-Zip : {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "7-Zip a échoué (code {:?}) : {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

/// Déplace un dossier : `rename` si même volume, sinon copie récursive + suppression.
pub fn move_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if std::fs::rename(src, dst).is_ok() {
        return Ok(());
    }
    copy_dir(src, dst)?;
    std::fs::remove_dir_all(src)?;
    Ok(())
}

/// Copie récursive d'un dossier (fallback inter-volumes).
pub fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_dir(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}
