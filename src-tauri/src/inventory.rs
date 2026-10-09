//! L'inventaire des compléments (REFONTE§4).
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
    /// Zones du jeu que la ligne touche ([`crate::others::CATEGORY_ORDER`]) —
    /// vide pour les sources qui n'en ont pas (livrées, sons, couches).
    ///
    /// Affiché **à la place du type** quand la ligne est un mod « autre » : le
    /// type y vaut « Mod », c'est-à-dire le mot qui reste quand on n'a rien de
    /// plus précis à dire. Une police du jeu se lit « Font », pas « Mod », et
    /// l'information existait déjà — elle ne vivait que sur la fiche.
    ///
    /// Ce n'est pas une quatrième classification : c'est la même donnée que
    /// celle dont `attach::nature_of` tire la nature, remontée d'un cran.
    pub areas: Vec<String>,
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
    /// Without its files (ESPACE§5.4, ESPACE§5.6): it followed its host into the
    /// showcase, or arrived with an imported library export.
    pub showcase: bool,
    pub source_archive: Option<String>,
    pub imported_at: String,
    /// What it weighs — or, without its files, what it weighed: what
    /// recovering it will bring back, as for a mod in the showcase (ESPACE§4.1).
    pub size_bytes: i64,
}

/// L'inventaire complet, à plat.
///
/// **Les mannequins y figurent, comme les livrées.** Le §5 veut que
/// l'inventaire d'un type vive avec son sélecteur, et un premier jet les avait
/// donc retirés dès qu'ils étaient posés dans le jeu. C'était une erreur, pour
/// une raison qu'on ne voit qu'à l'usage : la galerie de l'écran Pilote est un
/// **sélecteur**, elle ne gère rien — ni désactivation, ni suppression, ni
/// ouverture du dossier. Les retirer d'ici revenait donc à retirer le seul
/// endroit d'où on pouvait agir sur eux.
///
/// Les livrées tranchent la question par l'exemple : elles vivent dans le
/// sélecteur de la fiche voiture **et** ici, et personne ne s'en plaint —
/// choisir et gérer sont deux gestes, ils peuvent avoir deux écrans. Le type
/// « Mannequin » et sa facette suffisent à ce que le doublon se lise.
///
/// The base lock is held only while the base is read: the inventory walks the
/// files of every "other" mod and weighs the folder of every row
/// (`dir_size_bytes`), and a command needing the base waited for all of it.
pub fn list_shared(db: &overlay::Db, cfg: &AppConfig) -> Result<Vec<InventoryRow>, String> {
    let base = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        inventory_base(&conn).map_err(|e| e.to_string())?
    };
    Ok(rows_from_disk(cfg, base))
}

/// What the inventory reads from the base: the rows of its sources, and the
/// index of the library their hosts are looked up in.
struct InventoryBase {
    others: Vec<overlay::OtherModRow>,
    subs: Vec<overlay::SubModRow>,
    layers: Vec<overlay::LayerRow>,
    index: attach::EntityIndex,
}

fn inventory_base(conn: &Connection) -> rusqlite::Result<InventoryBase> {
    // One index for every source: the "other" mods' attachment is deduced
    // against the same library the sub-elements and layers name their host in.
    let (others, index) = crate::others::others_base(conn)?;
    Ok(InventoryBase {
        others,
        subs: overlay::list_all_subs(conn)?,
        layers: overlay::list_all_layers(conn)?,
        index,
    })
}

/// The rows, from the files alone - no connection at hand.
fn rows_from_disk(cfg: &AppConfig, base: InventoryBase) -> Vec<InventoryRow> {
    let InventoryBase {
        others,
        subs,
        layers,
        index,
    } = base;
    let mut out: Vec<InventoryRow> = Vec::new();

    // --- Mods « autres » : la seule source dont le rattachement se déduit ---
    for card in crate::others::others_from_disk(cfg, others, &index) {
        let dir = crate::libpath::resolve(cfg.library_path.as_deref(), &card.row.library_path);
        // Un mannequin ne touche QUE `content/driver` — c'est ce qui le
        // distingue d'un pack qui en livrerait un parmi d'autres choses.
        //
        // La catégorie fourre-tout ne compte pas dans ce jugement : elle
        // ramasse ce qui ne va nulle part dans le jeu, c'est-à-dire l'emballage
        // de l'auteur. `DORIKIN_DRIVER_MOD` livre deux mannequins **et** sa
        // notice d'installation en japonais avec deux captures d'écran ; une
        // égalité stricte en faisait un mod quelconque à cause d'elles.
        let is_driver = card.categories.iter().any(|c| c == "driver")
            && card
                .categories
                .iter()
                .all(|c| c == "driver" || c == crate::others::OTHER_CATEGORY);
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
            name: card
                .row
                .display_name_user
                .clone()
                .unwrap_or_else(|| card.row.id.clone()),
            tech_id: card.row.id.clone(),
            areas: card.categories.clone(),
            attachment,
            active: Some(card.row.is_active),
            priority: card.row.is_priority,
            has_note: card.row.notes_user.is_some(),
            showcase: card.row.is_skeleton(),
            source_archive: card.row.source_archive.clone(),
            imported_at: card.row.imported_at.clone(),
            size_bytes: row_size(dir.as_deref(), card.row.is_skeleton()),
            id: card.row.id,
        });
    }

    // --- Sous-éléments : le rattachement est certain, il est dans la table ---
    for sub in subs {
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
            areas: Vec::new(),
            attachment: host_attachment(&index, &sub.parent_id, nature_of_sub(kind)),
            // Voir `active` : seuls les sons et les habillages de circuit ont
            // un état de déploiement propre.
            active: match kind {
                RowKind::Sound | RowKind::TrackSkin => Some(sub.is_active),
                _ => None,
            },
            priority: false,
            has_note: sub.notes_user.is_some(),
            showcase: sub.is_skeleton(),
            source_archive: sub.source_archive.clone(),
            imported_at: sub.imported_at.clone(),
            size_bytes: row_size(dir.as_deref(), sub.is_skeleton()),
            id: sub.id,
        });
    }

    // --- Couches : hôte connu par construction lui aussi ---
    for layer in layers {
        let dir = crate::libpath::resolve(cfg.library_path.as_deref(), &layer.library_path);
        out.push(InventoryRow {
            uid: format!("LAYER:{}", layer.id),
            kind: RowKind::Layer,
            // Le nom dérivé (retrait de l'extension, du préfixe de l'hôte) est
            // calculé côté front, qui le fait déjà pour la fiche de couche : le
            // dupliquer ici en ferait deux versions à garder d'accord.
            name: layer.display_name_user.clone().unwrap_or_else(|| layer.name.clone()),
            tech_id: layer.source_archive.clone().unwrap_or_else(|| layer.name.clone()),
            areas: Vec::new(),
            attachment: host_attachment(&index, &layer.parent_id, Nature::Appearance),
            active: Some(layer.is_active),
            priority: false,
            has_note: layer.notes_user.is_some(),
            showcase: layer.is_skeleton(),
            source_archive: layer.source_archive.clone(),
            imported_at: layer.imported_at.clone(),
            size_bytes: row_size(dir.as_deref(), layer.is_skeleton()),
            id: layer.id,
        });
    }

    out
}

/// The size of a row's folder; without its files, what its manifest says left
/// is added back — a layer keeps its host's skeleton (ESPACE§5.4), the rest
/// keeps nothing.
fn row_size(dir: Option<&std::path::Path>, freed: bool) -> i64 {
    let Some(dir) = dir else { return 0 };
    let left = crate::inspect::dir_size_bytes(dir);
    let gone = if freed {
        crate::skeleton::read_manifest(dir).map_or(0, |m| m.removed_bytes)
    } else {
        0
    };
    (left + gone) as i64
}

/// Une livrée change ce qu'on voit, un son ce qu'on entend : les deux relèvent
/// de l'apparence au sens de la facette — ce qui se choisit pour lui-même, par
/// opposition à une dépendance qu'on subit.
fn nature_of_sub(_kind: RowKind) -> Nature {
    Nature::Appearance
}

/// Rattachement d'un élément dont l'hôte est écrit dans sa propre table.
///
/// C'est le signal n°2 du REFONTE§2.1, le seul qui soit **certain** : rien n'est
/// déduit, `parent_id` dit l'hôte. Un hôte inconnu de la bibliothèque (mod
/// supprimé, couche en attente de son contenu) laisse la ligne rattachée « au
/// jeu » plutôt que de la faire disparaître — REFONTE§14 en fera un état d'attente
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

/// Ce qui est greffé sur une entité (REFONTE§4.3).
///
/// **C'est ce qui rend le non-dogmatisme sûr** : une déduction ratée coûte un
/// raccourci manquant sur la fiche de l'hôte, jamais un mod introuvable — il
/// reste dans l'inventaire quoi qu'il arrive. Et une déduction réussie répond à
/// la question qu'on se pose devant une voiture : qu'est-ce qui a été posé
/// dessus ? Les notices livrées avec elle comprises, qui sont les ressources du
/// mod qui les porte et que sa fiche à elle ne montrera jamais.
///
/// The whole inventory is built for it, so it goes through `list_shared` too.
pub fn attached_to(db: &overlay::Db, cfg: &AppConfig, entity_id: &str) -> Result<Vec<InventoryRow>, String> {
    let key = entity_id.to_ascii_lowercase();
    Ok(list_shared(db, cfg)?
        .into_iter()
        .filter(|r| r.attachment.target_id.as_deref() == Some(key.as_str()))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The inventory on a plain connection - `list_shared`, lock aside.
    fn list(conn: &Connection, cfg: &AppConfig) -> rusqlite::Result<Vec<InventoryRow>> {
        Ok(rows_from_disk(cfg, inventory_base(conn)?))
    }

    /// The inventory behind `list_inventory` reads the base first, then the
    /// files with the lock released: it must build exactly the rows of a
    /// listing on a plain connection, every source included - an "other" mod
    /// (whose attachment is deduced from its files), a livery and a layer,
    /// each weighed on the disk.
    #[test]
    fn inventory_on_the_shared_base_matches_inventory_on_a_connection() {
        let base = crate::testutil::temp_dir("inventory-shared");
        let library = base.join("library");
        for dir in ["subs/livery", "layers/layer"] {
            std::fs::create_dir_all(library.join(dir)).unwrap();
            std::fs::write(library.join(dir).join("file.bin"), b"12345").unwrap();
        }
        let cfg = AppConfig {
            library_path: Some(library.clone()),
            ..Default::default()
        };
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let now = chrono::Local::now().to_rfc3339();
        overlay::upsert_mod(&conn, "spa", "Track", None, Some("Spa"), "h", None, &now).unwrap();
        overlay::insert_sub_mod(&conn, "livery", "SKIN", "spa", "livery", "subs/livery", None, &now).unwrap();
        overlay::insert_layer(
            &conn,
            "layer",
            "spa",
            "Track",
            "layer.7z",
            "layers/layer",
            Some("layer.7z"),
            1,
            0,
            0,
            &now,
        )
        .unwrap();
        let src = base.join("src").join("Weather");
        std::fs::create_dir_all(src.join("extension").join("weather")).unwrap();
        std::fs::write(src.join("extension").join("weather").join("w.ini"), b"x").unwrap();
        crate::others::import_other(
            &conn,
            &library,
            "Weather.zip",
            &src,
            true,
            crate::resources::ExtractionMode::InfoOnly,
        )
        .unwrap();

        let expected = serde_json::to_value(list(&conn, &cfg).unwrap()).unwrap();
        let db = overlay::Db(std::sync::Mutex::new(conn));
        let shared = serde_json::to_value(list_shared(&db, &cfg).unwrap()).unwrap();
        assert_eq!(shared.as_array().map(Vec::len), Some(3), "one row per source");
        assert_eq!(shared, expected, "same rows, sizes and attachments included");
    }

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
        overlay::upsert_mod(
            &conn,
            "ks_nordschleife",
            "Track",
            None,
            Some("Nordschleife"),
            "h",
            None,
            &now,
        )
        .unwrap();

        // Même id « pack » des deux côtés : c'est tout le sujet.
        overlay::insert_sub_mod(
            &conn,
            "pack",
            "SKIN",
            "ks_nordschleife",
            "pack",
            "subs/pack",
            None,
            &now,
        )
        .unwrap();
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
        assert_eq!(
            skin.active, None,
            "une livrée ne s'active pas : pas d'état de déploiement"
        );
        let layer = rows.iter().find(|r| r.kind == RowKind::Layer).unwrap();
        assert_eq!(layer.active, Some(true), "une couche, si");
        let uids: Vec<&str> = rows.iter().map(|r| r.uid.as_str()).collect();
        assert!(
            uids.contains(&"SUB:pack") && uids.contains(&"LAYER:pack"),
            "ids distincts: {uids:?}"
        );
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

    /// Rule (§4, decided with the user): a mannequin stays in the inventory
    /// **whether or not it is deployed**, exactly like a livery.
    ///
    /// Removing the deployed ones looked faithful to §5 and took away the only
    /// place they could be acted on: the Pilote gallery chooses, it does not
    /// manage. Choosing and managing are two gestures and may have two screens
    /// — which is already how liveries work, and nobody trips over it.
    #[test]
    fn a_mannequin_stays_listed_whether_deployed_or_not() {
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

        let rows = list(&conn, &cfg).unwrap();
        let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
        assert!(
            ids.contains(&"ada"),
            "déployé, et listé quand même : c'est ici qu'on agit dessus"
        );
        assert!(
            rows.iter().all(|r| r.uid.starts_with("OTHER:")),
            "mannequin ou pas, la ligne vient de la table des mods « autres » — c'est d'elle que              dépendent sa fiche et ses actions, pas du type affiché"
        );
        assert!(ids.contains(&"dormant"), "pas déployé, listé aussi");
        assert!(
            rows.iter().all(|r| r.kind == RowKind::Driver),
            "reconnus comme mannequins des deux côtés"
        );
        drop(base);
    }

    /// Rule (§4.2): an "other" row carries the game areas it touches, so the
    /// screen can name it instead of falling back on "Mod".
    ///
    /// Measured on the real library: the `content/fonts` folder of a pack of
    /// nine NSX, left over once the cars were recognised (§7.3). Its row read
    /// "Mod", which is the word that remains when there is nothing more precise
    /// to say — while the app knew perfectly well it was a font, and said so on
    /// the fiche. Same data, one screen short.
    #[test]
    fn an_other_row_names_the_areas_it_touches() {
        let base = crate::testutil::temp_dir("inventory-areas");
        let library = base.join("library");
        let cfg = AppConfig {
            library_path: Some(library.clone()),
            ..Default::default()
        };
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let now = chrono::Local::now().to_rfc3339();

        let dir = library.join("others").join("nsx_fonts");
        std::fs::create_dir_all(dir.join("content").join("fonts")).unwrap();
        std::fs::write(dir.join("content").join("fonts").join("some1_nsx.txt"), b"font").unwrap();
        overlay::insert_other_mod(&conn, "nsx_fonts", "others/nsx_fonts", None, &now).unwrap();

        let rows = list(&conn, &cfg).unwrap();
        let row = rows.iter().find(|r| r.id == "nsx_fonts").unwrap();
        assert_eq!(row.kind, RowKind::Other, "une police reste un mod « autre »");
        assert_eq!(
            row.areas,
            vec!["fonts".to_string()],
            "la zone touchée voyage avec la ligne"
        );
        assert_eq!(
            row.attachment.nature,
            Nature::Dependency,
            "et c'est d'elle que la nature est tirée : une police est un moyen, pas un sujet"
        );
        drop(base);
    }

    /// Rule (REFONTE§14, partiel): a host the library no longer knows leaves the row
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

    /// Rule (ESPACE§5.6): a mannequin without its files is still a mannequin
    /// in the inventory, marked as such, and weighs what it weighed — what
    /// recovering it will bring back.
    #[test]
    fn a_mannequin_without_its_files_is_still_a_mannequin_and_weighs_what_it_weighed() {
        let base = crate::testutil::temp_dir("inventory-freed");
        let library = base.join("library");
        let cfg = AppConfig {
            library_path: Some(library.clone()),
            ..Default::default()
        };
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let now = chrono::Local::now().to_rfc3339();
        let dir = library.join("others").join("ada");
        std::fs::create_dir_all(dir.join("content").join("driver")).unwrap();
        std::fs::write(dir.join("content").join("driver").join("ada.kn5"), vec![0u8; 4096]).unwrap();
        overlay::insert_other_mod(&conn, "ada", "others/ada", Some("ada.7z"), &now).unwrap();
        crate::testutil::free_addon(&conn, "other_mods", "ada", &dir, Some("ada.7z"));

        let rows = list(&conn, &cfg).unwrap();
        assert_eq!(
            rows[0].kind,
            RowKind::Driver,
            "still a mannequin, read from the manifest"
        );
        assert!(rows[0].showcase, "marked as having no files");
        assert!(
            rows[0].size_bytes >= 4096,
            "its original size, not the manifest's few bytes"
        );
    }
}
