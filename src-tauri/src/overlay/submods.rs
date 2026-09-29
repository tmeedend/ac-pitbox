//! Attached sub-elements (§8.3): skins, sounds and the like, hung on a parent.

use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubModRow {
    pub id: String,
    pub sub_type: String,
    pub parent_id: String,
    pub name: String,
    pub library_path: String,
    pub source_archive: Option<String>,
    pub is_active: bool,
    /// Faux si fourni avec le contenu initial du mod (découvert sur disque,
    /// §8) — non supprimable individuellement, seulement le mod entier.
    pub removable: bool,
    pub imported_at: String,
    /// Auteur, saisi par l'utilisateur : aucun fichier de mod ne le porte.
    pub author: Option<String>,
    /// Taille sur disque du dossier stocké, octets. Jamais mémorisée en base
    /// (le contenu d'un skin peut changer sous nos pieds) : mesurée à la
    /// demande, donc `None` partout sauf là où on la réclame explicitement
    /// (vue transversale, cf. `submods::list_by_type_sized`).
    pub size_bytes: Option<i64>,
    /// Nom repris à la main (REFONTE§6.1) : `skin_01` ne dit rien de ce que
    /// la livrée montre.
    pub display_name_user: Option<String>,
    /// Note libre (REFONTE§9).
    pub notes_user: Option<String>,
    /// [`CONTENT_FULL`] or [`CONTENT_SKELETON`]: an attached skin or sound
    /// follows its host into the showcase (ESPACE§5.4).
    pub content_state: String,
}

impl SubModRow {
    pub fn is_skeleton(&self) -> bool {
        self.content_state == CONTENT_SKELETON
    }
}

#[allow(clippy::too_many_arguments)]
pub fn insert_sub_mod(
    conn: &Connection,
    id: &str,
    sub_type: &str,
    parent_id: &str,
    name: &str,
    library_path: &str,
    source_archive: Option<&str>,
    imported_at: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        r#"INSERT INTO sub_mods (id, sub_type, parent_id, name, library_path, source_archive, imported_at)
           VALUES (?1,?2,?3,?4,?5,?6,?7)"#,
        params![id, sub_type, parent_id, name, library_path, source_archive, imported_at],
    )?;
    Ok(())
}

/// Enregistre un skin de circuit découvert sur disque, fourni avec le
/// contenu initial du mod (§8) — jamais importé séparément par Pit Box,
/// donc `removable = 0` : reconnu et activable, mais non supprimable
/// individuellement (seulement le mod entier).
pub fn insert_bundled_track_skin(
    conn: &Connection,
    id: &str,
    parent_id: &str,
    name: &str,
    library_path: &str,
    imported_at: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        r#"INSERT INTO sub_mods (id, sub_type, parent_id, name, library_path, source_archive, removable, imported_at)
           VALUES (?1,'TRACK_SKIN',?2,?3,?4,NULL,0,?5)"#,
        params![id, parent_id, name, library_path, imported_at],
    )?;
    Ok(())
}

pub(super) fn map_sub(row: &rusqlite::Row) -> rusqlite::Result<SubModRow> {
    Ok(SubModRow {
        id: row.get(0)?,
        sub_type: row.get(1)?,
        parent_id: row.get(2)?,
        name: row.get(3)?,
        library_path: row.get(4)?,
        source_archive: row.get(5)?,
        is_active: row.get::<_, i64>(6)? != 0,
        removable: row.get::<_, i64>(7)? != 0,
        imported_at: row.get(8)?,
        author: row.get(9)?,
        size_bytes: None,
        display_name_user: row.get(10)?,
        notes_user: row.get(11)?,
        content_state: row.get(12)?,
    })
}

pub(super) const SUB_SELECT: &str =
    "SELECT id, sub_type, parent_id, name, library_path, source_archive, is_active, removable, imported_at, author, display_name_user, notes_user, content_state FROM sub_mods";

/// Sous-éléments rattachés à une entité (fiche détail, §8.3).
/// Sous-éléments (skins, sons) dont le parent n'existe plus (§10). Conservés
/// **délibérément** à la suppression du mod — voir `delete_mod` — mais devenus
/// inutiles dès qu'on ne compte plus réimporter le parent. Listés ici pour être
/// nettoyés sur décision de l'utilisateur, jamais automatiquement.
pub fn orphan_subs(conn: &Connection) -> rusqlite::Result<Vec<SubModRow>> {
    // Colonnes nommées, jamais `SELECT *` : `map_sub` lit par index, or l'ordre
    // des colonnes de la table est celui des `ALTER` successifs, pas celui du
    // mappeur. Les deux ont divergé dès l'ajout de `notes_user`.
    let mut stmt = conn.prepare(&format!(
        "{SUB_SELECT} WHERE parent_id NOT IN (SELECT id_interne FROM mods)
          ORDER BY parent_id, name"
    ))?;
    let rows = stmt.query_map([], map_sub)?;
    rows.collect()
}

pub fn list_subs_for_parent(conn: &Connection, parent_id: &str) -> rusqlite::Result<Vec<SubModRow>> {
    let mut stmt = conn.prepare(&format!(
        "{SUB_SELECT} WHERE parent_id = ?1 ORDER BY name COLLATE NOCASE"
    ))?;
    let rows = stmt.query_map([parent_id], map_sub)?;
    rows.collect()
}

/// Tous les sous-éléments d'un type (vue transversale, §8.3).
pub fn list_subs_by_type(conn: &Connection, sub_type: &str) -> rusqlite::Result<Vec<SubModRow>> {
    let mut stmt = conn.prepare(&format!(
        "{SUB_SELECT} WHERE sub_type = ?1 ORDER BY parent_id, name COLLATE NOCASE"
    ))?;
    let rows = stmt.query_map([sub_type], map_sub)?;
    rows.collect()
}

/// Tous les sous-éléments, quel que soit leur type (inventaire, REFONTE§4).
pub fn list_all_subs(conn: &Connection) -> rusqlite::Result<Vec<SubModRow>> {
    let mut stmt = conn.prepare(&format!("{SUB_SELECT} ORDER BY parent_id, name COLLATE NOCASE"))?;
    let rows = stmt.query_map([], map_sub)?;
    rows.collect()
}

pub fn get_sub_mod(conn: &Connection, id: &str) -> rusqlite::Result<Option<SubModRow>> {
    let mut stmt = conn.prepare(&format!("{SUB_SELECT} WHERE id = ?1"))?;
    let mut rows = stmt.query_map([id], map_sub)?;
    match rows.next() {
        Some(r) => Ok(Some(r?)),
        None => Ok(None),
    }
}

/// Enregistre l'auteur d'un sous-élément. `None` efface la saisie.
pub fn set_sub_author(conn: &Connection, id: &str, author: Option<&str>) -> rusqlite::Result<()> {
    let cleaned = author.map(str::trim).filter(|s| !s.is_empty());
    conn.execute(
        "UPDATE sub_mods SET author = ?2 WHERE id = ?1",
        rusqlite::params![id, cleaned],
    )?;
    Ok(())
}

/// Existe-t-il déjà un sous-élément de ce type/parent/nom ? (idempotence import).
/// The attached skin or sound of this type, parent and name — the identity
/// an import recognizes it by (ESPACE§7.5).
pub fn find_sub(conn: &Connection, sub_type: &str, parent_id: &str, name: &str) -> rusqlite::Result<Option<SubModRow>> {
    let mut stmt = conn.prepare(&format!(
        "{SUB_SELECT} WHERE sub_type = ?1 AND parent_id = ?2 AND name = ?3"
    ))?;
    let mut rows = stmt.query_map(params![sub_type, parent_id, name], map_sub)?;
    rows.next().transpose()
}

/// An attached skin or sound followed its host into the showcase (ESPACE§5.4).
/// `is_active` is left as the user set it — a track skin switched on comes
/// back switched on; what is laid in the game filters skeletons out.
pub fn mark_sub_freed(conn: &Connection, id: &str, freed_at: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE sub_mods SET content_state = ?2, freed_at = ?3 WHERE id = ?1",
        params![id, CONTENT_SKELETON, freed_at],
    )?;
    Ok(())
}

/// Its files are back (ESPACE§7.5), stored at `library_path` — the folder
/// they were laid in, which is its own unless that one was outside the
/// library.
pub fn mark_sub_full(conn: &Connection, id: &str, library_path: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE sub_mods SET content_state = ?2, freed_at = NULL, library_path = ?3 WHERE id = ?1",
        params![id, CONTENT_FULL, library_path],
    )?;
    Ok(())
}

pub fn sub_exists(conn: &Connection, sub_type: &str, parent_id: &str, name: &str) -> rusqlite::Result<bool> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sub_mods WHERE sub_type = ?1 AND parent_id = ?2 AND name = ?3",
        params![sub_type, parent_id, name],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

pub fn delete_sub_mod(conn: &Connection, id: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM sub_mods WHERE id = ?1", [id])?;
    Ok(())
}

/// Bascule exclusive du son actif d'une voiture (§8.3) : un seul SOUND actif
/// par parent. `id = None` désactive tout (retour au son d'origine).
pub fn set_active_sound(conn: &Connection, parent_id: &str, id: Option<&str>) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE sub_mods SET is_active = 0 WHERE parent_id = ?1 AND sub_type = 'SOUND'",
        [parent_id],
    )?;
    if let Some(id) = id {
        conn.execute("UPDATE sub_mods SET is_active = 1 WHERE id = ?1", [id])?;
    }
    Ok(())
}

/// Active/désactive un skin de circuit par nom (§8) — PAS exclusif,
/// contrairement au son : plusieurs TRACK_SKIN peuvent être `is_active` en
/// même temps pour un même circuit.
pub fn set_track_skin_active(conn: &Connection, parent_id: &str, name: &str, active: bool) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE sub_mods SET is_active = ?3 WHERE parent_id = ?1 AND sub_type = 'TRACK_SKIN' AND name = ?2",
        params![parent_id, name, active as i64],
    )?;
    Ok(())
}
