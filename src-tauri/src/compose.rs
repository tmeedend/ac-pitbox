//! Moteur de composition base + couches (§4.4). Quand une base (mod géré OU
//! contenu de base Kunos) a ≥1 couche active, ce que le jeu voit dans
//! `content/<type>s/<id>` est un **résultat composé** : une copie de la base
//! sur laquelle on superpose les couches actives dans l'ordre de priorité, le
//! tout projeté par junction. Désactiver/réordonner/retirer une couche
//! **recompose** depuis les entités intactes — jamais de « défaisage »
//! chirurgical, donc aucun état corrompu possible.
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
use crate::modscan::ModKind;
use crate::overlay::{self, LayerRow};

fn kind_of(s: &str) -> ModKind {
    if s == "Track" { ModKind::Track } else { ModKind::Car }
}

fn composed_dir(library: &Path, kind: ModKind, id: &str) -> PathBuf {
    library.join("composed").join(kind.content_folder()).join(id)
}

fn stock_base_dir(library: &Path, kind: ModKind, id: &str) -> PathBuf {
    library.join("stock_base").join(kind.content_folder()).join(id)
}

/// Reconstruit `composed` = copie de `base_dir` + superposition des couches
/// actives (la plus prioritaire en dernier écrase).
fn build_composed(base_dir: &Path, layers: &[LayerRow], composed: &Path) -> Result<(), String> {
    let _ = std::fs::remove_dir_all(composed);
    archive::copy_dir(base_dir, composed).map_err(|e| format!("composition (base) : {e}"))?;
    for l in layers {
        archive::copy_dir(Path::new(&l.library_path), composed)
            .map_err(|e| format!("composition (couche {}) : {e}", l.name))?;
    }
    Ok(())
}

/// Rend `content/<type>s/<id>` conforme à l'état courant (version active + couches
/// actives). Best-effort : sans dossier AC/bibliothèque configuré, no-op.
pub fn recompose(conn: &Connection, cfg: &AppConfig, mod_id: &str) -> Result<(), String> {
    let m = overlay::get_mod(conn, mod_id).map_err(|e| e.to_string())?.ok_or("mod introuvable")?;
    let kind = kind_of(&m.kind);
    let (Some(library), Some(link)) = (cfg.library_path.as_ref(), activation::content_link(cfg, kind, mod_id)) else {
        return Ok(()); // rien à projeter tant que les chemins ne sont pas configurés
    };
    let layers = overlay::active_layers(conn, mod_id).map_err(|e| e.to_string())?;
    let composed = composed_dir(library, kind, mod_id);

    if m.is_stock {
        recompose_stock(&link, library, kind, mod_id, &layers, &composed)
    } else {
        recompose_managed(conn, cfg, &m, kind, mod_id, &link, &layers, &composed)
    }
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
    composed: &Path,
) -> Result<(), String> {
    let stock_base = stock_base_dir(library, kind, id);

    if layers.is_empty() {
        // Plus de couche : restaurer le vrai dossier Kunos d'origine.
        if stock_base.is_dir() {
            if is_junction(link) {
                activation::remove_junction(link)?;
            }
            archive::move_dir(&stock_base, link).map_err(|e| format!("restauration du contenu de base : {e}"))?;
        }
        let _ = std::fs::remove_dir_all(composed);
        return Ok(());
    }

    // Sauvegarde de la base Kunos (une seule fois), AVANT toute suppression.
    if !stock_base.is_dir() {
        if is_junction(link) {
            // Anormal : une junction alors qu'aucune sauvegarde n'existe. On ne
            // peut pas déduire la base → on s'abstient plutôt que de risquer.
            return Err("contenu de base incohérent (junction sans sauvegarde)".into());
        }
        archive::copy_dir(link, &stock_base).map_err(|e| format!("sauvegarde du contenu de base : {e}"))?;
        // Vérification : la sauvegarde doit être non vide avant de toucher content/.
        let ok = std::fs::read_dir(&stock_base).map(|mut d| d.next().is_some()).unwrap_or(false);
        if !ok {
            let _ = std::fs::remove_dir_all(&stock_base);
            return Err("sauvegarde du contenu de base vide — projection annulée".into());
        }
    }

    build_composed(&stock_base, layers, composed)?;

    // Projeter : remplacer le lien/dossier par une junction vers le composé.
    if is_junction(link) {
        activation::remove_junction(link)?;
    } else if link.exists() {
        // Le vrai dossier Kunos est déjà sauvegardé dans stock_base : on peut le
        // retirer (seule exception au garde-fou, strictement après sauvegarde).
        std::fs::remove_dir_all(link).map_err(|e| format!("retrait du dossier de base : {e}"))?;
    }
    activation::create_junction(link, composed)
}

/// Mod géré : la base est sa version active en bibliothèque (intacte). On ne
/// projette que si le mod est actif (junction présente) — sinon on laisse
/// `content/` tel quel, l'état des couches est mémorisé pour la prochaine
/// activation.
#[allow(clippy::too_many_arguments)]
fn recompose_managed(
    conn: &Connection,
    cfg: &AppConfig,
    m: &overlay::ModRow,
    kind: ModKind,
    mod_id: &str,
    link: &Path,
    layers: &[LayerRow],
    composed: &Path,
) -> Result<(), String> {
    if !activation::is_mod_active(cfg, kind, mod_id) {
        return Ok(()); // mod inactif : rien à projeter
    }
    let vid = m.active_version_id.clone().ok_or("aucune version active")?;
    let base = overlay::get_version_path(conn, &vid).map_err(|e| e.to_string())?.ok_or("version introuvable")?;
    let base = PathBuf::from(base);

    // Garde-fou : un mod géré ne doit jamais recouvrir un vrai dossier de content/.
    if link.exists() && !is_junction(link) {
        return Err("un vrai dossier existe dans content/ — projection refusée (garde-fou)".into());
    }

    if layers.is_empty() {
        // Pas de couche : junction simple vers la version de base.
        if is_junction(link) {
            activation::remove_junction(link)?;
        }
        activation::create_junction(link, &base)?;
        let _ = std::fs::remove_dir_all(composed);
    } else {
        build_composed(&base, layers, composed)?;
        if is_junction(link) {
            activation::remove_junction(link)?;
        }
        activation::create_junction(link, composed)?;
    }
    Ok(())
}

// --- Actions couche (recomposent après coup) --------------------------------

/// Active/désactive une couche puis recompose son parent (§4.4).
pub fn set_layer_active(conn: &Connection, cfg: &AppConfig, layer_id: &str, active: bool) -> Result<(), String> {
    let layer = overlay::get_layer(conn, layer_id).map_err(|e| e.to_string())?.ok_or("couche introuvable")?;
    overlay::set_layer_active(conn, layer_id, active).map_err(|e| e.to_string())?;
    recompose(conn, cfg, &layer.parent_id)
}

/// Réordonne une couche (échange sa priorité avec la voisine) puis recompose.
/// `direction` : "up" = plus prioritaire, "down" = moins prioritaire.
pub fn reorder_layer(conn: &Connection, cfg: &AppConfig, layer_id: &str, direction: &str) -> Result<(), String> {
    let layer = overlay::get_layer(conn, layer_id).map_err(|e| e.to_string())?.ok_or("couche introuvable")?;
    let siblings = overlay::list_layers(conn, &layer.parent_id).map_err(|e| e.to_string())?;
    let pos = siblings.iter().position(|l| l.id == layer_id).ok_or("couche introuvable")?;
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

/// Retire la projection composée d'un mod géré désactivé (le composé devient
/// inutile). Les couches restent enregistrées et réappliquées à la réactivation.
pub fn clear_composed(cfg: &AppConfig, kind: ModKind, id: &str) {
    if let Some(library) = &cfg.library_path {
        let _ = std::fs::remove_dir_all(composed_dir(library, kind, id));
    }
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
        if !cfg!(windows) {
            return;
        }
        let base = std::env::temp_dir().join(format!("pitbox-cmp-mng-{}", Uuid::new_v4()));
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
        assert!(is_junction(&link), "mod actif = junction");
        assert!(link.join("base.txt").is_file());

        // Couche qui ajoute un fichier.
        let layerdir = library.join("layers").join("spa").join("ext");
        write(&layerdir.join("new.txt"), "layer");
        let lid = add_layer(&conn, "spa", ModKind::Track, &layerdir);
        recompose(&conn, &cfg, "spa").unwrap();

        assert!(is_junction(&link));
        assert!(link.join("base.txt").is_file(), "base présente dans le composé");
        assert!(link.join("new.txt").is_file(), "couche appliquée dans le composé");
        assert!(composed_dir(&library, ModKind::Track, "spa").is_dir());

        // Désactiver la couche → retour à la base seule.
        set_layer_active(&conn, &cfg, &lid, false).unwrap();
        assert!(link.join("base.txt").is_file());
        assert!(!link.join("new.txt").exists(), "couche retirée du contenu projeté");
        assert!(!composed_dir(&library, ModKind::Track, "spa").exists());

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn compose_over_stock_backs_up_and_restores() {
        if !cfg!(windows) {
            return;
        }
        let base = std::env::temp_dir().join(format!("pitbox-cmp-stk-{}", Uuid::new_v4()));
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

        assert!(is_junction(&link), "stock composé = junction vers le composé");
        assert!(
            library.join("stock_base").join("tracks").join("spa").join("kunos.txt").is_file(),
            "base Kunos sauvegardée en bibliothèque"
        );
        assert!(link.join("kunos.txt").is_file(), "base présente dans le composé");
        assert!(link.join("2022.txt").is_file(), "couche 2022 appliquée");

        // Désactiver la couche → l'original Kunos est restauré tel quel.
        set_layer_active(&conn, &cfg, &lid, false).unwrap();
        assert!(!is_junction(&link), "retour à un vrai dossier");
        assert!(link.join("kunos.txt").is_file(), "contenu de base restauré");
        assert!(!link.join("2022.txt").exists(), "couche retirée");
        assert!(!composed_dir(&library, ModKind::Track, "spa").exists());

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn priority_order_last_wins() {
        if !cfg!(windows) {
            return;
        }
        let base = std::env::temp_dir().join(format!("pitbox-cmp-prio-{}", Uuid::new_v4()));
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

        let _ = std::fs::remove_dir_all(&base);
    }
}
