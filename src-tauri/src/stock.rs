//! Indexation du contenu de base Kunos (§12bis.1). Référence les voitures et
//! circuits **présents dans `content/` comme vrais dossiers** (pas des junctions
//! gérées, pas déjà des mods) avec `is_stock=1` : lecture seule, non
//! désactivable. But : permettre aux sous-éléments (skins, sons) de s'y
//! rattacher, et afficher le contenu de base avec les mêmes métadonnées que les
//! mods (nom, marque, tags harmonisés, fiche technique, vignette).
//!
//! Chaque entrée reçoit une **version synthétique** pointant vers son dossier
//! `content/` (pour preview/tags/specs) et passe par la **même harmonisation**
//! que l'import. Auteur par défaut « Kunos ». Ré-indexation idempotente
//! (`clear_stock` puis reconstruction).

use chrono::Local;
use rusqlite::Connection;
use uuid::Uuid;

use crate::config::AppConfig;
use crate::modscan::ModKind;
use crate::rules::Rules;
use crate::{harmonize, inspect, overlay, uijson};

/// (Re)construit l'index du contenu de base. Renvoie le nombre d'entrées indexées.
pub fn index_stock_content(conn: &Connection, cfg: &AppConfig, rules: &Rules) -> Result<usize, String> {
    let ac = cfg.ac_install_path.as_ref().ok_or("dossier AC non configuré")?;
    let now = Local::now().to_rfc3339();

    // Repart de zéro : corrige aussi les entrées indexées avant une bonne config.
    overlay::clear_stock(conn).map_err(|e| e.to_string())?;

    let mut count = 0;
    for kind in [ModKind::Car, ModKind::Track] {
        let dir = ac.join("content").join(kind.content_folder());
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if !p.is_dir() || crate::activation::is_junction(&p) {
                continue; // junction = mod géré, pas du contenu de base
            }
            let id = e.file_name().to_string_lossy().into_owned();
            // Ne jamais toucher un vrai mod (non-stock) installé dans content/.
            if let Ok(Some(m)) = overlay::get_mod(conn, &id) {
                if !m.is_stock {
                    continue;
                }
            }

            let ui = match kind {
                ModKind::Car => uijson::read_car(&p),
                ModKind::Track => uijson::read_track(&p),
            }
            .unwrap_or_default();
            let name = ui.name.clone().unwrap_or_else(|| id.clone());

            overlay::upsert_stock_mod(conn, &id, &format!("{kind:?}"), ui.brand.as_deref(), Some(&name), &now)
                .map_err(|e| e.to_string())?;

            // Version synthétique pointant vers content/ : preview, tags fichier,
            // CSP, skins/layouts, auteur (Kunos par défaut).
            let vid = Uuid::new_v4().to_string();
            let csp = inspect::csp_features(&p);
            let skins = if matches!(kind, ModKind::Car) { inspect::car_skins(&p) } else { Vec::new() };
            let layouts = if matches!(kind, ModKind::Track) { inspect::track_layouts(&p) } else { Vec::new() };
            let author = ui.author.clone().or_else(|| Some("Kunos".to_string()));
            overlay::insert_version(
                conn,
                &vid,
                &id,
                ui.version.as_deref(),
                author.as_deref(),
                &now,
                &p.to_string_lossy(),
                None,
                "",
                &csp,
                &skins,
                &layouts,
                &ui.tags,
            )
            .map_err(|e| e.to_string())?;
            overlay::set_active_version(conn, &id, &vid).map_err(|e| e.to_string())?;

            // Harmonisation (tags règle, catégorie, classe, specs dérivées, pays).
            let class = ui.class.clone().unwrap_or_default();
            let h = harmonize::compute(rules, kind, &ui.tags, &name, &class, ui.country.as_deref());
            harmonize::store(conn, &id, &h, ui.country.as_deref()).map_err(|e| e.to_string())?;

            count += 1;
        }
    }
    Ok(count)
}
