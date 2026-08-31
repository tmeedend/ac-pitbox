//! Couches / extensions (§4.4). Quand un import cible un contenu existant sans
//! être une mise à jour (surtout des chemins nouveaux, peu d'écrasements) — ou
//! qu'il vise un contenu de base Kunos (`is_stock`, jamais remplaçable) — le
//! contenu entrant est rangé **à part**, rattaché à la base, sans jamais toucher
//! la version active de celle-ci. Rien n'est détruit : « restaurer » = supprimer
//! la couche, la base est intacte dessous.
//!
//! Les couches sont **composées** : `compose::recompose` fusionne la base et
//! les couches actives, par ordre de priorité, dans `content/<type>/<id>`
//! (`deploy::compose_tree`). Retirer une couche recompose sans elle, et la base
//! réapparaît intacte — c'est ce qui fait de la couche la seule réponse non
//! destructive à « copiez ces fichiers dans le dossier du mod », que ce soit
//! une extension importée ou un dossier proposé par l'auteur (§4.6ter).

use std::path::{Path, PathBuf};

use chrono::Local;
use rusqlite::Connection;
use uuid::Uuid;

use crate::identity::DiffStats;
use crate::modscan::ModKind;
use crate::resources::{self, ExtractionMode};
use crate::{importer, overlay};

/// Ce à quoi une couche peut se rattacher (§4.4). C'est exactement ce que porte
/// `layers.parent_kind`, et ce que le marqueur de déploiement enregistre.
///
/// Distinct de [`ModKind`] parce qu'une **app** en reçoit elle aussi : un mod
/// qui ajoute des fichiers dans le dossier d'une app est très exactement une
/// couche (§12bis.4). Elle n'a pourtant ni dossier `content/<type>s/`, ni
/// version, ni fiche technique — ce n'est pas un mod, et l'élargissement de
/// `ModKind` aurait contaminé tout ce qui s'en sert pour choisir un dossier de
/// contenu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostKind {
    Car,
    Track,
    App,
}

impl HostKind {
    /// Valeur persistée (`layers.parent_kind`, marqueur de déploiement).
    pub fn as_str(self) -> &'static str {
        match self {
            HostKind::Car => "Car",
            HostKind::Track => "Track",
            HostKind::App => "App",
        }
    }

    /// Relit ce qu'[`Self::as_str`] a écrit dans `layers.parent_kind`.
    /// Tolérante à la casse et au pluriel, pour la raison qui vaut déjà pour
    /// `OwnerKind::parse` : ce type est persisté et comparé à plusieurs
    /// endroits, et un écart y serait **silencieux** — la couche irait chercher
    /// son hôte dans le mauvais espace de noms et n'y trouverait rien.
    /// Repli sur `Car` : les deux types de mod partagent un espace de noms, donc
    /// seule la confusion avec `App` aurait des conséquences.
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "app" | "apps" => HostKind::App,
            "track" | "tracks" => HostKind::Track,
            _ => HostKind::Car,
        }
    }

    /// Segment de bibliothèque du dossier ressources : `<lib>/resources/<cat>/<id>`.
    /// Aligné sur ce qu'écrivent déjà `resources_dir` (mods) et `import_apps`
    /// (apps) — une couche range ses annexes au même endroit que son hôte.
    pub fn resources_category(self) -> &'static str {
        match self {
            HostKind::Car => "cars",
            HostKind::Track => "tracks",
            HostKind::App => "apps",
        }
    }
}

impl From<ModKind> for HostKind {
    fn from(k: ModKind) -> Self {
        match k {
            ModKind::Car => HostKind::Car,
            ModKind::Track => HostKind::Track,
        }
    }
}

/// Range un contenu entrant comme couche/extension rattachée à `parent_id`.
/// Ne modifie jamais la base. Fichiers annexes (§4.5.2) redirigés vers le
/// dossier ressources du mod, jamais dans la couche elle-même. Renvoie l'id de
/// couche créé et le nombre de fichiers annexes extraits.
#[allow(clippy::too_many_arguments)]
pub fn store_layer(
    conn: &Connection,
    library: &Path,
    parent_id: &str,
    kind: HostKind,
    name: &str,
    src_dir: &Path,
    copy: bool,
    diff: &DiffStats,
    archive_name: &str,
    mode: ExtractionMode,
) -> Result<(String, usize), String> {
    // Garde-fou (§12bis.1bis) : un mod installé hors Pit Box ne reçoit jamais
    // de couche. Poser une couche entraîne la composition dans `content/`,
    // donc la sauvegarde puis **l'effacement** du vrai dossier par
    // `compose::recompose_stock` — sur un dossier que l'utilisateur a posé
    // lui-même et que l'app n'a pas mis là. L'app n'y touche donc jamais : le
    // seul chemin vers un mod géré est que l'utilisateur retire lui-même le
    // dossier du jeu et importe le mod (décision assumée — adopter un dossier
    // en place demandait d'écrire dans `content/` pour un gain que le
    // réimport donne sans risque).
    //
    // Le garde vit ici plutôt que chez les appelants parce qu'ils sont quatre
    // (import, dossier proposé par l'auteur §4.6ter, projection de sous-mod,
    // et le prochain qui viendra) : un point d'étranglement les couvre tous,
    // y compris celui qui n'existe pas encore.
    if let Ok(Some(parent)) = overlay::get_mod(conn, parent_id) {
        if parent.is_unmanaged {
            return Err(crate::errors::UNMANAGED_NO_LAYER.into());
        }
    }
    let dest = importer::unique_dir(&library.join("layers").join(parent_id).join(name));
    let res_dir = resources::resources_dir_for(library, kind.resources_category(), &[parent_id]);
    let extracted = resources::file_mod(src_dir, &dest, &res_dir, mode, !copy, resources::Source::ModFolder)?;

    let now = Local::now().to_rfc3339();
    let id = Uuid::new_v4().to_string();
    let priority = overlay::next_layer_priority(conn, parent_id, kind).map_err(|e| e.to_string())?;
    overlay::insert_layer(
        conn,
        &id,
        parent_id,
        kind.as_str(),
        name,
        &crate::libpath::to_relative(Some(library), &dest),
        Some(archive_name),
        diff.added as i64,
        diff.overwritten as i64,
        priority,
        &now,
    )
    .map_err(|e| e.to_string())?;

    // Détails d'historique = payload structuré (§ i18n) : le front rend le texte
    // localisé via `history.<key>` + params ; les lignes héritées (texte brut)
    // restent affichées telles quelles.
    overlay::add_history(
        conn,
        parent_id,
        &now,
        "EXTENSION_ADDED",
        &serde_json::json!({
            "key": "extensionAdded",
            "archive": archive_name,
            "added": diff.added,
            "overwritten": diff.overwritten,
        })
        .to_string(),
    )
    .map_err(|e| e.to_string())?;

    Ok((id, extracted))
}

/// Un fichier apporté par une couche (§4.4).
#[derive(Debug, Clone, serde::Serialize)]
pub struct LayerFile {
    /// Chemin **relatif au dossier de l'hôte** — c'est l'information utile :
    /// elle dit où le fichier atterrit une fois composé (`ui/<layout>/preview.png`,
    /// `data/<circuit>.json` pour une couche d'app).
    pub rel_path: String,
    pub size_bytes: u64,
    /// Ce chemin existe déjà dans la base : la couche le **remplace** au lieu
    /// de l'ajouter. C'est le seul détail qui mérite d'être vu d'un coup d'œil,
    /// les décomptes globaux ne disant pas *lesquels*.
    pub overwrites: bool,
}

/// Fichiers apportés par une couche, avec ce que chacun fait à la base.
///
/// La fiche n'affichait que deux décomptes (« n ajoutés · m écrasés ») : de quoi
/// savoir qu'il se passe quelque chose, pas de quoi savoir quoi. Sur une couche
/// d'app — un `.lua` de réglages, des fichiers de caméras — c'est précisément
/// la liste qui dit à quoi elle sert.
pub fn list_files(conn: &Connection, cfg: &crate::config::AppConfig, layer_id: &str) -> Result<Vec<LayerFile>, String> {
    let layer = overlay::get_layer(conn, layer_id)
        .map_err(|e| e.to_string())?
        .ok_or(crate::errors::LAYER_NOT_FOUND)?;
    let dir = crate::libpath::resolve(cfg.library_path.as_deref(), &layer.library_path)
        .ok_or(crate::errors::LIBRARY_NOT_CONFIGURED)?;
    // Base de l'hôte, quand il est là : c'est elle qui dit si un chemin est un
    // ajout ou un remplacement. Absente (couche en attente de son hôte), tout
    // est un ajout — ce qui est exact.
    let base = host_base_dir(conn, cfg, &layer.parent_id);

    let mut out: Vec<LayerFile> = walkdir::WalkDir::new(&dir)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| {
            let rel = e.path().strip_prefix(&dir).ok()?;
            Some(LayerFile {
                // Séparateurs unifiés : le chemin est affiché tel quel, et un
                // mélange `\`/`/` selon la plateforme d'origine se voit.
                rel_path: rel.to_string_lossy().replace('\\', "/"),
                size_bytes: e.metadata().map(|m| m.len()).unwrap_or(0),
                overwrites: base.as_ref().is_some_and(|b| b.join(rel).is_file()),
            })
        })
        .collect();
    out.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    Ok(out)
}

/// Dossier de la couche, pour l'ouvrir dans l'explorateur.
pub fn folder_path(conn: &Connection, cfg: &crate::config::AppConfig, layer_id: &str) -> Result<PathBuf, String> {
    let layer = overlay::get_layer(conn, layer_id)
        .map_err(|e| e.to_string())?
        .ok_or(crate::errors::LAYER_NOT_FOUND)?;
    crate::libpath::resolve(cfg.library_path.as_deref(), &layer.library_path)
        .ok_or_else(|| crate::errors::LIBRARY_NOT_CONFIGURED.to_string())
}

/// Dossier de base de l'hôte d'une couche, quel que soit son type (§4.4) : la
/// version active pour un mod, le dossier de bibliothèque pour une app.
/// `None` quand l'hôte n'est pas (encore) là — une couche en attente.
fn host_base_dir(conn: &Connection, cfg: &crate::config::AppConfig, parent_id: &str) -> Option<PathBuf> {
    if overlay::get_mod(conn, parent_id).ok().flatten().is_some() {
        return crate::submods::parent_content_dir(conn, cfg, parent_id);
    }
    let app = overlay::get_app(conn, parent_id).ok().flatten()?;
    crate::libpath::resolve(cfg.library_path.as_deref(), &app.library_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::DiffStats;

    /// Rule (§12bis.1bis): nothing is ever layered onto a mod the user
    /// installed outside Pit Box. The guard lives in `store_layer` rather than
    /// in its callers precisely so that this test covers all of them — import,
    /// author-supplied folder (§4.6ter) and sub-mod projection alike.
    ///
    /// What it protects against is not the layer row but its consequence:
    /// composing means `compose::recompose_stock` backs up and then **deletes**
    /// the real folder in `content/` — a folder the app never put there.
    #[test]
    fn an_unmanaged_mod_never_receives_a_layer() {
        let base = crate::testutil::temp_dir("layer-unmanaged");
        let library = base.join("library");
        let src = base.join("incoming");
        std::fs::create_dir_all(&library).unwrap();
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("extra.kn5"), b"x").unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        overlay::upsert_stock_mod(&conn, "srp", "Track", None, Some("Shutoko"), "now", true).unwrap();

        let err = store_layer(
            &conn,
            &library,
            "srp",
            HostKind::Track,
            "extra",
            &src,
            true,
            &DiffStats::default(),
            "extra.zip",
            ExtractionMode::None,
        )
        .unwrap_err();

        assert_eq!(err, crate::errors::UNMANAGED_NO_LAYER, "refused, with a reason");
        assert_eq!(
            overlay::list_layers(&conn, "srp", HostKind::Track).unwrap().len(),
            0,
            "no layer stored"
        );
        assert!(
            !library.join("layers").join("srp").exists(),
            "nothing written to the library either"
        );
    }
}
