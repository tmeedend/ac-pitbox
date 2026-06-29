//! Indexation du contenu de base Kunos (§12bis.1). L'app référence les voitures
//! et circuits **présents dans `content/` comme vrais dossiers** (pas des
//! junctions gérées, pas déjà des mods connus) avec un flag `is_stock` :
//! lecture seule, non désactivable, non supprimable. But : permettre aux
//! sous-éléments (skins, sons) de se rattacher aussi au contenu de base.

use chrono::Local;
use rusqlite::Connection;

use crate::config::AppConfig;
use crate::modscan::ModKind;
use crate::{activation, overlay, uijson};

/// Scanne `content/cars` et `content/tracks` et enregistre le contenu de base
/// non encore connu. Renvoie le nombre d'entrées indexées.
pub fn index_stock_content(conn: &Connection, cfg: &AppConfig) -> Result<usize, String> {
    let ac = cfg.ac_install_path.as_ref().ok_or("dossier AC non configuré")?;
    let now = Local::now().to_rfc3339();
    let mut count = 0;

    for kind in [ModKind::Car, ModKind::Track] {
        let dir = ac.join("content").join(kind.content_folder());
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if !p.is_dir() {
                continue;
            }
            // Une junction = un mod géré par l'app, pas du contenu de base.
            if activation::is_junction(&p) {
                continue;
            }
            let id = e.file_name().to_string_lossy().into_owned();
            // Déjà connu (mod ou stock déjà indexé) → on ne touche pas.
            if overlay::mod_exists(conn, &id).map_err(|e| e.to_string())? {
                continue;
            }
            let ui = match kind {
                ModKind::Car => uijson::read_car(&p),
                ModKind::Track => uijson::read_track(&p),
            }
            .unwrap_or_default();
            overlay::upsert_stock_mod(
                conn,
                &id,
                &format!("{kind:?}"),
                ui.brand.as_deref(),
                ui.name.as_deref().or(Some(&id)),
                &now,
            )
            .map_err(|e| e.to_string())?;
            count += 1;
        }
    }
    Ok(count)
}
