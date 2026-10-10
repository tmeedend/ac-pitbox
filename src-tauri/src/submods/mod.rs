//! Attached sub-elements (§8.3): liveries and sounds. Routed at import into a
//! **separate** storage in the library (`<lib>/skins/<parent>/<skin>`,
//! `<lib>/track_skins/…` and `<lib>/sounds/<parent>/<name>`), tracked in the
//! overlay's `sub_mods`, never mixed into the main library (§8.3).
//!
//! The three kinds behave differently, hence one module each:
//! - [`skins`] — car and track liveries: stored apart, then **projected** by a
//!   junction into the host's `skins/` so the game can load them. A car
//!   livery needs nothing more: every livery present is available, the game
//!   picks one through `SkinId` at launch.
//! - [`track_skins`] — what only track liveries have: **several active at
//!   once**, composed into `skins/default/` the way Content Manager does it.
//! - [`sounds`] — exclusive: one active per car, swapped into its `sfx/`.
//!
//! What stays here is what the three share: the import entry points, the gate
//! asking before storing an element whose host is missing (§4.3bis), where a
//! host's folders are, and removal.

mod skins;
mod sounds;
mod track_skins;

pub(crate) use skins::project_attached;
pub use skins::{repair_projections, skin_detail, skin_folder, RepairReport, SkinDetail};
pub(crate) use sounds::sound_backup_dir;
pub use sounds::{activate_sound, restore_sound};
pub use track_skins::{
    list_active_track_skins, list_track_skin_options, set_track_skin_active, sync_bundled_track_skins, TrackSkinOption,
};

use std::path::{Path, PathBuf};

use chrono::Local;
use rusqlite::Connection;
use serde::Serialize;
use uuid::Uuid;

use crate::config::AppConfig;
use crate::modscan::{FoundSub, SubKind};
use crate::resources::ExtractionMode;
use crate::{activation, overlay};

/// The host a found sub-element will be filed under — the import's own answer,
/// for a screen that announces it beforehand (bulk analysis, §4.2). A sound's
/// `parent_id` is often only `sfx`: the car is read from the path, the bank
/// name or `source_name` (the archive or folder imported).
pub fn target_of(conn: &Connection, sub: &FoundSub, source_name: &str) -> String {
    match sub.kind {
        SubKind::Sound => sounds::resolve_sound_parent(conn, sub, source_name),
        SubKind::Skin => sub.parent_id.clone(),
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SubImported {
    /// "SKIN" | "SOUND"
    pub sub_type: String,
    pub parent_id: String,
    pub name: String,
    /// Skin projeté (visible par AC) ; faux si le parent est inconnu/conflit.
    pub projected: bool,
    pub warning: Option<String>,
    /// La voiture/le circuit visé est-il **dans la bibliothèque** ?
    ///
    /// Faux = le sous-élément est rangé sous l'id qu'il vise, mais rien n'est
    /// posé dans le jeu et rien ne le sera tant que l'hôte n'est pas là
    /// (§4.3bis). C'est exactement ce que faisait déjà l'app — en silence :
    /// `warning` porte bien le fait, mais en texte libre français, donc
    /// intraduisible et de fait jamais affiché. Ce booléen est ce que le
    /// rapport peut montrer dans les six locales.
    pub parent_known: bool,
    /// **Rien n'a été rangé** : l'hôte manque et l'utilisateur n'a pas encore
    /// dit s'il fallait garder (§4.3bis). Une livrée est du contenu posé
    /// *dans* une voiture — la même question qu'une couche sans sa base mérite
    /// la même réponse.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub awaiting_decision: bool,
    /// Fichiers annexes redirigés vers le dossier ressources (§4.5.2).
    pub resources_extracted: usize,
}

/// Ce que l'import a le droit de ranger quand l'hôte d'un sous-élément manque
/// (§4.3bis).
///
/// La clé d'un sous-élément est `<parent_id>/<name>` — stable d'une extraction
/// à l'autre, contrairement au dossier temporaire d'où il sort, et c'est ce qui
/// permet à une reprise après arbitrage de retrouver *le* sous-élément tranché.
/// Elle contient un `/`, qu'un id de mod ne porte jamais : les deux espaces de
/// clés cohabitent donc sans ambiguïté dans la même liste de décisions.
#[derive(Debug, Clone, Copy)]
pub struct SubGate<'a> {
    /// Import unitaire : on s'arrête et on demande. Import de masse : jamais
    /// (§4.2bis), le défaut sûr étant de garder.
    pub ask: bool,
    /// Clés déjà tranchées « garder » par l'utilisateur.
    pub approved: &'a [String],
}

impl SubGate<'_> {
    /// Aucun blocage — le comportement des chemins qui ne posent pas de
    /// question : archive imbriquée, import de masse, tests.
    pub fn open() -> Self {
        SubGate {
            ask: false,
            approved: &[],
        }
    }
}

/// Faut-il s'arrêter et demander avant de ranger ce sous-élément ?
fn gated(conn: &Connection, gate: &SubGate, parent_id: &str, name: &str) -> bool {
    if !gate.ask || host_exists(conn, parent_id) {
        return false;
    }
    let key = format!("{parent_id}/{name}");
    !gate.approved.iter().any(|k| k == &key)
}

/// Signalement d'avancement des sous-éléments : `(rangés, total, nom)`.
/// Le total est estimé avant la boucle (voir `count_sub_items`).
pub type SubReport<'a> = dyn Fn(usize, usize, &str) + 'a;

/// Compteur passé aux importateurs de packs, pour qu'ils n'aient pas à
/// connaître la façon dont l'avancement est présenté à l'écran.
struct SubProgress<'a> {
    done: usize,
    total: usize,
    report: &'a SubReport<'a>,
}

impl SubProgress<'_> {
    fn step(&mut self, name: &str) {
        self.done += 1;
        (self.report)(self.done, self.total, name);
    }
}

/// Nombre de sous-éléments qu'un lot va traiter, pour donner un dénominateur à
/// la progression. Approximatif par construction — un skin déjà connu est
/// ignoré plus loin sans être décompté, donc un ré-import finit plus tôt que
/// prévu. Sans conséquence : ce ré-import ne coûte rien de toute façon.
fn count_sub_items(subs: &[FoundSub]) -> usize {
    subs.iter()
        .map(|s| match s.kind {
            SubKind::Skin => std::fs::read_dir(&s.dir)
                .map(|entries| entries.flatten().filter(|e| e.path().is_dir()).count())
                .unwrap_or(1),
            SubKind::Sound => 1,
        })
        .sum()
}

/// Importe les sous-éléments détectés (§8.3). `copy` préserve la source.
#[allow(clippy::too_many_arguments)]
pub fn import_subs(
    conn: &Connection,
    cfg: &AppConfig,
    library: &Path,
    source_name: &str,
    subs: &[FoundSub],
    copy: bool,
    mode: ExtractionMode,
) -> Vec<SubImported> {
    import_subs_reported(
        conn,
        cfg,
        library,
        source_name,
        subs,
        copy,
        mode,
        &SubGate::open(),
        &|_, _, _| {},
    )
}

/// Comme [`import_subs`], en signalant chaque sous-élément rangé (§4.2bis).
///
/// Un pack de deux cents livrées tient dans une seule entrée `FoundSub` : sans
/// ce signalement, la barre de l'item restait immobile pendant tout son
/// rangement, à l'endroit précis où l'utilisateur se demande si l'app est
/// bloquée.
#[allow(clippy::too_many_arguments)]
pub fn import_subs_reported(
    conn: &Connection,
    cfg: &AppConfig,
    library: &Path,
    source_name: &str,
    subs: &[FoundSub],
    copy: bool,
    mode: ExtractionMode,
    gate: &SubGate,
    report: &SubReport,
) -> Vec<SubImported> {
    let mut out = Vec::new();
    let mut progress = SubProgress {
        done: 0,
        total: count_sub_items(subs),
        report,
    };
    for sub in subs {
        match sub.kind {
            SubKind::Skin => skins::import_skin_pack(
                conn,
                cfg,
                library,
                source_name,
                sub,
                copy,
                mode,
                gate,
                &mut out,
                &mut progress,
            ),
            SubKind::Sound => sounds::import_sound(
                conn,
                library,
                source_name,
                sub,
                copy,
                mode,
                gate,
                &mut out,
                &mut progress,
            ),
        }
    }
    out
}

/// Where an incoming skin or sound goes. `None` when it is already there,
/// complete: nothing to import. Its own folder when it is in the showcase
/// (ESPACE§7.5) — emptied of its manifest, it comes back into its own row,
/// with its name and notes, and the id to mark complete comes with it. A new
/// folder otherwise.
fn destination(
    conn: &Connection,
    library: &Path,
    sub_type: &str,
    parent: &str,
    name: &str,
    new_dest: PathBuf,
) -> Option<(PathBuf, Option<String>)> {
    match overlay::find_sub(conn, sub_type, parent, name) {
        Ok(Some(freed)) if freed.is_skeleton() => {
            // A stored path is a library path: anything else is not ours to empty.
            let dir = crate::libpath::resolve(Some(library), &freed.library_path)
                .filter(|d| d.starts_with(library))
                .unwrap_or(new_dest);
            if dir.exists() {
                if let Err(e) = std::fs::remove_dir_all(&dir) {
                    log::warn!("sub {} not emptied of its manifest: {e}", dir.display());
                    return None;
                }
            }
            Some((dir, Some(freed.id)))
        }
        Ok(Some(_)) => None,
        Ok(None) => Some((new_dest, None)),
        Err(e) => {
            log::warn!("find_sub {sub_type} {parent}/{name}: {e}");
            Some((new_dest, None))
        }
    }
}

/// Records an imported skin or sound: a new row, or the row it came back into.
/// `library_path` is where it is stored, relative to the library.
fn record_sub(
    conn: &Connection,
    freed: Option<&str>,
    sub_type: &str,
    parent: &str,
    name: &str,
    library_path: &str,
    source_name: &str,
) {
    let res = match freed {
        Some(id) => overlay::mark_sub_full(conn, id, library_path),
        None => overlay::insert_sub_mod(
            conn,
            &Uuid::new_v4().to_string(),
            sub_type,
            parent,
            name,
            library_path,
            Some(source_name),
            &Local::now().to_rfc3339(),
        ),
    };
    if let Err(e) = res {
        log::warn!("record {sub_type} {parent}/{name}: {e}");
    }
}

/// Pushes into the game what was just written into the **library** copy of the
/// host: a projected livery, a recomposed `skins/default/`.
///
/// Only a managed mod needs it, and only because of where its liveries live.
/// `parent_content_dir` points at the library version folder for a managed mod,
/// while `content/<type>s/<id>` is a hardlink mirror of that folder taken at the
/// last deployment (§2) — nothing propagates between the two on its own. Stock
/// content has no such gap (its `parent_content_dir` *is* the deployed folder),
/// and recomposing it would rebuild the composed tree from `stock_base/`,
/// dropping the very junction we just created.
///
/// Real bug this closes: a 20-livery skin pack imported onto a managed car was
/// stored, projected, listed in the fiche and shown in the 3D preview — all of
/// which read the library — and was nowhere to be found in the game. Nothing in
/// the import report said so, because from the library's point of view nothing
/// had failed.
///
/// Best-effort by design (§4.5): what is stored but not deployed is picked up
/// again by the general repair (§10), and a redeployment that fails must not
/// lose the rest of a pack. Hence the `log::warn!` — on a packaged build there
/// is no console to catch it otherwise.
fn redeploy_host(conn: &Connection, cfg: &AppConfig, parent_id: &str) {
    let managed = matches!(overlay::get_mod(conn, parent_id), Ok(Some(m)) if !m.is_stock);
    if !managed {
        return;
    }
    if let Err(e) = crate::compose::recompose(conn, cfg, parent_id) {
        log::warn!("redeploy_host {parent_id}: {e}");
    }
}

fn parent_skins_dir(conn: &Connection, cfg: &AppConfig, parent_id: &str) -> Option<PathBuf> {
    parent_subdir(conn, cfg, parent_id, "skins")
}

/// Vue transversale (§8.3) : tous les sous-éléments d'un type, avec leur
/// taille sur disque renseignée. Le poids sert à repérer d'un coup d'œil quel
/// pack occupe le plus de place, donc il doit refléter le disque au moment de
/// l'affichage — d'où le parcours récursif ici plutôt qu'une colonne en base.
/// Cantonné à cette vue : les listes chaudes (`list_subs_for_parent`, activation
/// des skins de circuit) restent sans accès disque.
pub fn list_by_type_sized(
    conn: &Connection,
    cfg: &AppConfig,
    sub_type: &str,
) -> rusqlite::Result<Vec<overlay::SubModRow>> {
    let mut rows = overlay::list_subs_by_type(conn, sub_type)?;
    for r in &mut rows {
        r.size_bytes = crate::libpath::resolve(cfg.library_path.as_deref(), &r.library_path)
            .map(|dir| crate::inspect::dir_size_bytes(&dir) as i64);
    }
    Ok(rows)
}

/// Dossier `<sub>/` de l'entité cible (voiture ou circuit) : version active en
/// bibliothèque si c'est un mod géré, sinon `content/<type>s/<id>/<sub>` (base
/// Kunos). Le type est déduit de l'overlay (Car → cars, Track → tracks).
/// `pub(crate)` pour `enginesound`, qui doit trouver le `sfx/` d'une voiture
/// sans supposer où vit sa version active.
pub(crate) fn parent_subdir(conn: &Connection, cfg: &AppConfig, parent_id: &str, sub: &str) -> Option<PathBuf> {
    Some(parent_content_dir(conn, cfg, parent_id)?.join(sub))
}

/// Dossier de contenu de l'entite cible : version active en bibliotheque si
/// c'est un mod gere, sinon `content/<type>s/<id>` (base Kunos). Le type est
/// deduit de l'overlay (Car -> cars, Track -> tracks).
///
/// `pub(crate)` parce que les dossiers proposes (§4.6ter) en ont besoin pour la
/// meme raison que les skins : comparer ce qu'un dossier livre a ce que le mod
/// contient deja, sans supposer ou vit sa version active.
pub(crate) fn parent_content_dir(conn: &Connection, cfg: &AppConfig, parent_id: &str) -> Option<PathBuf> {
    let m = overlay::get_mod(conn, parent_id).ok().flatten();
    if let Some(m) = &m {
        if !m.is_stock {
            if let Some(vid) = &m.active_version_id {
                if let Ok(Some(p)) = overlay::get_version_path(conn, vid) {
                    if let Some(resolved) = crate::libpath::resolve(cfg.library_path.as_deref(), &p) {
                        return Some(resolved);
                    }
                }
            }
        }
    }
    let folder = if m.as_ref().map(|m| m.kind.as_str()) == Some("Track") {
        "tracks"
    } else {
        "cars"
    };
    cfg.ac_install_path
        .as_ref()
        .map(|ac| ac.join("content").join(folder).join(parent_id))
}

/// L'hôte visé par un sous-élément est-il dans la bibliothèque ?
///
/// Sans lui, rien n'est posé dans le jeu — le sous-élément est simplement rangé
/// sous l'id qu'il vise, en attendant (§4.3bis). Ce n'est pas une erreur, mais
/// ça ne doit plus être silencieux : c'est le seul cas où l'utilisateur croit
/// avoir installé quelque chose qui n'apparaîtra nulle part.
fn host_exists(conn: &Connection, id: &str) -> bool {
    overlay::get_mod(conn, id).ok().flatten().is_some()
}

/// Supprime proprement un sous-élément (§8.3) : retire la junction de
/// projection (skin) ou restaure le son d'origine (son actif), efface les
/// fichiers stockés, puis la ligne overlay. Garde-fou junction respecté.
pub fn remove_sub(conn: &Connection, cfg: &AppConfig, sub_id: &str) -> Result<(), String> {
    let sub = overlay::get_sub_mod(conn, sub_id)
        .map_err(|e| e.to_string())?
        .ok_or(crate::errors::SUB_MOD_NOT_FOUND)?;
    if !sub.removable {
        return Err(crate::errors::BUNDLED_NOT_REMOVABLE.into());
    }
    match sub.sub_type.as_str() {
        "SKIN" => {
            // Retire la junction de projection dans le skins/ de l'entité cible.
            if let Some(skins_dir) = parent_subdir(conn, cfg, &sub.parent_id, "skins") {
                let link = skins_dir.join(&sub.name);
                if activation::is_junction(&link) {
                    let _ = activation::remove_junction(&link);
                }
            }
            // Retirée de la bibliothèque, la livrée est encore déployée dans le
            // jeu : même écart qu'à l'import, dans l'autre sens.
            redeploy_host(conn, cfg, &sub.parent_id);
        }
        "TRACK_SKIN" => {
            // Même chose, sous skins/cm_skins/ (convention CM, §8) — et
            // retiré du marqueur d'activation s'il y était.
            if let Some(skins_dir) = parent_subdir(conn, cfg, &sub.parent_id, "skins") {
                let link = skins_dir.join("cm_skins").join(&sub.name);
                if activation::is_junction(&link) {
                    let _ = activation::remove_junction(&link);
                }
            }
            let _ = set_track_skin_active(conn, cfg, &sub.parent_id, &sub.name, false);
        }
        "SOUND"
            // Si actif, on rétablit d'abord le son d'origine.
            if sub.is_active => {
                restore_sound(conn, cfg, &sub.parent_id)?;
            }
        _ => {}
    }
    // Fichiers stockés à part.
    if let Some(dir) = crate::libpath::resolve(cfg.library_path.as_deref(), &sub.library_path) {
        let _ = std::fs::remove_dir_all(dir);
    }
    overlay::delete_sub_mod(conn, sub_id).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transversal_list_carries_disk_size() {
        // La vue transversale pèse chaque skin pour cumuler le poids d'un pack :
        // la taille vient du disque, pas de la base (qui n'en garde aucune trace).
        let base = crate::testutil::temp_dir("subsize");
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let now = Local::now().to_rfc3339();

        let skin = base.join("library").join("skins").join("car").join("Rosso");
        std::fs::create_dir_all(skin.join("nested")).unwrap();
        std::fs::write(skin.join("preview.jpg"), vec![0u8; 700]).unwrap();
        std::fs::write(skin.join("nested").join("body.dds"), vec![0u8; 324]).unwrap();

        overlay::insert_sub_mod(&conn, "s1", "SKIN", "car", "Rosso", &skin.to_string_lossy(), None, &now).unwrap();

        // Sans taille par défaut (pas de parcours disque sur les listes chaudes).
        assert_eq!(overlay::list_subs_by_type(&conn, "SKIN").unwrap()[0].size_bytes, None);

        let sized = list_by_type_sized(&conn, &AppConfig::default(), "SKIN").unwrap();
        assert_eq!(sized[0].size_bytes, Some(1024), "somme récursive des fichiers réels");
    }
}
