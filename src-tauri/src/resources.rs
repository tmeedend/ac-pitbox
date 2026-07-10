//! Fichiers annexes du mod — docs, templates (§4.6). Beaucoup de mods embarquent
//! des fichiers hors contenu de jeu (PDF de présentation, templates `.psd`,
//! changelog/readme, images de présentation, archives de templates) : AC ne les
//! lit pas, ils ne doivent **jamais** finir dans `content/` (donc jamais dans le
//! dossier junctionné). Selon le réglage global (§11), ils sont soit rangés à
//! part dans un dossier **ressources** du mod en bibliothèque, soit abandonnés —
//! mais jamais mélangés au contenu de jeu.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Serialize;
use walkdir::WalkDir;

use crate::archive;
use crate::modscan::ModKind;

/// Réglage global d'extraction (§11), persisté en préférence sous forme de
/// chaîne ("none" | "info_only" | "all") — voir `Prefs::resource_extraction_mode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtractionMode {
    /// Rien n'est extrait : les annexes restent dans l'archive/source, non
    /// copiées en bibliothèque (mais jamais copiées dans le contenu de jeu non plus).
    None,
    /// Fichiers légers d'information seulement (défaut).
    InfoOnly,
    /// Informations + fichiers lourds (templates, archives, sources 3D, vidéos).
    All,
}

impl ExtractionMode {
    /// Interprète la préférence persistée ; repli sur le défaut (Informations
    /// seulement) pour toute valeur inconnue ou absente.
    pub fn parse(s: &str) -> Self {
        match s {
            "none" => Self::None,
            "all" => Self::All,
            _ => Self::InfoOnly,
        }
    }
}

// Documents/infos légers — jamais du contenu AC légitime, donc classés annexes
// quelle que soit leur profondeur dans l'arborescence du mod.
const INFO_EXTS: &[&str] = &["txt", "pdf", "md", "doc", "docx", "rtf", "nfo", "html", "url", "lnk"];
// Images ambiguës (capture de présentation vs aperçu de skin) : seulement
// classées annexes **à la racine** du mod — jamais en profondeur, où elles
// sont presque toujours de vraies previews/textures (`skins/<x>/preview.jpg`).
const IMAGE_EXTS: &[&str] = &["jpg", "jpeg", "png"];
// Fichiers lourds (mode "Tout" seulement) : templates d'édition, archives
// jointes, sources 3D, vidéos — jamais lus par AC.
const HEAVY_EXTS: &[&str] = &[
    "psd", "xcf", "ai", "zip", "7z", "rar", "fbx", "blend", "3ds", "max", "mp4", "mov", "avi", "mkv", "webm",
];

/// Dossier ressources générique : `<lib>/resources/<category>/<segments...>`.
/// Base commune à tous les types de mods (voiture/circuit, skin, son, app,
/// mod « autre ») — chacun a son propre sous-arbre, jamais mélangé.
pub fn resources_dir_for(library: &Path, category: &str, segments: &[&str]) -> PathBuf {
    let mut p = library.join("resources").join(category);
    for s in segments {
        p = p.join(s);
    }
    p
}

/// Dossier ressources d'un mod voiture/circuit (§4.6) : **au niveau du mod**
/// (pas de la version), hors de l'arborescence junctionnée — survit aux mises
/// à jour, partagé par toutes les couches posées sur ce mod (§4.3).
pub fn resources_dir(library: &Path, kind: ModKind, id: &str) -> PathBuf {
    resources_dir_for(library, kind.content_folder(), &[id])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Route {
    /// Contenu de jeu : copié normalement, sera junctionné.
    Content,
    /// Fichier annexe capturé : rangé dans le dossier ressources.
    Resources,
    /// Fichier annexe non capturé (mode Aucun) : ni content, ni ressources —
    /// laissé dans la source (jamais supprimé), conformément à « Aucun ».
    Drop,
}

fn ext_lower(path: &Path) -> Option<String> {
    path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase())
}

/// `allow_root_images` : les images à la racine ne sont ambiguës (capture de
/// présentation vs contenu réel) que pour une **voiture/circuit** — pour un
/// skin, une app ou un mod « autre », une image à la racine (preview de skin,
/// icône d'app, texture) est **toujours** du vrai contenu, jamais une annexe.
fn classify(path: &Path, is_root: bool, mode: ExtractionMode, allow_root_images: bool) -> Route {
    // Fichier requis par le moteur audio AC (mapping GUID des .bank FMOD,
    // §12bis.2) — jamais une annexe malgré son extension .txt.
    if path.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.eq_ignore_ascii_case("GUIDs.txt")) {
        return Route::Content;
    }
    let Some(ext) = ext_lower(path) else {
        return Route::Content;
    };
    let ext = ext.as_str();
    let is_info = INFO_EXTS.contains(&ext);
    let is_image_root = allow_root_images && is_root && IMAGE_EXTS.contains(&ext);
    let is_heavy = HEAVY_EXTS.contains(&ext);
    if !(is_info || is_image_root || is_heavy) {
        return Route::Content;
    }
    // Classé annexe (jamais du contenu de jeu, quel que soit le réglage) : la
    // destination dépend seulement du réglage utilisateur.
    match mode {
        ExtractionMode::None => Route::Drop,
        ExtractionMode::InfoOnly => {
            if is_info || is_image_root {
                Route::Resources
            } else {
                Route::Drop
            }
        }
        ExtractionMode::All => Route::Resources,
    }
}

/// Détecte les fichiers annexes sous `dir`, sans rien copier. Vide pour la
/// grande majorité des mods (aucun fichier annexe) — sert à décider si le
/// rangement peut emprunter le chemin rapide (dossier entier déplacé/copié
/// d'un bloc) ou doit être partitionné fichier par fichier.
fn scan(dir: &Path, mode: ExtractionMode, allow_root_images: bool) -> HashMap<PathBuf, Route> {
    let mut out = HashMap::new();
    for entry in WalkDir::new(dir).into_iter().flatten() {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let is_root = path.parent() == Some(dir);
        let route = classify(path, is_root, mode, allow_root_images);
        if route != Route::Content {
            out.insert(path.to_path_buf(), route);
        }
    }
    out
}

fn copy_or_move_one(src: &Path, dest: &Path, move_files: bool) -> std::io::Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if move_files && std::fs::rename(src, dest).is_ok() {
        return Ok(());
    }
    std::fs::copy(src, dest)?;
    if move_files {
        let _ = std::fs::remove_file(src);
    }
    Ok(())
}

/// Range `src` dans `content_dest` (contenu de jeu) en redirigeant les fichiers
/// annexes (§4.6) vers `resources_dest` selon `mode`, ou en les laissant dans
/// `src` (mode Aucun — jamais supprimés). Renvoie le nombre de fichiers
/// effectivement rangés en ressources. Chemin rapide inchangé (déplacement/
/// copie de dossier entier, comme avant cette fonctionnalité) quand `src` ne
/// contient aucun fichier annexe — le cas de la grande majorité des mods.
/// `allow_root_images` : `true` uniquement pour voiture/circuit (voir `classify`).
pub fn file_mod(
    src: &Path,
    content_dest: &Path,
    resources_dest: &Path,
    mode: ExtractionMode,
    move_files: bool,
    allow_root_images: bool,
) -> Result<usize, String> {
    let ancillary = scan(src, mode, allow_root_images);
    if ancillary.is_empty() {
        if move_files {
            archive::move_dir(src, content_dest)
        } else {
            archive::copy_dir(src, content_dest)
        }
        .map_err(|e| format!("rangement bibliothèque : {e}"))?;
        return Ok(0);
    }

    let mut extracted = 0usize;
    for entry in WalkDir::new(src).into_iter().flatten() {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let rel = path.strip_prefix(src).unwrap_or(path);
        match ancillary.get(path).copied().unwrap_or(Route::Content) {
            Route::Content => copy_or_move_one(path, &content_dest.join(rel), move_files)
                .map_err(|e| format!("rangement bibliothèque : {e}"))?,
            Route::Resources => {
                extracted += 1;
                copy_or_move_one(path, &resources_dest.join(rel), move_files)
                    .map_err(|e| format!("extraction des fichiers annexes : {e}"))?;
            }
            Route::Drop => { /* ni content, ni ressources : reste dans la source (§4.6, mode Aucun) */ }
        }
    }
    Ok(extracted)
}

/// Fichier annexe listé sur la fiche (§4.6, « Bloc Ressources »).
#[derive(Debug, Clone, Serialize)]
pub struct ResourceFile {
    pub name: String,
    /// Chemin relatif au dossier ressources (affichage, sous-dossiers éventuels).
    pub rel_path: String,
    pub size_bytes: u64,
}

/// Liste le contenu du dossier ressources d'un mod, **lu en direct sur disque**
/// (§4.6) — jamais mémorisé en base : un fichier déposé manuellement apparaît
/// sans réimport, et un mod déjà installé avant cette fonctionnalité n'a rien
/// à réimporter pour que le bloc se remplisse.
pub fn list_resources(dir: &Path) -> Vec<ResourceFile> {
    if !dir.is_dir() {
        return Vec::new();
    }
    let mut out: Vec<ResourceFile> = WalkDir::new(dir)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file())
        .map(|e| {
            let path = e.path();
            let rel = path.strip_prefix(dir).unwrap_or(path).to_string_lossy().replace('\\', "/");
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let size_bytes = e.metadata().map(|m| m.len()).unwrap_or(0);
            ResourceFile { name, rel_path: rel, size_bytes }
        })
        .collect();
    out.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    out
}

/// Résout le chemin absolu d'un fichier ressources à partir de son chemin
/// relatif, avec garde-fou anti-traversée (`../..`) : le résultat doit rester
/// à l'intérieur de `dir`. Utilisé avant toute ouverture (§4.6).
pub fn resolve_resource_path(dir: &Path, rel_path: &str) -> Result<PathBuf, String> {
    let candidate = dir.join(rel_path);
    let canon_dir = dir.canonicalize().map_err(|e| format!("dossier ressources introuvable : {e}"))?;
    let canon_candidate = candidate.canonicalize().map_err(|e| format!("fichier introuvable : {e}"))?;
    if !canon_candidate.starts_with(&canon_dir) {
        return Err("chemin hors du dossier ressources".into());
    }
    Ok(canon_candidate)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"x").unwrap();
    }

    fn make_mod(root: &Path) {
        write(&root.join("ui").join("ui_car.json"));
        write(&root.join("model.kn5"));
        write(&root.join("skins").join("red").join("preview.jpg")); // jamais annexe : en profondeur
        write(&root.join("changelog.txt")); // info, racine
        write(&root.join("presentation.pdf")); // info, racine
        write(&root.join("preview.jpg")); // image, racine : ambigu mais léger
        write(&root.join("livery_template.psd")); // lourd, racine
        write(&root.join("old_templates.zip")); // lourd, racine
    }

    #[test]
    fn info_only_extracts_light_files_keeps_skins_and_drops_heavy() {
        let base = std::env::temp_dir().join(format!("pitbox-res-{}", uuid::Uuid::new_v4()));
        let src = base.join("src");
        make_mod(&src);
        let content = base.join("content");
        let resources = base.join("resources");

        let n = file_mod(&src, &content, &resources, ExtractionMode::InfoOnly, false, true).unwrap();
        assert_eq!(n, 3, "changelog.txt + presentation.pdf + preview.jpg (racine)");

        assert!(content.join("ui").join("ui_car.json").is_file());
        assert!(content.join("model.kn5").is_file());
        assert!(content.join("skins").join("red").join("preview.jpg").is_file(), "preview de skin jamais extrait");
        assert!(!content.join("changelog.txt").exists());
        assert!(!content.join("livery_template.psd").exists(), "annexe lourde jamais dans le contenu de jeu");

        assert!(resources.join("changelog.txt").is_file());
        assert!(resources.join("presentation.pdf").is_file());
        assert!(resources.join("preview.jpg").is_file());
        assert!(!resources.join("livery_template.psd").exists(), "mode info_only : pas les lourds");

        // Mode Aucun côté copie : la source reste intacte (copy=false ici teste move,
        // donc on vérifie plutôt que le lourd n'est nulle part en bibliothèque.
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn all_mode_extracts_heavy_files_too() {
        let base = std::env::temp_dir().join(format!("pitbox-res-{}", uuid::Uuid::new_v4()));
        let src = base.join("src");
        make_mod(&src);
        let content = base.join("content");
        let resources = base.join("resources");

        let n = file_mod(&src, &content, &resources, ExtractionMode::All, true, true).unwrap();
        assert_eq!(n, 5, "3 légers + template.psd + archive.zip");
        assert!(resources.join("livery_template.psd").is_file());
        assert!(resources.join("old_templates.zip").is_file());
        assert!(content.join("skins").join("red").join("preview.jpg").is_file());

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn none_mode_drops_ancillary_and_leaves_source_untouched_on_copy() {
        let base = std::env::temp_dir().join(format!("pitbox-res-{}", uuid::Uuid::new_v4()));
        let src = base.join("src");
        make_mod(&src);
        let content = base.join("content");
        let resources = base.join("resources");

        // copy=true (préserve la source) : les annexes doivent rester dans src.
        let n = file_mod(&src, &content, &resources, ExtractionMode::None, false, true).unwrap();
        assert_eq!(n, 0, "rien n'est extrait en mode Aucun");
        assert!(!resources.exists(), "aucun dossier ressources créé");
        assert!(!content.join("changelog.txt").exists());
        assert!(src.join("changelog.txt").is_file(), "annexe laissée dans la source (§4.6, mode Aucun)");
        assert!(content.join("model.kn5").is_file(), "le contenu de jeu, lui, est bien rangé");

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn fast_path_used_when_no_ancillary_files() {
        // Pas de fichier annexe : le dossier entier est déplacé/copié d'un bloc
        // (comportement historique préservé, aucune perte de perf).
        let base = std::env::temp_dir().join(format!("pitbox-res-{}", uuid::Uuid::new_v4()));
        let src = base.join("src");
        write(&src.join("ui").join("ui_car.json"));
        write(&src.join("model.kn5"));
        let content = base.join("content");
        let resources = base.join("resources");

        let n = file_mod(&src, &content, &resources, ExtractionMode::All, false, true).unwrap();
        assert_eq!(n, 0);
        assert!(content.join("model.kn5").is_file());
        assert!(!resources.exists());

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn guids_txt_never_extracted() {
        // Fichier requis par le moteur audio AC (§12bis.2) : ne doit jamais
        // être traité comme une annexe malgré son extension .txt.
        let base = std::env::temp_dir().join(format!("pitbox-res-guids-{}", uuid::Uuid::new_v4()));
        let src = base.join("src");
        write(&src.join("GUIDs.txt"));
        write(&src.join("car.bank"));
        write(&src.join("readme.txt")); // vraie annexe, elle
        let content = base.join("content");
        let resources = base.join("resources");

        let n = file_mod(&src, &content, &resources, ExtractionMode::All, false, false).unwrap();
        assert_eq!(n, 1, "seul readme.txt est une annexe");
        assert!(content.join("GUIDs.txt").is_file(), "GUIDs.txt reste du contenu");
        assert!(content.join("car.bank").is_file());
        assert!(resources.join("readme.txt").is_file());

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn root_images_never_extracted_for_non_car_track_mods() {
        // Skin/app/mod « autre » : une image à la racine est TOUJOURS du vrai
        // contenu (preview de skin, icône d'app) — jamais une annexe, même en
        // mode info_only. Seuls les documents/fichiers lourds sont concernés.
        let base = std::env::temp_dir().join(format!("pitbox-res-imgs-{}", uuid::Uuid::new_v4()));
        let src = base.join("src");
        write(&src.join("ui_skin.json"));
        write(&src.join("preview.jpg")); // aperçu de skin, racine : jamais annexe ici
        write(&src.join("livery.dds"));
        write(&src.join("readme.txt")); // doc : toujours annexe, quel que soit le type
        let content = base.join("content");
        let resources = base.join("resources");

        let n = file_mod(&src, &content, &resources, ExtractionMode::InfoOnly, false, false).unwrap();
        assert_eq!(n, 1, "seul readme.txt est capturé");
        assert!(content.join("preview.jpg").is_file(), "preview de skin jamais extrait");
        assert!(content.join("livery.dds").is_file());
        assert!(resources.join("readme.txt").is_file());

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn list_and_resolve_resources() {
        let base = std::env::temp_dir().join(format!("pitbox-res-list-{}", uuid::Uuid::new_v4()));
        let dir = base.join("resources");
        write(&dir.join("changelog.txt"));
        write(&dir.join("templates").join("livery.psd"));

        let files = list_resources(&dir);
        assert_eq!(files.len(), 2);
        assert!(files.iter().any(|f| f.rel_path == "changelog.txt"));
        assert!(files.iter().any(|f| f.rel_path == "templates/livery.psd"));

        // Résolution valide.
        let p = resolve_resource_path(&dir, "changelog.txt").unwrap();
        assert!(p.is_file());

        // Garde-fou anti-traversée : refuse de sortir du dossier ressources.
        let outside = base.join("secret.txt");
        std::fs::write(&outside, b"x").unwrap();
        assert!(resolve_resource_path(&dir, "../secret.txt").is_err());

        // Dossier ressources absent : liste vide, pas d'erreur.
        assert!(list_resources(&base.join("nope")).is_empty());

        let _ = std::fs::remove_dir_all(&base);
    }
}
