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
//! chemin absolu, reconnu et utilisé tel quel par `resolve` — jamais cassé
//! sans migration explicite (`maintenance::relativize_library_paths`).

use std::path::{Path, PathBuf};

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

/// Cherche `marker` (une séquence de composants consécutifs, ex.
/// `["cars", "ferrari_488"]`) dans `path` et renvoie tout ce qui suit **à
/// partir du premier composant du marqueur**, comparaison insensible à la
/// casse et à `/` vs `\`. Sert à retrouver le chemin relatif à la
/// bibliothèque d'une ligne encore écrite en absolu (§11, migration), sans
/// avoir à connaître l'ancienne racine — la structure interne connue
/// (`<type>/<id>`, `layers/<parent>`, `skins/<parent>`…) suffit à la
/// retrouver telle quelle, jamais reconstruite.
pub fn relative_from_marker(path: &str, marker: &[&str]) -> Option<String> {
    let comps: Vec<_> = Path::new(path).components().collect();
    if marker.is_empty() {
        return None;
    }
    for start in 0..comps.len() {
        if start + marker.len() > comps.len() {
            break;
        }
        let matches = marker
            .iter()
            .enumerate()
            .all(|(i, m)| comps[start + i].as_os_str().to_string_lossy().eq_ignore_ascii_case(m));
        if matches {
            let rel: PathBuf = comps[start..].iter().collect();
            return Some(rel.to_string_lossy().into_owned());
        }
    }
    None
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

    #[test]
    fn relative_from_marker_finds_known_suffix_regardless_of_old_root() {
        // Le cas réel d'une migration multi-PC : l'ancienne racine (ici
        // `D:\AC-Library` sur le PC source) n'a plus aucun sens sur le nouveau
        // PC — seule la structure interne connue (cars/<id>/<version>) permet
        // de retrouver la partie portable, sans qu'on ait besoin de la saisir.
        let stored = r"D:\AC-Library\cars\ferrari_488\v1.0";
        assert_eq!(
            relative_from_marker(stored, &["cars", "ferrari_488"]),
            Some(r"cars\ferrari_488\v1.0".to_string())
        );
    }

    #[test]
    fn relative_from_marker_is_case_insensitive() {
        let stored = r"D:\lib\CARS\Ferrari_488\v1";
        assert_eq!(
            relative_from_marker(stored, &["cars", "ferrari_488"]),
            Some(r"CARS\Ferrari_488\v1".to_string())
        );
    }

    #[test]
    fn relative_from_marker_none_when_not_found() {
        let stored = r"D:\somewhere\else\entirely";
        assert_eq!(relative_from_marker(stored, &["cars", "ferrari_488"]), None);
    }

    #[test]
    fn relative_from_marker_single_component_marker() {
        // kept_archive_path : on ne connaît pas l'uuid du sous-dossier,
        // seul le nom fixe `_source_archives` sert de marqueur.
        let stored = r"D:\AC-Library\_source_archives\a1b2c3\mod.zip";
        assert_eq!(
            relative_from_marker(stored, &["_source_archives"]),
            Some(r"_source_archives\a1b2c3\mod.zip".to_string())
        );
    }
}
