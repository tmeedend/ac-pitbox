//! Apps Python ou Lua/CSP d'AC (§12bis.4) : type **autonome** (ni voiture, ni
//! circuit, ni sous-élément). Stockées dans la bibliothèque, activables/
//! désactivables par junction comme le reste, vers `<ac>/apps/python/<id>` ou
//! `<ac>/apps/lua/<id>` selon le langage détecté (`app_lang`). Pas de fiche ni
//! de tags en v1 — juste nom, état, activation, ressources annexes (§4.5.2).

use std::path::{Path, PathBuf};

use chrono::Local;
use rusqlite::Connection;
use serde::Serialize;

use crate::config::AppConfig;
use crate::modscan::FoundApp;
use crate::resources::{self, ExtractionMode};
use crate::{activation, overlay};

#[derive(Debug, Clone, Serialize)]
pub struct AppImported {
    pub name: String,
    /// Fichiers annexes redirigés vers le dossier ressources (§4.5.2).
    pub resources_extracted: usize,
}

/// App avec son état d'activation (junction présente) pour la vue dédiée.
#[derive(Debug, Clone, Serialize)]
pub struct AppItem {
    pub id: String,
    pub source_archive: Option<String>,
    pub imported_at: String,
    pub active: bool,
    /// "python" | "lua" — déduit des fichiers stockés ([`app_lang`]), pas d'une
    /// colonne. Affiché sur la fiche : c'est ce qui dit si l'app suit la
    /// convention historique d'AC ou celle de CSP, et donc où elle est posée.
    pub lang: String,
}

/// Sous-dossier `apps/<langue>/` où pointe la junction d'activation d'une app
/// (§12bis.4) : `lua` si les fichiers stockés incluent un script `<id>.lua`
/// (convention CSP), sinon `python` (convention historique `<id>.py`, aussi
/// le repli si aucun des deux n'est trouvé). Déduit des fichiers réellement
/// stockés plutôt que d'une colonne overlay dédiée — pas de migration de
/// schéma, et toujours juste même si le contenu de l'app change entre deux
/// réimports.
pub(crate) fn app_lang(stored_dir: &Path, id: &str) -> &'static str {
    if stored_dir.join(format!("{id}.lua")).is_file() {
        "lua"
    } else {
        "python"
    }
}

/// Lien d'activation d'une app : `<ac>/apps/<lang>/<id>`.
pub(crate) fn app_link(cfg: &AppConfig, id: &str, lang: &str) -> Option<PathBuf> {
    cfg.ac_install_path
        .as_ref()
        .map(|ac| ac.join("apps").join(lang).join(id))
}

/// Importe les apps détectées : stockage bibliothèque + enregistrement (§12bis.4).
pub fn import_apps(
    conn: &Connection,
    library: &Path,
    source_name: &str,
    apps: &[FoundApp],
    copy: bool,
    mode: ExtractionMode,
) -> Vec<AppImported> {
    let mut out = Vec::new();
    for app in apps {
        let dest = library.join("apps").join(&app.name);
        // Ré-import : on remplace les fichiers existants (les ressources déjà
        // extraites, elles, sont conservées — dossier séparé, mod-level).
        if dest.exists() {
            let _ = std::fs::remove_dir_all(&dest);
        }
        // Fichiers annexes (§4.5.2) redirigés à part : une image à la racine
        // d'une app peut être une icône réellement utilisée par le script
        // (allow_root_images=false, jamais présumée annexe).
        let res_dir = resources::resources_dir_for(library, "apps", &[&app.name]);
        let Ok(resources_extracted) =
            resources::file_mod(&app.dir, &dest, &res_dir, mode, !copy, resources::Source::ModFolder)
        else {
            continue;
        };
        let _ = overlay::insert_app(
            conn,
            &app.name,
            &crate::libpath::to_relative(Some(library), &dest),
            Some(source_name),
            &Local::now().to_rfc3339(),
        );
        out.push(AppImported {
            name: app.name.clone(),
            resources_extracted,
        });
    }
    out
}

/// Dossier bibliothèque d'une app, pour l'ouvrir dans l'explorateur.
pub fn app_folder_path(conn: &Connection, cfg: &AppConfig, id: &str) -> Result<PathBuf, String> {
    let app = overlay::get_app(conn, id)
        .map_err(|e| e.to_string())?
        .ok_or(crate::errors::APP_NOT_FOUND)?;
    crate::libpath::resolve(cfg.library_path.as_deref(), &app.library_path)
        .ok_or_else(|| crate::errors::LIBRARY_NOT_CONFIGURED.to_string())
}

/// Liste les apps avec leur état d'activation (junction présente).
pub fn list_apps(conn: &Connection, cfg: &AppConfig) -> Result<Vec<AppItem>, String> {
    let rows = overlay::list_apps(conn).map_err(|e| e.to_string())?;
    Ok(rows
        .into_iter()
        .map(|a| {
            let stored = crate::libpath::resolve(cfg.library_path.as_deref(), &a.library_path);
            let lang = stored.as_deref().map(|d| app_lang(d, &a.id)).unwrap_or("python");
            // Même définition d'« active » que `is_app_active` : junction pour
            // une app nue, arbre composé dès qu'une couche l'est (§12bis.4).
            // La dupliquer ici affichait « inactive » une app pourtant posée.
            let active = is_app_active(cfg, &a.id);
            AppItem {
                id: a.id,
                source_archive: a.source_archive,
                imported_at: a.imported_at,
                active,
                lang: lang.to_string(),
            }
        })
        .collect())
}

/// Active une app dans `<ac>/apps/<lang>/<id>` : junction vers le dossier
/// bibliothèque quand elle est nue, arbre composé par hardlinks dès qu'une
/// couche est active (§12bis.4, même règle qu'au §2 pour les mods).
pub fn activate_app(conn: &Connection, cfg: &AppConfig, id: &str) -> Result<(), String> {
    let app = overlay::get_app(conn, id)
        .map_err(|e| e.to_string())?
        .ok_or(crate::errors::APP_NOT_FOUND)?;
    let target = crate::libpath::resolve(cfg.library_path.as_deref(), &app.library_path)
        .ok_or(crate::errors::LIBRARY_NOT_CONFIGURED)?;
    let lang = app_lang(&target, id);
    let link = app_link(cfg, id, lang).ok_or(crate::errors::AC_NOT_CONFIGURED)?;

    // Garde-fou : ne jamais écraser un vrai dossier (app installée hors de
    // l'app). Un déploiement à nous — junction ou arbre composé marqué — se
    // retire, lui, sans autre forme de procès : c'est nous qui l'avons posé.
    if link.exists() {
        if activation::is_junction(&link) {
            activation::remove_junction(&link)?;
        } else if crate::deploy::is_deployed(&link) {
            crate::deploy::remove_deployment(&link)?;
        } else {
            return Err(crate::errors::REAL_APP_FOLDER_EXISTS.into());
        }
    }
    if let Some(parent) = link.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    // Junction d'abord, systématiquement : c'est ce qui rend l'app « active »
    // (`is_app_active`), donc ce que `recompose` exige avant de projeter quoi
    // que ce soit. Il la remplacera lui-même par un arbre composé s'il y a des
    // couches.
    activation::create_junction(&link, &target)?;
    if let Err(e) = crate::compose::recompose(conn, cfg, id) {
        log::warn!("recompose app {id}: {e}");
    }
    // Ajouts au jeu (§4.5.3) : une app en a autant qu'une voiture — configs
    // CSP, textures, fichiers de `cfg/` livrés à côté de son dossier. Même
    // best-effort qu'ailleurs : un ajout non posé ne doit pas empêcher l'app
    // de tourner, mais laisse une trace.
    if let Err(e) = crate::extras::deploy(conn, cfg, crate::extras::OwnerKind::App, id) {
        log::warn!("deploy_extras app {id}: {e}");
    }
    Ok(())
}

/// L'app est-elle posée dans AC ? Les deux emplacements sont testés : le
/// langage se déduit des fichiers stockés ([`app_lang`]), et un appelant qui
/// n'a pas la bibliothèque sous la main n'a pas à le savoir.
///
/// **Les deux formes de déploiement comptent** (§12bis.4) : junction pour une
/// app nue, arbre composé marqué dès qu'une couche est active. Ne tester que la
/// junction ferait passer pour inactive toute app à couche — et `recompose`,
/// qui s'appuie là-dessus, refuserait alors de la reprojeter.
pub fn is_app_active(cfg: &AppConfig, id: &str) -> bool {
    let Some(ac) = cfg.ac_install_path.as_ref() else {
        return false;
    };
    ["python", "lua"].iter().any(|lang| {
        let link = ac.join("apps").join(lang).join(id);
        activation::is_junction(&link) || crate::deploy::is_deployed(&link)
    })
}

/// Désactive une app : retire ce que nous avons posé dans `apps/python/` ou
/// `apps/lua/`, selon celui des deux qui est effectivement occupé — pas besoin
/// de rouvrir la bibliothèque pour deviner le langage à l'avance.
///
/// Les ajouts au jeu partent **avant** : l'ordre n'a pas d'importance
/// fonctionnelle (les deux mécanismes sont indépendants), mais un retrait raté
/// ne doit pas laisser d'ajouts posés sans rien qui les réclame.
pub fn deactivate_app(conn: &Connection, cfg: &AppConfig, id: &str) -> Result<(), String> {
    if let Err(e) = crate::extras::undeploy(conn, cfg, id) {
        log::warn!("undeploy_extras app {id}: {e}");
    }
    let ac = cfg.ac_install_path.as_ref().ok_or(crate::errors::AC_NOT_CONFIGURED)?;
    for lang in ["python", "lua"] {
        let link = ac.join("apps").join(lang).join(id);
        if activation::is_junction(&link) {
            return activation::remove_junction(&link);
        }
        if crate::deploy::is_deployed(&link) {
            return crate::deploy::remove_deployment(&link);
        }
        if link.exists() {
            return Err(crate::errors::REAL_APP_FOLDER_UNTOUCHED.into());
        }
    }
    Ok(()) // déjà inactive dans les deux emplacements
}

/// Supprime proprement une app : désactive (retire la junction), efface les
/// fichiers de bibliothèque, puis la ligne overlay (§12bis.4).
pub fn remove_app(conn: &Connection, cfg: &AppConfig, id: &str) -> Result<(), String> {
    let app = overlay::get_app(conn, id)
        .map_err(|e| e.to_string())?
        .ok_or(crate::errors::APP_NOT_FOUND)?;
    // Désactive si une junction est présente, python ou lua (ignore l'absence
    // des deux, et un vrai dossier étranger — rien à faire dans ce cas ici,
    // seuls les fichiers de bibliothèque et l'overlay nous appartiennent).
    // Retire aussi ses ajouts au jeu d'AC (§4.5.3), avant d'effacer leur source
    // en bibliothèque : « l'ajout vit et meurt avec son mod ».
    let _ = deactivate_app(conn, cfg, id);
    if let Some(dir) = crate::libpath::resolve(cfg.library_path.as_deref(), &app.library_path) {
        let _ = std::fs::remove_dir_all(dir);
    }
    if let Some(lib) = &cfg.library_path {
        crate::extras::remove_tree(lib, crate::extras::OwnerKind::App, id);
    }
    overlay::delete_app(conn, id).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modscan;

    #[test]
    fn app_detected_and_imported() {
        let base = crate::testutil::temp_dir("app");
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();

        // App AC : apps/python/MyApp/MyApp.py
        let app = base.join("src").join("apps").join("python").join("MyApp");
        std::fs::create_dir_all(&app).unwrap();
        std::fs::write(app.join("MyApp.py"), b"# app").unwrap();

        let found = modscan::scan_apps(&base.join("src"));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "MyApp");

        let res = import_apps(&conn, &library, "myapp.7z", &found, true, ExtractionMode::InfoOnly);
        assert_eq!(res.len(), 1);
        assert!(library.join("apps").join("MyApp").join("MyApp.py").is_file());
        assert!(overlay::app_exists(&conn, "MyApp").unwrap());

        // Suppression propre : fichiers + overlay effacés (pas de junction ici).
        let cfg = AppConfig {
            library_path: Some(library.clone()),
            ..Default::default()
        };
        remove_app(&conn, &cfg, "MyApp").unwrap();
        assert!(!library.join("apps").join("MyApp").exists());
        assert!(!overlay::app_exists(&conn, "MyApp").unwrap());
    }

    #[test]
    fn lua_app_detected_and_activated_under_apps_lua_not_python() {
        // Bug réel : les apps Lua/CSP (`apps/lua/<App>/<App>.lua`, convention
        // aussi répandue que Python en pratique — HUD, réglages de voiture…)
        // n'étaient reconnues ni à l'import (`is_app` ne testait que `.py`),
        // ni correctement activées (toujours liées sous `apps/python/`, où AC
        // ne les charge jamais).
        let base = crate::testutil::temp_dir("app-lua");
        let library = base.join("library");
        let ac = base.join("ac");
        std::fs::create_dir_all(&library).unwrap();
        std::fs::create_dir_all(&ac).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            ac_install_path: Some(ac.clone()),
            library_path: Some(library.clone()),
            ..Default::default()
        };

        let app = base.join("src").join("apps").join("lua").join("MyLuaApp");
        std::fs::create_dir_all(&app).unwrap();
        std::fs::write(app.join("MyLuaApp.lua"), b"-- app").unwrap();

        let found = modscan::scan_apps(&base.join("src"));
        assert_eq!(found.len(), 1, "détecté malgré l'extension .lua");
        assert_eq!(found[0].name, "MyLuaApp");

        import_apps(&conn, &library, "myluaapp.7z", &found, true, ExtractionMode::InfoOnly);
        assert!(overlay::app_exists(&conn, "MyLuaApp").unwrap());

        activate_app(&conn, &cfg, "MyLuaApp").unwrap();
        assert!(
            activation::is_junction(&ac.join("apps").join("lua").join("MyLuaApp")),
            "junction posée sous apps/lua/, pas apps/python/"
        );
        assert!(!ac.join("apps").join("python").join("MyLuaApp").exists());

        deactivate_app(&conn, &cfg, "MyLuaApp").unwrap();
        assert!(
            !ac.join("apps").join("lua").join("MyLuaApp").exists(),
            "désactivée proprement"
        );
    }

    #[test]
    fn python_app_still_activates_under_apps_python() {
        // Non-régression : la convention historique reste le repli par défaut.
        let base = crate::testutil::temp_dir("app-python-activate");
        let library = base.join("library");
        let ac = base.join("ac");
        std::fs::create_dir_all(&library).unwrap();
        std::fs::create_dir_all(&ac).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            ac_install_path: Some(ac.clone()),
            library_path: Some(library.clone()),
            ..Default::default()
        };

        let app = base.join("src").join("apps").join("python").join("MyApp");
        std::fs::create_dir_all(&app).unwrap();
        std::fs::write(app.join("MyApp.py"), b"# app").unwrap();
        let found = modscan::scan_apps(&base.join("src"));
        import_apps(&conn, &library, "myapp.7z", &found, true, ExtractionMode::InfoOnly);

        activate_app(&conn, &cfg, "MyApp").unwrap();
        assert!(activation::is_junction(&ac.join("apps").join("python").join("MyApp")));

        deactivate_app(&conn, &cfg, "MyApp").unwrap();
        assert!(!ac.join("apps").join("python").join("MyApp").exists());
    }

    #[test]
    fn an_app_with_a_layer_is_composed_instead_of_junctioned() {
        // Règle (§12bis.4) : une app nue reste jonctionnée — c'est plus léger et
        // c'est ce qui existait — mais dès qu'une couche est active, elle bascule
        // en composition par hardlinks. Même règle qu'au §2 pour les mods, et
        // pour la même raison physique : une junction ne pointe que vers UNE
        // cible, elle ne sait rien fusionner.
        //
        // Cas réel à l'origine : une voiture et un circuit livrent des fichiers
        // dans le dossier d'une app (`RSS_Settings`, `CamTool_2`). Faute de
        // couches, ils étaient posés comme « ajouts au jeu » — donc écrits À
        // TRAVERS la junction, dans le dossier bibliothèque de l'app, qu'un
        // réimport de celle-ci effaçait.
        let base = crate::testutil::temp_dir("app-layer");
        let library = base.join("library");
        let ac = base.join("ac");
        std::fs::create_dir_all(&library).unwrap();
        std::fs::create_dir_all(&ac).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            ac_install_path: Some(ac.clone()),
            library_path: Some(library.clone()),
            ..Default::default()
        };

        let src = base.join("src").join("CamTool_2");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("CamTool_2.py"), b"# app").unwrap();
        let found = modscan::scan_apps(&base.join("src"));
        import_apps(&conn, &library, "camtool.7z", &found, true, ExtractionMode::InfoOnly);

        // App nue : junction, comme avant.
        activate_app(&conn, &cfg, "CamTool_2").unwrap();
        let link = ac.join("apps").join("python").join("CamTool_2");
        assert!(activation::is_junction(&link), "app nue : junction");

        // La couche qu'un circuit apporte : ses caméras, dans data/.
        let layer = base.join("layer");
        std::fs::create_dir_all(layer.join("data")).unwrap();
        std::fs::write(layer.join("data").join("gunma-1.json"), b"{}").unwrap();
        let (layer_id, _) = crate::layers::store_layer(
            &conn,
            &library,
            "CamTool_2",
            crate::layers::HostKind::App,
            "pk_gunma_cycle_sports_center",
            &layer,
            true,
            &crate::identity::DiffStats {
                added: 1,
                overwritten: 0,
                existing_total: 1,
            },
            "gunma.7z",
            ExtractionMode::InfoOnly,
        )
        .unwrap();

        crate::compose::recompose(&conn, &cfg, "CamTool_2").unwrap();
        assert!(
            !activation::is_junction(&link),
            "couche active : plus de junction, une junction ne fusionne pas"
        );
        assert!(crate::deploy::is_deployed(&link), "arbre composé marqué comme le nôtre");
        assert!(link.join("CamTool_2.py").is_file(), "la base de l'app est là");
        assert!(
            link.join("data").join("gunma-1.json").is_file(),
            "ce que la couche apporte est là aussi"
        );
        assert!(is_app_active(&cfg, "CamTool_2"), "un composé compte comme actif");

        // Retirer la couche rend l'app à son état nu, sans rien perdre.
        crate::compose::remove_layer(&conn, &cfg, &layer_id).unwrap();
        assert!(activation::is_junction(&link), "retour à la junction");
        assert!(
            !link.join("data").join("gunma-1.json").exists(),
            "la couche a bien disparu du jeu"
        );
        assert!(
            library.join("apps").join("CamTool_2").join("CamTool_2.py").is_file(),
            "la base en bibliothèque n'a jamais été touchée"
        );

        deactivate_app(&conn, &cfg, "CamTool_2").unwrap();
        assert!(!link.exists(), "désactivation propre");
    }
}
