//! Moteur d'identité (§4.1) : signature de contenu et normalisation pour le
//! rapprochement flou. Le nom de dossier (`id_interne`) reste le signal le plus
//! fort ; brand+name survit aux renommages ; la signature détecte « le même
//! contenu à peu de chose près ».

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use walkdir::WalkDir;

/// Normalise pour comparaison floue : minuscule, espaces compactés.
pub fn norm(s: &str) -> String {
    s.trim().to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Signature des fichiers clés (modèles `.kn5`, `data.acd`) par chemin relatif
/// + taille. Stable au renommage du dossier, sensible au vrai contenu.
///
/// **It only says "probably the same"**: neither the other files
/// (`ui_car.json`, `data/*.ini`, skins…) nor the content enter it. It stays what
/// it is because every version stores it, it is what finds a mod's showcase
/// version again, and it travels in the export manifests: redefining it would
/// make every re-import look like an update. A duplicate is therefore only
/// declared after [`content_differs`].
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

/// Every file under `dir` by its normalised relative path (lower case, `/`),
/// with where it is and how big. Unlike [`rel_files`] an unreadable folder is an
/// error, not an empty list: "nothing there" must not read as "all different".
///
/// Only regular files, like `rel_files` and the copy that files a mod in the
/// library: a link is not followed. Two paths differing only by case cannot
/// exist side by side on the file systems the app runs on (Windows).
fn files_by_rel(dir: &Path) -> std::io::Result<BTreeMap<String, (PathBuf, u64)>> {
    if !std::fs::metadata(dir)?.is_dir() {
        return Err(std::io::Error::new(std::io::ErrorKind::NotADirectory, "not a folder"));
    }
    let mut files = BTreeMap::new();
    for entry in WalkDir::new(dir) {
        let entry = entry?;
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
        files.insert(rel, (entry.path().to_path_buf(), entry.metadata()?.len()));
    }
    Ok(files)
}

/// Fills `buf` from `r` as far as it goes: the whole buffer, or what is left.
fn fill(r: &mut impl Read, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut n = 0;
    while n < buf.len() {
        match r.read(&mut buf[n..])? {
            0 => break,
            k => n += k,
        }
    }
    Ok(n)
}

/// Whether two files hold the same bytes, stopping at the first difference.
fn same_bytes(a: &Path, b: &Path, buf: &mut [Vec<u8>; 2]) -> std::io::Result<bool> {
    let (mut fa, mut fb) = (std::fs::File::open(a)?, std::fs::File::open(b)?);
    loop {
        let [ba, bb] = buf;
        let (na, nb) = (fill(&mut fa, ba)?, fill(&mut fb, bb)?);
        if na != nb || ba[..na] != bb[..nb] {
            return Ok(false);
        }
        if na < ba.len() {
            return Ok(true);
        }
    }
}

/// Whether the incoming folder holds anything the existing one does not: a file
/// more, a file less, one of another size, or - same path, same size - other
/// bytes. The check an equal [`content_signature`] must pass before a
/// re-import is called a duplicate (SPEC §4.1).
///
/// Two stages, the dear one paid only when it must. Paths and sizes of EVERY
/// file first - a walk of the folders, no byte read - which settles added,
/// removed and resized files, `ui_car.json` and `data/*.ini` included. Then the
/// bytes of the files that look alike, **smallest first** (what a modder edits
/// is small: json, ini) and stopping at the first difference: an update costs
/// next to nothing, only a true duplicate reads everything - both copies, once.
///
/// `Err`: one side cannot be read (library disk unplugged, folder moved). The
/// caller keeps the signature's answer then - nothing to compare is no reason
/// to turn a re-import into an update.
pub fn content_differs(incoming: &Path, existing: &Path) -> std::io::Result<bool> {
    let (a, b) = (files_by_rel(incoming)?, files_by_rel(existing)?);
    if a.len() != b.len()
        || a.iter()
            .any(|(rel, (_, size))| b.get(rel).map(|(_, s)| s) != Some(size))
    {
        return Ok(true);
    }
    let mut alike: Vec<(&String, &(PathBuf, u64))> = a.iter().collect();
    alike.sort_by_key(|(_, (_, size))| *size);
    let mut buf = [vec![0u8; 128 * 1024], vec![0u8; 128 * 1024]];
    for (rel, (path, _)) in alike {
        if !same_bytes(path, &b[rel].0, &mut buf)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Ensemble des chemins de fichiers relatifs (normalisés : minuscules, `/`)
/// contenus dans `dir`. Sert à comparer deux arborescences fichier par fichier.
pub(crate) fn rel_files(dir: &Path) -> BTreeSet<String> {
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
#[derive(Debug, Clone, Copy, Default)]
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
    diff_against(incoming, &rel_files(existing))
}

/// Same comparison, against the paths the existing content is known to hold
/// — `rel_files` form: lower-case, `/`-separated. A version in the showcase
/// is compared to what it held before its files went (ESPACE§7.5), not to
/// its skeleton.
pub fn diff_against(incoming: &Path, existing: &BTreeSet<String>) -> DiffStats {
    let inc = rel_files(incoming);
    let overwritten = inc.intersection(existing).count();
    DiffStats {
        added: inc.len() - overwritten,
        overwritten,
        existing_total: existing.len(),
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

    fn put(dir: &Path, rel: &str, bytes: &[u8]) {
        let p = dir.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, bytes).unwrap();
    }

    /// SPEC §4.1: a folder differs from another by a file more or less, by a
    /// size, or by bytes - and by nothing else (not by the case of a name, not by
    /// the order the files were written in).
    #[test]
    fn content_differs_sees_a_file_a_size_and_a_byte() {
        let base = crate::testutil::temp_dir("differs");
        let (a, b) = (base.join("a"), base.join("b"));
        for d in [&a, &b] {
            put(d, "ui/ui_car.json", b"{}");
            put(d, "data/engine.ini", b"LIMITER=8300");
            put(d, "model.kn5", &[7u8; 5000]);
        }
        assert!(!content_differs(&a, &b).unwrap(), "the same folder twice");

        put(&b, "extra.txt", b"x");
        assert!(content_differs(&a, &b).unwrap(), "a file more on the existing side");
        assert!(content_differs(&b, &a).unwrap(), "and on the incoming side");
        std::fs::remove_file(b.join("extra.txt")).unwrap();

        put(&b, "data/engine.ini", b"LIMITER=83000");
        assert!(content_differs(&a, &b).unwrap(), "another size");
        put(&b, "data/engine.ini", b"LIMITER=8400");
        assert!(content_differs(&a, &b).unwrap(), "same size, one character");
        put(&b, "data/engine.ini", b"LIMITER=8300");
        assert!(!content_differs(&a, &b).unwrap(), "put back");

        put(&b, "DATA/Engine.INI", b"LIMITER=8300");
        std::fs::remove_file(b.join("data/engine.ini")).unwrap();
        assert!(
            !content_differs(&a, &b).unwrap(),
            "paths are compared as the rest of the import does: lower case"
        );
    }

    /// The bytes are read in blocks: a difference in the LAST byte of a file
    /// several blocks long, and one in the first byte after a block boundary,
    /// are both seen - a comparison that stopped at the first block would not.
    #[test]
    fn content_differs_reads_a_file_to_its_end() {
        let base = crate::testutil::temp_dir("differs-big");
        let (a, b) = (base.join("a"), base.join("b"));
        let block = 128 * 1024;
        let mut bytes = vec![3u8; block * 3 + 17];
        put(&a, "big.kn5", &bytes);
        put(&b, "big.kn5", &bytes);
        assert!(!content_differs(&a, &b).unwrap());
        for at in [block, bytes.len() - 1] {
            let keep = bytes[at];
            bytes[at] ^= 1;
            put(&b, "big.kn5", &bytes);
            assert!(content_differs(&a, &b).unwrap(), "byte {at}");
            bytes[at] = keep;
        }
        put(&b, "big.kn5", &bytes);
        assert!(!content_differs(&a, &b).unwrap(), "put back");
        // Exactly a block long: the end of the file is the end of a block.
        put(&a, "exact.acd", &vec![1u8; block]);
        put(&b, "exact.acd", &vec![1u8; block]);
        assert!(!content_differs(&a, &b).unwrap(), "a file the size of one block");
    }

    /// A folder that cannot be read is an error, never "different": the caller
    /// keeps the signature's answer (a library disk that is not plugged in).
    #[test]
    fn content_differs_is_an_error_where_a_side_cannot_be_read() {
        let base = crate::testutil::temp_dir("differs-missing");
        put(&base.join("a"), "model.kn5", b"x");
        assert!(content_differs(&base.join("a"), &base.join("gone")).is_err());
        assert!(content_differs(&base.join("gone"), &base.join("a")).is_err());
        put(&base, "file.txt", b"x");
        assert!(
            content_differs(&base.join("a"), &base.join("file.txt")).is_err(),
            "a file is no folder"
        );
    }

    #[test]
    fn diff_content_counts_added_and_overwritten() {
        let base = crate::testutil::temp_dir("diff");
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
    }
}
