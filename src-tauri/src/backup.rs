//! Sauvegarde automatique de la base et des petits fichiers de préférences
//! (§6.2/§9.4) : copie best-effort dans un sous-dossier horodaté à chaque
//! démarrage, avec rotation sur les `BACKUP_KEEP` plus récentes. Filet de
//! sécurité contre une base corrompue ou un fichier de préférences écrasé
//! par erreur — pas un vrai système de restauration point-in-time, juste
//! « une copie récente existe quelque part si le pire arrive ». Restauration
//! manuelle pour l'instant : ouvrir `app_config_dir/backups/<horodatage>/`
//! et recopier les fichiers voulus, app fermée.

use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};

const BACKUP_KEEP: usize = 7;

/// Tout ce qui vit directement dans `app_config_dir` et n'est pas
/// régénérable automatiquement (le cache de miniatures ou les logs, par
/// exemple, ne le sont pas ici : ils se reconstruisent tout seuls).
const BACKED_UP_FILES: &[&str] = &[
    "overlay.sqlite",
    "config.json",
    "ui_prefs.json",
    "library_columns.json",
    "session.json",
    "launch_state.json",
    "saved_sessions.json",
    "music.json",
    "tag-rules.json",
];

fn backups_root(base: &Path) -> PathBuf {
    base.join("backups")
}

/// Appelée une fois au démarrage (`lib.rs`, avant l'ouverture de la connexion
/// SQLite — on veut la base exactement telle que la session précédente l'a
/// laissée). Ne remonte jamais d'erreur bloquante : un échec (disque plein,
/// permission) ne doit jamais empêcher le démarrage de l'app, seulement
/// laisser une trace pour rester diagnosticable sur une install packagée
/// (règle d'or n°… du CLAUDE.md).
pub fn run_startup_backup(app: &AppHandle) {
    let Ok(base) = app.path().app_config_dir() else {
        log::warn!("backup: app_config_dir indisponible, sauvegarde de démarrage ignorée");
        return;
    };
    if let Err(e) = backup_now(&base) {
        log::warn!("backup: sauvegarde de démarrage échouée : {e}");
    }
}

fn backup_now(base: &Path) -> Result<(), String> {
    let root = backups_root(base);
    let stamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
    let dest = root.join(&stamp);
    std::fs::create_dir_all(&dest).map_err(|e| e.to_string())?;

    let mut copied = 0;
    for name in BACKED_UP_FILES {
        let src = base.join(name);
        if !src.is_file() {
            continue;
        }
        std::fs::copy(&src, dest.join(name)).map_err(|e| format!("{name}: {e}"))?;
        copied += 1;
    }
    // Premier lancement (rien à sauvegarder encore) : pas la peine de garder
    // un dossier horodaté vide.
    if copied == 0 {
        let _ = std::fs::remove_dir(&dest);
        return Ok(());
    }

    prune(&root)
}

/// Garde les `BACKUP_KEEP` sauvegardes les plus récentes — le format
/// horodaté (`%Y-%m-%d_%H-%M-%S`) est lexicographiquement croissant, un tri
/// de noms suffit, pas besoin de reparser les dates.
fn prune(root: &Path) -> Result<(), String> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(root)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    entries.sort();
    if entries.len() > BACKUP_KEEP {
        for old in &entries[..entries.len() - BACKUP_KEEP] {
            let _ = std::fs::remove_dir_all(old);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Règle protégée : les fichiers présents sont copiés, les absents
    /// n'empêchent pas la sauvegarde de réussir (mod pas encore utilisé —
    /// `music.json` par ex. — ne doit jamais faire échouer le reste).
    #[test]
    fn backs_up_existing_files_and_skips_missing_ones() {
        let dir = crate::testutil::temp_dir("backup-basic");
        std::fs::write(dir.join("overlay.sqlite"), b"fake db").unwrap();
        std::fs::write(dir.join("config.json"), b"{}").unwrap();
        backup_now(&dir).unwrap();

        let root = backups_root(&dir);
        let stamps: Vec<_> = std::fs::read_dir(&root).unwrap().filter_map(|e| e.ok()).collect();
        assert_eq!(stamps.len(), 1, "un seul dossier horodaté créé");
        let snapshot = stamps[0].path();
        assert!(snapshot.join("overlay.sqlite").is_file(), "base copiée");
        assert!(snapshot.join("config.json").is_file(), "config copié");
        assert!(
            !snapshot.join("ui_prefs.json").exists(),
            "fichier absent non recréé de toutes pièces"
        );
    }

    /// Règle protégée : la rotation élague au-delà de `BACKUP_KEEP`, jamais
    /// une croissance illimitée du dossier `backups/`.
    #[test]
    fn prunes_beyond_keep_limit() {
        let dir = crate::testutil::temp_dir("backup-prune");
        let root = backups_root(&dir);
        std::fs::create_dir_all(&root).unwrap();
        for i in 0..(BACKUP_KEEP + 3) {
            std::fs::create_dir_all(root.join(format!("2020-01-{i:02}_00-00-00"))).unwrap();
        }
        prune(&root).unwrap();
        let remaining = std::fs::read_dir(&root).unwrap().filter_map(|e| e.ok()).count();
        assert_eq!(remaining, BACKUP_KEEP, "élague au-delà de BACKUP_KEEP");
    }

    /// Règle protégée : premier lancement (dossier `app_config_dir` vide,
    /// rien à sauvegarder) ne laisse pas un dossier horodaté vide traîner.
    #[test]
    fn skips_creating_empty_backup_when_nothing_to_copy() {
        let dir = crate::testutil::temp_dir("backup-empty");
        backup_now(&dir).unwrap();
        let root = backups_root(&dir);
        assert!(
            !root.exists() || std::fs::read_dir(&root).unwrap().next().is_none(),
            "aucun dossier vide créé"
        );
    }
}
