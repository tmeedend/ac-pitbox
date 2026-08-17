//! Résolution des chemins stockés en overlay (§11) : `versions`, `layers`,
//! `sub_mods`, `apps`, `other_mods` gardent tous un `library_path` — et
//! `versions.kept_archive_path` — pointant vers un emplacement sous la
//! bibliothèque. Stockés **relatifs** à la bibliothèque depuis ce module
//! (portable d'une machine à l'autre : un seul réglage à changer dans les
//! Réglages, plutôt qu'une base entière de chemins absolus figés sur la
//! machine d'origine — cause réelle d'un `library files not found` en masse
//! après une migration, même avec une copie de bibliothèque parfaite).
//!
//! **Compat ascendante** : les lignes écrites avant ce format restent en
//! chemin absolu, reconnu et utilisé tel quel par `resolve` — jamais cassé.

use std::path::{Path, PathBuf};

/// Free bytes on the volume holding `path`, `None` if it cannot be told.
///
/// Used to refuse a batch import that cannot possibly fit (§4.2bis) rather than
/// let it die halfway through, leaving a half-extracted work directory and a
/// library the user then has to clean up by hand.
#[cfg(windows)]
pub fn free_space(path: &Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

    // The path must exist for the call to resolve a volume: walk up to the
    // first ancestor that does (the library folder may not be created yet).
    let existing = path.ancestors().find(|p| p.exists())?;
    let wide: Vec<u16> = existing.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let mut available: u64 = 0;
    unsafe { GetDiskFreeSpaceExW(PCWSTR(wide.as_ptr()), Some(&mut available), None, None) }.ok()?;
    Some(available)
}

/// Non-Windows builds never reach the import pipeline (the app is Windows-only,
/// §Stack); the stub only keeps the module compiling everywhere.
#[cfg(not(windows))]
pub fn free_space(_path: &Path) -> Option<u64> {
    None
}

/// Relativise `path` par rapport à `library` pour l'écriture en overlay.
/// Repli sur le chemin absolu si `library` est absente ou si `path` n'est pas
/// sous elle — cas normal pour un skin de circuit fourni avec le contenu de
/// base (`content/`, hors bibliothèque), jamais portable de toute façon.
pub fn to_relative(library: Option<&Path>, path: &Path) -> String {
    library
        .and_then(|lib| path.strip_prefix(lib).ok())
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

/// Résout un chemin stocké en overlay : relatif à `library` (format courant)
/// si `stored` ne l'est pas déjà — absolu tel quel sinon (ligne pas encore
/// migrée, ou bibliothèque non configurée). `None` seulement si `stored` est
/// relatif et qu'aucune bibliothèque n'est configurée pour le résoudre.
pub fn resolve(library: Option<&Path>, stored: &str) -> Option<PathBuf> {
    let p = Path::new(stored);
    if p.is_absolute() {
        Some(p.to_path_buf())
    } else {
        library.map(|lib| lib.join(p))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_relative_strips_library_prefix() {
        let library = Path::new(r"D:\AC-Library");
        let path = Path::new(r"D:\AC-Library\cars\ferrari_488\v1");
        assert_eq!(to_relative(Some(library), path), r"cars\ferrari_488\v1");
    }

    #[test]
    fn to_relative_falls_back_to_absolute_outside_library() {
        let library = Path::new(r"D:\AC-Library");
        let path = Path::new(r"E:\elsewhere\car");
        assert_eq!(to_relative(Some(library), path), r"E:\elsewhere\car");
    }

    #[test]
    fn to_relative_falls_back_to_absolute_without_library() {
        let path = Path::new(r"C:\ac\content\tracks\spa\skins\cm_skins\Stock");
        assert_eq!(to_relative(None, path), path.to_string_lossy());
    }

    #[test]
    fn resolve_joins_relative_paths_under_library() {
        let library = Path::new(r"E:\NewLibrary");
        assert_eq!(
            resolve(Some(library), r"cars\ferrari_488\v1"),
            Some(PathBuf::from(r"E:\NewLibrary\cars\ferrari_488\v1"))
        );
    }

    #[test]
    fn resolve_keeps_absolute_paths_as_is_for_legacy_rows() {
        // Compat ascendante : une ligne écrite avant ce format reste absolue,
        // jamais réinterprétée relative à la bibliothèque courante.
        let library = Path::new(r"E:\NewLibrary");
        assert_eq!(
            resolve(Some(library), r"D:\OldLibrary\cars\ferrari_488\v1"),
            Some(PathBuf::from(r"D:\OldLibrary\cars\ferrari_488\v1"))
        );
    }

    #[test]
    fn resolve_relative_without_library_configured_is_none() {
        assert_eq!(resolve(None, r"cars\ferrari_488\v1"), None);
    }
}
