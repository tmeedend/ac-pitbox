//! Layers (§4.4): what an extension adds on top of a mod or an app.

use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerRow {
    pub id: String,
    pub parent_id: String,
    pub parent_kind: String,
    pub name: String,
    pub library_path: String,
    pub source_archive: Option<String>,
    pub added_count: i64,
    pub overwritten_count: i64,
    pub is_active: bool,
    pub priority: i64,
    pub imported_at: String,
    /// Nom repris à la main (REFONTE§8.3). Le nom dérivé de l'archive est
    /// faillible par construction, donc corrigeable.
    pub display_name_user: Option<String>,
    /// Note libre (REFONTE§9). Distincte de la description : elle n'a pas de
    /// valeur d'origine, donc vide veut dire vide.
    pub notes_user: Option<String>,
    /// [`CONTENT_FULL`] or [`CONTENT_SKELETON`]: a layer follows its host into
    /// the showcase (ESPACE§5.4), its row kept and its files gone.
    pub content_state: String,
}

impl LayerRow {
    pub fn is_skeleton(&self) -> bool {
        self.content_state == CONTENT_SKELETON
    }
}

/// Fragment SQL isolant les couches d'un hôte : son id **et son espace de noms**.
///
/// `parent_id` seul ne suffit pas depuis que les apps reçoivent des couches
/// (§8.4). Une voiture et un circuit, eux, ne peuvent **pas** porter le même
/// id — ils vivent dans la même table, dont `id_interne` est la clé primaire —
/// mais une app vit dans `apps`, avec sa propre clé : rien n'empêche un circuit
/// et une app de s'appeler pareil. Sans ce filtre, les couches de l'un
/// remonteraient sur l'autre, et `recompose` composerait celles de l'app dans le
/// dossier du circuit.
///
/// Seule la distinction app / pas-app est donc nécessaire, et c'est tout ce que
/// ce fragment fait — comparer `parent_kind` à `'App'`.
pub(super) const LAYER_HOST: &str = "parent_id = ?1 AND (parent_kind = 'App') = ?2";

/// Priorité à attribuer à une nouvelle couche du parent (max + 1 : empilée en tête).
pub fn next_layer_priority(conn: &Connection, parent_id: &str, host: HostKind) -> rusqlite::Result<i64> {
    conn.query_row(
        &format!("SELECT COALESCE(MAX(priority), -1) + 1 FROM layers WHERE {LAYER_HOST}"),
        params![parent_id, host == HostKind::App],
        |r| r.get(0),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn insert_layer(
    conn: &Connection,
    id: &str,
    parent_id: &str,
    parent_kind: &str,
    name: &str,
    library_path: &str,
    source_archive: Option<&str>,
    added_count: i64,
    overwritten_count: i64,
    priority: i64,
    imported_at: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        r#"INSERT INTO layers
           (id, parent_id, parent_kind, name, library_path, source_archive,
            added_count, overwritten_count, priority, imported_at)
           VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)"#,
        params![
            id,
            parent_id,
            parent_kind,
            name,
            library_path,
            source_archive,
            added_count,
            overwritten_count,
            priority,
            imported_at
        ],
    )?;
    Ok(())
}

pub(super) fn map_layer(row: &rusqlite::Row) -> rusqlite::Result<LayerRow> {
    Ok(LayerRow {
        id: row.get(0)?,
        parent_id: row.get(1)?,
        parent_kind: row.get(2)?,
        name: row.get(3)?,
        library_path: row.get(4)?,
        source_archive: row.get(5)?,
        added_count: row.get(6)?,
        overwritten_count: row.get(7)?,
        is_active: row.get::<_, i64>(8)? != 0,
        priority: row.get(9)?,
        imported_at: row.get(10)?,
        display_name_user: row.get(11)?,
        notes_user: row.get(12)?,
        content_state: row.get(13)?,
    })
}

pub(super) const LAYER_SELECT: &str = "SELECT id, parent_id, parent_kind, name, library_path, source_archive, added_count, overwritten_count, is_active, priority, imported_at, display_name_user, notes_user, content_state FROM layers";

/// Couches/extensions rattachées à une base (fiche détail, §4.4), par priorité.
pub fn list_layers(conn: &Connection, parent_id: &str, host: HostKind) -> rusqlite::Result<Vec<LayerRow>> {
    let mut stmt = conn.prepare(&format!("{LAYER_SELECT} WHERE {LAYER_HOST} ORDER BY priority"))?;
    let rows = stmt.query_map(params![parent_id, host == HostKind::App], map_layer)?;
    rows.collect()
}

/// Toutes les couches d'un type (Car|Track), pour la vue transversale add-ons.
/// Toutes les couches, tous hôtes confondus (inventaire, REFONTE§4).
pub fn list_all_layers(conn: &Connection) -> rusqlite::Result<Vec<LayerRow>> {
    let mut stmt = conn.prepare(&format!("{LAYER_SELECT} ORDER BY parent_id, priority"))?;
    let rows = stmt.query_map([], map_layer)?;
    rows.collect()
}

pub fn list_layers_by_kind(conn: &Connection, kind: &str) -> rusqlite::Result<Vec<LayerRow>> {
    let mut stmt = conn.prepare(&format!(
        "{LAYER_SELECT} WHERE parent_kind = ?1 ORDER BY parent_id, priority"
    ))?;
    let rows = stmt.query_map([kind], map_layer)?;
    rows.collect()
}

/// Couches **actives** d'une base, dans l'ordre de priorité (la + haute en dernier
/// → gagne à la superposition). Base de la composition (§4.4).
/// Active **and complete**: a layer in the showcase (ESPACE§5.4) has only its
/// manifest left, and composing it would lay that file in `content/`. The one
/// place every composition reads its layers from.
pub fn active_layers(conn: &Connection, parent_id: &str, host: HostKind) -> rusqlite::Result<Vec<LayerRow>> {
    let mut stmt = conn.prepare(&format!(
        "{LAYER_SELECT} WHERE {LAYER_HOST} AND is_active = 1 AND content_state = '{CONTENT_FULL}' ORDER BY priority"
    ))?;
    let rows = stmt.query_map(params![parent_id, host == HostKind::App], map_layer)?;
    rows.collect()
}

pub fn get_layer(conn: &Connection, id: &str) -> rusqlite::Result<Option<LayerRow>> {
    let mut stmt = conn.prepare(&format!("{LAYER_SELECT} WHERE id = ?1"))?;
    let mut rows = stmt.query_map([id], map_layer)?;
    match rows.next() {
        Some(r) => Ok(Some(r?)),
        None => Ok(None),
    }
}

/// New content for an existing layer, which keeps its id, place in the order
/// and on/off state (§4.6ter: a proposed folder answered "layer" again).
pub fn update_layer_content(
    conn: &Connection,
    id: &str,
    source_archive: &str,
    added: i64,
    overwritten: i64,
    imported_at: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE layers SET source_archive = ?2, added_count = ?3, overwritten_count = ?4, imported_at = ?5,
                           content_state = 'full', freed_at = NULL
         WHERE id = ?1",
        params![id, source_archive, added, overwritten, imported_at],
    )?;
    Ok(())
}

/// A layer followed its host into the showcase (ESPACE§5.4). Refilling it
/// (`update_layer_content`) makes it complete again.
pub fn mark_layer_freed(conn: &Connection, id: &str, freed_at: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE layers SET content_state = ?2, freed_at = ?3 WHERE id = ?1",
        params![id, CONTENT_SKELETON, freed_at],
    )?;
    Ok(())
}

pub fn set_layer_active(conn: &Connection, id: &str, active: bool) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE layers SET is_active = ?2 WHERE id = ?1",
        params![id, active as i64],
    )?;
    Ok(())
}

pub fn set_layer_priority(conn: &Connection, id: &str, priority: i64) -> rusqlite::Result<()> {
    conn.execute("UPDATE layers SET priority = ?2 WHERE id = ?1", params![id, priority])?;
    Ok(())
}

pub fn delete_layer(conn: &Connection, id: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM layers WHERE id = ?1", [id])?;
    Ok(())
}

/// Couches dont l'hôte n'est **ni** un mod **ni** une app de la bibliothèque
/// (§4.3bis) : une couche « en attente », rangée sous l'id qu'elle vise avant
/// que celui-ci n'arrive — ou dont l'hôte a été supprimé depuis.
///
/// Les apps comptent, sans quoi toute couche d'app passerait pour orpheline :
/// une app ne vit pas dans `mods`.
pub fn orphan_layers(conn: &Connection) -> rusqlite::Result<Vec<LayerRow>> {
    let mut stmt = conn.prepare(&format!(
        "{LAYER_SELECT}
          WHERE parent_id NOT IN (SELECT id_interne FROM mods)
            AND parent_id NOT IN (SELECT id FROM apps)
          ORDER BY parent_id, priority"
    ))?;
    let rows = stmt.query_map([], map_layer)?;
    rows.collect()
}
