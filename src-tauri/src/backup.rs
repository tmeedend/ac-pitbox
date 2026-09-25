//! Sauvegarde automatique de la base et des petits fichiers de préférences
//! (§6.2/SESSION§4) : copie best-effort dans un sous-dossier horodaté à chaque
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
/// Où les sessions enregistrées atterrissent dans la sauvegarde. Un
/// sous-dossier parce qu'elles ne vivent plus dans `app_config_dir` mais chez
/// Content Manager (SESSION§3.6) : le filet doit les suivre là-bas, sinon la
/// seule chose que l'utilisateur ait composée à la main serait la seule qui ne
/// soit pas sauvegardée.
const SESSIONS_SUBDIR: &str = "saved-sessions";

const BACKED_UP_FILES: &[&str] = &[
    "overlay.sqlite",
    "config.json",
    "ui_prefs.json",
    "library_columns.json",
    "session.json",
    "launch_state.json",
    // Migré en presets `.cmpreset` (SESSION§3.6) : gardé dans la liste tant
    // qu'il peut exister chez quelqu'un qui n'a pas encore redémarré après la
    // mise à jour — c'est justement le fichier qu'on n'aimerait pas perdre
    // juste avant sa migration.
    "saved_sessions.json",
    "music.json",
    // The user's decisions on the rules catalogue (REGLES§2): since the
    // catalogue is no longer copied, these two files ARE everything he did in
    // the Rules, Categories and Countries screens - out of the backup, a bad
    // write would lose it with nothing to fall back on.
    "taxonomy.json",
    "rules-overlay.json",
    // Which logo each brand shows (TAXO§4, §9). The files of his it names
    // live in `logos/`, kept as long as they are named.
    "brand_logos.json",
    // The pre-overlay rules file, while it exists (not yet migrated).
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
    let presets = crate::sessionpreset::own_dir(app);
    if let Err(e) = backup_now(&base, presets.as_deref()) {
        log::warn!("backup: sauvegarde de démarrage échouée : {e}");
    }
}

fn backup_now(base: &Path, presets: Option<&Path>) -> Result<(), String> {
    let root = backups_root(base);
    let stamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
    let dest = root.join(&stamp);
    std::fs::create_dir_all(&dest).map_err(|e| e.to_string())?;

    let mut copied = 0;
    let mut database_healthy = true;
    for name in BACKED_UP_FILES {
        let src = base.join(name);
        if !src.is_file() {
            continue;
        }
        if *name == DATABASE {
            database_healthy = snapshot_database(&src, &dest.join(name))?;
        } else {
            std::fs::copy(&src, dest.join(name)).map_err(|e| format!("{name}: {e}"))?;
        }
        copied += 1;
    }
    copied += copy_presets(presets, &dest)?;
    // Premier lancement (rien à sauvegarder encore) : pas la peine de garder
    // un dossier horodaté vide.
    if copied == 0 {
        let _ = std::fs::remove_dir(&dest);
        return Ok(());
    }

    // A damaged database is still saved (the bytes are evidence), but it
    // must not rotate the healthy snapshots out. Measured on 2026-09-23/24:
    // seven `tauri dev` restarts in one morning replaced all seven backups,
    // so the last healthy copy was a few hours away from being pruned.
    if !database_healthy {
        log::warn!("backup: {DATABASE} is damaged, rotation skipped so older snapshots survive");
        return Ok(());
    }
    prune(&root)
}

const DATABASE: &str = "overlay.sqlite";

/// Writes a consistent snapshot of the overlay database to `target`, and says
/// whether the source read back healthy.
///
/// **`VACUUM INTO`, not a file copy.** The database runs in WAL mode and the
/// app never closes its connection cleanly (the process exits without
/// dropping managed state), so recent commits live in `overlay.sqlite-wal`
/// until an automatic checkpoint — which only fires at 1 000 pages and can
/// wait days. Copying `overlay.sqlite` alone saved the state of the last
/// checkpoint: backups "looked healthy" precisely because they ignored
/// everything the WAL held. `VACUUM INTO` reads through the WAL and writes a
/// self-contained file with no `-wal` to lose.
///
/// The connection is read-only: a backup never writes the live database.
/// When SQLite cannot read the source (corruption, not a database at all),
/// the raw `overlay.sqlite` and its `-wal` are copied as they are — a pair
/// kept together is what a later diagnosis needs — and `false` comes back.
fn snapshot_database(src: &Path, target: &Path) -> Result<bool, String> {
    match vacuum_into(src, target) {
        Ok(()) => Ok(true),
        Err(e) => {
            log::warn!("backup: {} unreadable by SQLite ({e}), raw copy kept", src.display());
            let _ = std::fs::remove_file(target);
            std::fs::copy(src, target).map_err(|e| format!("{DATABASE}: {e}"))?;
            let wal = wal_path(src);
            if wal.is_file() {
                std::fs::copy(&wal, wal_path(target)).map_err(|e| format!("{DATABASE}-wal: {e}"))?;
            }
            Ok(false)
        }
    }
}

fn vacuum_into(src: &Path, target: &Path) -> rusqlite::Result<()> {
    use rusqlite::{Connection, OpenFlags};
    let conn = Connection::open_with_flags(src, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    // `quick_check` first: `VACUUM INTO` only copies what the b-trees reach,
    // and can succeed on a file whose damage lies in pages it never visits.
    let verdict: String = conn.query_row("PRAGMA quick_check(1)", [], |r| r.get(0))?;
    if verdict != "ok" {
        return Err(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CORRUPT),
            Some(verdict),
        ));
    }
    conn.execute("VACUUM INTO ?1", [target.to_string_lossy()])?;
    Ok(())
}

fn wal_path(db: &Path) -> PathBuf {
    let mut name = db.as_os_str().to_owned();
    name.push("-wal");
    PathBuf::from(name)
}

/// Copie les presets de session dans le sous-dossier dédié, et rend leur
/// nombre. Un dossier absent (Content Manager jamais lancé, aucune session
/// enregistrée) rend zéro : c'est un non-résultat, pas une panne. Les
/// sous-dossiers ne sont pas parcourus — on ne sauvegarde que ce qu'on écrit,
/// et on écrit à plat.
fn copy_presets(presets: Option<&Path>, dest: &Path) -> Result<usize, String> {
    let Some(src) = presets.filter(|p| p.is_dir()) else {
        return Ok(0);
    };
    let mut copied = 0;
    let mut target_made = false;
    for entry in std::fs::read_dir(src)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file() || !path.extension().is_some_and(|x| x.eq_ignore_ascii_case("cmpreset")) {
            continue;
        }
        let target = dest.join(SESSIONS_SUBDIR);
        if !target_made {
            std::fs::create_dir_all(&target).map_err(|e| e.to_string())?;
            target_made = true;
        }
        let Some(name) = path.file_name() else { continue };
        std::fs::copy(&path, target.join(name)).map_err(|e| format!("{}: {e}", name.to_string_lossy()))?;
        copied += 1;
    }
    Ok(copied)
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

    /// Règle protégée : les sessions enregistrées sont sauvegardées elles
    /// aussi, alors qu'elles ne vivent plus dans `app_config_dir` mais chez
    /// Content Manager (SESSION§3.6). Sans ça, la seule chose que
    /// l'utilisateur ait composée à la main serait la seule hors du filet.
    #[test]
    fn saved_sessions_follow_the_backup_out_of_the_config_folder() {
        let dir = crate::testutil::temp_dir("backup-presets");
        let presets = dir.join("Quick Drive").join("Pit Box");
        std::fs::create_dir_all(&presets).unwrap();
        std::fs::write(presets.join("Spa dusk.cmpreset"), b"{}").unwrap();
        // Un fichier qui n'est pas un preset ne part pas : on ne sauvegarde
        // que ce qu'on écrit.
        std::fs::write(presets.join("notes.txt"), b"x").unwrap();
        backup_now(&dir, Some(&presets)).unwrap();

        let root = backups_root(&dir);
        let stamp = std::fs::read_dir(&root).unwrap().next().unwrap().unwrap().path();
        let copied = stamp.join(SESSIONS_SUBDIR);
        assert!(
            copied.join("Spa dusk.cmpreset").is_file(),
            "le preset est dans la sauvegarde"
        );
        assert!(!copied.join("notes.txt").exists(), "seuls les presets partent");
    }

    /// Règle protégée : un dossier de presets absent (Content Manager jamais
    /// lancé, aucune session enregistrée) est un non-résultat — le reste de la
    /// sauvegarde se fait quand même.
    #[test]
    fn a_missing_preset_folder_does_not_stop_the_rest() {
        let dir = crate::testutil::temp_dir("backup-no-presets");
        std::fs::write(dir.join("config.json"), b"{}").unwrap();
        backup_now(&dir, Some(&dir.join("nowhere"))).unwrap();

        let root = backups_root(&dir);
        let stamp = std::fs::read_dir(&root).unwrap().next().unwrap().unwrap().path();
        assert!(stamp.join("config.json").is_file(), "le reste est bien sauvegardé");
        assert!(!stamp.join(SESSIONS_SUBDIR).exists(), "pas de sous-dossier vide");
    }

    /// Règle protégée : les fichiers présents sont copiés, les absents
    /// n'empêchent pas la sauvegarde de réussir (mod pas encore utilisé —
    /// `music.json` par ex. — ne doit jamais faire échouer le reste).
    #[test]
    fn backs_up_existing_files_and_skips_missing_ones() {
        let dir = crate::testutil::temp_dir("backup-basic");
        std::fs::write(dir.join("overlay.sqlite"), b"fake db").unwrap();
        std::fs::write(dir.join("config.json"), b"{}").unwrap();
        backup_now(&dir, None).unwrap();

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

    /// Protected rule (§6.2): the snapshot holds what the WAL holds. The app
    /// keeps its connection open and never checkpoints on exit, so a plain
    /// copy of `overlay.sqlite` silently dropped every commit since the last
    /// checkpoint — days of work in the 2026-09 incident.
    #[test]
    fn backup_includes_commits_still_in_the_wal() {
        let dir = crate::testutil::temp_dir("backup-wal");
        let live = crate::overlay::open(&dir.join(DATABASE)).unwrap();
        crate::overlay::set_meta(&live, "probe", "in the wal").unwrap();
        assert!(
            wal_path(&dir.join(DATABASE)).is_file(),
            "the commit is still in the WAL"
        );

        backup_now(&dir, None).unwrap();

        let stamp = std::fs::read_dir(backups_root(&dir))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        assert!(
            !wal_path(&stamp.join(DATABASE)).exists(),
            "the snapshot is self-contained, no -wal to lose"
        );
        let copy = rusqlite::Connection::open(stamp.join(DATABASE)).unwrap();
        assert_eq!(
            crate::overlay::get_meta(&copy, "probe").unwrap().as_deref(),
            Some("in the wal"),
            "the snapshot carries the uncheckpointed commit"
        );
        drop(live);
    }

    /// Protected rule (§6.2): a damaged database is still saved, raw, but it
    /// never rotates the healthy snapshots out — otherwise a handful of
    /// restarts on a broken base erase the only copies worth restoring.
    #[test]
    fn a_damaged_database_does_not_prune_healthy_backups() {
        let dir = crate::testutil::temp_dir("backup-damaged");
        let root = backups_root(&dir);
        for i in 0..BACKUP_KEEP {
            std::fs::create_dir_all(root.join(format!("2020-01-{i:02}_00-00-00"))).unwrap();
        }
        std::fs::write(dir.join(DATABASE), b"not a database").unwrap();

        backup_now(&dir, None).unwrap();

        let dirs: Vec<_> = std::fs::read_dir(&root).unwrap().filter_map(|e| e.ok()).collect();
        assert_eq!(dirs.len(), BACKUP_KEEP + 1, "every older snapshot survives");
        let newest = dirs.iter().map(|e| e.path()).max().unwrap();
        assert_eq!(
            std::fs::read(newest.join(DATABASE)).unwrap(),
            b"not a database",
            "the damaged bytes are kept as they are"
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
        backup_now(&dir, None).unwrap();
        let root = backups_root(&dir);
        assert!(
            !root.exists() || std::fs::read_dir(&root).unwrap().next().is_none(),
            "aucun dossier vide créé"
        );
    }
}
