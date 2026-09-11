//! L'inventaire des compléments (refonte §4).
//!
//! **Tout ce qui n'est pas un contenu autonome**, dans une seule liste : ce qui
//! se greffe, s'ajoute ou se superpose au jeu — livrées, sons, habillages de
//! circuit, couches, et tout ce que Pit Box n'a pas su reconnaître. Les quatre
//! contenus autonomes — voitures, circuits, modèles de pilote, apps — ont leur
//! écran et n'y figurent pas.
//!
//! Trois écrans le précédaient : « Add-ons voiture », « Add-ons circuit » et un
//! fourre-tout. Ils classaient par **mécanique d'installation**, c'est-à-dire
//! par la complexité que l'app existe justement pour absorber. Un même mod
//! pouvait y figurer deux fois sans que rien ne le dise.
//!
//! Cet écran n'organise pas, il **inventorie** : une ligne par chose, et les
//! facettes font le tri. D'où un type de ligne unique pour cinq sources, et
//! c'est là tout le travail de ce module — chacune a son id, son état et son
//! nom ailleurs.

use rusqlite::Connection;
use serde::Serialize;

use crate::attach::{self, AttachKind, Attachment, Nature, Signal};
use crate::config::AppConfig;
use crate::overlay;

/// Ce qu'une ligne est, pour l'icône et pour savoir quelle fiche ouvrir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RowKind {
    Skin,
    Sound,
    TrackSkin,
    Layer,
    /// Notice, manuel, template : une livraison partie tout entière en
    /// ressources (§4.5.2). Sa ligne subsiste pour que le fichier reste
    /// atteignable.
    Document,
    /// Mannequin de pilote. Distingué des autres mods parce que c'est un
    /// **contenu autonome** (§4) : il ne se greffe sur rien, il se choisit.
    /// C'est aussi ce qui le fera sortir de cet inventaire pour rejoindre
    /// l'écran Pilote (L8).
    Driver,
    /// Mod « autre » (§7.3), y compris ce qui n'a pas été reconnu.
    Other,
}

/// Une ligne d'inventaire, quelle que soit sa source.
#[derive(Debug, Clone, Serialize)]
pub struct InventoryRow {
    /// Identifiant **unique dans l'inventaire** : les cinq tables ont chacune
    /// leurs ids, et rien n'empêche une livrée et une couche de porter le même.
    /// Préfixé par le type, donc, sinon deux lignes distinctes se confondent
    /// dans un `{#each}` clé par id.
    pub uid: String,
    pub kind: RowKind,
    /// Id dans sa propre table, pour les commandes qui l'attendent.
    pub id: String,
    /// Nom lisible : la saisie de l'utilisateur si elle existe, sinon le nom
    /// dérivé ou le nom brut.
    pub name: String,
    /// Identifiant technique, montré en dessous du nom. C'est ce qui permet de
    /// renommer sans rien perdre (§4.2).
    pub tech_id: String,
    pub attachment: Attachment,
    /// Déployé ou non — **`None` quand la notion ne s'applique pas**.
    ///
    /// Une livrée ne s'active pas : elle est projetée dans le dossier `skins/`
    /// de sa voiture et le jeu la voit, point. La colonne `is_active` de
    /// `sub_mods` ne sert qu'aux sons (exclusifs) et aux habillages de circuit
    /// — mesuré sur une bibliothèque réelle : les 26 livrées y sont toutes à
    /// 0, quand les sons se répartissent 6/4. Lire cette colonne pour une
    /// livrée affichait donc « Inactif » sur chacune : faux, et pire que faux
    /// puisque ça suggère une action qui n'existe pas.
    pub active: Option<bool>,
    /// Prioritaire en cas de conflit (§7.3) — affiché comme un **état** (une
    /// étoile), jamais comme un bouton.
    pub priority: bool,
    pub has_note: bool,
    pub source_archive: Option<String>,
    pub imported_at: String,
    pub size_bytes: i64,
}

/// L'inventaire complet, à plat.
///
/// **Un mannequin déployé n'y figure pas** : l'inventaire d'un type vit avec
/// son sélecteur quand il en existe un (§5), et celui des mannequins est
/// l'écran Pilote. C'était le doublon que la refonte vient supprimer.
///
/// « Déployé » et pas « importé », et la nuance porte tout : l'écran Pilote lit
/// `content/driver` **du jeu**, donc un mannequin désactivé n'y apparaît pas et
/// doit rester listé ici, sans quoi il n'existerait plus nulle part. Le test de
/// présence du fichier est une question au disque, à trois francs six sous —
/// répondre exactement (« l'écran Pilote le montre-t-il ? ») exigerait de
/// parser chaque KN5, une quinzaine de millisecondes pour quinze mégaoctets.
/// L'écart résiduel — déployé mais sans squelette, donc écarté par l'écran
/// Pilote — est couvert par le décompte que celui-ci affiche
/// (`driver::BodyList::discarded`).
pub fn list(conn: &Connection, cfg: &AppConfig) -> rusqlite::Result<Vec<InventoryRow>> {
    let index = attach::EntityIndex::build(conn)?;
    let mut out: Vec<InventoryRow> = Vec::new();

    let driver_dir = cfg.ac_install_path.as_ref().map(|ac| ac.join("content").join("driver"));

    // --- Mods « autres » : la seule source dont le rattachement se déduit ---
    for card in crate::others::list_others(conn, cfg)? {
        let dir = crate::libpath::resolve(cfg.library_path.as_deref(), &card.row.library_path);
        // Un mannequin ne pose QUE dans `content/driver` : c'est ce qui le
        // distingue d'un pack qui en livrerait un parmi d'autres choses.
        let is_driver = card.categories == ["driver"];
        // Déployé dans le jeu = montré par l'écran Pilote, donc rien à faire
        // ici. Le fichier porte le nom du modèle, que la junction pose tel quel.
        if is_driver && driver_dir.as_ref().is_some_and(|d| deployed_driver(d, &card)) {
            continue;
        }
        // Aucun fichier stocké : tout est parti en ressources (§4.5.2). C'est
        // une notice ou un manuel — le seul cas où un mod « autre » ne pose
        // rien du tout, et il a un nom.
        let is_document = !is_driver && card.file_count == 0;
        let attachment = if is_document {
            crate::attach::Attachment {
                nature: Nature::Document,
                ..card.attachment.clone()
            }
        } else if is_driver {
            // « Autonome », pas « le jeu » : un modèle de pilote ne se greffe
            // sur rien, il se choisit (§2). Et sa nature est **contenu** — il
            // n'habille pas quelque chose d'autre, il EST la chose.
            crate::attach::Attachment {
                kind: crate::attach::AttachKind::Standalone,
                target_id: None,
                target_name: None,
                signal: card.attachment.signal,
                nature: Nature::Content,
            }
        } else {
            card.attachment
        };
        out.push(InventoryRow {
            uid: format!("OTHER:{}", card.row.id),
            kind: if is_driver {
                RowKind::Driver
            } else if is_document {
                RowKind::Document
            } else {
                RowKind::Other
            },
            name: card.row.display_name_user.clone().unwrap_or_else(|| card.row.id.clone()),
            tech_id: card.row.id.clone(),
            attachment,
            active: Some(card.row.is_active),
            priority: card.row.is_priority,
            has_note: card.row.notes_user.is_some(),
            source_archive: card.row.source_archive.clone(),
            imported_at: card.row.imported_at.clone(),
            size_bytes: dir.map(|d| crate::inspect::dir_size_bytes(&d) as i64).unwrap_or(0),
            id: card.row.id,
        });
    }

    // --- Sous-éléments : le rattachement est certain, il est dans la table ---
    for sub in overlay::list_all_subs(conn)? {
        let kind = match sub.sub_type.as_str() {
            "SOUND" => RowKind::Sound,
            "TRACK_SKIN" | "TRACK_MOD" => RowKind::TrackSkin,
            _ => RowKind::Skin,
        };
        let dir = crate::libpath::resolve(cfg.library_path.as_deref(), &sub.library_path);
        // Nom lisible d'une livrée : celui de son `ui_skin.json`, le même que
        // montre le sélecteur de la fiche. Sans lui, la ligne répétait deux
        // fois `chp_unit_118` — en blanc puis en gris.
        let readable = if kind == RowKind::Skin {
            dir.as_deref().and_then(crate::library::read_skin_name)
        } else {
            None
        };
        out.push(InventoryRow {
            uid: format!("SUB:{}", sub.id),
            kind,
            name: sub
                .display_name_user
                .clone()
                .or(readable)
                .unwrap_or_else(|| sub.name.clone()),
            tech_id: sub.name.clone(),
            attachment: host_attachment(&index, &sub.parent_id, nature_of_sub(kind)),
            // Voir `active` : seuls les sons et les habillages de circuit ont
            // un état de déploiement propre.
            active: match kind {
                RowKind::Sound | RowKind::TrackSkin => Some(sub.is_active),
                _ => None,
            },
            priority: false,
            has_note: sub.notes_user.is_some(),
            source_archive: sub.source_archive.clone(),
            imported_at: sub.imported_at.clone(),
            size_bytes: dir.map(|d| crate::inspect::dir_size_bytes(&d) as i64).unwrap_or(0),
            id: sub.id,
        });
    }

    // --- Couches : hôte connu par construction lui aussi ---
    for layer in overlay::list_all_layers(conn)? {
        let dir = crate::libpath::resolve(cfg.library_path.as_deref(), &layer.library_path);
        out.push(InventoryRow {
            uid: format!("LAYER:{}", layer.id),
            kind: RowKind::Layer,
            // Le nom dérivé (retrait de l'extension, du préfixe de l'hôte) est
            // calculé côté front, qui le fait déjà pour la fiche de couche : le
            // dupliquer ici en ferait deux versions à garder d'accord.
            name: layer.display_name_user.clone().unwrap_or_else(|| layer.name.clone()),
            tech_id: layer.source_archive.clone().unwrap_or_else(|| layer.name.clone()),
            attachment: host_attachment(&index, &layer.parent_id, Nature::Appearance),
            active: Some(layer.is_active),
            priority: false,
            has_note: layer.notes_user.is_some(),
            source_archive: layer.source_archive.clone(),
            imported_at: layer.imported_at.clone(),
            size_bytes: dir.map(|d| crate::inspect::dir_size_bytes(&d) as i64).unwrap_or(0),
            id: layer.id,
        });
    }

    Ok(out)
}

/// Le mannequin de ce mod est-il posé dans `content/driver` du jeu ?
///
/// On regarde les jonctions enregistrées à l'activation — ce sont les chemins
/// réellement posés — et on vérifie qu'au moins un existe encore. Un fichier
/// effacé à la main derrière le dos de l'app rend donc la ligne à l'inventaire,
/// ce qui est exactement ce qu'on veut : elle y redevient réparable.
fn deployed_driver(driver_dir: &std::path::Path, card: &crate::others::OtherModCard) -> bool {
    card.row.junctions.iter().any(|j| {
        let path = std::path::Path::new(j);
        path.starts_with(driver_dir) && path.exists()
    })
}

/// Une livrée change ce qu'on voit, un son ce qu'on entend : les deux relèvent
/// de l'apparence au sens de la facette — ce qui se choisit pour lui-même, par
/// opposition à une dépendance qu'on subit.
fn nature_of_sub(_kind: RowKind) -> Nature {
    Nature::Appearance
}

/// Rattachement d'un élément dont l'hôte est écrit dans sa propre table.
///
/// C'est le signal n°2 du §2.1, le seul qui soit **certain** : rien n'est
/// déduit, `parent_id` dit l'hôte. Un hôte inconnu de la bibliothèque (mod
/// supprimé, couche en attente de son contenu) laisse la ligne rattachée « au
/// jeu » plutôt que de la faire disparaître — §14.3 en fera un état d'attente
/// à part entière.
fn host_attachment(index: &attach::EntityIndex, parent_id: &str, nature: Nature) -> Attachment {
    match index.lookup(parent_id) {
        Some((kind, name, key)) => Attachment {
            kind,
            target_id: Some(key),
            target_name: name,
            signal: Signal::Layer,
            nature,
        },
        None => Attachment {
            kind: AttachKind::Game,
            target_id: None,
            target_name: None,
            signal: Signal::None,
            nature,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rule (§4): one row per thing, whatever table it comes from — and the
    /// identifier is unique across the inventory. Nothing forbids a livery and
    /// a layer from carrying the same id in their own tables; keyed by that id
    /// alone, the list would fold two distinct rows into one.
    #[test]
    fn one_row_per_thing_with_an_id_unique_across_sources() {
        let base = crate::testutil::temp_dir("inventory");
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let cfg = AppConfig {
            library_path: Some(library.clone()),
            ..Default::default()
        };
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let now = chrono::Local::now().to_rfc3339();
        overlay::upsert_mod(&conn, "ks_nordschleife", "Track", None, Some("Nordschleife"), "h", None, &now).unwrap();

        // Même id « pack » des deux côtés : c'est tout le sujet.
        overlay::insert_sub_mod(&conn, "pack", "SKIN", "ks_nordschleife", "pack", "subs/pack", None, &now).unwrap();
        overlay::insert_layer(
            &conn,
            "pack",
            "ks_nordschleife",
            "Track",
            "pack.7z",
            "layers/pack",
            Some("pack.7z"),
            1,
            0,
            0,
            &now,
        )
        .unwrap();

        let rows = list(&conn, &cfg).unwrap();
        assert_eq!(rows.len(), 2, "une ligne par chose");
        let skin = rows.iter().find(|r| r.kind == RowKind::Skin).unwrap();
        assert_eq!(skin.active, None, "une livrée ne s'active pas : pas d'état de déploiement");
        let layer = rows.iter().find(|r| r.kind == RowKind::Layer).unwrap();
        assert_eq!(layer.active, Some(true), "une couche, si");
        let uids: Vec<&str> = rows.iter().map(|r| r.uid.as_str()).collect();
        assert!(uids.contains(&"SUB:pack") && uids.contains(&"LAYER:pack"), "ids distincts: {uids:?}");
        for r in &rows {
            assert_eq!(
                r.attachment.target_id.as_deref(),
                Some("ks_nordschleife"),
                "l'hôte est certain, il est dans la table"
            );
            assert_eq!(r.attachment.signal, Signal::Layer, "aucune déduction ici");
        }
        drop(base);
    }

    /// Rule (§5): a mannequin deployed in the game is shown by the Pilote
    /// screen, which is its selector — so the inventory stops listing it. One
    /// that is NOT deployed stays: that screen reads `content/driver` of the
    /// game, so nothing else would show it, and it would exist nowhere.
    #[test]
    fn a_deployed_mannequin_leaves_the_inventory_but_a_dormant_one_stays() {
        let base = crate::testutil::temp_dir("inventory-driver");
        let library = base.join("library");
        let ac = base.join("ac");
        let driver_dir = ac.join("content").join("driver");
        std::fs::create_dir_all(&driver_dir).unwrap();
        std::fs::write(driver_dir.join("ada.kn5"), b"KN5").unwrap();
        let cfg = AppConfig {
            library_path: Some(library.clone()),
            ac_install_path: Some(ac.clone()),
            ..Default::default()
        };
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let now = chrono::Local::now().to_rfc3339();

        // Deux mannequins : l'un posé dans le jeu, l'autre seulement importé.
        for (id, deployed) in [("ada", true), ("dormant", false)] {
            let dir = library.join("others").join(id);
            std::fs::create_dir_all(dir.join("content").join("driver")).unwrap();
            std::fs::write(dir.join("content").join("driver").join(format!("{id}.kn5")), b"KN5").unwrap();
            overlay::insert_other_mod(&conn, id, &format!("others/{id}"), None, &now).unwrap();
            if deployed {
                let posed = driver_dir.join(format!("{id}.kn5"));
                overlay::set_other_active(&conn, id, true, &[posed.to_string_lossy().into_owned()]).unwrap();
            }
        }

        let ids: Vec<String> = list(&conn, &cfg).unwrap().into_iter().map(|r| r.id).collect();
        assert!(!ids.contains(&"ada".to_string()), "posé dans le jeu : l'écran Pilote le montre");
        assert!(ids.contains(&"dormant".to_string()), "pas posé : sinon il n'existerait nulle part");
        drop(base);
    }

    /// Rule (§14.3, partiel): a host the library no longer knows leaves the row
    /// attached to "the game" rather than making it vanish. Nothing is ever
    /// lost from this screen — that is its whole purpose.
    #[test]
    fn an_unknown_host_does_not_swallow_the_row() {
        let base = crate::testutil::temp_dir("inventory-orphan");
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let cfg = AppConfig {
            library_path: Some(library.clone()),
            ..Default::default()
        };
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let now = chrono::Local::now().to_rfc3339();
        overlay::insert_sub_mod(&conn, "S1", "SOUND", "disparue", "son", "subs/s1", None, &now).unwrap();

        let rows = list(&conn, &cfg).unwrap();
        assert_eq!(rows.len(), 1, "toujours listé");
        assert_eq!(rows[0].active, Some(false), "un son a bien un état, lui");
        assert_eq!(rows[0].attachment.kind, AttachKind::Game);
        assert_eq!(rows[0].attachment.target_id, None);
        drop(base);
    }
}
