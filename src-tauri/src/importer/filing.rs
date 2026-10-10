//! Filing one mod found in an archive or a folder (§4.2–§4.4): what the
//! folder says of itself, whether it stands alone (§4.3bis), what it is to
//! the mod already there under its id — the same files, a mod of the
//! showcase coming back, an update, a layer, a question — and then the
//! library and the overlay.
//!
//! Out of `importer.rs`, where `process_found` had grown into one function of
//! 618 lines. **The order of the questions is the rule** (a duplicate is
//! recognised before a layer is laid, a fragment is never an update…), so the
//! orchestrator below reads as that order and each answer is a function of
//! its own. The comments are those of the original function, moved with the
//! code they explain.

use std::path::{Path, PathBuf};

use chrono::Local;
use rusqlite::Connection;
use uuid::Uuid;

use super::{
    classify_diff, incoming_name, layer_name, source_size, unique_dir, version_folder_name, ArchiveSource,
    FuzzyConflict, ImportClass, ImportedMod,
};
use crate::config::AppConfig;
use crate::fragment::Host;
use crate::identity::DiffStats;
use crate::modscan::{self, ModKind};
use crate::overlay::ModRow;
use crate::resources::ExtractionMode;
use crate::rules::Rules;
use crate::{harmonize, identity, inspect, layers, uijson};

#[allow(clippy::too_many_arguments)]
pub(super) fn process_found(
    conn: &Connection,
    cfg: &AppConfig,
    rules: &Rules,
    library: &Path,
    archive_name: &str,
    fm: &modscan::FoundMod,
    copy: bool,
    // Source de pack commune (§4.4) si l'import contient plusieurs mods.
    pack: Option<&str>,
    // Décision utilisateur pour un cas ambigu (§4.4) : "update" | "extension".
    decision: Option<&str>,
    // Import unitaire (true) : bloque et demande sur cas ambigu. Import en masse
    // (false) : jamais de blocage, un cas ambigu retombe sur le défaut sûr.
    block_ambiguous: bool,
    source: ArchiveSource<'_>,
    // Avancement du rangement de CE mod, dans [0,1] (§4.2bis). Une simple
    // fonction plutôt que le contexte de progression : `process_found` n'a pas
    // à connaître les bandes de la barre ni le rang de l'item dans le lot.
    on_progress: &dyn Fn(f64),
) -> Result<ImportedMod, String> {
    let filing = Filing {
        conn,
        cfg,
        rules,
        library,
        archive_name,
        fm,
        copy,
        pack,
        decision,
        block_ambiguous,
        source,
        on_progress,
    };
    let incoming = filing.read_incoming()?;
    if let Some(done) = filing.fragment_without_host(&incoming)? {
        return Ok(done);
    }
    // --- Résolution d'identité (§4.2/§4.4) ---
    let existing = crate::overlay::get_mod(conn, &incoming.id_interne).map_err(|e| e.to_string())?;
    if let Some(existing) = &existing {
        if let Some(done) = filing.over_existing(&incoming, existing)? {
            return Ok(done);
        }
    }
    filing.new_version(incoming, existing.is_some())
}

/// Everything `process_found` was handed, so that each step takes `&self`
/// rather than a dozen arguments again.
struct Filing<'a> {
    conn: &'a Connection,
    cfg: &'a AppConfig,
    rules: &'a Rules,
    library: &'a Path,
    archive_name: &'a str,
    fm: &'a modscan::FoundMod,
    copy: bool,
    pack: Option<&'a str>,
    decision: Option<&'a str>,
    block_ambiguous: bool,
    source: ArchiveSource<'a>,
    on_progress: &'a dyn Fn(f64),
}

/// What the incoming folder says of itself, read once, before any decision —
/// read-only, nothing is written until a step decides to.
struct Incoming {
    folder_name: String,
    host: Host,
    is_fragment: bool,
    id_interne: String,
    source_name: Option<String>,
    ui: uijson::UiInfo,
    kind_str: String,
    brand: String,
    name: String,
    res_mode: ExtractionMode,
    signature: String,
    id_hash: String,
    now: String,
    csp: Vec<String>,
    skins: Vec<String>,
    layouts: Vec<String>,
    published_at: Option<String>,
}

impl Incoming {
    /// The report of a step that stops here, under the mod's identity; the
    /// step fills in what it knows on top (`..incoming.report("…")`).
    fn report(&self, outcome: &str) -> ImportedMod {
        ImportedMod {
            id_interne: self.id_interne.clone(),
            kind: self.kind_str.clone(),
            display_name: Some(self.name.clone()),
            outcome: outcome.into(),
            version_label: self.ui.version.clone(),
            ..Default::default()
        }
    }

    /// Same, for a fragment that is not filed under its host: the report
    /// names the folder that came in.
    fn fragment_report(&self, outcome: &str) -> ImportedMod {
        ImportedMod {
            id_interne: self.folder_name.clone(),
            fragment: true,
            ..self.report(outcome)
        }
    }

    /// The counts of a comparison, as the report shows them.
    fn counted(&self, outcome: &str, diff: Option<DiffStats>) -> ImportedMod {
        ImportedMod {
            added_count: diff.map(|d| d.added),
            overwritten_count: diff.map(|d| d.overwritten),
            existing_total: diff.map(|d| d.existing_total),
            ..self.report(outcome)
        }
    }
}

impl Filing<'_> {
    fn read_incoming(&self) -> Result<Incoming, String> {
        let fm = self.fm;
        let folder_name = incoming_name(&fm.dir, self.archive_name);
        if folder_name.is_empty() {
            return Err(crate::errors::UNNAMED_MOD_FOLDER.into());
        }

        // --- Fragment ou mod ? (§4.3bis) ---
        //
        // L'identité d'un mod, c'était le nom de son dossier et rien d'autre. Un
        // dossier nommé d'après son auteur (`Mike08_santamonica01`) ne rencontrait
        // donc jamais l'arbitrage mise à jour / couche ci-dessous : il devenait un
        // circuit de plus, que le jeu ne peut pas charger faute de géométrie.
        // `fragment::resolve` répond d'abord à la vraie question — ce dossier
        // tient-il debout seul ? — avant celle de son identité.
        let host = crate::fragment::resolve(self.conn, self.cfg, fm.kind, &fm.dir);
        let is_fragment = host != Host::SelfStanding;
        // Un fragment rattaché prend l'identité de son hôte : c'est ce qui le fait
        // entrer dans le chemin « couche » plus bas, au lieu de créer une entrée.
        let id_interne = match &host {
            Host::Known(id) => id.clone(),
            _ => folder_name.clone(),
        };
        let source_name = is_fragment.then(|| folder_name.clone());

        let ui = match fm.kind {
            ModKind::Car => uijson::read_car(&fm.dir),
            ModKind::Track => uijson::read_track(&fm.dir),
        }
        .unwrap_or_default();

        let kind_str = format!("{:?}", fm.kind); // "Car" | "Track"
        let brand = ui.brand.clone().unwrap_or_default();
        // Circuit : racine commune de ses layouts (§5bis.3), pas le nom du premier
        // d'entre eux — `ui.name` vaut ici « Highlands Drift » pour un circuit qui
        // s'appelle « Highlands ».
        let name = match fm.kind {
            ModKind::Track => uijson::read_track_name(&fm.dir),
            ModKind::Car => ui.name.clone(),
        }
        .unwrap_or_else(|| id_interne.clone());
        // Extraction des fichiers annexes (§4.5.2) : réglage global, jamais reposé
        // à chaque import.
        let res_mode = ExtractionMode::parse(&self.cfg.prefs.resource_extraction_mode);
        let signature = identity::content_signature(&fm.dir);
        let id_hash = identity::identity_hash(&id_interne, &brand, &name);
        let now = Local::now().to_rfc3339();

        // Features lues à la volée (lecture seule).
        let csp = inspect::csp_features(&fm.dir);
        let skins = match fm.kind {
            ModKind::Car => inspect::car_skins(&fm.dir),
            ModKind::Track => Vec::new(),
        };
        let layouts = match fm.kind {
            ModKind::Track => inspect::track_layouts(&fm.dir),
            ModKind::Car => Vec::new(),
        };
        // Date de publication estimée (§6.2), lue sur les fichiers avant rangement.
        let published_at = inspect::estimate_published_at(&fm.dir);

        Ok(Incoming {
            folder_name,
            host,
            is_fragment,
            id_interne,
            source_name,
            ui,
            kind_str,
            brand,
            name,
            res_mode,
            signature,
            id_hash,
            now,
            csp,
            skins,
            layouts,
            published_at,
        })
    }

    /// A fragment whose host is not here: asked, parked, or let through to be
    /// imported as a mod (`None`).
    ///
    /// --- Fragment dont l'hôte n'est pas là (§4.3bis) ---
    ///
    /// Rien à composer : l'hôte n'existe pas. Poser le fragment comme un mod
    /// donnerait l'entrée fantôme qu'on cherche justement à éviter, alors on
    /// demande — et par défaut on propose de ne pas importer. En import de
    /// masse, où l'on ne peut pas demander, le défaut sûr est de **garder** :
    /// la couche est rangée sous l'id attendu, sans rien poser dans le jeu, et
    /// l'hôte la reprendra le jour où il arrivera (`compose::recompose` lit les
    /// couches par `parent_id`, que le mod existe ou non). Même parti que
    /// `submods::sounds::resolve_sound_parent` pour un son dont la voiture manque :
    /// ranger au bon endroit pour le jour où il y aura quelque chose dessous.
    fn fragment_without_host(&self, incoming: &Incoming) -> Result<Option<ImportedMod>, String> {
        match &incoming.host {
            Host::Missing(host_id) => {
                let park = match self.decision {
                    Some("standalone") => false,
                    Some("park") => true,
                    // Import unitaire : on ne décide pas à la place de l'utilisateur.
                    _ if self.block_ambiguous => {
                        return Ok(Some(ImportedMod {
                            host_id: Some(host_id.clone()),
                            ..incoming.fragment_report("HOST_MISSING")
                        }));
                    }
                    _ => true,
                };
                if park {
                    let (_, resources_extracted) = layers::store_layer(
                        self.conn,
                        self.library,
                        host_id,
                        self.fm.kind.into(),
                        &layer_name(self.fm, self.archive_name),
                        &self.fm.dir,
                        self.copy,
                        &DiffStats::default(),
                        self.archive_name,
                        incoming.res_mode,
                    )?;
                    return Ok(Some(ImportedMod {
                        resources_extracted,
                        host_id: Some(host_id.clone()),
                        ..incoming.fragment_report("PARKED")
                    }));
                }
                Ok(None)
            }
            // Aucun id à attendre : impossible de ranger la couche « quelque part ».
            // Il ne reste que le choix entre renoncer et importer tel quel —
            // c'est-à-dire l'ancien comportement, assumé cette fois.
            Host::Unknown if self.block_ambiguous && self.decision.is_none() => {
                Ok(Some(incoming.fragment_report("HOST_UNKNOWN")))
            }
            _ => Ok(None),
        }
    }

    /// Content aimed at an id already known: decide update vs layer/extension
    /// BEFORE acting (§4.4), so that a false "update" never destroys content.
    /// `None` = it is an update, filed as a new version by `new_version`.
    fn over_existing(&self, incoming: &Incoming, existing: &ModRow) -> Result<Option<ImportedMod>, String> {
        let (conn, cfg, fm) = (self.conn, self.cfg, self.fm);
        let id_interne = &incoming.id_interne;

        // The archive of a version in the showcase (ESPACE§7.3): its files
        // come back into that very version, instead of the duplicate the
        // signature would otherwise make of it — a skeleton keeps the
        // signature of its complete files, precisely for this.
        if let Some(v) = crate::overlay::version_by_signature(conn, id_interne, &incoming.signature)
            .map_err(|e| e.to_string())?
            .filter(|v| v.is_skeleton())
        {
            let back = crate::showcase::rehydrate(
                conn,
                cfg,
                self.library,
                fm.kind,
                &v,
                &fm.dir,
                !self.copy,
                incoming.res_mode,
                self.source,
                self.on_progress,
            )?;
            return Ok(Some(ImportedMod {
                version_label: v.version_label,
                resources_extracted: back.resources_extracted,
                missing_files: back.missing.len(),
                ..incoming.report("REHYDRATED")
            }));
        }

        // Ré-import à l'identique : même id ET même signature → ni version ni
        // couche (évite le faux « MAJ » quand on réimporte la même archive).
        let active_sig = crate::overlay::active_signature(conn, id_interne).map_err(|e| e.to_string())?;
        if active_sig.as_deref() == Some(incoming.signature.as_str()) {
            return Ok(Some(incoming.report("DUPLICATE")));
        }

        // Mod installé hors Pit Box (§8.2) : **rien n'est écrit**. Le
        // dossier de `content/` est à l'utilisateur, l'app ne l'a pas mis là.
        // Le classer en extension — ce que faisait la branche `is_stock`
        // ci-dessous, qui ramassait tout ce qui traînait dans `content/` —
        // revenait à sauvegarder puis **effacer** son vrai dossier pour
        // reconstruire un composé à la place, sans le lui dire. Le décompte
        // est calculé quand même : c'est ce qui permet au rapport de dire de
        // quoi il s'agit avant qu'il décide.
        if existing.is_unmanaged {
            return Ok(Some(incoming.counted("UNMANAGED", self.diff_against_game(id_interne))));
        }

        let (class, diff) = self.classify(id_interne, existing)?;

        // La décision explicite de l'utilisateur (§4.4) prime sur l'auto-classement.
        let resolved = match self.decision {
            Some("update") => ImportClass::Update,
            Some("extension") => ImportClass::Extension,
            _ => class,
        };
        // Sauf pour un fragment (§4.3bis) : **jamais** de mise à jour. Il n'a
        // pas de géométrie — remplacer la base par lui la rendrait injouable,
        // et c'est le seul cas où le décompte de fichiers peut mentir (un
        // fragment qui ne fait que retoucher des `ui/` recouvre proportionnellement
        // beaucoup d'un circuit qui en a peu). Même règle absolue que `is_stock`.
        let resolved = if incoming.is_fragment {
            ImportClass::Extension
        } else {
            resolved
        };

        match resolved {
            // Poursuit vers le chemin UPDATE_REPLACE (`new_version`).
            ImportClass::Update => Ok(None),
            ImportClass::Extension => self.as_layer(incoming, diff).map(Some),
            ImportClass::Ambiguous if self.block_ambiguous => {
                // Rien écrit : on attend le choix de l'utilisateur (§4.4).
                Ok(Some(incoming.counted("AMBIGUOUS", diff)))
            }
            ImportClass::Ambiguous => {
                // Import en masse : défaut sûr = extension (jamais destructif).
                let (_, resources_extracted) = self.store_layer(incoming, diff)?;
                let _ = crate::compose::recompose(conn, cfg, id_interne);
                Ok(Some(ImportedMod {
                    resources_extracted,
                    ..incoming.counted("EXTENSION", diff)
                }))
            }
        }
    }

    /// The incoming files compared with the base game's own folder for this
    /// id (`content/<type>s/<id>`), when there is one.
    fn diff_against_game(&self, id_interne: &str) -> Option<DiffStats> {
        self.cfg.ac_install_path.as_ref().and_then(|ac| {
            let base = ac.join("content").join(self.fm.kind.content_folder()).join(id_interne);
            base.is_dir().then(|| identity::diff_content(&self.fm.dir, &base))
        })
    }

    /// Update, layer or question, from the file counts — before the user's
    /// decision and the fragment rule, which the caller applies on top.
    fn classify(&self, id_interne: &str, existing: &ModRow) -> Result<(ImportClass, Option<DiffStats>), String> {
        // Règle absolue (§4.4) : le contenu de base Kunos (is_stock) ne reçoit
        // JAMAIS de remplacement — toujours une couche par-dessus. Sinon,
        // comparer les fichiers pour classer update / extension / ambigu.
        if existing.is_stock {
            // Toujours une extension (règle absolue). On calcule néanmoins le
            // décompte pour l'affichage, en comparant au dossier de base Kunos
            // (content/<type>s/<id>) : le stock n'a pas de version bibliothèque.
            return Ok((ImportClass::Extension, self.diff_against_game(id_interne)));
        }
        let active_dir = crate::overlay::active_library_path(self.conn, id_interne)
            .map_err(|e| e.to_string())?
            .and_then(|p| crate::libpath::resolve(self.cfg.library_path.as_deref(), &p));
        let diff = if existing.showcase {
            // A mod in the showcase (ESPACE§7.3, ESPACE§7.5): compared with what its
            // version held before its files went, as its manifest says — not
            // with its skeleton, next to which almost everything looks new. A
            // complete archive is then an update, the skeleton staying in the
            // timeline, and a layer (a new layout, a texture pack) stays a
            // layer.
            let original = active_dir
                .map(|dir| crate::skeleton::original_files(&dir))
                .unwrap_or_default();
            identity::diff_against(&self.fm.dir, &original)
        } else {
            match active_dir {
                Some(active_path) => identity::diff_content(&self.fm.dir, &active_path),
                // Pas de dossier de base à comparer : on ne peut pas prouver que
                // c'est une extension → comportement historique (mise à jour).
                None => DiffStats::default(),
            }
        };
        Ok((classify_diff(&diff), Some(diff)))
    }

    /// Lays the incoming files as a layer on the existing mod.
    ///
    /// Range comme couche à part — ne touche jamais la base (§4.4).
    ///
    /// **Une couche a une identité** : son parent et l'archive dont
    /// elle vient. Réimporter la même archive remplace donc la
    /// couche qu'elle avait posée, au lieu d'en empiler une seconde,
    /// identique et rigoureusement inutile.
    ///
    /// Bug réel signalé sur `spa2022-release_V1-03.rar`, un layout
    /// posé sur le circuit Kunos : deux imports de la même archive
    /// donnaient deux couches, et le décompte affiché sur la fiche
    /// trahissait la mécanique — « 109 ajoutés · 0 écrasés » pour la
    /// première (comparée au circuit Kunos nu), « 0 ajouté · 109
    /// écrasés » pour la seconde (comparée au circuit **déjà
    /// composé** avec la première).
    ///
    /// Remplacer plutôt qu'ignorer : une archive au même nom peut
    /// avoir été mise à jour, et c'est ce que fait déjà un mod
    /// réimporté (§4.3). La priorité de la couche est reprise, sans
    /// quoi elle repasserait en tête de pile à chaque réimport.
    fn as_layer(&self, incoming: &Incoming, diff: Option<DiffStats>) -> Result<ImportedMod, String> {
        let (conn, cfg, fm) = (self.conn, self.cfg, self.fm);
        let id_interne = &incoming.id_interne;
        // A layer that followed its host into the showcase comes back
        // into its own row (ESPACE§7.5): same id, name, notes, place
        // in the order and switch — never a second one next to it.
        if let Some(freed) = crate::overlay::list_layers(conn, id_interne, fm.kind.into())
            .unwrap_or_default()
            .into_iter()
            .find(|l| l.is_skeleton() && l.source_archive.as_deref() == Some(self.archive_name))
        {
            layers::refill_layer(
                conn,
                self.library,
                &freed,
                &fm.dir,
                diff.as_ref().unwrap_or(&DiffStats::default()),
                self.archive_name,
                incoming.res_mode,
                !self.copy,
            )?;
            if let Err(e) = crate::compose::recompose(conn, cfg, id_interne) {
                log::warn!("recompose {id_interne} after a layer came back: {e}");
            }
            return Ok(ImportedMod {
                fragment: incoming.is_fragment,
                source_name: incoming.source_name.clone(),
                // Not REHYDRATED: that one offers to activate the mod,
                // and a layer's host may well still be in the showcase.
                ..incoming.report("LAYER_REHYDRATED")
            });
        }
        let replaced_priority = crate::overlay::list_layers(conn, id_interne, fm.kind.into())
            .unwrap_or_default()
            .into_iter()
            .find(|l| l.source_archive.as_deref() == Some(self.archive_name))
            .map(|l| {
                let priority = l.priority;
                if let Err(e) = crate::compose::remove_layer(conn, cfg, &l.id) {
                    log::warn!("replace_layer {}: {e}", l.id);
                }
                priority
            });
        let (layer_id, resources_extracted) = self.store_layer(incoming, diff)?;
        if let Some(priority) = replaced_priority {
            let _ = crate::overlay::set_layer_priority(conn, &layer_id, priority);
        }
        // Couche active par défaut : composer tout de suite pour qu'elle
        // apparaisse en jeu (§4.4). Best-effort, comme auto_activate.
        let _ = crate::compose::recompose(conn, cfg, id_interne);
        // Une couche qui en remplace une autre est une mise à jour,
        // pas une nouvelle extension : la fiche doit le dire.
        let outcome = if replaced_priority.is_some() {
            "UPDATE_REPLACE"
        } else {
            "EXTENSION"
        };
        Ok(ImportedMod {
            resources_extracted,
            fragment: incoming.is_fragment,
            source_name: incoming.source_name.clone(),
            ..incoming.counted(outcome, diff)
        })
    }

    /// Stores the incoming files as a new layer of the existing mod, named
    /// after the incoming folder or the archive.
    fn store_layer(&self, incoming: &Incoming, diff: Option<DiffStats>) -> Result<(String, usize), String> {
        layers::store_layer(
            self.conn,
            self.library,
            &incoming.id_interne,
            self.fm.kind.into(),
            &layer_name(self.fm, self.archive_name),
            &self.fm.dir,
            self.copy,
            diff.as_ref().unwrap_or(&DiffStats::default()),
            self.archive_name,
            incoming.res_mode,
        )
    }

    /// Files the incoming folder as a new version — of a new mod, or of the
    /// existing one when it is an update — and writes the overlay.
    fn new_version(&self, incoming: Incoming, is_update: bool) -> Result<ImportedMod, String> {
        let conn = self.conn;
        let Incoming {
            id_interne,
            kind_str,
            name,
            ui,
            ..
        } = &incoming;

        let conflict = if is_update {
            None
        } else {
            crate::overlay::find_fuzzy(conn, kind_str, &incoming.brand, name, id_interne)
                .map_err(|e| e.to_string())?
                .into_iter()
                .next()
                .map(|m| FuzzyConflict {
                    existing_id: m.id_interne,
                    existing_name: m.display_name,
                })
        };
        let (dest, resources_extracted) = self.file_into_library(&incoming)?;
        self.record_version(&incoming, &dest)?;

        let outcome = if is_update { "UPDATE_REPLACE" } else { "IMPORT" };
        // Détails structurés (§ i18n) : rendus localisés côté front via `history.<key>`.
        let details = match (is_update, &ui.version) {
            (true, Some(v)) => serde_json::json!({ "key": "updated", "version": v }).to_string(),
            (true, None) => serde_json::json!({ "key": "updatedNoVersion" }).to_string(),
            (false, _) => serde_json::json!({ "key": "imported", "archive": self.archive_name }).to_string(),
        };
        crate::overlay::add_history(conn, id_interne, &incoming.now, outcome, &details).map_err(|e| e.to_string())?;

        Ok(ImportedMod {
            conflict,
            resources_extracted,
            // Un fragment qui arrive ici a été importé **comme mod** : hôte
            // introuvable et décision prise en ce sens (ou import de masse, où l'on
            // préfère un mod de trop à un contenu perdu). Le rapport doit le dire —
            // c'est la seule issue où l'entrée créée risque d'être injouable.
            fragment: incoming.is_fragment,
            ..incoming.report(outcome)
        })
    }

    /// --- Rangement bibliothèque ---
    ///
    /// Copies or moves the incoming folder into a version folder of its own,
    /// reporting progress as it goes; returns where it landed and how many
    /// ancillary files went to the mod's resources.
    fn file_into_library(&self, incoming: &Incoming) -> Result<(PathBuf, usize), String> {
        let (fm, library) = (self.fm, self.library);
        let (id_interne, ui) = (&incoming.id_interne, &incoming.ui);
        let version_folder = version_folder_name(ui.version.as_deref(), &incoming.now);
        let dest = unique_dir(
            &library
                .join(fm.kind.content_folder())
                .join(id_interne)
                .join(&version_folder),
        );
        // Copie (préserve la source) ou déplacement adaptatif (rename / copie+suppr).
        // Fichiers annexes (§4.5.2) redirigés vers le dossier ressources du mod,
        // jamais dans le contenu de jeu, selon le réglage global.
        let resources_dest = crate::resources::resources_dir(library, fm.kind, id_interne);
        // Taille mesurée avant le rangement : c'est le dénominateur de la
        // progression. Un parcours de métadonnées de plus, négligeable devant la
        // copie qu'il sert à commenter.
        let total_bytes = source_size(&fm.dir).max(1);
        let copied = std::cell::Cell::new(0u64);
        let resources_extracted = crate::resources::file_mod_reported(
            &fm.dir,
            &dest,
            &resources_dest,
            incoming.res_mode,
            !self.copy,
            crate::resources::Source::ModFolder,
            &|bytes| {
                copied.set(copied.get() + bytes);
                (self.on_progress)((copied.get() as f64 / total_bytes as f64).min(1.0));
            },
        )?;
        Ok((dest, resources_extracted))
    }

    /// --- Écriture overlay ---
    ///
    /// The mod, its new version made active, and what the files of that
    /// version say: tech sheet, tags, country.
    fn record_version(&self, incoming: &Incoming, dest: &Path) -> Result<(), String> {
        let (conn, fm) = (self.conn, self.fm);
        let Incoming {
            id_interne,
            kind_str,
            name,
            ui,
            ..
        } = incoming;
        let library_path = crate::libpath::to_relative(Some(self.library), dest);

        // Le journal (§4.6) décrit le **dernier** import de ce mod : réimporter un
        // mod corrigé doit effacer l'explication de l'import fautif, sinon la
        // fiche garderait indéfiniment à l'écran une décision qui n'a plus cours.
        // Effacé ici, avant le balayage des restes qui réenregistrera les nouvelles.
        crate::overlay::clear_decisions(conn, id_interne);
        crate::overlay::upsert_mod(
            conn,
            id_interne,
            kind_str,
            ui.brand.as_deref(),
            Some(name),
            &incoming.id_hash,
            ui.year,
            &incoming.now,
        )
        .map_err(|e| e.to_string())?;

        // Source de pack (§4.4) — uniquement quand l'import regroupe plusieurs mods.
        if self.pack.is_some() {
            crate::overlay::set_source(conn, id_interne, self.pack, None).map_err(|e| e.to_string())?;
        }

        let version_id = Uuid::new_v4().to_string();
        crate::overlay::insert_version(
            conn,
            &version_id,
            id_interne,
            ui.version.as_deref(),
            ui.author.as_deref(),
            &incoming.now,
            &library_path,
            Some(self.archive_name),
            &incoming.signature,
            &incoming.csp,
            &incoming.skins,
            &incoming.layouts,
            &ui.tags,
            incoming.published_at.as_deref(),
        )
        .map_err(|e| e.to_string())?;

        // Archive/dossier source conservé (§10/§11), s'il y en a un pour cet import.
        if let Some(kept) = self.source.kept {
            crate::overlay::set_kept_archive(conn, &version_id, kept).map_err(|e| e.to_string())?;
        }
        if let Some(origin) = self.source.origin {
            crate::overlay::set_version_origin(conn, &version_id, origin).map_err(|e| e.to_string())?;
        }

        // Taille sur disque (§10) : calculée maintenant, le dossier final venant
        // d'être créé par la copie/le déplacement ci-dessus.
        let size_bytes = inspect::dir_size_bytes(dest) as i64;
        crate::overlay::update_version_size(conn, &version_id, size_bytes).map_err(|e| e.to_string())?;

        crate::overlay::set_active_version(conn, id_interne, &version_id).map_err(|e| e.to_string())?;

        // What the files of this version say, for the tech sheet (FICHE§6.2) —
        // before the harmonisation, which refreshes the sheet's cache.
        if fm.kind == ModKind::Car {
            crate::techsheet::record(conn, id_interne, &version_id, dest, false).map_err(|e| e.to_string())?;
        }

        // Harmonisation des tags + extraction specs/pays (§5), stockée en overlay.
        let class = ui.class.clone().unwrap_or_default();
        let h = harmonize::compute(
            self.rules,
            fm.kind,
            &ui.tags,
            name,
            &class,
            ui.country.as_deref(),
            ui.brand.as_deref(),
        );
        harmonize::store(conn, id_interne, &h, ui.country.as_deref(), self.rules).map_err(|e| e.to_string())?;
        Ok(())
    }
}
