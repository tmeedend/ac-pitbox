//! Couches / extensions (§4.4). Quand un import cible un contenu existant sans
//! être une mise à jour (surtout des chemins nouveaux, peu d'écrasements) — ou
//! qu'il vise un contenu de base Kunos (`is_stock`, jamais remplaçable) — le
//! contenu entrant est rangé **à part**, rattaché à la base, sans jamais toucher
//! la version active de celle-ci. Rien n'est détruit : « restaurer » = supprimer
//! la couche, la base est intacte dessous.
//!
//! Portée v1 : détection + rangement sûr. Le moteur de composition (fusionner
//! base + couches actives pour rendre l'extension visible en jeu) est un lot
//! ultérieur — la couche est stockée et listée, pas encore appliquée.

use std::path::Path;

use chrono::Local;
use rusqlite::Connection;
use uuid::Uuid;

use crate::identity::DiffStats;
use crate::modscan::ModKind;
use crate::resources::{self, ExtractionMode};
use crate::{importer, overlay};

/// Range un contenu entrant comme couche/extension rattachée à `parent_id`.
/// Ne modifie jamais la base. Fichiers annexes (§4.6) redirigés vers le
/// dossier ressources du mod, jamais dans la couche elle-même. Renvoie l'id de
/// couche créé et le nombre de fichiers annexes extraits.
#[allow(clippy::too_many_arguments)]
pub fn store_layer(
    conn: &Connection,
    library: &Path,
    parent_id: &str,
    kind: ModKind,
    name: &str,
    src_dir: &Path,
    copy: bool,
    diff: &DiffStats,
    archive_name: &str,
    mode: ExtractionMode,
) -> Result<(String, usize), String> {
    let dest = importer::unique_dir(&library.join("layers").join(parent_id).join(name));
    let res_dir = resources::resources_dir(library, kind, parent_id);
    let extracted = resources::file_mod(src_dir, &dest, &res_dir, mode, !copy, resources::Source::ModFolder)?;

    let now = Local::now().to_rfc3339();
    let id = Uuid::new_v4().to_string();
    let priority = overlay::next_layer_priority(conn, parent_id).map_err(|e| e.to_string())?;
    overlay::insert_layer(
        conn,
        &id,
        parent_id,
        &format!("{kind:?}"),
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
