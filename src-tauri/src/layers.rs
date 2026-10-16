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
/// couche (§8.4). Elle n'a pourtant ni dossier `content/<type>s/`, ni
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

    /// Segment de bibliothèque par type : `<lib>/layers/<cat>/<id>` et
    /// `<lib>/resources/<cat>/<id>`. Aligné sur ce qu'écrivent déjà
    /// `resources_dir` (mods), `import_apps` (apps) et `extras::dir`.
    pub fn category(self) -> &'static str {
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
    // Garde-fou (§8.2) : un mod installé hors Pit Box ne reçoit jamais
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
    // `<lib>/layers/<type>/<hôte>/<nom>` — le segment de type aligne cet arbre
    // sur `extras/` et `resources/`, et rend lisible à l'œil ce que la base sait
    // déjà : un circuit et une app peuvent porter le même id (§4.4). Les couches
    // rangées **avant** ce segment gardent leur chemin, qui est lu en base et
    // jamais recalculé — elles continuent donc de fonctionner là où elles sont.
    let dest = importer::unique_dir(&library.join("layers").join(kind.category()).join(parent_id).join(name));
    let res_dir = resources::resources_dir_for(library, kind.category(), &[parent_id]);
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

/// Gives an existing layer new content (§4.6ter): the same proposed folder,
/// answered "add to the mod's folder" again, in a newer archive. The layer
/// keeps its id, its place in the order and its on/off state — only what it
/// brings changes. Storing it as a new layer instead stacked a second "2K
/// Skins" on the car at every update, and the order between two variants
/// (which livery covers which) would depend on the order of the archive.
///
/// Also how a layer in the showcase gets its files back (ESPACE§7.5): same
/// row, same name and notes — refilling makes it complete again.
///
/// `move_files`: the source is moved (a folder the import owns) or copied
/// (a folder of the user's, imported with "keep the source").
///
/// The caller recomposes afterwards, as after `store_layer`.
#[allow(clippy::too_many_arguments)]
pub fn refill_layer(
    conn: &Connection,
    library: &Path,
    layer: &overlay::LayerRow,
    src_dir: &Path,
    diff: &DiffStats,
    archive_name: &str,
    mode: ExtractionMode,
    move_files: bool,
) -> Result<(), String> {
    let dest =
        crate::libpath::resolve(Some(library), &layer.library_path).ok_or(crate::errors::LIBRARY_NOT_CONFIGURED)?;
    // A stored path is a library path: anything else is not ours to empty.
    if !dest.starts_with(library.join("layers")) {
        return Err(format!("layer outside the library: {}", dest.display()));
    }
    if dest.exists() {
        std::fs::remove_dir_all(&dest).map_err(|e| e.to_string())?;
    }
    let kind = HostKind::parse(&layer.parent_kind);
    let res_dir = resources::resources_dir_for(library, kind.category(), &[&layer.parent_id]);
    resources::file_mod(src_dir, &dest, &res_dir, mode, move_files, resources::Source::ModFolder)?;
    overlay::update_layer_content(
        conn,
        &layer.id,
        archive_name,
        diff.added as i64,
        diff.overwritten as i64,
        &Local::now().to_rfc3339(),
    )
    .map_err(|e| e.to_string())
}

/// Turns a mod into a layer of another mod of the same kind (§4.3bis), and
/// returns the layer's id.
///
/// The way out of the fuzzy conflict of §4.2 when detection missed: real case,
/// `grandvalleyspeedway2010.rar`, 2010 layouts meant to be extracted **into**
/// `grand_valley_speedway_LL`, imported as a track of their own, and only
/// "keep both" or "replace" on offer. Neither was right: one leaves a track
/// the game cannot load, the other throws away the base it needs.
///
/// Only a freshly imported mod qualifies — one complete version, no layers of
/// its own, nothing attached, no additions outside its folder — which is what
/// the conflict hands over. Anything richer (a
/// history of versions, layers stacked on it) would lose something in the
/// move, so it is refused rather than half-converted.
///
/// In order: out of the game by the normal path, files moved into the layer
/// (the base of the host is never touched), resources handed to the host,
/// the mod's library folder and row removed, the host recomposed.
pub fn mod_into_layer(
    conn: &Connection,
    cfg: &crate::config::AppConfig,
    mod_id: &str,
    host_id: &str,
) -> Result<String, String> {
    let library = cfg
        .library_path
        .as_deref()
        .ok_or(crate::errors::LIBRARY_NOT_CONFIGURED)?;
    let m = overlay::get_mod(conn, mod_id)
        .map_err(|e| e.to_string())?
        .ok_or(crate::errors::MOD_NOT_FOUND)?;
    let host = overlay::get_mod(conn, host_id)
        .map_err(|e| e.to_string())?
        .ok_or(crate::errors::MOD_NOT_FOUND)?;
    if mod_id == host_id || m.kind != host.kind || m.is_stock || m.is_unmanaged {
        return Err(crate::errors::NOT_LAYERABLE.into());
    }
    // `store_layer` refuses it too, but only once the mod is out of the game:
    // asked first, a refusal leaves everything as it was.
    if host.is_unmanaged {
        return Err(crate::errors::UNMANAGED_NO_LAYER.into());
    }
    let kind = ModKind::from_column(&m.kind);
    let versions = overlay::get_versions(conn, mod_id).map_err(|e| e.to_string())?;
    let [version] = versions.as_slice() else {
        return Err(crate::errors::NOT_LAYERABLE.into());
    };
    let own_layers = overlay::list_layers(conn, mod_id, kind.into()).map_err(|e| e.to_string())?;
    let subs = overlay::list_subs_for_parent(conn, mod_id).map_err(|e| e.to_string())?;
    // What a layer cannot carry is refused, never dropped: skins or sounds
    // attached to the mod (their projections are junctions inside the version
    // folder, which the move would walk through into the stored skin), and
    // additions laid outside its folder (a layer composes `content/<type>/<id>`
    // only).
    let attached = subs.iter().any(|s| s.removable);
    let extras = crate::extras::dir(library, kind.into(), mod_id).exists();
    if version.is_skeleton() || !own_layers.is_empty() || attached || extras {
        return Err(crate::errors::NOT_LAYERABLE.into());
    }
    let src = crate::libpath::resolve(Some(library), &version.library_path).ok_or(crate::errors::NOT_LAYERABLE)?;
    // Our own version folder, and nothing else: it is moved, then its
    // parent removed below.
    let mod_dir = library.join(kind.content_folder()).join(mod_id);
    if !src.starts_with(&mod_dir) || !src.is_dir() {
        return Err(crate::errors::NOT_LAYERABLE.into());
    }

    if crate::activation::is_mod_active(cfg, kind, mod_id) {
        crate::activation::deactivate(conn, cfg, mod_id)?;
    }
    let diff = crate::submods::parent_content_dir(conn, cfg, host_id)
        .filter(|base| base.is_dir())
        .map(|base| crate::identity::diff_content(&src, &base))
        .unwrap_or_default();
    // The archive name is the layer's identity (`as_layer`): importing the
    // same archive again replaces this layer instead of stacking a second.
    let archive = version.source_archive.clone().unwrap_or_else(|| mod_id.to_string());
    let mode = ExtractionMode::parse(&cfg.prefs.resource_extraction_mode);
    let (layer_id, _) = store_layer(
        conn,
        library,
        host_id,
        kind.into(),
        mod_id,
        &src,
        false,
        &diff,
        &archive,
        mode,
    )?;

    hand_over_resources(
        &resources::resources_dir(library, kind, mod_id),
        &resources::resources_dir(library, kind, host_id),
    );
    if let Err(e) = std::fs::remove_dir_all(&mod_dir) {
        log::warn!("mod_into_layer: {} left behind: {e}", mod_dir.display());
    }
    // The skins delivered with the mod moved with its files: their rows
    // would point into a folder that no longer exists.
    for sub in subs.iter().filter(|s| !s.removable) {
        overlay::delete_sub_mod(conn, &sub.id).map_err(|e| e.to_string())?;
    }
    overlay::delete_mod(conn, mod_id).map_err(|e| e.to_string())?;
    if let Err(e) = crate::compose::recompose(conn, cfg, host_id) {
        log::warn!("recompose {host_id} after {mod_id} became its layer: {e}");
    }
    Ok(layer_id)
}

/// Moves what a mod's resources folder holds into its host's, where a layer's
/// resources live (§4.5.2). Best-effort: a resource left behind is a notice
/// that stays readable in the explorer, never content the game misses.
fn hand_over_resources(from: &Path, to: &Path) {
    let Ok(entries) = std::fs::read_dir(from) else {
        return;
    };
    if let Err(e) = std::fs::create_dir_all(to) {
        log::warn!("hand_over_resources: {}: {e}", to.display());
        return;
    }
    for entry in entries.flatten() {
        let target = importer::unique_dir(&to.join(entry.file_name()));
        if let Err(e) = std::fs::rename(entry.path(), &target) {
            log::warn!("hand_over_resources: {}: {e}", entry.path().display());
        }
    }
    if let Err(e) = std::fs::remove_dir(from) {
        log::warn!("hand_over_resources: {} not empty: {e}", from.display());
    }
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
/// Un tracé de circuit apporté par une couche (REFONTE§7.7).
///
/// La carte des tracés montre l'**état composé** — ce que l'utilisateur verra
/// au lancement, couches comprises — mais elle ne disait pas d'où venait chaque
/// tracé. Sur un circuit dont une extension ajoute une variante, « 2 tracés »
/// est exact et trompeur à la fois : l'un des deux n'est pas dans le mod.
#[derive(Debug, Clone, serde::Serialize)]
pub struct LayoutOrigin {
    /// Identifiant du tracé, tel que le porte le dossier (`""` = tracé unique).
    pub layout: String,
    pub layer_id: String,
    /// Nom de l'archive/du dossier de la couche, pour l'afficher tel quel.
    pub layer_name: String,
}

/// Les tracés qu'une **couche active** ajoute à un circuit.
///
/// Deux conditions, et la seconde compte autant que la première : la couche
/// doit poser des fichiers sous le tracé, **et** la base ne doit pas déjà le
/// connaître. Une couche qui ne fait que remplacer la texture d'un tracé
/// existant n'en est pas l'origine — elle l'habille.
///
/// Seules les couches **actives** sont regardées : la carte décrit l'état
/// composé, pas ce qui pourrait l'être.
pub fn layout_origins(
    conn: &Connection,
    cfg: &crate::config::AppConfig,
    parent_id: &str,
    host: HostKind,
) -> Result<Vec<LayoutOrigin>, String> {
    let base = host_base_dir(conn, cfg, parent_id);
    let layers = overlay::list_layers(conn, parent_id, host).map_err(|e| e.to_string())?;
    let mut out: Vec<LayoutOrigin> = Vec::new();
    for layer in layers.iter().filter(|l| l.is_active) {
        let Some(dir) = crate::libpath::resolve(cfg.library_path.as_deref(), &layer.library_path) else {
            continue;
        };
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        for entry in walkdir::WalkDir::new(&dir).into_iter().flatten() {
            if !entry.file_type().is_file() {
                continue;
            }
            let Ok(rel) = entry.path().strip_prefix(&dir) else {
                continue;
            };
            let parts: Vec<String> = rel
                .components()
                .map(|c| c.as_os_str().to_string_lossy().to_string())
                .collect();
            // Un tracé se reconnaît à son dossier : `<tracé>/…` pour les
            // données, `ui/<tracé>/…` pour sa fiche. Les deux désignent le
            // même, d'où la normalisation avant de dédoublonner.
            let layout = match parts.as_slice() {
                [first, _rest @ ..] if first.eq_ignore_ascii_case("ui") && parts.len() > 2 => parts[1].clone(),
                [first, _rest @ ..] if parts.len() > 1 => first.clone(),
                _ => continue,
            };
            if layout.eq_ignore_ascii_case("ui") || !seen.insert(layout.clone()) {
                continue;
            }
            // Déjà dans la base : la couche l'habille, elle ne l'apporte pas.
            let known = base
                .as_ref()
                .is_some_and(|b| b.join(&layout).is_dir() || b.join("ui").join(&layout).is_dir());
            if known {
                continue;
            }
            out.push(LayoutOrigin {
                layout,
                layer_id: layer.id.clone(),
                layer_name: layer.source_archive.clone().unwrap_or_else(|| layer.name.clone()),
            });
        }
    }
    Ok(out)
}

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

    /// Rule (§8.2): nothing is ever layered onto a mod the user
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

    /// Rule (REFONTE§7.7): a layer is the ORIGIN of a track layout only when
    /// the base does not already know it. A layer that merely replaces a file
    /// of an existing layout dresses it up, it does not bring it — and saying
    /// otherwise on the card would mislabel the track's own content as an
    /// add-on.
    ///
    /// Inactive layers are ignored: the card describes the composed state, the
    /// one the user will get at launch, not what could be composed.
    #[test]
    fn only_layouts_the_base_does_not_have_count_as_brought_by_a_layer() {
        let base = crate::testutil::temp_dir("layout-origins");
        let library = base.join("library");
        let ac = base.join("ac");
        // Base du circuit : un seul tracé, `gp`, avec sa fiche.
        let track = ac.join("content").join("tracks").join("shuto");
        std::fs::create_dir_all(track.join("gp")).unwrap();
        std::fs::create_dir_all(track.join("ui").join("gp")).unwrap();
        let cfg = crate::config::AppConfig {
            ac_install_path: Some(ac.clone()),
            library_path: Some(library.clone()),
            ..Default::default()
        };
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let now = chrono::Local::now().to_rfc3339();
        overlay::upsert_mod(&conn, "shuto", "Track", None, Some("Shuto"), "h", None, &now).unwrap();

        // Trois couches : l'une apporte un tracé, l'autre habille celui de la
        // base, la troisième apporterait un tracé mais elle est inactive.
        let make = |name: &str, files: &[&str], active: i64| {
            let dir = library.join("layers").join("tracks").join("shuto").join(name);
            for rel in files {
                let path = dir.join(rel);
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(&path, b"X").unwrap();
            }
            overlay::insert_layer(
                &conn,
                name,
                "shuto",
                "Track",
                name,
                &format!("layers/tracks/shuto/{name}"),
                Some(&format!("{name}.7z")),
                1,
                0,
                0,
                &now,
            )
            .unwrap();
            overlay::set_layer_active(&conn, name, active != 0).unwrap();
        };
        make("club", &["club/data.ini", "ui/club/ui_track.json"], 1);
        make("reskin", &["gp/map.png"], 1);
        make("night", &["night/data.ini"], 0);

        let origins = layout_origins(&conn, &cfg, "shuto", HostKind::Track).unwrap();
        let names: Vec<&str> = origins.iter().map(|o| o.layout.as_str()).collect();
        assert_eq!(names, vec!["club"], "seul le tracé absent de la base est apporté");
        assert_eq!(origins[0].layer_name, "club.7z", "la couche est nommée par son archive");
        drop(base);
    }

    /// Règle (§4.4) : une couche est rangée sous le type de son hôte —
    /// `<lib>/layers/<type>/<hôte>/<nom>` — comme `extras/` et `resources/`.
    ///
    /// Ce n'est pas une exigence de correction : la base distingue déjà les
    /// espaces de noms. C'est de la lisibilité sur le disque, et ça évite qu'un
    /// circuit et une app homonymes voient leurs couches se mêler dans un même
    /// dossier. Les couches rangées avant ce segment gardent leur chemin, lu en
    /// base et jamais recalculé.
    #[test]
    fn a_layer_is_filed_under_its_host_type() {
        let base = crate::testutil::temp_dir("layer-path");
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let now = chrono::Local::now().to_rfc3339();
        overlay::upsert_mod(&conn, "shuto", "Track", None, Some("Shuto"), "h", None, &now).unwrap();
        overlay::insert_app(&conn, "shuto", "apps/shuto", None, &now).unwrap();

        let src = base.join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("a.ini"), b"[X]").unwrap();

        let zero = DiffStats {
            added: 1,
            overwritten: 0,
            existing_total: 0,
        };
        for kind in [HostKind::Track, HostKind::App] {
            store_layer(
                &conn,
                &library,
                "shuto",
                kind,
                "pack",
                &src,
                true,
                &zero,
                "pack.7z",
                ExtractionMode::None,
            )
            .unwrap();
        }

        assert!(
            library
                .join("layers")
                .join("tracks")
                .join("shuto")
                .join("pack")
                .is_dir(),
            "la couche du circuit sous tracks/"
        );
        assert!(
            library.join("layers").join("apps").join("shuto").join("pack").is_dir(),
            "celle de l'app sous apps/, sans se mêler à la précédente"
        );
    }

    /// A track `<root>/<id>` with one layout, one kn5 and a notice beside it.
    fn make_track(root: &Path, id: &str, layout: &str, kn5: &str) -> PathBuf {
        let dir = root.join(id);
        let ui = dir.join("ui").join(layout);
        std::fs::create_dir_all(&ui).unwrap();
        std::fs::write(
            ui.join("ui_track.json"),
            format!(r#"{{"name":"Grand Valley {layout}"}}"#),
        )
        .unwrap();
        std::fs::write(dir.join(kn5), b"FAKE_KN5_DATA").unwrap();
        std::fs::write(root.join(format!("{id}_readme.pdf")), b"PDF").unwrap();
        root.to_path_buf()
    }

    /// Rule (§4.3bis): "lay it on the existing one", the third answer to the
    /// fuzzy conflict, turns the mod just imported into a layer of the old one
    /// — out of the game, files in the layer, no entry left — and the base of
    /// the host is never touched. Real case: `grandvalleyspeedway2010.rar`,
    /// offered only "keep both" or "replace" against the track it completes.
    #[test]
    fn a_mod_just_imported_becomes_a_layer_of_the_existing_one() {
        let base = crate::testutil::temp_dir("mod-into-layer");
        let ac = base.join("ac");
        let library = base.join("library");
        std::fs::create_dir_all(ac.join("content").join("tracks")).unwrap();
        std::fs::create_dir_all(&library).unwrap();
        let db = overlay::Db(std::sync::Mutex::new(
            overlay::open(&base.join("overlay.sqlite")).unwrap(),
        ));
        let mut cfg = crate::config::AppConfig {
            ac_install_path: Some(ac.clone()),
            library_path: Some(library.clone()),
            ..Default::default()
        };
        cfg.prefs.resource_extraction_mode = "all".into();
        let rules = crate::rules::default_rules();
        let import = |src: PathBuf| {
            let path = src.to_string_lossy().into_owned();
            importer::import_folders(
                &crate::import_progress::ImportCtx::silent(),
                &db,
                &cfg,
                &rules,
                &[path],
                true,
                &[],
            )
            .unwrap()
        };
        import(make_track(&base.join("src1"), "gv_base", "full_layout", "gv_base.kn5"));
        let r = import(make_track(
            &base.join("src2"),
            "gv2010",
            "full_layout_2010",
            "gv_2010.kn5",
        ));
        assert_eq!(
            r[0].mods[0].id_interne, "gv2010",
            "imported as a track of its own first"
        );
        let conn = db.0.lock().unwrap();
        assert!(
            crate::activation::is_mod_active(&cfg, ModKind::Track, "gv2010"),
            "precondition: in the game, as every import"
        );

        importer::resolve_conflict(&conn, &cfg, "gv2010", "gv_base", "layer").unwrap();

        assert!(overlay::get_mod(&conn, "gv2010").unwrap().is_none(), "no entry left");
        assert!(
            !ac.join("content").join("tracks").join("gv2010").exists(),
            "taken out of the game"
        );
        assert!(
            !library.join("tracks").join("gv2010").exists(),
            "its library folder gone"
        );
        let layers = overlay::list_layers(&conn, "gv_base", HostKind::Track).unwrap();
        assert_eq!(layers.len(), 1, "one layer on the host");
        let layer_dir = crate::libpath::resolve(Some(&library), &layers[0].library_path).unwrap();
        assert!(
            layer_dir.join("gv_2010.kn5").is_file(),
            "the files moved into the layer"
        );
        assert!(
            layer_dir
                .join("ui")
                .join("full_layout_2010")
                .join("ui_track.json")
                .is_file(),
            "its layout with them"
        );
        let host_dir = crate::submods::parent_content_dir(&conn, &cfg, "gv_base").unwrap();
        assert!(
            host_dir.join("gv_base.kn5").is_file() && !host_dir.join("gv_2010.kn5").exists(),
            "the base of the host untouched"
        );
        let resources = resources::resources_dir(&library, ModKind::Track, "gv_base");
        assert!(
            resources.join("gv2010_readme.pdf").is_file(),
            "its notice handed to the host"
        );
        assert!(
            !resources::resources_dir(&library, ModKind::Track, "gv2010").exists(),
            "no resources folder left for a mod that is gone"
        );
    }

    /// Rule (§4.3bis): a mod with a past — several versions — is refused
    /// whole rather than converted with part of it lost.
    #[test]
    fn a_mod_with_several_versions_is_not_turned_into_a_layer() {
        let base = crate::testutil::temp_dir("mod-into-layer-refused");
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = crate::config::AppConfig {
            library_path: Some(library.clone()),
            ..Default::default()
        };
        for id in ["host", "guest"] {
            overlay::upsert_mod(&conn, id, "Track", None, Some(id), id, None, "now").unwrap();
        }
        for (v, label) in [("v1", "1.0"), ("v2", "2.0")] {
            let dir = library.join("tracks").join("guest").join(label);
            std::fs::create_dir_all(&dir).unwrap();
            let rel = crate::libpath::to_relative(Some(&library), &dir);
            overlay::insert_version(
                &conn,
                v,
                "guest",
                Some(label),
                None,
                "now",
                &rel,
                None,
                "sig",
                &[],
                &[],
                &[],
                &[],
                None,
            )
            .unwrap();
        }

        let err = mod_into_layer(&conn, &cfg, "guest", "host").unwrap_err();
        assert_eq!(err, crate::errors::NOT_LAYERABLE, "refused, with a reason");
        assert!(overlay::get_mod(&conn, "guest").unwrap().is_some(), "nothing removed");
        assert!(
            library.join("tracks").join("guest").join("1.0").is_dir(),
            "no file moved"
        );
    }
}
