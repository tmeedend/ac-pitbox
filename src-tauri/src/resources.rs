//! Fichiers annexes du mod — docs, templates (§4.5.2). Beaucoup de mods embarquent
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

// Documents/infos légers — jamais du contenu AC légitime, mais classés
// annexes seulement **à la racine** de ce qui est livré à côté du mod : un
// `.txt`/`.md` en profondeur (ex. dans `extension/`, `data/`) fait presque
// toujours partie du contenu réel (note de config CSP, etc.).
const INFO_EXTS: &[&str] = &["txt", "pdf", "md", "doc", "docx", "rtf", "nfo", "html", "url", "lnk"];
// Fichiers lourds (mode "Tout" seulement) : templates d'édition, archives
// jointes, sources 3D, vidéos — jamais lus par AC. Racine uniquement, comme
// les documents.
const HEAVY_EXTS: &[&str] = &[
    "psd", "xcf", "ai", "zip", "7z", "rar", "fbx", "blend", "3ds", "max", "mp4", "mov", "avi", "mkv", "webm",
];

// Les **images** ne sont jamais des annexes, à aucune profondeur. Elles l'ont
// été (à la racine, présumées captures de présentation) et c'est la cause du
// bug corrigé ici : `body_shadow.png`, `tyre_*_shadow.png`, `logo.png`,
// `map.png` sont de vrais assets AC qui vivent précisément à la racine du
// dossier du mod. Aucune heuristique d'extension ne les distingue d'une
// capture d'écran — donc on ne tranche pas, on laisse.

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

/// Dossier ressources d'un mod voiture/circuit (§4.5.2) : **au niveau du mod**
/// (pas de la version), hors de l'arborescence junctionnée — survit aux mises
/// à jour, partagé par toutes les couches posées sur ce mod (§4.3).
pub fn resources_dir(library: &Path, kind: ModKind, id: &str) -> PathBuf {
    resources_dir_for(library, kind.content_folder(), &[id])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// Contenu de jeu : copié normalement, sera junctionné.
    Content,
    /// Fichier annexe capturé : rangé dans le dossier ressources.
    Resources,
    /// Fichier annexe non capturé (mode Aucun) : ni content, ni ressources —
    /// laissé dans la source (jamais supprimé), conformément à « Aucun ».
    Drop,
}

fn ext_lower(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
}

/// Route d'un fichier posé **à la racine de ce qui entoure le mod** (§4.5.2) —
/// le seul endroit où un document isolé est une annexe. Sert au routage des
/// restes (§7.3) avant de décider ajout au jeu vs annexe : sans ce test, le
/// `Read Me.pdf` livré à côté d'une voiture deviendrait un ajout au jeu et
/// atterrirait à la racine d'AC.
pub fn route_beside_root(path: &Path, mode: ExtractionMode) -> Route {
    classify(path, true, mode)
}

fn classify(path: &Path, is_root: bool, mode: ExtractionMode) -> Route {
    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        // GUIDs.txt : requis par le moteur audio AC (mapping GUID des .bank
        // FMOD, §12bis.2) — jamais une annexe malgré son extension .txt.
        if name.eq_ignore_ascii_case("GUIDs.txt") {
            return Route::Content;
        }
    }
    let Some(ext) = ext_lower(path) else {
        return Route::Content;
    };
    let ext = ext.as_str();
    // Toutes les catégories d'annexes sont scopées à la racine : dès qu'un
    // fichier est dans un sous-dossier (extension/, data/, skins/…), il fait
    // partie d'un contenu structuré et n'est jamais une annexe.
    let is_info = is_root && INFO_EXTS.contains(&ext);
    let is_heavy = is_root && HEAVY_EXTS.contains(&ext);
    if !(is_info || is_heavy) {
        return Route::Content;
    }
    // Classé annexe (jamais du contenu de jeu, quel que soit le réglage) : la
    // destination dépend seulement du réglage utilisateur.
    match mode {
        ExtractionMode::None => Route::Drop,
        ExtractionMode::InfoOnly => {
            if is_info {
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
fn scan(dir: &Path, mode: ExtractionMode) -> HashMap<PathBuf, Route> {
    let mut out = HashMap::new();
    for entry in WalkDir::new(dir).into_iter().flatten() {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let is_root = path.parent() == Some(dir);
        // Dossier `extension/` (n'importe où dans l'arborescence) : toujours du
        // contenu réel — configs/ressources CSP (ex. extension/sfx), jamais des
        // annexes de présentation, quelle que soit l'extension.
        let rel = path.strip_prefix(dir).unwrap_or(path);
        let under_extension = rel.parent().is_some_and(|p| {
            p.components().any(|c| {
                c.as_os_str()
                    .to_str()
                    .is_some_and(|s| s.eq_ignore_ascii_case("extension"))
            })
        });
        let route = if under_extension {
            Route::Content
        } else {
            classify(path, is_root, mode)
        };
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

/// Ce que représente l'arborescence confiée à `file_mod` — c'est **la** donnée
/// qui décide si l'extraction des annexes s'applique (§4.5.1, règle d'or).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// Le dossier du mod lui-même : celui que l'auteur a conçu pour être posé
    /// dans `content/` (`rss_gtm_lanzo_v8/`), ou son équivalent pour un skin,
    /// un son, une app, une couche. **Rien n'en sort jamais**, à aucune
    /// profondeur : tout ce qu'il contient est du contenu du mod, y compris
    /// ce qui ressemble à une annexe. Un `.pdf` de notice posé au milieu de la
    /// voiture reste où l'auteur l'a mis — dans le doute on ne touche pas.
    ModFolder,
    /// Ce qui était livré **à côté** du dossier du mod : racine de l'archive,
    /// dossiers frères, restes ramassés par le balayage (§7.3). Là, un
    /// document isolé est bien une annexe et n'a rien à faire dans `content/`.
    BesideMod,
}

/// Range `src` dans `content_dest` (contenu de jeu) en redirigeant les fichiers
/// annexes (§4.5.2) vers `resources_dest` selon `mode`, ou en les laissant dans
/// `src` (mode Aucun — jamais supprimés). Renvoie le nombre de fichiers
/// effectivement rangés en ressources.
///
/// **Règle d'or (§4.5.1)** : avec `Source::ModFolder`, rien n'est extrait, quel
/// que soit le réglage — le dossier part d'un bloc. Le tri par extension ne
/// s'applique qu'à ce qui est livré à côté du mod. C'est l'inverse qui avait
/// été codé (tri par extension + profondeur, sans regarder l'appartenance au
/// mod), et de vrais assets AC vivant à la racine du dossier voiture —
/// `body_shadow.png`, `tyre_*_shadow.png`, `logo.png` — ont été sortis de
/// 23 mods.
///
/// Chemin rapide (déplacement/copie du dossier entier) dès qu'il n'y a rien à
/// extraire : toujours pour `ModFolder`, et pour la grande majorité des mods.
pub fn file_mod(
    src: &Path,
    content_dest: &Path,
    resources_dest: &Path,
    mode: ExtractionMode,
    move_files: bool,
    source: Source,
) -> Result<usize, String> {
    file_mod_reported(src, content_dest, resources_dest, mode, move_files, source, &|_| {})
}

/// Comme [`file_mod`], en signalant les octets rangés au fil de l'eau (§4.2bis).
///
/// Ranger un mod est une seule opération, mais elle peut durer des minutes sur
/// un mod de plusieurs Go — et c'était la dernière étape de l'import à ne rien
/// dire d'elle-même. Un `rename` sur le même volume ne signale rien : il est
/// instantané, il n'y a pas de progression à montrer.
#[allow(clippy::too_many_arguments)]
pub fn file_mod_reported(
    src: &Path,
    content_dest: &Path,
    resources_dest: &Path,
    mode: ExtractionMode,
    move_files: bool,
    source: Source,
    on_bytes: &archive::BytesReport,
) -> Result<usize, String> {
    let ancillary = match source {
        Source::ModFolder => HashMap::new(),
        Source::BesideMod => scan(src, mode),
    };
    if ancillary.is_empty() {
        if move_files {
            archive::move_dir_reported(src, content_dest, on_bytes)
        } else {
            archive::copy_dir_reported(src, content_dest, on_bytes)
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
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        match ancillary.get(path).copied().unwrap_or(Route::Content) {
            Route::Content => {
                copy_or_move_one(path, &content_dest.join(rel), move_files)
                    .map_err(|e| format!("rangement bibliothèque : {e}"))?;
                on_bytes(size);
            }
            Route::Resources => {
                extracted += 1;
                copy_or_move_one(path, &resources_dest.join(rel), move_files)
                    .map_err(|e| format!("extraction des fichiers annexes : {e}"))?;
                on_bytes(size);
            }
            Route::Drop => { /* ni content, ni ressources : reste dans la source (§4.5.2, mode Aucun) */ }
        }
    }
    Ok(extracted)
}

/// Fichier annexe listé sur la fiche (§4.5.2, « Bloc Ressources »).
#[derive(Debug, Clone, Serialize)]
pub struct ResourceFile {
    pub name: String,
    /// Chemin relatif au dossier ressources (affichage, sous-dossiers éventuels).
    pub rel_path: String,
    pub size_bytes: u64,
}

/// Liste le contenu du dossier ressources d'un mod, **lu en direct sur disque**
/// (§4.5.2) — jamais mémorisé en base : un fichier déposé manuellement apparaît
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
            let rel = path
                .strip_prefix(dir)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/");
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let size_bytes = e.metadata().map(|m| m.len()).unwrap_or(0);
            ResourceFile {
                name,
                rel_path: rel,
                size_bytes,
            }
        })
        .collect();
    out.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    out
}

/// Résout le chemin absolu d'un fichier ressources à partir de son chemin
/// relatif, avec garde-fou anti-traversée (`../..`) : le résultat doit rester
/// à l'intérieur de `dir`. Utilisé avant toute ouverture (§4.5.2).
pub fn resolve_resource_path(dir: &Path, rel_path: &str) -> Result<PathBuf, String> {
    let candidate = dir.join(rel_path);
    let canon_dir = dir
        .canonicalize()
        .map_err(|e| format!("dossier ressources introuvable : {e}"))?;
    let canon_candidate = candidate
        .canonicalize()
        .map_err(|e| format!("fichier introuvable : {e}"))?;
    if !canon_candidate.starts_with(&canon_dir) {
        return Err(crate::errors::PATH_OUTSIDE_RESOURCES.into());
    }
    Ok(canon_candidate)
}

/// Plafond de lecture d'une ressource prévisualisée (§4.5.2). Le contenu
/// traverse l'IPC puis vit en mémoire dans la WebView : au-delà de ce seuil on
/// renvoie l'utilisateur vers l'application par défaut plutôt que de figer
/// l'interface le temps du transfert. 32 Mio couvre très largement les notices
/// PDF et les changelogs livrés avec un mod.
pub const PREVIEW_MAX_BYTES: u64 = 32 * 1024 * 1024;

/// Lit un fichier du dossier ressources pour prévisualisation (§4.5.2). Même
/// garde-fou anti-traversée que l'ouverture ; les octets sont renvoyés bruts,
/// c'est le front qui décide comment les interpréter (texte, PDF).
pub fn read_resource(dir: &Path, rel_path: &str) -> Result<Vec<u8>, String> {
    let path = resolve_resource_path(dir, rel_path)?;
    let size = std::fs::metadata(&path)
        .map_err(|e| format!("lecture de la ressource : {e}"))?
        .len();
    if size > PREVIEW_MAX_BYTES {
        return Err(crate::errors::RESOURCE_TOO_LARGE.into());
    }
    std::fs::read(&path).map_err(|e| format!("lecture de la ressource : {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"x").unwrap();
    }

    /// Dossier de mod réaliste : du contenu de jeu, et à sa racine des fichiers
    /// que l'ancien tri par extension prenait pour des annexes — dont de vrais
    /// assets AC (`logo.png`, `body_shadow.png`).
    fn make_mod(root: &Path) {
        write(&root.join("ui").join("ui_car.json"));
        write(&root.join("model.kn5"));
        write(&root.join("skins").join("red").join("preview.jpg"));
        write(&root.join("logo.png"));
        write(&root.join("body_shadow.png"));
        write(&root.join("tyre_0_shadow.png"));
        write(&root.join("changelog.txt"));
        write(&root.join("presentation.pdf"));
        write(&root.join("livery_template.psd"));
        write(&root.join("old_templates.zip"));
    }

    #[test]
    fn nothing_is_ever_taken_out_of_a_mod_folder() {
        // Règle d'or (§4.5.1). Bug réel : `body_shadow.png`, `tyre_*_shadow.png`
        // et `logo.png` — de vrais assets AC vivant à la racine du dossier
        // voiture — ont été déplacés en `resources/` sur 23 mods, parce que le
        // classement se fondait sur l'extension et la profondeur au lieu de
        // l'appartenance au mod. Le mode d'extraction le plus agressif ne doit
        // rien pouvoir en sortir.
        for mode in [ExtractionMode::None, ExtractionMode::InfoOnly, ExtractionMode::All] {
            let base = crate::testutil::temp_dir("res-golden");
            let src = base.join("src");
            make_mod(&src);
            let content = base.join("content");
            let resources = base.join("resources");

            let n = file_mod(&src, &content, &resources, mode, false, Source::ModFolder).unwrap();
            assert_eq!(n, 0, "aucune extraction depuis un dossier de mod ({mode:?})");
            assert!(!resources.exists(), "aucun dossier ressources créé ({mode:?})");

            for rel in [
                "ui/ui_car.json",
                "model.kn5",
                "skins/red/preview.jpg",
                "logo.png",
                "body_shadow.png",
                "tyre_0_shadow.png",
                "changelog.txt",
                "presentation.pdf",
                "livery_template.psd",
                "old_templates.zip",
            ] {
                let mut p = content.to_path_buf();
                for seg in rel.split('/') {
                    p = p.join(seg);
                }
                assert!(p.is_file(), "{rel} conservé dans le mod ({mode:?})");
            }
        }
    }

    #[test]
    fn documents_beside_the_mod_are_still_extracted() {
        // L'autre moitié de la règle : ce qui est livré **à côté** du dossier
        // du mod (racine d'archive, reste ramassé §7.3) reste trié — un PDF
        // de présentation n'a rien à faire dans `content/`.
        let base = crate::testutil::temp_dir("res-beside");
        let src = base.join("src");
        write(&src.join("Read Me.pdf"));
        write(&src.join("changelog.txt"));
        write(&src.join("content").join("driver").join("pro.kn5"));
        let content = base.join("content");
        let resources = base.join("resources");

        let n = file_mod(
            &src,
            &content,
            &resources,
            ExtractionMode::InfoOnly,
            false,
            Source::BesideMod,
        )
        .unwrap();
        assert_eq!(n, 2, "les deux documents de la racine sont capturés");
        assert!(resources.join("Read Me.pdf").is_file());
        assert!(resources.join("changelog.txt").is_file());
        assert!(!content.join("Read Me.pdf").exists(), "jamais dans le contenu de jeu");
        assert!(
            content.join("content").join("driver").join("pro.kn5").is_file(),
            "le vrai contenu, lui, est rangé normalement"
        );
    }

    #[test]
    fn images_are_never_ancillary_even_beside_the_mod() {
        // Aucune heuristique d'extension ne distingue une capture de
        // présentation d'un asset AC (`map.png`, `logo.png`, preview de skin).
        // On ne tranche donc plus du tout : une image est toujours du contenu.
        let base = crate::testutil::temp_dir("res-img");
        let src = base.join("src");
        write(&src.join("screenshot.jpg"));
        write(&src.join("map.png"));
        let content = base.join("content");
        let resources = base.join("resources");

        let n = file_mod(
            &src,
            &content,
            &resources,
            ExtractionMode::All,
            false,
            Source::BesideMod,
        )
        .unwrap();
        assert_eq!(n, 0, "aucune image n'est une annexe");
        assert!(content.join("screenshot.jpg").is_file());
        assert!(content.join("map.png").is_file(), "mini-carte AC jamais extraite");
        assert!(!resources.exists());
    }

    #[test]
    fn deep_files_in_extension_folder_never_extracted_as_resources() {
        // Bug réel : un .txt/.pdf niché dans un sous-dossier fonctionnel
        // (ex. `extension/`, config CSP) n'est pas une annexe — seuls les
        // fichiers à la racine le sont. Avant le fix, INFO_EXTS/HEAVY_EXTS
        // étaient extraits quelle que soit la profondeur, cassant ce cas.
        let base = crate::testutil::temp_dir("res-ext");
        let src = base.join("src");
        write(&src.join("ui").join("ui_track.json"));
        write(&src.join("extension").join("config").join("tracks").join("readme.txt"));
        write(&src.join("extension").join("weather_notes.pdf"));
        write(&src.join("data").join("templates.zip")); // lourd, en profondeur : jamais annexe non plus
        write(&src.join("changelog.txt")); // annexe : celui-là est à la racine
        let content = base.join("content");
        let resources = base.join("resources");

        let n = file_mod(
            &src,
            &content,
            &resources,
            ExtractionMode::All,
            false,
            Source::BesideMod,
        )
        .unwrap();
        assert_eq!(n, 1, "seul changelog.txt (racine) est une annexe");

        assert!(
            content
                .join("extension")
                .join("config")
                .join("tracks")
                .join("readme.txt")
                .is_file(),
            "fichier profond dans extension/ conservé comme contenu"
        );
        assert!(content.join("extension").join("weather_notes.pdf").is_file());
        assert!(content.join("data").join("templates.zip").is_file());
        assert!(
            !resources.join("extension").exists(),
            "rien d'extension/ ne doit finir en ressources"
        );
        assert!(resources.join("changelog.txt").is_file());
    }

    #[test]
    fn info_only_extracts_light_files_and_drops_heavy() {
        let base = crate::testutil::temp_dir("res");
        let src = base.join("src");
        write(&src.join("changelog.txt"));
        write(&src.join("presentation.pdf"));
        write(&src.join("livery_template.psd"));
        write(&src.join("old_templates.zip"));
        write(&src.join("content").join("gui").join("flag.dds"));
        let content = base.join("content");
        let resources = base.join("resources");

        let n = file_mod(
            &src,
            &content,
            &resources,
            ExtractionMode::InfoOnly,
            false,
            Source::BesideMod,
        )
        .unwrap();
        assert_eq!(n, 2, "changelog.txt + presentation.pdf");
        assert!(resources.join("changelog.txt").is_file());
        assert!(resources.join("presentation.pdf").is_file());
        assert!(
            !resources.join("livery_template.psd").exists(),
            "mode info_only : pas les lourds"
        );
        assert!(
            !content.join("livery_template.psd").exists(),
            "annexe lourde jamais dans le contenu de jeu"
        );
        assert!(content.join("content").join("gui").join("flag.dds").is_file());
    }

    #[test]
    fn all_mode_extracts_heavy_files_too() {
        let base = crate::testutil::temp_dir("res");
        let src = base.join("src");
        write(&src.join("changelog.txt"));
        write(&src.join("livery_template.psd"));
        write(&src.join("old_templates.zip"));
        let content = base.join("content");
        let resources = base.join("resources");

        let n = file_mod(&src, &content, &resources, ExtractionMode::All, true, Source::BesideMod).unwrap();
        assert_eq!(n, 3, "le léger + template.psd + archive.zip");
        assert!(resources.join("livery_template.psd").is_file());
        assert!(resources.join("old_templates.zip").is_file());
    }

    #[test]
    fn none_mode_drops_ancillary_and_leaves_source_untouched_on_copy() {
        let base = crate::testutil::temp_dir("res");
        let src = base.join("src");
        write(&src.join("changelog.txt"));
        write(&src.join("content").join("gui").join("flag.dds"));
        let content = base.join("content");
        let resources = base.join("resources");

        // copy=true (préserve la source) : les annexes doivent rester dans src.
        let n = file_mod(
            &src,
            &content,
            &resources,
            ExtractionMode::None,
            false,
            Source::BesideMod,
        )
        .unwrap();
        assert_eq!(n, 0, "rien n'est extrait en mode Aucun");
        assert!(!resources.exists(), "aucun dossier ressources créé");
        assert!(!content.join("changelog.txt").exists());
        assert!(
            src.join("changelog.txt").is_file(),
            "annexe laissée dans la source (§4.5.2, mode Aucun)"
        );
        assert!(
            content.join("content").join("gui").join("flag.dds").is_file(),
            "le contenu de jeu, lui, est bien rangé"
        );
    }

    #[test]
    fn fast_path_used_when_no_ancillary_files() {
        // Pas de fichier annexe : le dossier entier est déplacé/copié d'un bloc
        // (comportement historique préservé, aucune perte de perf).
        let base = crate::testutil::temp_dir("res");
        let src = base.join("src");
        write(&src.join("ui").join("ui_car.json"));
        write(&src.join("model.kn5"));
        let content = base.join("content");
        let resources = base.join("resources");

        let n = file_mod(
            &src,
            &content,
            &resources,
            ExtractionMode::All,
            false,
            Source::BesideMod,
        )
        .unwrap();
        assert_eq!(n, 0);
        assert!(content.join("model.kn5").is_file());
        assert!(!resources.exists());
    }

    #[test]
    fn guids_txt_never_extracted() {
        // Fichier requis par le moteur audio AC (§12bis.2) : ne doit jamais
        // être traité comme une annexe malgré son extension .txt.
        let base = crate::testutil::temp_dir("res-guids");
        let src = base.join("src");
        write(&src.join("GUIDs.txt"));
        write(&src.join("car.bank"));
        write(&src.join("readme.txt")); // vraie annexe, elle
        let content = base.join("content");
        let resources = base.join("resources");

        let n = file_mod(
            &src,
            &content,
            &resources,
            ExtractionMode::All,
            false,
            Source::BesideMod,
        )
        .unwrap();
        assert_eq!(n, 1, "seul readme.txt est une annexe");
        assert!(content.join("GUIDs.txt").is_file(), "GUIDs.txt reste du contenu");
        assert!(content.join("car.bank").is_file());
        assert!(resources.join("readme.txt").is_file());
    }

    #[test]
    fn list_and_resolve_resources() {
        let base = crate::testutil::temp_dir("res-list");
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
    }

    #[test]
    fn read_resource_honours_the_traversal_guard() {
        // La prévisualisation (§4.5.2) lit le fichier au lieu de le confier à
        // l'OS : elle doit rester tenue par le même garde-fou que l'ouverture,
        // sans quoi un `rel_path` forgé lirait n'importe quel fichier du disque
        // et en renverrait le contenu au front.
        let base = crate::testutil::temp_dir("res-read");
        let dir = base.join("resources");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("readme.txt"), b"hello").unwrap();
        std::fs::write(base.join("secret.txt"), b"nope").unwrap();

        assert_eq!(
            read_resource(&dir, "readme.txt").unwrap(),
            b"hello",
            "contenu brut rendu tel quel"
        );
        assert!(
            read_resource(&dir, "../secret.txt").is_err(),
            "aucune lecture hors du dossier ressources"
        );
        assert!(
            read_resource(&dir, "absent.txt").is_err(),
            "fichier inexistant : erreur, pas de panique"
        );
    }

    #[test]
    fn read_resource_refuses_oversized_files() {
        // Au-delà du plafond, la prévisualisation refuse plutôt que de faire
        // transiter des dizaines de Mo par l'IPC : l'utilisateur garde le clic
        // « ouvrir avec l'application par défaut ».
        let base = crate::testutil::temp_dir("res-big");
        let dir = base.join("resources");
        std::fs::create_dir_all(&dir).unwrap();
        let big = vec![0u8; (PREVIEW_MAX_BYTES + 1) as usize];
        std::fs::write(dir.join("huge.pdf"), &big).unwrap();

        assert_eq!(
            read_resource(&dir, "huge.pdf").unwrap_err(),
            crate::errors::RESOURCE_TOO_LARGE,
            "erreur traduisible, pas un message technique"
        );
    }
}
