//! Moteur de composition base + couches (§4.4). Quand une base (mod géré OU
//! contenu de base Kunos) a ≥1 couche active, ce que le jeu voit dans
//! `content/<type>s/<id>` est un **résultat composé** : la base et les
//! couches actives superposées dans l'ordre de priorité, **directement dans
//! `content/<type>s/<id>` par hardlinks** (§4.3, `deploy::compose_tree`) —
//! pas de dossier de composition intermédiaire, `content/<id>` EST le
//! résultat composé. Désactiver/réordonner/retirer une couche **recompose**
//! depuis les entités intactes — jamais de « défaisage » chirurgical, donc
//! aucun état corrompu possible.
//!
//! Les entités restent intactes : la base d'un mod géré est sa version en
//! bibliothèque ; la base Kunos (qui vit dans `content/`) est **sauvegardée**
//! en bibliothèque (`stock_base/`) avant toute projection, et restaurée à
//! l'identique quand la dernière couche est retirée.

use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::activation::{self, is_junction};
use crate::archive;
use crate::config::AppConfig;
use crate::deploy;
use crate::modscan::ModKind;
use crate::overlay::{self, LayerRow};

fn kind_of(s: &str) -> ModKind {
    if s == "Track" { ModKind::Track } else { ModKind::Car }
}

/// Dossier de sauvegarde du contenu de base Kunos original (§4.4), avant toute
/// projection de couche. Aussi utilisé par `stock.rs` : un contenu de base
/// actuellement composé doit être (ré)indexé depuis cette sauvegarde intacte,
/// jamais depuis le composé (qui contient la/les couche(s)).
pub(crate) fn stock_base_dir(library: &Path, kind: ModKind, id: &str) -> PathBuf {
    library.join("stock_base").join(kind.content_folder()).join(id)
}

/// Retire, quelle que soit sa forme, ce qui occupe déjà `link` — symlink
/// hérité ou déploiement hardlinks marqué. `Ok(())` si rien n'y était.
fn clear_link(link: &Path) -> Result<(), String> {
    if is_junction(link) {
        activation::remove_junction(link)
    } else if deploy::is_deployed(link) {
        deploy::remove_deployment(link)
    } else {
        Ok(())
    }
}

/// Rend `content/<type>s/<id>` conforme à l'état courant (version active + couches
/// actives). Best-effort : sans dossier AC/bibliothèque configuré, no-op.
pub fn recompose(conn: &Connection, cfg: &AppConfig, mod_id: &str) -> Result<(), String> {
    let m = overlay::get_mod(conn, mod_id).map_err(|e| e.to_string())?.ok_or(crate::errors::MOD_NOT_FOUND)?;
    let kind = kind_of(&m.kind);
    let (Some(library), Some(link)) = (cfg.library_path.as_ref(), activation::content_link(cfg, kind, mod_id)) else {
        return Ok(()); // rien à projeter tant que les chemins ne sont pas configurés
    };
    // Nettoyage best-effort d'un ancien dossier de composition intermédiaire
    // (`<lib>/composed/<type>s/<id>`) : reliquat de l'ancien mécanisme par
    // junction, plus jamais écrit ni lu depuis la bascule hardlinks (§4.3) —
    // `content/<id>` EST directement le composé. Peut traîner sur une
    // bibliothèque utilisée avant cette bascule ; sans ce nettoyage,
    // `library::entity_dir` d'une vieille version continuerait par erreur à
    // le préférer au contenu réellement déployé (bug réel).
    let _ = std::fs::remove_dir_all(library.join("composed").join(kind.content_folder()).join(mod_id));

    let layers = overlay::active_layers(conn, mod_id).map_err(|e| e.to_string())?;

    let result = if m.is_stock {
        recompose_stock(&link, library, kind, mod_id, &layers)
    } else {
        recompose_managed(conn, cfg, &m, kind, mod_id, &link, &layers)
    };
    result?;

    // Les champs mis en cache en overlay (layouts, skins, CSP, tags fichier…)
    // ne reflètent que l'état au dernier import/indexage — une couche
    // ajoutée/retirée change ce que contient réellement `link` sans jamais les
    // rafraîchir autrement. Sans ça, un layout apporté par une couche continue
    // d'apparaître dans l'app après désactivation de la couche, alors qu'il a
    // bien disparu du disque (§4.4).
    crate::maintenance::reindex_mod(conn, cfg, mod_id, false)
}

/// Contenu de base Kunos : la base vit dans `content/` (ou déjà sauvegardée en
/// `stock_base/`). Toujours « présent », donc toujours projeté quand il y a des
/// couches ; restauré à l'original quand il n'y en a plus.
fn recompose_stock(
    link: &Path,
    library: &Path,
    kind: ModKind,
    id: &str,
    layers: &[LayerRow],
) -> Result<(), String> {
    let stock_base = stock_base_dir(library, kind, id);

    if layers.is_empty() {
        // Plus de couche : restaurer le vrai dossier Kunos d'origine.
        if stock_base.is_dir() {
            clear_link(link)?;
            archive::move_dir(&stock_base, link).map_err(|e| format!("restauration du contenu de base : {e}"))?;
        }
        return Ok(());
    }

    // Sauvegarde de la base Kunos (une seule fois), AVANT toute suppression.
    if !stock_base.is_dir() {
        if is_junction(link) || deploy::is_deployed(link) {
            // Anormal : déjà projeté alors qu'aucune sauvegarde n'existe. On ne
            // peut pas déduire la base → on s'abstient plutôt que de risquer.
            return Err(crate::errors::INCONSISTENT_STOCK.into());
        }
        archive::copy_dir(link, &stock_base).map_err(|e| format!("sauvegarde du contenu de base : {e}"))?;
        // Vérification : la sauvegarde doit être non vide avant de toucher content/.
        let ok = std::fs::read_dir(&stock_base).map(|mut d| d.next().is_some()).unwrap_or(false);
        if !ok {
            let _ = std::fs::remove_dir_all(&stock_base);
            return Err(crate::errors::EMPTY_STOCK_BACKUP.into());
        }
    }

    // Remplacer ce qui occupe `link` (junction / hardlinks précédents / vrai
    // dossier Kunos déjà sauvegardé — seule exception au garde-fou, strictement
    // après sauvegarde) puis reconstruire directement le composé dans `link`.
    if is_junction(link) || deploy::is_deployed(link) {
        clear_link(link)?;
    } else if link.exists() {
        std::fs::remove_dir_all(link).map_err(|e| format!("retrait du dossier de base : {e}"))?;
    }
    let layer_paths: Vec<PathBuf> = layers.iter().map(|l| PathBuf::from(&l.library_path)).collect();
    deploy::compose_tree(&stock_base, &layer_paths, link, id, kind)
}

/// Mod géré : la base est sa version active en bibliothèque (intacte). On ne
/// projette que si le mod est actif — sinon on laisse `content/` tel quel,
/// l'état des couches est mémorisé pour la prochaine activation.
fn recompose_managed(
    conn: &Connection,
    cfg: &AppConfig,
    m: &overlay::ModRow,
    kind: ModKind,
    mod_id: &str,
    link: &Path,
    layers: &[LayerRow],
) -> Result<(), String> {
    if !activation::is_mod_active(cfg, kind, mod_id) {
        return Ok(()); // mod inactif : rien à projeter
    }
    let vid = m.active_version_id.clone().ok_or(crate::errors::NO_ACTIVE_VERSION)?;
    let base = overlay::get_version_path(conn, &vid).map_err(|e| e.to_string())?.ok_or(crate::errors::VERSION_NOT_FOUND)?;
    let base = PathBuf::from(base);

    // Garde-fou : un mod géré ne doit jamais recouvrir un vrai dossier de content/.
    if link.exists() && !is_junction(link) && !deploy::is_deployed(link) {
        return Err(crate::errors::REAL_FOLDER_IN_CONTENT.into());
    }
    clear_link(link)?;

    if layers.is_empty() {
        deploy::deploy_tree(&base, link, mod_id, kind)
    } else {
        let layer_paths: Vec<PathBuf> = layers.iter().map(|l| PathBuf::from(&l.library_path)).collect();
        deploy::compose_tree(&base, &layer_paths, link, mod_id, kind)
    }
}

// --- Actions couche (recomposent après coup) --------------------------------

/// Active/désactive une couche puis recompose son parent (§4.4).
pub fn set_layer_active(conn: &Connection, cfg: &AppConfig, layer_id: &str, active: bool) -> Result<(), String> {
    let layer = overlay::get_layer(conn, layer_id).map_err(|e| e.to_string())?.ok_or(crate::errors::LAYER_NOT_FOUND)?;
    overlay::set_layer_active(conn, layer_id, active).map_err(|e| e.to_string())?;
    recompose(conn, cfg, &layer.parent_id)
}

/// Réordonne une couche (échange sa priorité avec la voisine) puis recompose.
/// `direction` : "up" = plus prioritaire, "down" = moins prioritaire.
pub fn reorder_layer(conn: &Connection, cfg: &AppConfig, layer_id: &str, direction: &str) -> Result<(), String> {
    let layer = overlay::get_layer(conn, layer_id).map_err(|e| e.to_string())?.ok_or(crate::errors::LAYER_NOT_FOUND)?;
    let siblings = overlay::list_layers(conn, &layer.parent_id).map_err(|e| e.to_string())?;
    let pos = siblings.iter().position(|l| l.id == layer_id).ok_or(crate::errors::LAYER_NOT_FOUND)?;
    let other = match direction {
        "up" => (pos + 1 < siblings.len()).then(|| &siblings[pos + 1]),
        "down" => (pos > 0).then(|| &siblings[pos - 1]),
        _ => return Err(format!("direction inconnue : {direction}")),
    };
    if let Some(other) = other {
        overlay::set_layer_priority(conn, &layer.id, other.priority).map_err(|e| e.to_string())?;
        overlay::set_layer_priority(conn, &other.id, layer.priority).map_err(|e| e.to_string())?;
        recompose(conn, cfg, &layer.parent_id)?;
    }
    Ok(())
}

/// Supprime une couche (fichiers bibliothèque + overlay) puis recompose le parent.
pub fn remove_layer(conn: &Connection, cfg: &AppConfig, layer_id: &str) -> Result<(), String> {
    let layer = overlay::get_layer(conn, layer_id).map_err(|e| e.to_string())?;
    if let Some(layer) = &layer {
        let _ = std::fs::remove_dir_all(&layer.library_path);
    }
    overlay::delete_layer(conn, layer_id).map_err(|e| e.to_string())?;
    if let Some(layer) = layer {
        recompose(conn, cfg, &layer.parent_id)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn write(path: &Path, content: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    fn read(path: &Path) -> String {
        std::fs::read_to_string(path).unwrap()
    }

    fn add_layer(conn: &Connection, parent: &str, kind: ModKind, dir: &Path) -> String {
        let id = Uuid::new_v4().to_string();
        let prio = overlay::next_layer_priority(conn, parent).unwrap();
        overlay::insert_layer(
            conn, &id, parent, &format!("{kind:?}"), "ext", &dir.to_string_lossy(), None, 0, 0, prio, "now",
        )
        .unwrap();
        id
    }

    #[test]
    fn compose_over_managed_mod() {
        let base = crate::testutil::temp_dir("cmp-mng");
        let ac = base.join("ac");
        let library = base.join("library");
        std::fs::create_dir_all(ac.join("content").join("tracks")).unwrap();
        let basever = library.join("tracks").join("spa").join("v1");
        write(&basever.join("base.txt"), "base");
        write(&basever.join("ui").join("ui_track.json"), "{}");

        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        overlay::upsert_mod(&conn, "spa", "Track", Some("B"), Some("Spa"), "h", None, "now").unwrap();
        overlay::insert_version(
            &conn, "v1", "spa", Some("1.0"), None, "now", &basever.to_string_lossy(), None, "sig",
            &[], &[], &[], &[], None,
        )
        .unwrap();
        overlay::set_active_version(&conn, "spa", "v1").unwrap();
        let cfg = AppConfig { ac_install_path: Some(ac.clone()), library_path: Some(library.clone()), ..Default::default() };

        activation::activate(&conn, &cfg, "spa", None).unwrap();
        let link = ac.join("content").join("tracks").join("spa");
        assert!(deploy::is_deployed(&link), "mod actif = déploiement hardlinks");
        assert!(!is_junction(&link), "plus de reparse point");
        assert!(link.join("base.txt").is_file());

        // Couche qui ajoute un fichier.
        let layerdir = library.join("layers").join("spa").join("ext");
        write(&layerdir.join("new.txt"), "layer");
        let lid = add_layer(&conn, "spa", ModKind::Track, &layerdir);
        recompose(&conn, &cfg, "spa").unwrap();

        assert!(deploy::is_deployed(&link));
        assert!(link.join("base.txt").is_file(), "base présente dans le composé");
        assert!(link.join("new.txt").is_file(), "couche appliquée dans le composé — directement dans content/, sans dossier intermédiaire");

        // Désactiver la couche → retour à la base seule.
        set_layer_active(&conn, &cfg, &lid, false).unwrap();
        assert!(link.join("base.txt").is_file());
        assert!(!link.join("new.txt").exists(), "couche retirée du contenu projeté");
    }

    #[test]
    fn recompose_refreshes_cached_layouts_after_layer_deactivated() {
        // Bug réel : une couche qui ajoute un layout complet (ex. « Spa 2022 »)
        // sur un circuit de base doit voir ce layout disparaître du champ
        // `layouts` mis en cache dès la désactivation de la couche — sans
        // réindexation manuelle. Avant le fix, `recompose` ne rafraîchissait
        // jamais les champs mis en cache en overlay (§4.4).
        let base = crate::testutil::temp_dir("cmp-layouts");
        let ac = base.join("ac");
        let library = base.join("library");
        let link = ac.join("content").join("tracks").join("spa");
        write(&link.join("ui").join("ui_track.json"), "{}");

        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        overlay::upsert_stock_mod(&conn, "spa", "Track", Some("Kunos"), Some("Spa"), "now").unwrap();
        // Version synthétique telle que produite par un vrai « Indexer le
        // contenu de base » : mono-layout au départ (layout = chaîne vide).
        overlay::insert_version(
            &conn, "v1", "spa", None, Some("Kunos"), "now", &link.to_string_lossy(), None, "",
            &[], &[], &[String::new()], &[], None,
        )
        .unwrap();
        overlay::set_active_version(&conn, "spa", "v1").unwrap();
        let cfg = AppConfig { ac_install_path: Some(ac.clone()), library_path: Some(library.clone()), ..Default::default() };

        // Couche qui ajoute un layout complet « 2022 » (ui/2022/ui_track.json).
        let layerdir = library.join("layers").join("spa").join("ext");
        write(&layerdir.join("ui").join("2022").join("ui_track.json"), "{}");
        let lid = add_layer(&conn, "spa", ModKind::Track, &layerdir);
        recompose(&conn, &cfg, "spa").unwrap();

        let versions = overlay::get_versions(&conn, "spa").unwrap();
        assert_eq!(versions.len(), 1, "reindex_mod met à jour la version existante, n'en crée pas une nouvelle");
        assert!(versions[0].layouts.iter().any(|l| l == "2022"), "layout de la couche pris en compte dans le cache");

        // Désactiver la couche → le layout doit disparaître du cache tout seul.
        set_layer_active(&conn, &cfg, &lid, false).unwrap();
        let versions = overlay::get_versions(&conn, "spa").unwrap();
        assert!(
            !versions[0].layouts.iter().any(|l| l == "2022"),
            "layout de la couche retiré du cache après désactivation, sans réindexation manuelle"
        );
    }

    #[test]
    fn compose_over_stock_backs_up_and_restores() {
        let base = crate::testutil::temp_dir("cmp-stk");
        let ac = base.join("ac");
        let library = base.join("library");
        // Vrai dossier Kunos dans content/.
        let link = ac.join("content").join("tracks").join("spa");
        write(&link.join("kunos.txt"), "kunos");
        write(&link.join("ui").join("ui_track.json"), "{}");

        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        overlay::upsert_stock_mod(&conn, "spa", "Track", Some("Kunos"), Some("Spa"), "now").unwrap();
        let cfg = AppConfig { ac_install_path: Some(ac.clone()), library_path: Some(library.clone()), ..Default::default() };

        // Couche « 2022 ».
        let layerdir = library.join("layers").join("spa").join("ext");
        write(&layerdir.join("2022.txt"), "2022");
        let lid = add_layer(&conn, "spa", ModKind::Track, &layerdir);
        recompose(&conn, &cfg, "spa").unwrap();

        assert!(deploy::is_deployed(&link), "stock composé = déploiement hardlinks directement dans content/");
        assert!(
            library.join("stock_base").join("tracks").join("spa").join("kunos.txt").is_file(),
            "base Kunos sauvegardée en bibliothèque"
        );
        assert!(link.join("kunos.txt").is_file(), "base présente dans le composé");
        assert!(link.join("2022.txt").is_file(), "couche 2022 appliquée");

        // Désactiver la couche → l'original Kunos est restauré tel quel.
        set_layer_active(&conn, &cfg, &lid, false).unwrap();
        assert!(!is_junction(&link) && !deploy::is_deployed(&link), "retour à un vrai dossier");
        assert!(link.join("kunos.txt").is_file(), "contenu de base restauré");
        assert!(!link.join("2022.txt").exists(), "couche retirée");
    }

    #[test]
    fn recompose_cleans_up_stale_pre_hardlink_composed_leftover() {
        // Bug réel : `<lib>/composed/<type>s/<id>` (ancien mécanisme par
        // junction) doit disparaître dès qu'une entité est recomposée sous le
        // nouveau mécanisme — sinon `library::entity_dir` d'une bibliothèque
        // migrée continue de préférer ce reliquat périmé au vrai contenu.
        let base = crate::testutil::temp_dir("cmp-stale");
        let ac = base.join("ac");
        let library = base.join("library");
        let link = ac.join("content").join("tracks").join("spa");
        write(&link.join("ui").join("ui_track.json"), "{}");

        // Reliquat de l'ancien mécanisme, présent avant toute recomposition.
        let stale = library.join("composed").join("tracks").join("spa");
        write(&stale.join("ui").join("2022").join("ui_track.json"), "{}");
        assert!(stale.is_dir());

        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        overlay::upsert_stock_mod(&conn, "spa", "Track", Some("Kunos"), Some("Spa"), "now").unwrap();
        let cfg = AppConfig { ac_install_path: Some(ac), library_path: Some(library), ..Default::default() };

        recompose(&conn, &cfg, "spa").unwrap();

        assert!(!stale.exists(), "le reliquat périmé est nettoyé dès la première recomposition");
    }

    #[test]
    fn priority_order_last_wins() {
        let base = crate::testutil::temp_dir("cmp-prio");
        let ac = base.join("ac");
        let library = base.join("library");
        std::fs::create_dir_all(ac.join("content").join("tracks")).unwrap();
        let basever = library.join("tracks").join("spa").join("v1");
        write(&basever.join("conf.txt"), "base");

        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        overlay::upsert_mod(&conn, "spa", "Track", Some("B"), Some("Spa"), "h", None, "now").unwrap();
        overlay::insert_version(
            &conn, "v1", "spa", None, None, "now", &basever.to_string_lossy(), None, "sig", &[], &[], &[], &[], None,
        )
        .unwrap();
        overlay::set_active_version(&conn, "spa", "v1").unwrap();
        let cfg = AppConfig { ac_install_path: Some(ac.clone()), library_path: Some(library.clone()), ..Default::default() };
        activation::activate(&conn, &cfg, "spa", None).unwrap();
        let link = ac.join("content").join("tracks").join("spa");

        // Deux couches écrasant le même fichier ; B (priorité + haute) gagne.
        let la = library.join("layers").join("spa").join("a");
        let lb = library.join("layers").join("spa").join("b");
        write(&la.join("conf.txt"), "A");
        write(&lb.join("conf.txt"), "B");
        let a_id = add_layer(&conn, "spa", ModKind::Track, &la); // priorité 0
        add_layer(&conn, "spa", ModKind::Track, &lb); // priorité 1 (appliquée en dernier)
        recompose(&conn, &cfg, "spa").unwrap();
        assert_eq!(read(&link.join("conf.txt")), "B", "la couche la plus prioritaire gagne");

        // Remonter A au-dessus de B → A gagne.
        reorder_layer(&conn, &cfg, &a_id, "up").unwrap();
        assert_eq!(read(&link.join("conf.txt")), "A", "après réordonnancement, A gagne");
    }
}
