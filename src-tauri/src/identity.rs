//! Moteur d'identité (§4.1) : signature de contenu et normalisation pour le
//! rapprochement flou. Le nom de dossier (`id_interne`) reste le signal le plus
//! fort ; brand+name survit aux renommages ; la signature détecte « le même
//! contenu à peu de chose près ».

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
