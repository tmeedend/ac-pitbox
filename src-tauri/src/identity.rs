//! Moteur d'identité (§4.1) : signature de contenu et normalisation pour le
//! rapprochement flou. Le nom de dossier (`id_interne`) reste le signal le plus
//! fort ; brand+name survit aux renommages ; la signature détecte « le même
//! contenu à peu de chose près ».

use std::collections::BTreeSet;
use std::path::Path;

use sha2::{Digest, Sha256};
use walkdir::WalkDir;

/// Normalise pour comparaison floue : minuscule, espaces compactés.
pub fn norm(s: &str) -> String {
    s.trim()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Signature des fichiers clés (modèles `.kn5`, `data.acd`) par chemin relatif
/// + taille. Stable au renommage du dossier, sensible au vrai contenu.
pub fn content_signature(dir: &Path) -> String {
    let mut entries: Vec<(String, u64)> = Vec::new();
    for entry in WalkDir::new(dir).into_iter().flatten() {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase());
        if !matches!(ext.as_deref(), Some("kn5") | Some("acd")) {
            continue;
        }
        let rel = path
            .strip_prefix(dir)
            .unwrap_or(path)
            .to_string_lossy()
            .to_lowercase()
            .replace('\\', "/");
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        entries.push((rel, size));
    }
    entries.sort();
    let mut hasher = Sha256::new();
    for (rel, size) in &entries {
        hasher.update(rel.as_bytes());
        hasher.update(b"|");
        hasher.update(size.to_le_bytes());
        hasher.update(b"\n");
    }
    format!("{:x}", hasher.finalize())
}

/// Ensemble des chemins de fichiers relatifs (normalisés : minuscules, `/`)
/// contenus dans `dir`. Sert à comparer deux arborescences fichier par fichier.
fn rel_files(dir: &Path) -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    for entry in WalkDir::new(dir).into_iter().flatten() {
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(dir)
            .unwrap_or(entry.path())
            .to_string_lossy()
            .to_lowercase()
            .replace('\\', "/");
        set.insert(rel);
    }
    set
}

/// Décompte de la comparaison de deux contenus (§4.4) : combien de chemins le
/// contenu entrant **ajoute** vs **écrase** par rapport au contenu existant.
/// Base de la détection « mise à jour » vs « couche/extension ».
#[derive(Debug, Clone, Copy)]
pub struct DiffStats {
    /// Chemins présents dans l'entrant mais absents de l'existant.
    pub added: usize,
    /// Chemins présents dans les deux (l'entrant écraserait l'existant).
    pub overwritten: usize,
    /// Nombre total de fichiers de l'existant (le « Z » de « Y écrasés sur Z »).
    pub existing_total: usize,
}

/// Compare l'arborescence entrante à l'existante (chemins relatifs).
pub fn diff_content(incoming: &Path, existing: &Path) -> DiffStats {
    let inc = rel_files(incoming);
    let exi = rel_files(existing);
    let overwritten = inc.intersection(&exi).count();
    DiffStats {
        added: inc.len() - overwritten,
        overwritten,
        existing_total: exi.len(),
    }
}

/// Empreinte composite stockée sur le mod : id de dossier + brand/name normalisés.
pub fn identity_hash(id_interne: &str, brand: &str, name: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(norm(id_interne).as_bytes());
    hasher.update(b"|");
    hasher.update(norm(brand).as_bytes());
    hasher.update(b"|");
    hasher.update(norm(name).as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(path: &Path) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"x").unwrap();
    }

    #[test]
    fn diff_content_counts_added_and_overwritten() {
        let base = std::env::temp_dir().join(format!("pitbox-diff-{}", uuid::Uuid::new_v4()));
        let existing = base.join("existing");
        let incoming = base.join("incoming");

        // Base : 3 fichiers.
        touch(&existing.join("ui/ui_track.json"));
        touch(&existing.join("data/surfaces.ini"));
        touch(&existing.join("gp/models.ini"));

        // Entrant : réécrit 1 fichier de la base (ui_track.json), ajoute 2 layouts.
        touch(&incoming.join("ui/ui_track.json"));
        touch(&incoming.join("wet/models.ini"));
        touch(&incoming.join("wet/data/surfaces.ini"));

        let d = diff_content(&incoming, &existing);
        assert_eq!(d.existing_total, 3, "3 fichiers dans la base");
        assert_eq!(d.overwritten, 1, "seul ui_track.json est écrasé");
        assert_eq!(d.added, 2, "2 chemins nouveaux (extension probable)");

        let _ = std::fs::remove_dir_all(&base);
    }
}
