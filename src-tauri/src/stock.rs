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

use std::path::Path;

use chrono::Local;
use rusqlite::Connection;
use uuid::Uuid;

use crate::config::AppConfig;
use crate::modscan::ModKind;
use crate::rules::Rules;
use crate::{harmonize, inspect, kunos_dates, overlay, uijson};

/// Faut-il (ré)indexer le contenu de base après un enregistrement de config ?
///
/// Extrait de `commands::config::save_config` pour être testable : la règle
/// tient en deux cas, mais les avoir laissés implicites a coûté un bug de
/// premier démarrage (bibliothèque vide jusqu'au lancement suivant, parce que
/// le seul scan existant tournait au `setup` de l'app — avant que l'assistant
/// n'ait écrit le moindre chemin).
///
/// - le dossier du jeu **change** : l'index décrit l'ancienne install ;
/// - rien n'est indexé alors qu'un dossier est désigné : c'est le premier
///   démarrage, ou un scan qui a échoué.
pub fn needs_reindex(previous: Option<&Path>, current: Option<&Path>, indexed: usize) -> bool {
    match current {
        None => false,
        Some(cur) => previous != Some(cur) || indexed == 0,
    }
}

/// (Re)construit l'index du contenu de base. Renvoie le nombre d'entrées indexées.
///
/// `reset_user_edits` : repartir **vraiment** de zéro, en effaçant aussi ce que
/// l'utilisateur a saisi sur ce contenu (nom repris à la main, description,
/// tags manuels, favori, catégorie). C'était le comportement inconditionnel
/// jusqu'ici, et il perdait ces saisies sans le dire — un utilisateur ayant
/// renommé « Mugello » en « Autodromo del Mugello » retrouvait « Mugello »
/// après un simple réindex (bug réel signalé). Réservé désormais à une demande
/// explicite, confirmée à l'écran (§9.3bis).
pub fn index_stock_content(
    conn: &Connection,
    cfg: &AppConfig,
    rules: &Rules,
    reset_user_edits: bool,
) -> Result<usize, String> {
    let ac = cfg.ac_install_path.as_ref().ok_or(crate::errors::AC_NOT_CONFIGURED)?;
    let now = Local::now().to_rfc3339();

    if reset_user_edits {
        // Repart de zéro : corrige aussi les entrées indexées avant une bonne
        // config, au prix des saisies de l'utilisateur.
        overlay::clear_stock(conn).map_err(|e| e.to_string())?;
    } else {
        // Seules les versions synthétiques repartent : elles se refabriquent
        // avec un nouvel UUID à chaque passage et s'accumuleraient sinon. Les
        // lignes `mods`, qui portent l'overlay, survivent telles quelles.
        overlay::clear_stock_versions(conn).map_err(|e| e.to_string())?;
    }

    // Ids réellement trouvés sur disque : ce qui n'y est plus est retiré en fin
    // de parcours (sans le `DELETE` global, rien ne ferait disparaître un
    // contenu désinstallé du jeu).
    let mut present: Vec<String> = Vec::new();
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
            let id = e.file_name().to_string_lossy().into_owned();

            // Dossier actuellement composé (§4.4 : symlink hérité ou hardlinks,
            // §2) : contenu de base + couche(s) superposées. Lire depuis la
            // sauvegarde intacte (`stock_base/`), jamais depuis le composé —
            // sinon les fichiers/layouts apportés par une couche seraient mis
            // en cache comme s'ils étaient d'origine Kunos, et resteraient
            // affichés même après désactivation de la couche (le composé change
            // sur disque, ce cache non). Sans sauvegarde, c'est un mod géré actif
            // (pas du contenu de base) : jamais touché.
            let composed = crate::activation::is_junction(&p) || crate::deploy::is_deployed(&p);
            let source = if composed {
                match cfg
                    .library_path
                    .as_ref()
                    .map(|lib| crate::compose::stock_base_dir(lib, kind, &id))
                {
                    Some(backup) if backup.is_dir() => backup,
                    _ => continue,
                }
            } else {
                p.clone()
            };
            // Ne jamais toucher un vrai mod (non-stock) installé dans content/.
            if let Ok(Some(m)) = overlay::get_mod(conn, &id) {
                if !m.is_stock {
                    continue;
                }
            }

            let ui = match kind {
                ModKind::Car => uijson::read_car(&source),
                ModKind::Track => uijson::read_track(&source),
            }
            .unwrap_or_default();
            let name = ui.name.clone().unwrap_or_else(|| id.clone());

            overlay::upsert_stock_mod(conn, &id, &format!("{kind:?}"), ui.brand.as_deref(), Some(&name), &now)
                .map_err(|e| e.to_string())?;

            // Année du modèle (§6.2bis) : ui_car.json si renseigné, sinon la
            // table statique docs/kunos_content_dates.json (mods importés
            // n'ont pas ce repli, seul le contenu de base est concerné).
            let year = if matches!(kind, ModKind::Car) {
                ui.year.or_else(|| kunos_dates::car_year(&id))
            } else {
                None
            };
            overlay::update_mod_reindexed_fields(conn, &id, None, None, year).map_err(|e| e.to_string())?;

            // Version synthétique : `library_path` pointe toujours sur le vrai
            // dossier `content/` (là où AC/CM lisent réellement), même si les
            // champs ci-dessus sont lus depuis la sauvegarde quand composé.
            let vid = Uuid::new_v4().to_string();
            let csp = inspect::csp_features(&source);
            let skins = if matches!(kind, ModKind::Car) {
                inspect::car_skins(&source)
            } else {
                Vec::new()
            };
            let layouts = if matches!(kind, ModKind::Track) {
                inspect::track_layouts(&source)
            } else {
                Vec::new()
            };
            let author = ui.author.clone().or_else(|| Some("Kunos".to_string()));
            let published_at = kunos_dates::release_date(kind, &id);
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
                published_at.as_deref(),
            )
            .map_err(|e| e.to_string())?;
            overlay::set_active_version(conn, &id, &vid).map_err(|e| e.to_string())?;

            // Harmonisation (tags règle, catégorie, classe, specs dérivées, pays).
            let class = ui.class.clone().unwrap_or_default();
            let h = harmonize::compute(rules, kind, &ui.tags, &name, &class, ui.country.as_deref());
            harmonize::store(conn, &id, &h, ui.country.as_deref()).map_err(|e| e.to_string())?;

            present.push(id);
            count += 1;
        }
    }
    if !reset_user_edits {
        overlay::delete_stock_absent(conn, &present).map_err(|e| e.to_string())?;
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Arbo AC minimale : un circuit de contenu de base avec son `ui_track.json`.
    fn fake_stock_track(ac: &Path, id: &str, name: &str) {
        let dir = ac.join("content").join("tracks").join(id).join("ui");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("ui_track.json"), format!("{{\"name\":\"{name}\"}}")).unwrap();
    }

    /// Règle : un réindex ordinaire ne détruit RIEN de ce que l'utilisateur a
    /// saisi sur le contenu de base (§9.3bis). Bug réel signalé : « Mugello »
    /// renommé « Autodromo del Mugello » redevenait « Mugello » au premier
    /// réindex, parce que l'indexation repartait d'un `DELETE` global.
    #[test]
    fn plain_reindex_keeps_user_edits_on_stock_content() {
        let base = crate::testutil::temp_dir("stock-keep");
        let ac = base.join("ac");
        fake_stock_track(&ac, "mugello", "Mugello");
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            ac_install_path: Some(ac.clone()),
            ..Default::default()
        };
        let rules = Rules::default();

        assert_eq!(index_stock_content(&conn, &cfg, &rules, false).unwrap(), 1);
        overlay::set_mod_field(&conn, "mugello", "display_name_user", Some("Autodromo del Mugello")).unwrap();
        overlay::set_manual_tags(&conn, "mugello", &["mon-tag".to_string()]).unwrap();
        overlay::set_favorite(&conn, "mugello", true).unwrap();

        index_stock_content(&conn, &cfg, &rules, false).unwrap();

        let m = overlay::get_mod(&conn, "mugello").unwrap().unwrap();
        assert_eq!(
            m.display_name.as_deref(),
            Some("Autodromo del Mugello"),
            "le renommage tient"
        );
        assert_eq!(m.tags_manual, vec!["mon-tag".to_string()], "les tags manuels tiennent");
        assert!(m.is_favorite, "le favori tient");
        // Les versions synthétiques ne s'accumulent pas d'un passage à l'autre.
        assert_eq!(
            overlay::get_versions(&conn, "mugello").unwrap().len(),
            1,
            "une seule version"
        );
        assert!(m.active_version_id.is_some(), "et elle est bien la version active");
    }

    /// Règle : la réinitialisation explicite, elle, efface bien ces saisies —
    /// c'est toute sa raison d'être, et c'est pourquoi l'écran la fait
    /// confirmer.
    #[test]
    fn reset_reindex_clears_user_edits() {
        let base = crate::testutil::temp_dir("stock-reset");
        let ac = base.join("ac");
        fake_stock_track(&ac, "mugello", "Mugello");
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            ac_install_path: Some(ac.clone()),
            ..Default::default()
        };
        let rules = Rules::default();

        index_stock_content(&conn, &cfg, &rules, false).unwrap();
        overlay::set_mod_field(&conn, "mugello", "display_name_user", Some("Autodromo del Mugello")).unwrap();

        index_stock_content(&conn, &cfg, &rules, true).unwrap();

        let m = overlay::get_mod(&conn, "mugello").unwrap().unwrap();
        assert_eq!(m.display_name.as_deref(), Some("Mugello"), "retour au nom du fichier");
        assert!(m.display_name_user.is_none(), "la saisie est bien effacée");
    }

    /// Règle : ne plus tout supprimer ne doit pas laisser traîner du contenu
    /// désinstallé du jeu — c'est ce que le `DELETE` global faisait
    /// gratuitement, et qu'il faut remplacer explicitement.
    #[test]
    fn plain_reindex_drops_content_removed_from_the_game() {
        let base = crate::testutil::temp_dir("stock-gone");
        let ac = base.join("ac");
        fake_stock_track(&ac, "mugello", "Mugello");
        fake_stock_track(&ac, "spa", "Spa");
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            ac_install_path: Some(ac.clone()),
            ..Default::default()
        };
        let rules = Rules::default();

        assert_eq!(index_stock_content(&conn, &cfg, &rules, false).unwrap(), 2);
        std::fs::remove_dir_all(ac.join("content").join("tracks").join("spa")).unwrap();
        assert_eq!(index_stock_content(&conn, &cfg, &rules, false).unwrap(), 1);

        assert!(
            overlay::get_mod(&conn, "mugello").unwrap().is_some(),
            "toujours installé"
        );
        assert!(
            overlay::get_mod(&conn, "spa").unwrap().is_none(),
            "désinstallé du jeu, retiré de l'index"
        );
    }

    #[test]
    fn reindex_when_the_game_folder_is_set_or_changes() {
        use super::needs_reindex;
        use std::path::Path;
        let a = Path::new("D:/ac");
        let b = Path::new("E:/ac");

        // Premier démarrage : un dossier vient d'être désigné, rien n'est
        // indexé. C'est le cas qui échouait — l'assistant écrivait les chemins
        // et la bibliothèque restait vide jusqu'au lancement suivant.
        assert!(needs_reindex(None, Some(a), 0), "premier démarrage");
        // Changement de dossier de jeu : l'index décrit l'ancienne install.
        assert!(needs_reindex(Some(a), Some(b), 200), "dossier du jeu changé");
        // Scan précédemment échoué : rien en base, on retente.
        assert!(needs_reindex(Some(a), Some(a), 0), "index vide, on retente");

        // Cas courant : on enregistre d'autres réglages, l'index est bon.
        assert!(!needs_reindex(Some(a), Some(a), 200), "aucun changement utile");
        // Pas de dossier de jeu : rien à indexer, et surtout pas d'erreur.
        assert!(!needs_reindex(Some(a), None, 200), "jeu non configuré");
        assert!(!needs_reindex(None, None, 0));
    }

    #[test]
    fn reindex_fills_year_and_release_from_kunos_table() {
        // "abarth500" (voiture) et "imola" (circuit) sont référencés dans
        // docs/kunos_content_dates.json ; dossiers volontairement sans
        // ui_car.json/ui_track.json pour vérifier le repli sur la table.
        let base = crate::testutil::temp_dir("stockdates");
        let ac = base.join("ac");
        std::fs::create_dir_all(ac.join("content").join("cars").join("abarth500")).unwrap();
        std::fs::create_dir_all(ac.join("content").join("tracks").join("imola")).unwrap();

        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            ac_install_path: Some(ac.clone()),
            ..Default::default()
        };
        index_stock_content(&conn, &cfg, &Rules::default(), false).unwrap();

        let car = overlay::get_mod(&conn, "abarth500").unwrap().unwrap();
        assert_eq!(car.year, Some(2007), "année reprise de la table faute de ui_car.json");
        assert_eq!(car.published_at.as_deref(), Some("2014-12-19"));

        let track = overlay::get_mod(&conn, "imola").unwrap().unwrap();
        assert_eq!(track.year, None, "pas de notion d'année pour un circuit");
        assert_eq!(track.published_at.as_deref(), Some("2014-12-19"));
    }

    #[test]
    fn reindex_composed_stock_track_indexes_from_backup_not_layer() {
        // §2/§4.4 : un circuit de base actuellement composé (couche active,
        // déploiement hardlinks marqué) ne doit ni disparaître de l'overlay ni
        // être indexé depuis le composé (qui contient les fichiers de la
        // couche) — toujours depuis la sauvegarde intacte (`stock_base/`).
        let base = crate::testutil::temp_dir("stock-composed");
        let ac = base.join("ac");
        let library = base.join("library");
        let link = ac.join("content").join("tracks").join("spa");
        std::fs::create_dir_all(link.join("ui")).unwrap();
        std::fs::write(link.join("ui").join("ui_track.json"), br#"{"name":"Spa"}"#).unwrap();

        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            ac_install_path: Some(ac.clone()),
            library_path: Some(library.clone()),
            ..Default::default()
        };
        index_stock_content(&conn, &cfg, &Rules::default(), false).unwrap();
        assert!(overlay::get_mod(&conn, "spa").unwrap().is_some());

        // Couche qui ajoute un layout "2022" -> compose "spa" en hardlinks.
        let layerdir = library.join("layers").join("spa").join("ext");
        std::fs::create_dir_all(layerdir.join("ui").join("2022")).unwrap();
        std::fs::write(layerdir.join("ui").join("2022").join("ui_track.json"), "{}").unwrap();
        let lid = uuid::Uuid::new_v4().to_string();
        let prio = overlay::next_layer_priority(&conn, "spa").unwrap();
        overlay::insert_layer(
            &conn,
            &lid,
            "spa",
            "Track",
            "ext",
            &layerdir.to_string_lossy(),
            None,
            0,
            0,
            prio,
            "now",
        )
        .unwrap();
        crate::compose::recompose(&conn, &cfg, "spa").unwrap();
        assert!(
            crate::deploy::is_deployed(&link),
            "précondition : spa composé par hardlinks"
        );

        // Ré-indexer le contenu de base PENDANT que la couche est active.
        index_stock_content(&conn, &cfg, &Rules::default(), false).unwrap();

        let m = overlay::get_mod(&conn, "spa").unwrap();
        assert!(
            m.is_some(),
            "le circuit ne doit pas disparaître de l'overlay pendant qu'il est composé"
        );
        assert!(m.unwrap().is_stock);
        let versions = overlay::get_versions(&conn, "spa").unwrap();
        assert_eq!(versions.len(), 1);
        assert!(
            !versions[0].layouts.iter().any(|l| l == "2022"),
            "indexé depuis la sauvegarde intacte, pas depuis le composé — le layout de couche n'est pas mis en cache comme s'il était Kunos"
        );
    }
}
