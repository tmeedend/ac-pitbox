//! What a mod hangs onto, and what it does (refonte §2).
//!
//! Two independent axes, and they are attributes **of a mod**, not of its type:
//! a driver model is standalone in the general case and grafted onto a car when
//! it ships with one.
//!
//! The guiding principle is the one the whole inventory rests on:
//! **classification does not need to be exact, it needs to be harmless when it
//! is wrong.** Every deduction here is correctable by hand, and no mod becomes
//! unreachable when one fails — an unattached mod simply lands under "the game"
//! instead of next to its host, and it is still listed, searchable and usable.
//!
//! That is also why the **signal** travels with the answer. "Attached to the
//! Nordschleife" deduced from a posed path is near-certain; the same answer
//! deduced from "arrived in the same archive" is a guess. Storing only the
//! conclusion would make the two indistinguishable the day one of them has to
//! be doubted.
//!
//! **Nothing is stored.** The deduction is recomputed from the files at each
//! listing, which is free here: `others::list_others` already walks every mod's
//! files to find conflicts. A stored column would have to be invalidated at
//! every import, every activation and every reindex — and a stale attachment is
//! exactly the kind of wrong that is not harmless, because nothing on screen
//! would say it is out of date. Only the user's **correction** is stored
//! (`other_mods.attachment_user`), because that one has no other source.

use std::collections::HashMap;
use std::path::Path;

use rusqlite::Connection;
use serde::Serialize;

use crate::overlay;

/// How sure the attachment is. Ordered from strongest to weakest — the first
/// signal that answers wins, and the UI may degrade what it shows accordingly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Signal {
    /// Corrected by hand. Beats every deduction, by definition.
    User,
    /// A posed path carries the id of a known entity
    /// (`content/cars/ks_toyota_ae86/…`). Near-certain.
    Path,
    /// The host is written in the row itself — a livery, a sound, a layer all
    /// carry their `parent_id`. Nothing is deduced: this one is **certain**.
    Layer,
    /// A file (or the mod's own id) is named after an entity —
    /// `la_canyons__hide_pit_crew.ini`. Strong.
    ConfigName,
    /// Arrived in the same archive as a content. **A guess**, and the only one
    /// here: a pack can ship a font for a car it does not otherwise touch.
    Archive,
    /// Nothing pointed anywhere.
    None,
}

/// What the mod hangs onto (§2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AttachKind {
    Car,
    Track,
    App,
    /// Touches the game itself, not one of its contents.
    Game,
    /// Hangs onto nothing — it is its own thing.
    Standalone,
}

/// What the mod does (§2.2).
///
/// `Content` is the one an inventoried row should almost never carry: it
/// belongs to the four autonomous types, which have their own screens. The
/// exception is the driver model, which is one of those four and is still
/// listed here while the Pilote screen has no inventory of its own (L8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Nature {
    /// Changes nothing in the game: a notice, a manual, a livery template. The
    /// row exists so that the file stays reachable, not because it does
    /// anything — and calling that "unidentified" was wrong twice over, since
    /// the app knows exactly what it is.
    Document,
    /// A thing chosen for itself, not something dressing another: a driver
    /// model. Asking whether a mannequin is "appearance" is the right question
    /// with the wrong answer — it does not dress a car, it *is* the content.
    Content,
    /// Changes how something looks — a livery, a driver model, a PP filter.
    Appearance,
    /// Changes how something behaves — a CSP config, a weather script.
    Behaviour,
    /// There so that something else works: fonts, shared textures.
    Dependency,
    /// Pit Box could not tell. Listed all the same — never lost (§4).
    Unrecognised,
}

#[derive(Debug, Clone, Serialize)]
pub struct Attachment {
    pub kind: AttachKind,
    /// Id of the entity it hangs onto, when there is one.
    pub target_id: Option<String>,
    /// Readable name of that entity, resolved once here rather than by every
    /// caller that would otherwise have to index the library again.
    pub target_name: Option<String>,
    pub signal: Signal,
    pub nature: Nature,
}

/// The library, indexed for lookup: id → (kind, readable name).
///
/// Built once per listing and passed around: the deduction below asks this
/// question once per path of every mod, and rebuilding it each time is what
/// turns a free computation into a slow one.
pub struct EntityIndex {
    entities: HashMap<String, (AttachKind, Option<String>)>,
}

impl EntityIndex {
    pub fn build(conn: &Connection) -> rusqlite::Result<Self> {
        let mut entities = HashMap::new();
        for m in overlay::list_mods(conn)? {
            let kind = if m.kind == "Track" {
                AttachKind::Track
            } else {
                AttachKind::Car
            };
            entities.insert(m.id_interne.to_ascii_lowercase(), (kind, m.display_name.clone()));
        }
        for a in overlay::list_apps(conn)? {
            entities.insert(
                a.id.to_ascii_lowercase(),
                (AttachKind::App, a.display_name_user.clone()),
            );
        }
        Ok(Self { entities })
    }

    /// L'entité d'un id, si la bibliothèque la connaît. Public parce que
    /// l'inventaire (§4) s'en sert pour les sources dont l'hôte est **écrit**
    /// et non déduit — livrées, sons, couches.
    pub fn lookup(&self, id: &str) -> Option<(AttachKind, Option<String>, String)> {
        let key = id.to_ascii_lowercase();
        self.entities
            .get(&key)
            .map(|(kind, name)| (*kind, name.clone(), key.clone()))
    }
}

/// Ids too short or too common to be recognised inside a longer string. Matching
/// `ks` or `gp` as a "name prefix" would attach half the library to whatever mod
/// happens to start with those letters.
const MIN_NAME_MATCH: usize = 4;

/// Deduces where an "other" mod (§7.3) hangs, from the files it carries.
///
/// `files` are paths **relative to the AC root** — the same ones the conflict
/// detection walks. `id` is the mod's own identifier, which for an imported
/// loose file carries the archive name and sometimes the entity name
/// (`policeman__ext_config.ini`).
pub fn attachment_of(
    index: &EntityIndex,
    id: &str,
    source_archive: Option<&str>,
    files: &[impl AsRef<Path>],
    categories: &[String],
    user_override: Option<&str>,
) -> Attachment {
    let nature = nature_of(categories);

    // 0. La correction de l'utilisateur, quand elle existe. Elle ne se discute
    //    pas : c'est la seule information de cette fonction qui vienne de
    //    quelqu'un qui SAIT, au lieu d'être déduite.
    if let Some(target) = user_override.map(str::trim).filter(|s| !s.is_empty()) {
        if let Some((kind, name, key)) = index.lookup(target) {
            return Attachment {
                kind,
                target_id: Some(key),
                target_name: name,
                signal: Signal::User,
                nature,
            };
        }
        // Cible devenue inconnue (entité supprimée depuis) : on ne remonte pas
        // un rattachement fantôme, on retombe sur la déduction.
    }

    // 1. Un chemin posé porte l'id d'une entité connue. `content/cars/<id>/…`
    //    ne peut pas vouloir dire autre chose.
    for f in files {
        for seg in f.as_ref().components() {
            let Some(seg) = seg.as_os_str().to_str() else { continue };
            if let Some((kind, name, key)) = index.lookup(seg) {
                return Attachment {
                    kind,
                    target_id: Some(key),
                    target_name: name,
                    signal: Signal::Path,
                    nature,
                };
            }
        }
    }

    // 2. Un nom de config formé sur une entité : `la_canyons__hide_pit_crew.ini`,
    //    `<id>.ini` sous `extension/config/*/loaded/`. On regarde l'id du mod
    //    lui-même ET les noms de fichiers — l'import range parfois l'un dans
    //    l'autre.
    let mut names: Vec<String> = vec![id.to_string()];
    for f in files {
        if let Some(n) = f.as_ref().file_name().and_then(|n| n.to_str()) {
            names.push(n.to_string());
        }
    }
    for n in &names {
        if let Some((kind, name, key)) = name_prefix_entity(index, n) {
            return Attachment {
                kind,
                target_id: Some(key),
                target_name: name,
                signal: Signal::ConfigName,
                nature,
            };
        }
    }

    // 3. Arrivé dans la même archive qu'un contenu. **Conjecture** — un pack
    //    peut livrer une police pour une voiture qu'il ne touche pas autrement.
    if let Some(archive) = source_archive {
        if let Some((kind, name, key)) = archive_entity(index, archive) {
            return Attachment {
                kind,
                target_id: Some(key),
                target_name: name,
                signal: Signal::Archive,
                nature,
            };
        }
    }

    // Aucun signal : le mod touche le jeu lui-même. « Autonome » est réservé à
    // ce qui ne pose rien du tout — sans fichier, rien ne permet de dire qu'il
    // vise le jeu.
    Attachment {
        kind: if files.is_empty() {
            AttachKind::Standalone
        } else {
            AttachKind::Game
        },
        target_id: None,
        target_name: None,
        signal: Signal::None,
        nature,
    }
}

/// L'entité nommée en tête d'un nom de fichier, avant un séparateur.
///
/// `la_canyons__hide_pit_crew.ini` → `la_canyons`. On ne coupe que sur `__`
/// (la convention de l'import pour « untel, extrait de telle archive ») et sur
/// le point final : couper sur le simple `_` ferait de `ks_nordschleife` un
/// `ks`, qui ne veut rien dire et qui matcherait tout.
fn name_prefix_entity(index: &EntityIndex, name: &str) -> Option<(AttachKind, Option<String>, String)> {
    let mut candidates: Vec<&str> = Vec::new();
    if let Some((head, _)) = name.split_once("__") {
        candidates.push(head);
    }
    if let Some(stem) = name.rsplit_once('.').map(|(head, _)| head) {
        candidates.push(stem);
    }
    candidates
        .into_iter()
        .filter(|c| c.len() >= MIN_NAME_MATCH)
        .find_map(|c| index.lookup(c))
}

/// L'entité dont l'archive d'origine est la même. Le nom d'archive traîne
/// souvent dans l'id des fichiers isolés (`<archive>__content_fonts`), d'où la
/// comparaison sur la chaîne entière plutôt que sur un préfixe.
fn archive_entity(index: &EntityIndex, archive: &str) -> Option<(AttachKind, Option<String>, String)> {
    let hay = archive.to_ascii_lowercase();
    index
        .entities
        .iter()
        .filter(|(id, _)| id.len() >= MIN_NAME_MATCH)
        .find(|(id, _)| hay.contains(id.as_str()))
        .map(|(id, (kind, name))| (*kind, name.clone(), id.clone()))
}

/// Ce que le mod fait, d'après les zones du jeu qu'il touche
/// ([`crate::others::CATEGORY_ORDER`]).
///
/// Un mod touche souvent plusieurs zones : c'est la plus **conséquente** qui
/// donne sa nature, parce que c'est celle qu'on cherche. Un pack qui pose une
/// config CSP et une police est d'abord un mod de comportement — la police y
/// est un moyen, pas le sujet.
pub fn nature_of(categories: &[String]) -> Nature {
    let has = |c: &str| categories.iter().any(|x| x == c);
    if has("extension") || has("weather") {
        return Nature::Behaviour;
    }
    if has("driver") || has("textures") || has("objects3d") || has("showrooms") || has("gui") || has("ppfilters") {
        return Nature::Appearance;
    }
    // Une police n'est jamais choisie pour elle-même : elle est là parce qu'un
    // contenu la réclame. C'est le cas qui sépare « dépendance » d'« apparence »
    // — les deux changent ce qu'on voit, une seule se choisit.
    if has("fonts") {
        return Nature::Dependency;
    }
    Nature::Unrecognised
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn index(conn: &Connection) -> EntityIndex {
        EntityIndex::build(conn).unwrap()
    }

    fn db() -> (crate::testutil::TempDir, Connection) {
        let base = crate::testutil::temp_dir("attach");
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
        overlay::upsert_mod(&conn, "la_canyons", "Track", None, Some("LA Canyons"), "h", None, &now).unwrap();
        overlay::upsert_mod(
            &conn,
            "rss_formula_rss_supreme_25",
            "Car",
            None,
            Some("Formula RSS Supreme 25"),
            "h",
            None,
            &now,
        )
        .unwrap();
        overlay::insert_app(&conn, "camtool", "apps/camtool", None, &now).unwrap();
        (base, conn)
    }

    fn paths(list: &[&str]) -> Vec<PathBuf> {
        list.iter().map(PathBuf::from).collect()
    }

    /// Rule (§2.1, signal 1): a posed path carrying a known id says where the
    /// mod hangs, and it cannot mean anything else.
    #[test]
    fn a_posed_path_names_its_host() {
        let (base, conn) = db();
        let a = attachment_of(
            &index(&conn),
            "Sound - ferrari_f40 by Marti",
            None,
            &paths(&["content/tracks/ks_nordschleife/extension/ext_config.ini"]),
            &["extension".into()],
            None,
        );
        assert_eq!(a.kind, AttachKind::Track);
        assert_eq!(a.target_id.as_deref(), Some("ks_nordschleife"));
        assert_eq!(a.signal, Signal::Path, "le chemin est le signal le plus fort");
        assert_eq!(a.nature, Nature::Behaviour, "extension/ = comportement");
        drop(base);
    }

    /// Rule (§2.1, signal 3): a CSP config named after an entity. The cut falls
    /// on `__` — the import's own convention — never on a single `_`, which
    /// would reduce `ks_nordschleife` to `ks` and attach half the library to it.
    #[test]
    fn a_config_named_after_an_entity_attaches_to_it() {
        let (base, conn) = db();
        let a = attachment_of(
            &index(&conn),
            "la_canyons__hide_pit_crew.ini",
            None,
            &paths(&["extension/config/tracks/loaded/la_canyons__hide_pit_crew.ini"]),
            &["extension".into()],
            None,
        );
        assert_eq!(a.target_id.as_deref(), Some("la_canyons"));
        assert_eq!(a.signal, Signal::ConfigName);

        // Un mod dont l'id commence par un fragment trop court ne s'accroche à
        // rien : `ks` n'est pas une entité, et ne doit pas le devenir.
        let b = attachment_of(
            &index(&conn),
            "ks__something.ini",
            None,
            &paths(&["extension/x.ini"]),
            &["extension".into()],
            None,
        );
        assert_eq!(b.kind, AttachKind::Game, "aucun signal : le jeu");
        drop(base);
    }

    /// Rule (§2.1, signal 4): same archive as a content. It is a **guess**, and
    /// the signal says so — this is the RSS pack's fonts.
    #[test]
    fn the_archive_is_a_guess_and_says_so() {
        let (base, conn) = db();
        let a = attachment_of(
            &index(&conn),
            "RSS_Formula_RSS_Supreme_25-Assetto_Corsa-v2.rar__content_fonts",
            Some("RSS_Formula_RSS_Supreme_25-Assetto_Corsa-v2.rar"),
            &paths(&["content/fonts/supreme.txt"]),
            &["fonts".into()],
            None,
        );
        assert_eq!(a.target_id.as_deref(), Some("rss_formula_rss_supreme_25"));
        assert_eq!(
            a.signal,
            Signal::Archive,
            "conjecture, et elle est étiquetée comme telle"
        );
        assert_eq!(a.nature, Nature::Dependency, "une police est un moyen, pas un sujet");
        drop(base);
    }

    /// Rule (§2.3): the user's correction beats every deduction — and a
    /// correction pointing at an entity that no longer exists falls back on the
    /// deduction instead of showing a ghost.
    #[test]
    fn a_correction_wins_unless_its_target_is_gone() {
        let (base, conn) = db();
        let files = paths(&["content/tracks/ks_nordschleife/ui/x.png"]);
        let a = attachment_of(
            &index(&conn),
            "x",
            None,
            &files,
            &["textures".into()],
            Some("la_canyons"),
        );
        assert_eq!(a.target_id.as_deref(), Some("la_canyons"), "la correction l'emporte");
        assert_eq!(a.signal, Signal::User);

        let b = attachment_of(&index(&conn), "x", None, &files, &["textures".into()], Some("disparu"));
        assert_eq!(
            b.target_id.as_deref(),
            Some("ks_nordschleife"),
            "repli sur la déduction"
        );
        assert_eq!(b.signal, Signal::Path);
        drop(base);
    }

    /// Rule (§2.1): no signal at all means the mod touches the game itself —
    /// and a mod that poses nothing is standalone rather than "the game",
    /// because nothing lets us say it aims at the game.
    #[test]
    fn no_signal_means_the_game_unless_nothing_is_posed() {
        let (base, conn) = db();
        let a = attachment_of(
            &index(&conn),
            "naturalmod_v6",
            None,
            &paths(&["system/cfg/ppfilters/natural.ini"]),
            &["ppfilters".into()],
            None,
        );
        assert_eq!(a.kind, AttachKind::Game);
        assert_eq!(a.nature, Nature::Appearance);

        let empty: Vec<PathBuf> = Vec::new();
        let b = attachment_of(&index(&conn), "readme.pdf", None, &empty, &["other".into()], None);
        assert_eq!(b.kind, AttachKind::Standalone, "rien de posé : autonome");
        assert_eq!(b.nature, Nature::Unrecognised);
        drop(base);
    }

    /// Rule (§2.2): several zones, one nature — the most consequential wins,
    /// because it is the one being looked for.
    #[test]
    fn the_most_consequential_zone_gives_the_nature() {
        assert_eq!(nature_of(&["fonts".into(), "extension".into()]), Nature::Behaviour);
        assert_eq!(nature_of(&["fonts".into(), "driver".into()]), Nature::Appearance);
        assert_eq!(nature_of(&["fonts".into()]), Nature::Dependency);
        assert_eq!(nature_of(&["other".into()]), Nature::Unrecognised);
    }
}
