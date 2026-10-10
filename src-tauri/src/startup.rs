//! What the app does at startup, before its window answers anything — out of
//! `lib.rs`, which keeps the builder: plugins, protocol and command list.
//!
//! Four groups, run by `run`'s setup **in this order**, which is the one that
//! matters: the base is saved before anything opens it ([`open_base`]); what
//! an abnormal shutdown left behind is put right before anything reads it
//! ([`safety_nets`]); a base written by a previous version is brought up to
//! date ([`catch_up`]); and only then is it shared and are the background
//! services started ([`start_services`]). No test covers this order — the
//! startup bench (`npm run bench:startup`) times the steps, it does not check
//! their sequence: moving a step is a decision, not a tidy-up.

use rusqlite::Connection;
use tauri::{App, Manager};

use crate::config::AppConfig;
use crate::overlay::{self, Db};
use crate::{
    backup, brands, catalog_update, commands, config, cup, extras, fmod, gamebackup, gamestate, harmonize, music,
    nationalities, preview, rules, shadowdir, showcase, showroom, stock, timing, wiki,
};

/// Backs up the base and the preferences, then opens the base and checks it.
pub(crate) fn open_base(app: &App) -> Result<Connection, Box<dyn std::error::Error>> {
    // Sauvegarde de démarrage (§10), avant toute ouverture de
    // connexion : on veut la base et les préférences exactement
    // telles que la session précédente les a laissées.
    timing::step("setup.shadowdir", || {
        shadowdir::warn_at_startup(&app.config().identifier)
    });
    timing::step("setup.backup", || backup::run_startup_backup(app.handle()));

    let db_path = app.path().app_config_dir()?.join("overlay.sqlite");
    let conn = timing::step("setup.overlay_open", || overlay::open(&db_path))?;
    // Corruption shows up otherwise as scattered "malformed" warnings
    // from whichever startup pass happens to touch a damaged page —
    // and went unnoticed for three days in 2026-09. One line naming
    // the cause, before them.
    match timing::step("setup.quick_check", || overlay::quick_check(&conn)) {
        Ok(v) if v == "ok" => {}
        Ok(v) => log::warn!("overlay.sqlite failed its integrity check: {v}"),
        Err(e) => log::warn!("overlay.sqlite failed its integrity check: {e}"),
    }
    Ok(conn)
}

/// Puts right what an app killed halfway left behind.
pub(crate) fn safety_nets(app: &App, conn: &Connection) {
    // Filet de sécurité (§4.5.4) : un fichier du jeu remplacé par un mod
    // et que plus personne ne réclame redevient celui du jeu. Rattrape
    // une app tuée entre la sauvegarde et la pose, ou entre le retrait
    // et la restauration.
    timing::step("setup.restore_orphans", || gamebackup::restore_orphans(conn));

    // Filet de sécurité : le brouillon de conversion des vignettes de
    // grille (GRILLE§5.3) est effacé dès l'image rendue,
    // mais une fermeture brutale en laisse un — vingt mégaoctets que
    // rien d'autre ne ramasse, son dossier étant hors du plafond du
    // cache exprès.
    timing::step("setup.release_scratch", || preview::release_scratch(app.handle()));

    // Filet de sécurité : restaure video.ini si une
    // sauvegarde laissée par l'ancien aperçu 3D intégré traîne encore
    // (il forçait le mode fenêtré ; Pit Box n'y touche plus).
    timing::step("setup.video_ini", showroom::restore_orphaned_video_ini);

    // Safety net (ESPACE§5.5): a showcase removal stopped between its
    // manifest and the base is finished, from the manifest.
    match timing::step("setup.showcase_resume", || {
        showcase::resume_interrupted(conn, &config::load(app.handle()))
    }) {
        0 => {}
        n => log::warn!("finished {n} interrupted showcase removal(s)"),
    }
}

/// Brings a base written by a previous version up to date: shipped data that
/// may have grown, then the index, then the classification.
pub(crate) fn catch_up(app: &App, conn: &Connection, cfg: &AppConfig) {
    refresh_wiki(conn);
    catch_up_index(app, conn, cfg);
    catch_up_classification(app, conn, cfg);
}

fn refresh_wiki(conn: &Connection) {
    // Appariements Wikipédia livrés avec l'application (§10) : posés à
    // chaque démarrage parce que la table peut avoir grandi depuis la
    // version précédente. La précédence du WIKI§3.1 fait que c'est sans
    // risque — une correction locale (`manual`) n'est jamais écrasée.
    // Le cache d'articles est vidé quand il porte des textes écrits par
    // une version antérieure du code (§7.3 : l'introduction seule, puis
    // l'article entier). Rien dans une ligne ne le dirait, et sa date
    // de récupération est récente : sans ça, un article tronqué serait
    // servi trente jours de plus.
    match timing::step("setup.wiki_purge", || wiki::store::purge_outdated(conn)) {
        Ok(true) => log::warn!("wiki: cache d'articles vidé, son contenu datait d'une version antérieure"),
        Err(e) => log::warn!("wiki: purge du cache d'articles échouée — {e}"),
        _ => {}
    }

    let curated = wiki::curated::shipped();
    let (written, skipped) = timing::step("setup.wiki_seed", || wiki::curated::seed_links(conn, &curated));
    if written > 0 || skipped > 0 {
        log::debug!("wiki: {written} appariements livrés posés, {skipped} laissés en place");
    }
}

/// The index of what is installed: stock content, what is stock and what is
/// not, and what an older version stored in the wrong place.
fn catch_up_index(app: &App, conn: &Connection, cfg: &AppConfig) {
    // Contenu de base Kunos jamais indexé : scan auto, pour que les
    // skins/sons puissent s'y rattacher tout de suite (§8.1).
    // Ne couvre PAS le premier démarrage — la config n'existe pas
    // encore à ce moment-là, l'assistant ne l'écrit qu'après. C'est
    // `save_config` qui s'en charge, dès que le dossier du jeu est
    // désigné. Best-effort ici aussi, mais tracé.
    if cfg.ac_install_path.is_some() && overlay::count_stock(conn).unwrap_or(0) == 0 {
        let rules = rules::load(app.handle());
        if let Err(e) = timing::step("setup.index_stock", || {
            stock::index_stock_content(conn, cfg, &rules, false)
        }) {
            log::warn!("index_stock_content at startup: {e}");
        }
    }
    // Contenu de base vs mod installé hors Pit Box (§8.2) : une
    // base écrite avant cette distinction range les seconds avec les
    // premiers, ce qui autorise dessus des écritures qu'ils ne doivent
    // pas subir. Le scan complet ci-dessus ne se relancerait jamais
    // (il exige un index vide), donc cette passe — sans disque, une
    // comparaison par entrée — rattrape le classement à chaque
    // démarrage.
    match timing::step("setup.reclassify", || stock::reclassify_indexed_content(conn)) {
        Ok(n) if n > 0 => log::warn!("reclassified {n} indexed entries as unmanaged mods"),
        Err(e) => log::warn!("reclassify_indexed_content at startup: {e}"),
        _ => {}
    }

    // Reprise (§8.4) : les ajouts au jeu qui visaient l'intérieur du
    // dossier d'une app deviennent des couches de cette app. Rangés
    // avant que les couches d'app n'existent, ils créaient le dossier de
    // l'app en vrai dossier — ce qui bloquait ensuite son installation.
    // Idempotente par construction (plus aucun chemin `apps/<lang>/…`
    // ne subsiste après coup), donc sans drapeau à mémoriser.
    match timing::step("setup.app_extras", || extras::migrate_app_extras_to_layers(conn, cfg)) {
        0 => {}
        n => log::warn!("migrated {n} app extra tree(s) to app layers"),
    }

    // What a `ui_*.json` filled before its reading dropped the invisible
    // characters (`uijson::strip_invisible`) is cleaned once, here: a name is
    // otherwise only read again on a reindex.
    match timing::step("setup.strip_invisible", || {
        overlay::strip_invisible_from_stored_texts(conn)
    }) {
        Ok(0) => {}
        Ok(n) => log::warn!("stripped invisible characters from {n} stored text(s)"),
        Err(e) => log::warn!("strip_invisible_from_stored_texts at startup: {e}"),
    }
}

/// The classification: what it is computed against (nationalities, brand
/// spellings, the catalogue of rules), then the library harmonised again when
/// the engine changed.
fn catch_up_classification(app: &App, conn: &Connection, cfg: &AppConfig) {
    // The game's nationality table, for the country normalisation
    // (TAXO§7.1) run by every harmonisation below and after. Before the
    // catch-up, which would otherwise run without it.
    if let Some(root) = cfg.ac_install_path.as_deref() {
        timing::step("setup.nationalities", || {
            nationalities::set_known(nationalities::nationalities(std::path::Path::new(root)))
        });
    }

    // The brand spellings the library uses most, which case and accent
    // variants fold onto (TAXO§7.1) - elected before any harmonisation.
    timing::step("setup.brands", || brands::refresh_from(conn));

    let lib_ready = cfg.library_path.as_deref().is_some_and(|p| p.is_dir());

    // A new catalogue of rules shipped with this build (REGLES§6):
    // applied, the library re-classified under it, and a report kept
    // for the Workshop. Before anything reads the rules - it is also
    // what re-installs the previous catalogue of a user who went back.
    match app.path().app_config_dir() {
        Ok(dir) => timing::step("setup.catalog", || {
            catalog_update::on_startup(&dir, conn, cfg, lib_ready)
        }),
        Err(e) => log::warn!("catalogue update skipped: {e}"),
    }

    // L'harmonisation (§5) est calculée une fois à l'import puis
    // stockée : faire évoluer le moteur ne change donc rien à ce qui
    // est déjà en base, et personne ne devinerait qu'il faut rouvrir
    // l'écran Règles et réenregistrer pour la recalculer. Une passe au
    // démarrage, **une seule fois par version de moteur**, rattrape les
    // bases écrites par la précédente. Marqueur posé seulement en cas
    // de succès, et rien n'est tenté sans bibliothèque accessible :
    // sinon un disque externe non monté ferait passer un balayage à
    // vide pour un rattrapage fait, et la base resterait périmée pour
    // toujours.
    let engine = rules::ENGINE_VERSION.to_string();
    let stamped = overlay::get_meta(conn, overlay::META_ENGINE_VERSION).unwrap_or(None);
    if lib_ready && stamped.as_deref() != Some(engine.as_str()) {
        let rules = rules::load(app.handle());
        match timing::step("setup.harmonize", || harmonize::harmonize_all(conn, cfg, &rules)) {
            Ok(n) => {
                log::warn!("re-harmonized {n} mods for engine v{engine}");
                if let Err(e) = overlay::set_meta(conn, overlay::META_ENGINE_VERSION, &engine) {
                    log::warn!("stamping engine version failed, catch-up will run again: {e}");
                }
            }
            Err(e) => log::warn!("re-harmonize at startup: {e}"),
        }
    }
}

/// Shares the base with the commands, then registers the app's shared state
/// and starts what runs in the background for the app's whole life.
pub(crate) fn start_services(app: &App, conn: Connection) {
    app.manage(Db(std::sync::Mutex::new(conn)));
    // The tech sheet of every car never read (FICHE§9.3), once the
    // base is shared: in the background, the lock taken per car.
    commands::techsheet::spawn_backfill(app.handle());
    // Drapeau d'annulation d'un import en cours (§4.2bis).
    app.manage(commands::import::ImportControl::default());
    app.manage(commands::bulk_ops::BulkControl::default());
    // Mises à jour de mods (§4.7) : annulation du téléchargement en
    // cours, et ménage des téléchargements qu'un arrêt brutal a laissés
    // dans le dossier temporaire — au démarrage, aucun n'est en cours.
    app.manage(commands::updates::UpdateDownloadControl::default());
    // The game folder index of the session (DOSSIER§5.3): empty until
    // the screen asks for a first scan.
    app.manage(gamestate::Store::default());
    timing::step("setup.cup_sweep", || cup::sweep_leftovers(&std::env::temp_dir()));

    // Module musique du mode Big Picture (docs/spec-module-musique_2.md) :
    // dossiers par défaut créés au premier démarrage, peuplés du pack
    // embarqué (§16.1, `music/config.rs`), moteur audio + surveillance AC
    // démarrés pour toute la durée de vie de l'app.
    timing::step("setup.music_dirs", || music::config::ensure_default_dirs(app.handle()));
    let music_cfg = music::config::load(app.handle());
    // Préchauffe le cache d'index (MUSIQUE§3.4/§16.3) en tâche de fond dès
    // le démarrage, pour que la première navigation Big Picture de
    // la session ne subisse pas le scan complet du dossier.
    music::index::warm(app.handle(), music_cfg.clone());
    let track_handle = app.handle().clone();
    let music_engine = music::engine::spawn(app.handle().clone(), music_cfg, move |path| {
        commands::music::announce_track(&track_handle, path)
    });
    // Le même fil sert deux clients : la musique de Big Picture, et la
    // génération des vignettes de la grille, qui se suspend pendant une
    // session pour rendre la machine au jeu (GRILLE§5.4).
    // Un seul sondage de process pour les deux — le redécouvrir ailleurs
    // serait la même question posée deux fois.
    let watch_handle = app.handle().clone();
    // A third client: the layers an online session set aside come back
    // when the game closes — and at startup, the watch announcing its
    // first state (`online/session_layers.rs`).
    music::watch::spawn(music_engine.clone_sender(), move |running| {
        use tauri::Emitter;
        let _ = watch_handle.emit("ac://running", running);
        commands::online::on_game_running(&watch_handle, running);
    });
    app.manage(music_engine);
    app.manage(music::PreviewHandle::default());

    // Aperçu 3D des voitures : jeton de génération + créneau unique de
    // conversion (PREVIEW§7.3).
    app.manage(preview::PreviewState::default());
    // Thread propriétaire du système FMOD (§4.3). Rien n'est chargé
    // ici : les DLL du jeu ne sont touchées qu'à la première écoute,
    // donc une install sans Assetto Corsa ne paie rien.
    #[cfg(windows)]
    app.manage(fmod::engine::spawn());
}
