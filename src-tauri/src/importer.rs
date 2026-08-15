//! Pipeline d'import (L1) : extraction d'archive, descente récursive,
//! résolution d'identité (§4.2/§4.3), rangement dans la bibliothèque,
//! écriture overlay + historique. Le fichier du mod n'est jamais modifié.

use std::path::{Path, PathBuf};
use std::time::Instant;

use chrono::Local;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::config::AppConfig;
use crate::import_bench::{Bucket, ITEM_OVERHEAD_SECS};
use crate::import_progress::{ImportCtx, ItemPlan, PHASE_EXTRACT, PHASE_FILING, PHASE_SCAN};
use crate::modscan::{self, ModKind};
use crate::rules::Rules;
use crate::{archive, harmonize, identity, inspect, layers, uijson};

#[derive(Debug, Clone, Serialize)]
pub struct FuzzyConflict {
    pub existing_id: String,
    pub existing_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportedMod {
    pub id_interne: String,
    pub kind: String,
    pub display_name: Option<String>,
    /// "IMPORT" | "UPDATE_REPLACE" | "DUPLICATE" | "EXTENSION" | "AMBIGUOUS" (§4.4).
    /// - EXTENSION : rangé comme couche à part, la base n'est jamais touchée.
    /// - AMBIGUOUS : rien écrit, on attend le choix de l'utilisateur.
    pub outcome: String,
    pub version_label: Option<String>,
    /// Renseigné si un mod existant ressemble fortement à celui-ci (§4.2).
    pub conflict: Option<FuzzyConflict>,
    /// Décompte de comparaison (§4.4), pour la modale et le rapport. Renseigné
    /// pour EXTENSION et AMBIGUOUS.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub added_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overwritten_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub existing_total: Option<usize>,
    /// Fichiers annexes redirigés vers le dossier ressources du mod (§4.5.2),
    /// selon le réglage global. 0 si rien n'a été filé (doublon, ambigu bloqué).
    pub resources_extracted: usize,
}

/// Décision explicite de l'utilisateur pour un mod resté ambigu (§4.4),
/// renvoyée par le front pour reprendre l'import.
#[derive(Debug, Clone, Deserialize)]
pub struct ImportDecision {
    pub id: String,
    /// "update" | "extension"
    pub decision: String,
}

/// Classement d'un import ciblant un contenu existant (§4.4).
#[derive(Debug, Clone, Copy, PartialEq)]
enum ImportClass {
    /// Vraie mise à jour : remplace la version active.
    Update,
    /// Couche/extension : surtout des chemins nouveaux → rangée à part.
    Extension,
    /// Indécidable automatiquement → demander à l'utilisateur.
    Ambiguous,
}

/// Classe une comparaison de contenu (§4.4). `coverage` = part de la base qui
/// serait écrasée. Peu d'écrasements + surtout du neuf = extension ; recouvrement
/// large = mise à jour ; entre les deux = ambigu (on demande).
fn classify_diff(d: &crate::identity::DiffStats) -> ImportClass {
    if d.existing_total == 0 {
        return ImportClass::Update; // rien à comparer : comportement historique
    }
    if d.overwritten == 0 {
        return ImportClass::Extension; // addition pure (ex. nouveau layout)
    }
    let coverage = d.overwritten as f64 / d.existing_total as f64;
    if coverage >= 0.6 {
        ImportClass::Update
    } else if coverage <= 0.15 && d.added >= d.overwritten {
        ImportClass::Extension
    } else {
        ImportClass::Ambiguous
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ArchiveResult {
    pub archive: String,
    pub mods: Vec<ImportedMod>,
    pub error: Option<String>,
    /// Sous-éléments rattachés (skins/sons) routés vers la bibliothèque (§12bis.2).
    #[serde(default)]
    pub subs: Vec<crate::submods::SubImported>,
    /// Apps Python importées (§12bis.4).
    #[serde(default)]
    pub apps: Vec<crate::apps::AppImported>,
    /// Mods « autres » importés — type non reconnu, jamais perdus (§7.3).
    #[serde(default)]
    pub others: Vec<crate::others::OtherImported>,
    /// Fichiers rattachés à un mod de l'''archive et stockés comme ses
    /// ajouts au jeu (§4.5.3) — configs CSP, shaders, pilote…
    #[serde(default)]
    pub extras: usize,
}

/// Marge appliquée à la taille d'un lot pour le contrôle d'espace disque
/// (§4.2bis) : une archive rend plus d'octets qu'elle n'en pèse, et le contenu
/// extrait vit en temporaire **et** en bibliothèque le temps du rangement.
const DISK_HEADROOM: f64 = 1.5;

/// Part du rangement d'un item consommée par le scan, avant le premier mod.
const SCAN_SHARE: f64 = 0.05;

/// Part réservée à ce qui suit les mods (skins/sons, apps, balayage des restes,
/// §7.3) : la barre de l'item n'atteint sa fin qu'une fois l'item consommé,
/// jamais au dernier mod rangé — il reste du travail après lui.
const TAIL_SHARE: f64 = 0.10;

/// Fin du rangement des mods, début de la queue.
const TAIL_START: f64 = 1.0 - TAIL_SHARE;

/// Découpage de la queue (§4.2bis). Les skins/sons en prennent la moitié : un
/// pack de deux cents livrées y passe bien plus de temps que les apps, et
/// c'était l'endroit où la barre semblait bloquée.
const TAIL_SUBS_END: f64 = 0.95;
const TAIL_APPS_END: f64 = 0.96;

/// Avancement du rangement d'un item après `done` mods sur `total`.
fn filing_ratio(done: usize, total: usize) -> f64 {
    if total == 0 {
        return TAIL_START;
    }
    SCAN_SHARE + (TAIL_START - SCAN_SHARE) * (done as f64 / total as f64)
}

/// Avancement dans une bande de la queue, `done` sur `total`.
fn tail_ratio(from: f64, to: f64, done: usize, total: usize) -> f64 {
    if total == 0 {
        return to;
    }
    from + (to - from) * (done as f64 / total as f64).min(1.0)
}

/// Nom affichable d'une source d'import (archive ou dossier).
fn source_label(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

/// Taille d'une source, pour la pondération du lot et le contrôle d'espace.
/// 0 si illisible : mieux vaut une estimation absente qu'un import refusé.
fn source_size(path: &Path) -> u64 {
    match std::fs::metadata(path) {
        Ok(m) if m.is_file() => m.len(),
        Ok(m) if m.is_dir() => walkdir::WalkDir::new(path)
            .into_iter()
            .flatten()
            .filter_map(|e| e.metadata().ok())
            .filter(|m| m.is_file())
            .map(|m| m.len())
            .sum(),
        _ => 0,
    }
}

/// Pèse le lot avant de commencer. La phase `sizing` est émise au fil du
/// parcours : sans elle, désigner un dossier de quarante mods laisse l'app
/// muette le temps de la mesure, ce qui ressemble exactement à un blocage.
fn size_batch(ctx: &ImportCtx, paths: &[String]) -> Vec<u64> {
    paths
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let path = Path::new(p);
            ctx.sizing(i + 1, paths.len(), source_label(path));
            source_size(path)
        })
        .collect()
}

/// Refuse un lot qui ne peut pas tenir sur le volume de la bibliothèque
/// (§4.2bis). Un import mort à mi-parcours, disque plein, coûte bien plus cher
/// à nettoyer qu'à prévenir. Ne bloque jamais sur une information manquante
/// (bibliothèque non configurée, volume non interrogeable).
fn check_disk_space(cfg: &AppConfig, sizes: &[u64], headroom: f64) -> Result<(), String> {
    let Some(library) = &cfg.library_path else {
        return Ok(());
    };
    let Some(free) = crate::libpath::free_space(library) else {
        return Ok(());
    };
    let needed = (sizes.iter().sum::<u64>() as f64 * headroom) as u64;
    if needed > free {
        log::warn!(
            "import refused: needs ~{needed} bytes, only {free} free on {}",
            library.display()
        );
        return Err(crate::errors::NOT_ENOUGH_DISK_SPACE.to_string());
    }
    Ok(())
}

/// Une archive extraite, prête à être rangée. Produite hors verrou base.
struct Extracted {
    index: usize,
    label: String,
    /// Dossier temporaire d'extraction, supprimé après rangement.
    workdir: PathBuf,
    /// Taille de l'archive d'origine, pour alimenter le benchmark.
    size: u64,
    /// Copie de l'archive source (§10/§11), faite hors verrou elle aussi.
    kept: Option<String>,
}

/// Ce que le producteur remet au rangeur : une archive prête, ou l'échec de
/// son extraction — jamais rien, sinon l'index du lot se décalerait.
enum Staged {
    Ready(Box<Extracted>),
    Failed { label: String, error: String },
}

/// Importe une liste d'archives. Chaque archive est traitée indépendamment ;
/// une erreur sur l'une n'interrompt pas les autres.
pub fn import_archives(
    ctx: &ImportCtx,
    db: &crate::overlay::Db,
    cfg: &AppConfig,
    rules: &Rules,
    paths: &[String],
    decisions: &[ImportDecision],
) -> Result<Vec<ArchiveResult>, String> {
    let sizes = size_batch(ctx, paths);
    check_disk_space(cfg, &sizes, DISK_HEADROOM)?;
    ctx.plan(
        paths
            .iter()
            .zip(&sizes)
            .map(|(p, &size)| ItemPlan {
                label: source_label(Path::new(p)),
                extract_w: ctx.estimate(Bucket::ArchiveExtract, size),
                file_w: ITEM_OVERHEAD_SECS + ctx.estimate(Bucket::ArchiveFile, size),
            })
            .collect(),
    );
    Ok(run_import(ctx, db, cfg, rules, paths, &sizes, decisions))
}

/// Extraction et rangement en pipeline (§4.2bis) : l'archive N+1 se décompresse
/// pendant que la N se range. Les deux étapes saturent des ressources
/// différentes (7-Zip est limité par le CPU, le rangement par le disque), donc
/// les mener de front raccourcit nettement un gros lot.
///
/// Le canal est un rendez-vous (capacité 0) et pas un tampon : il borne
/// l'avance à **une** archive, donc à deux dossiers temporaires vivants au
/// plus. Avec un tampon, un lot de quarante archives pourrait en extraire
/// autant d'avance qu'il en tient — et remplir le disque avant d'en avoir
/// rangé trois.
fn run_import(
    ctx: &ImportCtx,
    db: &crate::overlay::Db,
    cfg: &AppConfig,
    rules: &Rules,
    paths: &[String],
    sizes: &[u64],
    decisions: &[ImportDecision],
) -> Vec<ArchiveResult> {
    let (tx, rx) = std::sync::mpsc::sync_channel::<Staged>(0);
    std::thread::scope(|scope| {
        scope.spawn(move || {
            for (i, p) in paths.iter().enumerate() {
                if ctx.cancelled() {
                    break;
                }
                let staged = extract_stage(ctx, cfg, i, Path::new(p), sizes[i]);
                if let Err(returned) = tx.send(staged) {
                    // Rangeur parti (annulation) : le temporaire qu'on vient
                    // d'extraire n'ira nulle part, il faut le retirer nous-mêmes.
                    if let Staged::Ready(ex) = returned.0 {
                        let _ = std::fs::remove_dir_all(&ex.workdir);
                    }
                    break;
                }
            }
        });

        let mut results = Vec::with_capacity(paths.len());
        for (i, p) in paths.iter().enumerate() {
            if ctx.cancelled() {
                break;
            }
            // Affiché AVANT l'attente : quand l'extraction n'a pas encore été
            // prise d'avance (première archive du lot, ou pipeline en retard),
            // c'est elle qu'on attend — et c'est sa progression que le
            // producteur alimente pendant ce temps.
            ctx.set_current(i, PHASE_EXTRACT, source_label(Path::new(p)));
            let Ok(staged) = rx.recv() else { break };
            match staged {
                Staged::Failed { label, error } => {
                    results.push(failed_result(&label, error));
                }
                Staged::Ready(ex) => {
                    ctx.phase(PHASE_SCAN, ex.label.clone());
                    let started = Instant::now();
                    // Verrou pris seulement pour le rangement : l'extraction et
                    // la copie de l'archive source, qui ne touchent pas la base,
                    // se sont faites en dehors. Avant, tout écran lisant
                    // l'overlay attendait aussi la décompression — plusieurs
                    // minutes sur un gros circuit.
                    let result = match db.0.lock() {
                        Ok(conn) => file_extracted(ctx, &conn, cfg, rules, &ex, decisions),
                        Err(e) => failed_result(&ex.label, e.to_string()),
                    };
                    ctx.record(Bucket::ArchiveFile, ex.size, started.elapsed().as_secs_f64());
                    let _ = std::fs::remove_dir_all(&ex.workdir);
                    results.push(result);
                }
            }
            ctx.finish_item(i);
        }
        // Libère le producteur s'il attend encore un rendez-vous : sans ça, une
        // annulation laisserait le fil d'extraction bloqué sur `send` et la
        // portée ne se refermerait jamais.
        drop(rx);
        results
    })
}

/// Phase hors verrou : décompression de l'archive et copie de la source.
fn extract_stage(ctx: &ImportCtx, cfg: &AppConfig, index: usize, archive_path: &Path, size: u64) -> Staged {
    let label = source_label(archive_path);
    let (Some(sevenzip), Some(_)) = (&cfg.sevenzip_exe, &cfg.library_path) else {
        return Staged::Failed {
            label,
            error: crate::errors::SEVENZIP_NOT_CONFIGURED.to_string(),
        };
    };
    let workdir = match make_temp_dir() {
        Ok(d) => d,
        Err(e) => {
            log::warn!("import {label}: temp dir: {e}");
            return Staged::Failed {
                label,
                error: crate::errors::TEMP_DIR_UNAVAILABLE.to_string(),
            };
        }
    };

    let started = Instant::now();
    let extracted = archive::extract_with_progress(
        sevenzip,
        archive_path,
        &workdir,
        &|pct| ctx.extract_ratio(index, pct as f64 / 100.0),
        &|| ctx.cancelled(),
    );
    if let Err(e) = extracted {
        let _ = std::fs::remove_dir_all(&workdir);
        return Staged::Failed { label, error: e };
    }
    ctx.record(Bucket::ArchiveExtract, size, started.elapsed().as_secs_f64());
    ctx.extract_ratio(index, 1.0);

    Staged::Ready(Box::new(Extracted {
        index,
        // Conservation de l'archive source (§10/§11) : une seule copie partagée
        // entre tous les mods trouvés dedans, faite ici parce qu'elle peut
        // peser plusieurs Go et n'a rien à faire sous le verrou base.
        kept: keep_source(cfg, archive_path, &label),
        label,
        workdir,
        size,
    }))
}

/// Résultat d'archive en échec — verrou base empoisonné, extraction ratée,
/// espace manquant. `error` est soit une clé i18n, soit un diagnostic brut
/// (7-Zip, E/S), les deux traversant `errorText()` sans dommage.
fn failed_result(label: &str, error: String) -> ArchiveResult {
    ArchiveResult {
        archive: label.to_string(),
        mods: Vec::new(),
        error: Some(error),
        subs: Vec::new(),
        apps: Vec::new(),
        others: Vec::new(),
        extras: 0,
    }
}

/// Décision utilisateur mémorisée pour cet id, s'il y en a une (§4.4).
fn decision_for<'a>(decisions: &'a [ImportDecision], id: &str) -> Option<&'a str> {
    decisions.iter().find(|d| d.id == id).map(|d| d.decision.as_str())
}

/// Vrai si cet import est une **reprise** après arbitrage (§4.4). `decisions`
/// est toujours vide au premier passage : sa présence signifie que
/// l'utilisateur vient de trancher un cas ambigu.
fn is_resume(decisions: &[ImportDecision]) -> bool {
    !decisions.is_empty()
}

/// Mods à ranger dans cette passe. Au premier import, tous. Sur reprise, **le
/// seul mod tranché** : rejouer les trente-neuf autres d'un pack les faisait
/// revenir en « doublon » — donc sans dégât, mais au prix du lot entier, et la
/// barre annonçait un travail qui n'en était pas un.
fn targets_of<'a>(found: &'a [modscan::FoundMod], decisions: &[ImportDecision]) -> Vec<&'a modscan::FoundMod> {
    if is_resume(decisions) {
        found
            .iter()
            .filter(|fm| decision_for(decisions, &fm_id(fm)).is_some())
            .collect()
    } else {
        found.iter().collect()
    }
}

/// Copie l'archive/dossier source dans un espace dédié de la bibliothèque
/// (§10/§11), pour permettre plus tard « Réinstaller depuis l'archive source ».
/// Uniquement si le réglage `keep_source_archive` est actif — sinon jamais
/// reposé à chaque import. Best-effort : une copie échouée (disque plein…)
/// ne doit jamais interrompre l'import (`None` en ce cas).
fn keep_source(cfg: &AppConfig, source: &Path, label: &str) -> Option<String> {
    if !cfg.prefs.keep_source_archive {
        return None;
    }
    let library = cfg.library_path.as_ref()?;
    let dest_dir = library.join("_source_archives").join(Uuid::new_v4().to_string());
    if source.is_dir() {
        let dest = dest_dir.join(label);
        archive::copy_dir(source, &dest).ok()?;
        Some(crate::libpath::to_relative(Some(library), &dest))
    } else {
        std::fs::create_dir_all(&dest_dir).ok()?;
        let dest = dest_dir.join(label);
        std::fs::copy(source, &dest).ok()?;
        Some(crate::libpath::to_relative(Some(library), &dest))
    }
}

/// Retire la copie de source qu'aucune version ne réclame (§10/§11).
///
/// `keep_source` copie **avant** de savoir si la copie servira : c'est
/// obligatoire, puisqu'un import de dossier en mode déplacement consomme sa
/// source pendant le rangement. Mais tous les classements ne stockent pas une
/// version — un doublon sort avant d'écrire quoi que ce soit (§4.4), une couche
/// passe par `layers::store_layer`, une archive au contenu non reconnu devient
/// un « autre mod » (§7.3). Dans ces cas-là, la copie restait dans
/// `_source_archives/` sans que rien ne la référence ni ne la nettoie : sur une
/// archive de 2 Go réimportée par erreur, 2 Go perdus à chaque fois.
///
/// L'overlay fait autorité plutôt que l'issue affichée : une issue ajoutée plus
/// tard n'a pas à être reportée ici pour que le nettoyage reste juste.
fn drop_unused_kept_source(conn: &Connection, cfg: &AppConfig, kept: Option<&str>) {
    let Some(kept) = kept else { return };
    match crate::overlay::kept_archive_in_use(conn, kept) {
        Ok(true) => return,
        Ok(false) => {}
        Err(e) => {
            // Base illisible : ne rien supprimer. Une copie orpheline coûte du
            // disque, une copie retirée à tort casse « Réinstaller depuis
            // l'archive source ».
            log::warn!("kept_archive_in_use {kept}: {e}");
            return;
        }
    }
    let Some(path) = crate::libpath::resolve(cfg.library_path.as_deref(), kept) else {
        return;
    };
    // Le dossier à retirer est celui à l'uuid, pas le fichier dedans : c'est lui
    // que `keep_source` a créé. Garde-fou sur le nom du parent — on ne supprime
    // récursivement que ce qu'on a soi-même posé sous `_source_archives/`.
    let Some(uuid_dir) = path.parent() else { return };
    let under_source_archives = uuid_dir
        .parent()
        .and_then(|p| p.file_name())
        .is_some_and(|n| n == "_source_archives");
    if !under_source_archives {
        log::warn!("kept source outside _source_archives, left alone: {}", path.display());
        return;
    }
    if let Err(e) = std::fs::remove_dir_all(uuid_dir) {
        log::warn!("drop_unused_kept_source {}: {e}", uuid_dir.display());
    }
}

/// Id interne dérivé du dossier d'un mod trouvé (nom du dossier `content/<type>s/<id>`).
fn fm_id(fm: &modscan::FoundMod) -> String {
    fm.dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Extensions d'archives reconnues pour une extraction imbriquée (§7.3).
/// Partagé avec `others::other_id`, qui ne doit retirer *que* ces extensions-là
/// en dérivant l'id d'un mod « autre ».
pub(crate) const NESTED_ARCHIVE_EXTS: &[&str] = &["zip", "7z", "rar"];

fn is_archive_file(p: &Path) -> bool {
    p.is_file()
        && p.extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| NESTED_ARCHIVE_EXTS.iter().any(|a| a.eq_ignore_ascii_case(e)))
}

/// Archives imbriquées sous `dir`, à n'importe quelle profondeur (§7.3).
///
/// Sert à décider si une source dont **rien** n'est reconnu mérite qu'on
/// descende avant de conclure. Recherche récursive et non limitée à la racine :
/// la même livraison se rencontre à plat (`readme.txt` + `Car.zip`) et
/// enveloppée d'un dossier (`Mod/{readme.txt, Car.zip}`), selon que l'auteur a
/// zippé le contenu ou le dossier.
fn nested_archives(dir: &Path) -> Vec<PathBuf> {
    walkdir::WalkDir::new(dir)
        .into_iter()
        .flatten()
        .map(|e| e.into_path())
        .filter(|p| is_archive_file(p))
        .collect()
}

/// Tente de sauver une source dont rien n'est reconnu en descendant dans les
/// archives qu'elle contient (§7.3). Renvoie `false` si elle n'en contient
/// aucune — l'appelant retombe alors sur « autre mod », comportement historique.
///
/// Bug réel : une archive livrant `readme.txt` + `Car.zip` (la voiture dans le
/// zip) n'était pas reconnue à la racine, donc rangée en bloc comme « autre
/// mod » — la voiture n'entrait jamais en bibliothèque, et le `.zip` brut se
/// retrouvait lié à la racine du dossier du jeu. La machinerie qui sait extraire
/// et reclasser une archive imbriquée existait déjà, ce chemin ne l'atteignait
/// simplement jamais.
#[allow(clippy::too_many_arguments)]
fn rescue_nested_archives(
    ctx: &ImportCtx,
    index: usize,
    conn: &Connection,
    cfg: &AppConfig,
    rules: &Rules,
    library: &Path,
    source_name: &str,
    root: &Path,
    copy: bool,
    res_mode: crate::resources::ExtractionMode,
    result: &mut ArchiveResult,
) -> bool {
    let nested = nested_archives(root);
    if nested.is_empty() {
        return false;
    }
    let mut discovered: Vec<(String, ModKind)> = Vec::new();
    for p in &nested {
        import_leftover(
            ctx,
            conn,
            cfg,
            rules,
            library,
            source_name,
            root,
            p,
            copy,
            res_mode,
            result,
            0,
            &mut discovered,
        );
    }
    // Le reste (la notice, et tout ce qui entourait le zip) est arbitré ensuite,
    // avec ce qui vient d'en sortir comme propriétaires possibles. `consumed`
    // porte les archives déjà traitées : `collect_leftover` descend dans un
    // dossier qui en contient une, ce qui couvre les deux formes de livraison.
    sweep_leftovers(
        ctx,
        index,
        conn,
        cfg,
        rules,
        library,
        source_name,
        root,
        &nested,
        &discovered,
        copy,
        res_mode,
        result,
    );
    true
}

/// Chemins, sous `dir`, qui ne sont couverts par aucun mod déjà reconnu
/// (`consumed` : dossiers de voitures/circuits/skins/sons/apps). Un dossier
/// entièrement sous un chemin `consumed` est ignoré (déjà pris en charge) ;
/// un dossier qui CONTIENT un chemin `consumed` plus profond est descendu pour
/// isoler ce qui, à côté, ne l'est pas ; tout le reste (fichier isolé ou
/// dossier sans rien de reconnu dessous) est renvoyé tel quel, en bloc.
fn collect_leftover(dir: &Path, consumed: &[PathBuf], out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if consumed.iter().any(|c| c == &p) {
            continue;
        }
        if p.is_dir() {
            if consumed.iter().any(|c| c.starts_with(&p)) {
                collect_leftover(&p, consumed, out);
            } else {
                out.push(p);
            }
        } else {
            out.push(p);
        }
    }
}

/// Chemins consommés par les mods déjà reconnus dans un dossier scanné — sert
/// à isoler ce qui reste (`collect_leftover`) pour ne plus jamais le perdre.
fn consumed_paths(found: &[modscan::FoundMod], subs: &[modscan::FoundSub], apps: &[modscan::FoundApp]) -> Vec<PathBuf> {
    found
        .iter()
        .map(|f| f.dir.clone())
        .chain(subs.iter().map(|s| s.dir.clone()))
        .chain(subs.iter().filter_map(|s| s.extra_root.clone()))
        .chain(apps.iter().map(|a| a.dir.clone()))
        .collect()
}

/// Importe tout ce qui reste après found/subs/apps (§7.3) : avant ce
/// correctif, dès qu'au moins un mod était reconnu ailleurs dans l'archive,
/// tout le reste — fichiers isolés, zips imbriqués comme les mods CMRT-style
/// (dossier `apps/` + zip séparé qui vise `content/gui/...`) — disparaissait
/// silencieusement au nettoyage du dossier temporaire (tri tout-ou-rien).
/// Un reste rattaché à un mod de la même archive devient un de ses **ajouts au jeu**
/// (§4.5.3) ; sinon il reste un « autre mod » autonome (id `<archive>__<nom>`),
/// jamais fusionné avec ses voisins.
///
/// Rattachement, dans cet ordre :
/// 1. le chemin du reste contient l'id d'exactement un mod reconnu
///    (`extension/config/cars/rss/rss_gtm_lanzo_v8/…`) — sans ambiguïté ;
/// 2. sinon, l'archive ne livre qu'un seul mod : tout ce qui l'entoure lui
///    appartient (`system/shaders/…`, `content/driver/…`).
///
/// **Limite assumée** : dans un pack multi-mods, un reste que rien ne rattache
/// reste un « autre mod ». Le rattacher à tous dupliquerait des arbres parfois
/// lourds ; il n'y a pas de bonne réponse sans regarder le contenu, et « autre
/// mod » ne perd rien.
fn owner_of_leftover<'a>(rel: &Path, mods: &'a [(String, ModKind)]) -> Option<&'a (String, ModKind)> {
    let named: Vec<&(String, ModKind)> = mods
        .iter()
        .filter(|(id, _)| {
            rel.components()
                .any(|c| c.as_os_str().to_str().is_some_and(|s| s.eq_ignore_ascii_case(id)))
        })
        .collect();
    if named.len() == 1 {
        return Some(named[0]);
    }
    if named.is_empty() && mods.len() == 1 {
        return Some(&mods[0]);
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn sweep_leftovers(
    ctx: &ImportCtx,
    index: usize,
    conn: &Connection,
    cfg: &AppConfig,
    rules: &Rules,
    library: &Path,
    archive_name: &str,
    workdir: &Path,
    consumed: &[PathBuf],
    mods: &[(String, ModKind)],
    copy: bool,
    res_mode: crate::resources::ExtractionMode,
    result: &mut ArchiveResult,
) {
    let mut leftovers = Vec::new();
    collect_leftover(workdir, consumed, &mut leftovers);
    let leftover_count = leftovers.len();
    // Mods ayant reçu au moins un ajout : ces ajouts sont posés en
    // fin de balayage. L'activation par défaut (§4.2) a lieu avant, quand
    // leur arbre n'existe pas encore — sans ce rattrapage, ils ne
    // seraient posés qu'à la réactivation suivante.
    let mut owners_with_extras: Vec<(String, ModKind)> = Vec::new();

    // Les archives imbriquées passent AVANT leurs voisins : ce qui en sort peut
    // devenir le propriétaire de ce qui les entoure. Cas réel : une archive qui
    // livre `readme.txt` + `Car.zip`, la voiture étant dans le zip — traités
    // dans l'ordre du disque, le readme serait arbitré alors qu'aucune voiture
    // n'est encore connue, et finirait « autre mod » au lieu d'être rangé dans
    // les ressources de la voiture (§4.5.2).
    let (nested, plain): (Vec<PathBuf>, Vec<PathBuf>) = leftovers.into_iter().partition(|p| is_archive_file(p));
    let mut owners: Vec<(String, ModKind)> = mods.to_vec();
    for (i, p) in nested.iter().enumerate() {
        ctx.file_ratio(index, tail_ratio(TAIL_APPS_END, 1.0, i, leftover_count));
        import_leftover(
            ctx,
            conn,
            cfg,
            rules,
            library,
            archive_name,
            workdir,
            p,
            copy,
            res_mode,
            result,
            0,
            &mut owners,
        );
    }

    for (i, p) in plain.into_iter().enumerate() {
        ctx.file_ratio(index, tail_ratio(TAIL_APPS_END, 1.0, nested.len() + i, leftover_count));
        let rel = p.strip_prefix(workdir).unwrap_or(&p).to_path_buf();

        {
            if let Some((owner_id, owner_kind)) = owner_of_leftover(&rel, &owners) {
                // Document isolé à la racine de ce qui entoure le mod : une
                // annexe (§4.5.2), pas un ajout au jeu — il n'a rien à faire dans
                // AC. Rangé dans les ressources du mod auquel il appartient,
                // là où l'utilisateur ira le lire.
                let is_root_file = p.is_file() && rel.parent().is_some_and(|d| d.as_os_str().is_empty());
                if is_root_file {
                    match crate::resources::route_beside_root(&p, res_mode) {
                        crate::resources::Route::Resources => {
                            let dest = crate::resources::resources_dir(library, *owner_kind, owner_id).join(&rel);
                            if let Err(e) = crate::extras::store(
                                &crate::resources::resources_dir(library, *owner_kind, owner_id),
                                &rel,
                                &p,
                                copy,
                            ) {
                                log::warn!("ancillary {}: {e}", dest.display());
                            }
                            continue;
                        }
                        // Mode « Aucun » : ni contenu, ni ressources — laissé
                        // dans la source, jamais supprimé.
                        crate::resources::Route::Drop => continue,
                        crate::resources::Route::Content => {}
                    }
                }
                let sat = crate::extras::dir(library, *owner_kind, owner_id);
                match crate::extras::store(&sat, &rel, &p, copy) {
                    Ok(()) => {
                        result.extras += 1;
                        let owner = (owner_id.clone(), *owner_kind);
                        if !owners_with_extras.contains(&owner) {
                            owners_with_extras.push(owner);
                        }
                        continue;
                    }
                    // Le repli sur « autre mod » ci-dessous ne perd rien : le
                    // reste est simplement moins bien rattaché.
                    Err(e) => log::warn!("extras {} <- {}: {e}", owner_id, rel.display()),
                }
            }
        }

        import_leftover(
            ctx,
            conn,
            cfg,
            rules,
            library,
            archive_name,
            workdir,
            &p,
            copy,
            res_mode,
            result,
            0,
            &mut owners,
        );
    }

    for (id, kind) in owners_with_extras {
        // Seulement si le mod est effectivement déployé : poser les ajouts au jeu
        // d'un mod inactif mettrait dans AC du contenu que rien n'y annonce.
        if !crate::activation::is_mod_active(cfg, kind, &id) {
            continue;
        }
        if let Err(e) = crate::extras::deploy(conn, cfg, kind, &id) {
            log::warn!("deploy_extras {id}: {e}");
        }
    }
}

/// Un seul reste isolé : archive imbriquée (extraite puis reclassée comme un
/// import à part entière, profondeur limitée à 2 contre une imbrication
/// pathologique) ou contenu déjà en clair (importé directement comme « autre
/// mod », `others::import_other` ne perd jamais rien non plus).
#[allow(clippy::too_many_arguments)]
fn import_leftover(
    ctx: &ImportCtx,
    conn: &Connection,
    cfg: &AppConfig,
    rules: &Rules,
    library: &Path,
    archive_name: &str,
    root: &Path,
    p: &Path,
    copy: bool,
    res_mode: crate::resources::ExtractionMode,
    result: &mut ArchiveResult,
    depth: u8,
    // Mods connus, complété par ceux qui sortent d'une archive imbriquée : c'est
    // ce qui permet ensuite de rattacher à une voiture la notice livrée à côté
    // du zip qui la contenait.
    discovered: &mut Vec<(String, ModKind)>,
) {
    // Chemin relatif à la racine balayée, et non simple nom de fichier : c'est
    // lui qui donne sa destination au reste à l'activation (`others::place`
    // rejoue `ac.join(rel)`), et lui qui distingue deux restes homonymes dans
    // des dossiers différents. Avec le seul nom, `content/driver` atterrissait
    // à `AC\driver` — et un `driver/` à la racine de l'archive aurait pris le
    // même id.
    let rel = p.strip_prefix(root).unwrap_or(p);
    let label = rel.to_string_lossy().into_owned();
    let nested_name = format!("{archive_name}__{label}");

    if depth < 2 && is_archive_file(p) {
        // Archive imbriquée : son extension n'a pas à rester dans l'id des mods
        // qui en sortiront.
        let nested_name = format!("{archive_name}__{}", rel.with_extension("").to_string_lossy());
        let Some(sevenzip) = &cfg.sevenzip_exe else {
            return;
        };
        let Ok(extracted) = make_temp_dir() else { return };
        // Une archive imbriquée peut peser autant que celle qui l'entoure (mods
        // CMRT-style). Sans ce signalement, la barre restait figée en fin d'item
        // sans rien dire de ce qu'elle attendait. Pas de pourcentage en
        // revanche : le nombre d'archives imbriquées n'est pas connu d'avance,
        // donc leur avancement ne se projette sur aucune part fiable de la
        // barre — une précision inventée serait pire que pas de précision.
        ctx.phase(PHASE_EXTRACT, label.clone());
        let nested = archive::extract(sevenzip, p, &extracted);
        if let Err(e) = &nested {
            // Sans cette trace, une archive imbriquée illisible disparaissait
            // au nettoyage du temporaire sans le moindre indice (§7.3 : rien ne
            // doit être perdu en silence).
            log::warn!("nested archive {}: {e}", p.display());
        }
        if nested.is_ok() {
            let found = modscan::scan(&extracted);
            let subs = modscan::scan_subs(&extracted);
            let apps = modscan::scan_apps(&extracted);
            if found.is_empty() && subs.is_empty() && apps.is_empty() {
                if let Some(other) =
                    crate::others::import_other(conn, library, &nested_name, &extracted, false, res_mode)
                {
                    if let Err(e) = crate::others::activate_other(conn, cfg, &other.id) {
                        log::warn!("auto_activate_other {}: {e}", other.id);
                    }
                    result.others.push(other);
                }
            } else {
                for fm in &found {
                    if let Ok(imported) = process_found(
                        conn,
                        cfg,
                        rules,
                        library,
                        &nested_name,
                        fm,
                        false,
                        None,
                        None,
                        false,
                        None,
                    ) {
                        discovered.push((imported.id_interne.clone(), fm.kind));
                        result.mods.push(imported);
                    }
                }
                auto_activate(conn, cfg, &result.mods);
                if !subs.is_empty() {
                    result.subs.extend(crate::submods::import_subs(
                        conn,
                        cfg,
                        library,
                        &nested_name,
                        &subs,
                        false,
                        res_mode,
                    ));
                }
                if !apps.is_empty() {
                    let imported_apps = crate::apps::import_apps(conn, library, &nested_name, &apps, false, res_mode);
                    auto_activate_apps(conn, cfg, &imported_apps);
                    result.apps.extend(imported_apps);
                }
                let consumed = consumed_paths(&found, &subs, &apps);
                let mut inner = Vec::new();
                collect_leftover(&extracted, &consumed, &mut inner);
                for lp in inner {
                    import_leftover(
                        ctx,
                        conn,
                        cfg,
                        rules,
                        library,
                        &nested_name,
                        &extracted,
                        &lp,
                        false,
                        res_mode,
                        result,
                        depth + 1,
                        discovered,
                    );
                }
            }
        }
        let _ = std::fs::remove_dir_all(&extracted);
        return;
    }

    // `p` est un reste isolé (fichier OU dossier, ex. `extension/` livré à
    // plat à côté d'une app) : enveloppé dans un temp dir qui reconstitue son
    // **chemin relatif à la racine balayée** avant d'appeler
    // `others::import_other`. `others::place` rejoue ensuite ce chemin depuis
    // cette racine (`ac.join(rel)`) — d'où deux exigences :
    //   - sans enveloppe du tout, `extension/` pris comme racine atterrirait à
    //     `ac/config/...` au lieu d'`ac/extension/config/...` ;
    //   - avec une enveloppe réduite au seul nom, `content/driver` atterrissait
    //     à `ac/driver` au lieu d'`ac/content/driver` (bug réel : le pilote du
    //     Lanzo jonctionné à la racine d'AC, donc invisible pour le jeu).
    let Ok(wrap) = make_temp_dir() else { return };
    let wrapped = wrap.join(rel);
    if let Some(parent) = wrapped.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            log::warn!("import_leftover {}: create wrap dir: {e}", rel.display());
            let _ = std::fs::remove_dir_all(&wrap);
            return;
        }
    }
    let placed = if p.is_dir() {
        if copy {
            archive::copy_dir(p, &wrapped).is_ok()
        } else {
            archive::move_dir(p, &wrapped).is_ok()
        }
    } else if copy {
        // `copy` = la source appartient à l'utilisateur (import d'un dossier
        // qu'il désigne, pas d'une extraction temporaire) : on n'y touche pas.
        // Le `rename` inconditionnel d'avant retirait ses fichiers isolés de
        // son propre dossier — un import « en copie » qui déplaçait quand même.
        std::fs::copy(p, &wrapped).is_ok()
    } else {
        std::fs::rename(p, &wrapped).is_ok() || std::fs::copy(p, &wrapped).is_ok()
    };
    if !placed {
        log::warn!("import_leftover {}: could not stage leftover", rel.display());
    } else if let Some(other) = crate::others::import_other(conn, library, &nested_name, &wrap, false, res_mode) {
        if let Err(e) = crate::others::activate_other(conn, cfg, &other.id) {
            log::warn!("auto_activate_other {}: {e}", other.id);
        }
        result.others.push(other);
    } else {
        // Id déjà connu : `import_other` ne remplace jamais (§7.3). Sans cette
        // trace, le reste disparaissait au nettoyage de `wrap` sans laisser le
        // moindre indice — c'est ce qui rendait la collision d'id indétectable.
        log::warn!("import_leftover {}: id already known, leftover dropped", nested_name);
    }
    let _ = std::fs::remove_dir_all(&wrap);
}

/// Active par défaut les mods fraîchement importés/mis à jour (§4.2) : on veut
/// pouvoir conduire tout de suite. Best-effort (l'échec — ex. vrai dossier Kunos
/// homonyme, dossier AC non configuré — n'interrompt pas l'import).
fn auto_activate(conn: &Connection, cfg: &AppConfig, mods: &[ImportedMod]) {
    for m in mods {
        if m.outcome == "IMPORT" || m.outcome == "UPDATE_REPLACE" {
            if let Err(e) = crate::activation::activate(conn, cfg, &m.id_interne, None) {
                log::warn!("auto_activate {}: {e}", m.id_interne);
            }
        }
    }
}

/// Active par défaut les apps fraîchement importées (§4.2, §12bis.4) — même
/// logique que les mods voiture/circuit et les mods « autres » : best-effort,
/// une app déjà active ou dont l'AC install n'est pas configurée ne bloque pas
/// le reste de l'import.
fn auto_activate_apps(conn: &Connection, cfg: &AppConfig, apps: &[crate::apps::AppImported]) {
    for a in apps {
        if let Err(e) = crate::apps::activate_app(conn, cfg, &a.name) {
            log::warn!("auto_activate_apps {}: {e}", a.name);
        }
    }
}

/// Range une archive déjà extraite (§4.2). Seule cette moitié du pipeline tient
/// le verrou base — voir `run_import`.
fn file_extracted(
    ctx: &ImportCtx,
    conn: &Connection,
    cfg: &AppConfig,
    rules: &Rules,
    ex: &Extracted,
    decisions: &[ImportDecision],
) -> ArchiveResult {
    let archive_name = ex.label.clone();
    let workdir = ex.workdir.as_path();

    let mut result = ArchiveResult {
        archive: archive_name.clone(),
        mods: Vec::new(),
        error: None,
        subs: Vec::new(),
        apps: Vec::new(),
        others: Vec::new(),
        extras: 0,
    };

    let Some(library) = &cfg.library_path else {
        result.error = Some(crate::errors::LIBRARY_NOT_CONFIGURED.into());
        return result;
    };
    // Extraction des fichiers annexes (§4.5.2) : réglage global, jamais reposé
    // à chaque import.
    let res_mode = crate::resources::ExtractionMode::parse(&cfg.prefs.resource_extraction_mode);

    let found = modscan::scan(workdir);
    // Sous-éléments rattachés (skins/sons) et apps — peuvent constituer une archive seuls.
    let subs = modscan::scan_subs(workdir);
    let apps = modscan::scan_apps(workdir);
    if found.is_empty() && subs.is_empty() && apps.is_empty() {
        // Rien à la racine ne veut pas dire rien du tout : le contenu peut être
        // enfermé dans une archive imbriquée (§7.3).
        let rescued = rescue_nested_archives(
            ctx,
            ex.index,
            conn,
            cfg,
            rules,
            library,
            &archive_name,
            workdir,
            false,
            res_mode,
            &mut result,
        );
        // Type non reconnu : jamais perdu, rangé comme « autre mod » (§7.3).
        if !rescued {
            if let Some(other) = crate::others::import_other(conn, library, &archive_name, workdir, false, res_mode) {
                if let Err(e) = crate::others::activate_other(conn, cfg, &other.id) {
                    log::warn!("auto_activate_other {}: {e}", other.id);
                }
                result.others.push(other);
            } else {
                result.error = Some(crate::errors::NOTHING_TO_IMPORT_IN_ARCHIVE.into());
            }
        }
        drop_unused_kept_source(conn, cfg, ex.kept.as_deref());
        return result;
    }
    let targets = targets_of(&found, decisions);
    ctx.sub(0, targets.len(), archive_name.clone());
    ctx.file_ratio(ex.index, SCAN_SHARE);

    let kept_archive = ex.kept.clone();

    // Pack (§4.4) : une archive multi-voitures regroupe ses mods sous sa source.
    // Compté sur `found`, jamais sur `targets` : une reprise ne doit pas
    // changer l'identité du pack sous prétexte qu'elle ne rejoue qu'un mod.
    let pack = (found.len() > 1).then_some(archive_name.as_str());
    for (i, fm) in targets.iter().enumerate() {
        let label = fm
            .dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        ctx.phase(PHASE_FILING, label);
        ctx.sub(i + 1, targets.len(), fm_id(fm));
        // Archive : le contenu vient d'un dossier temp → toujours déplacé.
        let decision = decision_for(decisions, &fm_id(fm));
        match process_found(
            conn,
            cfg,
            rules,
            library,
            &archive_name,
            fm,
            false,
            pack,
            decision,
            true,
            kept_archive.as_deref(),
        ) {
            Ok(imported) => result.mods.push(imported),
            Err(e) => {
                // On consigne l'erreur sur l'archive mais on continue les autres mods.
                result.error.get_or_insert_with(String::new).push_str(&format!("{e}; "));
            }
        }
        ctx.file_ratio(ex.index, filing_ratio(i + 1, targets.len()));
    }

    // Activation par défaut des mods importés (§4.2).
    auto_activate(conn, cfg, &result.mods);

    // Une reprise s'arrête au mod tranché : skins, apps et restes ont déjà été
    // rangés au premier passage, les rejouer ne ferait que payer deux fois.
    if !is_resume(decisions) {
        file_tail(
            ctx,
            ex.index,
            conn,
            cfg,
            rules,
            library,
            &archive_name,
            workdir,
            &found,
            &subs,
            &apps,
            false,
            res_mode,
            &mut result,
        );
    }

    drop_unused_kept_source(conn, cfg, kept_archive.as_deref());
    result
}

/// Import depuis des **dossiers déjà décompressés** (§4.2). Même pipeline que
/// les archives, sans décompression — donc sans rien à mettre en pipeline non
/// plus, c'est une simple boucle. `copy=true` préserve la source (copie),
/// sinon déplacement adaptatif (rename même disque, copie+suppression sinon).
pub fn import_folders(
    ctx: &ImportCtx,
    db: &crate::overlay::Db,
    cfg: &AppConfig,
    rules: &Rules,
    paths: &[String],
    copy: bool,
    decisions: &[ImportDecision],
) -> Result<Vec<ArchiveResult>, String> {
    let sizes = size_batch(ctx, paths);
    let bucket = if copy { Bucket::FolderCopy } else { Bucket::FolderMove };
    // Un déplacement ne consomme pas d'espace supplémentaire (même volume :
    // simple `rename`) — seule une copie doit tenir à côté de sa source.
    check_disk_space(cfg, &sizes, if copy { 1.0 } else { 0.0 })?;
    ctx.plan(
        paths
            .iter()
            .zip(&sizes)
            .map(|(p, &size)| ItemPlan {
                label: source_label(Path::new(p)),
                extract_w: 0.0,
                file_w: ITEM_OVERHEAD_SECS + ctx.estimate(bucket, size),
            })
            .collect(),
    );

    let mut results = Vec::with_capacity(paths.len());
    for (i, p) in paths.iter().enumerate() {
        if ctx.cancelled() {
            break;
        }
        let dir = Path::new(p);
        let label = source_label(dir);
        ctx.set_current(i, PHASE_SCAN, label.clone());
        // Conservation du dossier source (§10/§11) : hors verrou, comme pour
        // les archives — c'est une copie complète, parfois de plusieurs Go.
        let kept = keep_source(cfg, dir, &label);
        let started = Instant::now();
        let result = match db.0.lock() {
            Ok(conn) => import_one_folder(ctx, &conn, cfg, rules, i, dir, copy, decisions, kept.as_deref()),
            Err(e) => failed_result(&label, e.to_string()),
        };
        ctx.record(bucket, sizes[i], started.elapsed().as_secs_f64());
        results.push(result);
        ctx.finish_item(i);
    }
    Ok(results)
}

#[allow(clippy::too_many_arguments)]
fn import_one_folder(
    ctx: &ImportCtx,
    conn: &Connection,
    cfg: &AppConfig,
    rules: &Rules,
    index: usize,
    dir: &Path,
    copy: bool,
    decisions: &[ImportDecision],
    kept_archive: Option<&str>,
) -> ArchiveResult {
    let name = source_label(dir);
    let mut result = ArchiveResult {
        archive: name.clone(),
        mods: Vec::new(),
        error: None,
        subs: Vec::new(),
        apps: Vec::new(),
        others: Vec::new(),
        extras: 0,
    };

    let Some(library) = &cfg.library_path else {
        result.error = Some(crate::errors::LIBRARY_NOT_CONFIGURED.into());
        return result;
    };
    if !dir.is_dir() {
        result.error = Some(crate::errors::NOT_A_DIRECTORY.into());
        return result;
    }
    let res_mode = crate::resources::ExtractionMode::parse(&cfg.prefs.resource_extraction_mode);

    let found = modscan::scan(dir);
    let subs = modscan::scan_subs(dir);
    let apps = modscan::scan_apps(dir);
    if found.is_empty() && subs.is_empty() && apps.is_empty() {
        // Même sauvetage que pour une archive : le contenu peut être enfermé
        // dans un zip posé dans le dossier (§7.3).
        let rescued = rescue_nested_archives(
            ctx,
            index,
            conn,
            cfg,
            rules,
            library,
            &name,
            dir,
            copy,
            res_mode,
            &mut result,
        );
        // Type non reconnu : jamais perdu, rangé comme « autre mod » (§7.3).
        if !rescued {
            if let Some(other) = crate::others::import_other(conn, library, &name, dir, copy, res_mode) {
                if let Err(e) = crate::others::activate_other(conn, cfg, &other.id) {
                    log::warn!("auto_activate_other {}: {e}", other.id);
                }
                result.others.push(other);
            } else {
                result.error = Some(crate::errors::NOTHING_TO_IMPORT_IN_FOLDER.into());
            }
        }
        drop_unused_kept_source(conn, cfg, kept_archive);
        return result;
    }
    let targets = targets_of(&found, decisions);
    ctx.sub(0, targets.len(), name.clone());
    ctx.file_ratio(index, SCAN_SHARE);

    // Pack (§4.4) : un dossier multi-voitures regroupe ses mods sous sa source.
    let pack = (found.len() > 1).then_some(name.as_str());
    for (i, fm) in targets.iter().enumerate() {
        ctx.phase(
            PHASE_FILING,
            fm.dir
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
        );
        ctx.sub(i + 1, targets.len(), fm_id(fm));
        let decision = decision_for(decisions, &fm_id(fm));
        match process_found(
            conn,
            cfg,
            rules,
            library,
            &name,
            fm,
            copy,
            pack,
            decision,
            true,
            kept_archive,
        ) {
            Ok(imported) => result.mods.push(imported),
            Err(e) => {
                result.error.get_or_insert_with(String::new).push_str(&format!("{e}; "));
            }
        }
        ctx.file_ratio(index, filing_ratio(i + 1, targets.len()));
    }

    // Activation par défaut des mods importés (§4.2).
    auto_activate(conn, cfg, &result.mods);

    // Reprise : voir `file_extracted`, même raison.
    if !is_resume(decisions) {
        file_tail(
            ctx,
            index,
            conn,
            cfg,
            rules,
            library,
            &name,
            dir,
            &found,
            &subs,
            &apps,
            copy,
            res_mode,
            &mut result,
        );
    }

    drop_unused_kept_source(conn, cfg, kept_archive);
    result
}

/// Range ce qui suit les mods d'un item : sous-éléments rattachés (§12bis.2),
/// apps Python (§12bis.4) et balayage des restes (§7.3).
///
/// Extrait en commun des deux chemins d'import (archive et dossier) au moment
/// d'y ajouter la progression : les trois étapes occupent la queue de la barre,
/// et deux copies du découpage en bandes auraient divergé à la première
/// retouche.
#[allow(clippy::too_many_arguments)]
fn file_tail(
    ctx: &ImportCtx,
    index: usize,
    conn: &Connection,
    cfg: &AppConfig,
    rules: &Rules,
    library: &Path,
    name: &str,
    root: &Path,
    found: &[modscan::FoundMod],
    subs: &[modscan::FoundSub],
    apps: &[modscan::FoundApp],
    copy: bool,
    res_mode: crate::resources::ExtractionMode,
    result: &mut ArchiveResult,
) {
    if !subs.is_empty() {
        result.subs = crate::submods::import_subs_reported(
            conn,
            cfg,
            library,
            name,
            subs,
            copy,
            res_mode,
            &|done, total, sub_name| {
                ctx.sub(done, total, sub_name.to_string());
                ctx.file_ratio(index, tail_ratio(TAIL_START, TAIL_SUBS_END, done, total));
            },
        );
    }
    ctx.file_ratio(index, TAIL_SUBS_END);

    if !apps.is_empty() {
        result.apps = crate::apps::import_apps(conn, library, name, apps, copy, res_mode);
        auto_activate_apps(conn, cfg, &result.apps);
    }
    ctx.file_ratio(index, TAIL_APPS_END);

    // Ce qui reste à côté des mods reconnus (§7.3) : jamais perdu, y compris le
    // contenu de zips imbriqués (ex. mods CMRT-style qui livrent une app ET un
    // zip séparé visant `content/gui/...`).
    let consumed = consumed_paths(found, subs, apps);
    let found_ids: Vec<(String, ModKind)> = found.iter().map(|fm| (fm_id(fm), fm.kind)).collect();
    sweep_leftovers(
        ctx, index, conn, cfg, rules, library, name, root, &consumed, &found_ids, copy, res_mode, result,
    );
}

// --- Import en masse (§4.2) -------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct BulkMod {
    pub id: String,
    pub kind: String,
    pub name: Option<String>,
    /// "new" | "update" | "duplicate" | "ambiguous"
    pub status: String,
    pub existing_id: Option<String>,
    pub existing_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BulkEntry {
    pub subfolder: String,
    pub path: String,
    /// Sous-dossier sans structure AC reconnaissable.
    pub ignored: bool,
    pub mods: Vec<BulkMod>,
}

/// Classe un mod trouvé sans rien écrire (§4.2 phase d'analyse).
fn classify(
    conn: &Connection,
    id: &str,
    kind: &str,
    brand: &str,
    name: &str,
    dir: &Path,
) -> Result<(String, Option<String>, Option<String>), String> {
    if crate::overlay::mod_exists(conn, id).map_err(|e| e.to_string())? {
        let sig = identity::content_signature(dir);
        let existing = crate::overlay::active_signature(conn, id).map_err(|e| e.to_string())?;
        if existing.as_deref() == Some(sig.as_str()) {
            Ok(("duplicate".into(), None, None))
        } else {
            Ok(("update".into(), None, None))
        }
    } else {
        let fuzzy = crate::overlay::find_fuzzy(conn, kind, brand, name, id)
            .map_err(|e| e.to_string())?
            .into_iter()
            .next();
        match fuzzy {
            Some(m) => Ok(("ambiguous".into(), Some(m.id_interne), m.display_name)),
            None => Ok(("new".into(), None, None)),
        }
    }
}

/// Phase d'analyse (§4.2) : scanne chaque sous-dossier direct du parent et
/// classe les mods **sans rien écrire**. Un seul niveau de sous-dossiers.
pub fn analyze_bulk(conn: &Connection, _cfg: &AppConfig, parent: &Path) -> Result<Vec<BulkEntry>, String> {
    if !parent.is_dir() {
        return Err(crate::errors::NOT_A_DIRECTORY.into());
    }
    let mut subs: Vec<PathBuf> = std::fs::read_dir(parent)
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    subs.sort();

    let mut entries = Vec::new();
    for sub in subs {
        let subfolder = sub
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let path = sub.to_string_lossy().into_owned();
        let found = modscan::scan(&sub);
        if found.is_empty() {
            entries.push(BulkEntry {
                subfolder,
                path,
                ignored: true,
                mods: Vec::new(),
            });
            continue;
        }
        let mut mods = Vec::new();
        for fm in &found {
            let id = fm
                .dir
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let ui = match fm.kind {
                ModKind::Car => uijson::read_car(&fm.dir),
                ModKind::Track => uijson::read_track(&fm.dir),
            }
            .unwrap_or_default();
            let kind_str = format!("{:?}", fm.kind);
            let brand = ui.brand.clone().unwrap_or_default();
            let name = ui.name.clone().unwrap_or_else(|| id.clone());
            let (status, existing_id, existing_name) = classify(conn, &id, &kind_str, &brand, &name, &fm.dir)?;
            mods.push(BulkMod {
                id,
                kind: kind_str,
                name: ui.name,
                status,
                existing_id,
                existing_name,
            });
        }
        entries.push(BulkEntry {
            subfolder,
            path,
            ignored: false,
            mods,
        });
    }
    Ok(entries)
}

/// Instruction d'exécution pour un sous-dossier (après arbitrage).
#[derive(Debug, Clone, Deserialize)]
pub struct BulkExecItem {
    pub path: String,
    /// Ids de mods à ignorer (doublons que l'utilisateur ne réimporte pas).
    #[serde(default)]
    pub skip_ids: Vec<String>,
    /// Ids de mods ambigus à écraser (sinon « garder les deux »).
    #[serde(default)]
    pub replace_ids: Vec<String>,
}

/// Exécution de l'import en masse selon les décisions (§4.2). Reprenable :
/// relancer l'analyse après interruption reclasse les mods déjà importés en
/// « doublon »/« mise à jour », donc rien n'est traité de travers.
pub fn execute_bulk(
    ctx: &ImportCtx,
    db: &crate::overlay::Db,
    cfg: &AppConfig,
    rules: &Rules,
    items: &[BulkExecItem],
    copy: bool,
) -> Result<Vec<ArchiveResult>, String> {
    // Même protocole de progression que les autres chemins d'import (§4.2bis) :
    // c'est celui qui traite le plus de mods d'un coup, il serait le dernier à
    // devoir se contenter d'un compteur d'entrées.
    let paths: Vec<String> = items.iter().map(|it| it.path.clone()).collect();
    let sizes = size_batch(ctx, &paths);
    let bucket = if copy { Bucket::FolderCopy } else { Bucket::FolderMove };
    check_disk_space(cfg, &sizes, if copy { 1.0 } else { 0.0 })?;
    ctx.plan(
        paths
            .iter()
            .zip(&sizes)
            .map(|(p, &size)| ItemPlan {
                label: source_label(Path::new(p)),
                extract_w: 0.0,
                file_w: ITEM_OVERHEAD_SECS + ctx.estimate(bucket, size),
            })
            .collect(),
    );

    let mut results = Vec::with_capacity(items.len());
    for (i, it) in items.iter().enumerate() {
        if ctx.cancelled() {
            break;
        }
        let label = source_label(Path::new(&it.path));
        ctx.set_current(i, PHASE_FILING, label.clone());
        let started = Instant::now();
        // Verrou par entrée, jamais un seul pour tout le lot : un import en
        // masse ne doit pas geler les écrans qui lisent l'overlay.
        let result = match db.0.lock() {
            Ok(conn) => exec_one(ctx, &conn, cfg, rules, i, it, copy),
            Err(e) => failed_result(&label, e.to_string()),
        };
        ctx.record(bucket, sizes[i], started.elapsed().as_secs_f64());
        results.push(result);
        ctx.finish_item(i);
    }
    Ok(results)
}

fn exec_one(
    ctx: &ImportCtx,
    conn: &Connection,
    cfg: &AppConfig,
    rules: &Rules,
    index: usize,
    it: &BulkExecItem,
    copy: bool,
) -> ArchiveResult {
    let dir = Path::new(&it.path);
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut result = ArchiveResult {
        archive: name.clone(),
        mods: Vec::new(),
        error: None,
        subs: Vec::new(),
        apps: Vec::new(),
        others: Vec::new(),
        extras: 0,
    };

    let Some(library) = &cfg.library_path else {
        result.error = Some(crate::errors::LIBRARY_NOT_CONFIGURED.into());
        return result;
    };

    let found = modscan::scan(dir);
    ctx.sub(0, found.len(), name.clone());
    ctx.file_ratio(index, SCAN_SHARE);
    // Conservation du dossier source (§10/§11), avant tout rangement.
    let kept_archive = keep_source(cfg, dir, &name);
    // Pack (§4.4) : plusieurs mods issus du même dossier partagent leur source.
    let pack = (found.len() > 1).then_some(name.as_str());
    for (i, fm) in found.iter().enumerate() {
        let id = fm
            .dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        ctx.sub(i + 1, found.len(), id.clone());
        ctx.file_ratio(index, filing_ratio(i + 1, found.len()));
        if it.skip_ids.iter().any(|s| s == &id) {
            continue;
        }
        // Import en masse (§4.2) : jamais de blocage au fil de l'eau. Un cas
        // ambigu retombe sur le défaut sûr (extension, jamais destructif).
        match process_found(
            conn,
            cfg,
            rules,
            library,
            &name,
            fm,
            copy,
            pack,
            None,
            false,
            kept_archive.as_deref(),
        ) {
            Ok(imported) => {
                if let Some(conflict) = imported.conflict.clone() {
                    let action = if it.replace_ids.iter().any(|r| r == &imported.id_interne) {
                        "replace"
                    } else {
                        "keep_both"
                    };
                    let _ = resolve_conflict(conn, cfg, &imported.id_interne, &conflict.existing_id, action);
                }
                result.mods.push(imported);
            }
            Err(e) => {
                result.error.get_or_insert_with(String::new).push_str(&format!("{e}; "));
            }
        }
    }
    // Activation par défaut des mods importés (§4.2).
    auto_activate(conn, cfg, &result.mods);

    // Ce qui entoure les mods reconnus (§7.3/§4.5.3). Ce chemin d'import en
    // masse n'avait aucun balayage : tout ce qui n'était pas une voiture ou un
    // circuit y disparaissait, à l'exception des fonts et drivers qu'une copie
    // globale attrapait au passage. Skins, sons et apps sont marqués consommés
    // sans être importés — l'import en masse ne les traite pas davantage
    // qu'avant, mais il ne les prend plus pour des restes.
    let subs = modscan::scan_subs(dir);
    let apps = modscan::scan_apps(dir);
    let consumed = consumed_paths(&found, &subs, &apps);
    let found_ids: Vec<(String, ModKind)> = found.iter().map(|fm| (fm_id(fm), fm.kind)).collect();
    let res_mode = crate::resources::ExtractionMode::parse(&cfg.prefs.resource_extraction_mode);
    sweep_leftovers(
        ctx,
        index,
        conn,
        cfg,
        rules,
        library,
        &name,
        dir,
        &consumed,
        &found_ids,
        copy,
        res_mode,
        &mut result,
    );
    drop_unused_kept_source(conn, cfg, kept_archive.as_deref());
    result
}

/// Résout un conflit flou (§4.2) selon le choix utilisateur.
/// `keep_both` : conserve les deux mods séparément.
/// `replace`   : supprime l'ancien mod (fichiers + overlay) au profit du nouveau.
pub fn resolve_conflict(
    conn: &Connection,
    cfg: &AppConfig,
    new_id: &str,
    old_id: &str,
    action: &str,
) -> Result<(), String> {
    let now = Local::now().to_rfc3339();
    match action {
        "keep_both" => {
            crate::overlay::add_history(
                conn,
                new_id,
                &now,
                "UPDATE_KEPT_BOTH",
                &serde_json::json!({ "key": "keptBoth", "old": old_id }).to_string(),
            )
            .map_err(|e| e.to_string())?;
        }
        "replace" => {
            if let Some(old) = crate::overlay::get_mod(conn, old_id).map_err(|e| e.to_string())? {
                let kind = if old.kind == "Track" {
                    ModKind::Track
                } else {
                    ModKind::Car
                };
                // Supprime les fichiers bibliothèque de l'ancien mod.
                if let Some(lib) = &cfg.library_path {
                    let dir = lib.join(kind.content_folder()).join(old_id);
                    let _ = std::fs::remove_dir_all(&dir);
                }
                // Retire un déploiement orphelin dans content/ (garde-fou : symlink
                // hérité ou déploiement hardlinks marqué, jamais un vrai dossier).
                if let Some(ac) = &cfg.ac_install_path {
                    let cpath = ac.join("content").join(kind.content_folder()).join(old_id);
                    if let Ok(meta) = std::fs::symlink_metadata(&cpath) {
                        if meta.file_type().is_symlink() {
                            let _ = std::fs::remove_dir(&cpath);
                        } else if crate::deploy::is_deployed(&cpath) {
                            let _ = crate::deploy::remove_deployment(&cpath);
                        }
                    }
                }
                crate::overlay::delete_mod(conn, old_id).map_err(|e| e.to_string())?;
            }
            crate::overlay::add_history(
                conn,
                new_id,
                &now,
                "UPDATE_REPLACE",
                &serde_json::json!({ "key": "replaced", "old": old_id }).to_string(),
            )
            .map_err(|e| e.to_string())?;
        }
        _ => return Err(format!("action inconnue : {action}")),
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn process_found(
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
    // Archive/dossier source déjà conservé pour cet import (§10/§11), partagé
    // entre tous les mods d'une même archive/dossier. `None` si le réglage est
    // désactivé ou la copie a échoué.
    kept_archive: Option<&str>,
) -> Result<ImportedMod, String> {
    let id_interne = fm
        .dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or(crate::errors::UNNAMED_MOD_FOLDER)?;

    let ui = match fm.kind {
        ModKind::Car => uijson::read_car(&fm.dir),
        ModKind::Track => uijson::read_track(&fm.dir),
    }
    .unwrap_or_default();

    let kind_str = format!("{:?}", fm.kind); // "Car" | "Track"
    let brand = ui.brand.clone().unwrap_or_default();
    let name = ui.name.clone().unwrap_or_else(|| id_interne.clone());
    // Extraction des fichiers annexes (§4.5.2) : réglage global, jamais reposé
    // à chaque import.
    let res_mode = crate::resources::ExtractionMode::parse(&cfg.prefs.resource_extraction_mode);
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

    // --- Résolution d'identité (§4.2/§4.4) ---
    let existing = crate::overlay::get_mod(conn, &id_interne).map_err(|e| e.to_string())?;
    let is_update = existing.is_some();

    // Contenu ciblant un id déjà connu : décider mise à jour vs couche/extension
    // AVANT d'agir (§4.4), pour ne jamais détruire du contenu par un faux « MAJ ».
    if let Some(existing) = &existing {
        // Ré-import à l'identique : même id ET même signature → ni version ni
        // couche (évite le faux « MAJ » quand on réimporte la même archive).
        let active_sig = crate::overlay::active_signature(conn, &id_interne).map_err(|e| e.to_string())?;
        if active_sig.as_deref() == Some(signature.as_str()) {
            return Ok(ImportedMod {
                id_interne,
                kind: kind_str,
                display_name: Some(name),
                outcome: "DUPLICATE".into(),
                version_label: ui.version,
                conflict: None,
                added_count: None,
                overwritten_count: None,
                existing_total: None,
                resources_extracted: 0,
            });
        }

        // Règle absolue (§4.4) : le contenu de base Kunos (is_stock) ne reçoit
        // JAMAIS de remplacement — toujours une couche par-dessus. Sinon,
        // comparer les fichiers pour classer update / extension / ambigu.
        let (class, diff) = if existing.is_stock {
            // Toujours une extension (règle absolue). On calcule néanmoins le
            // décompte pour l'affichage, en comparant au dossier de base Kunos
            // (content/<type>s/<id>) : le stock n'a pas de version bibliothèque.
            let diff = cfg.ac_install_path.as_ref().and_then(|ac| {
                let base = ac.join("content").join(fm.kind.content_folder()).join(&id_interne);
                base.is_dir().then(|| identity::diff_content(&fm.dir, &base))
            });
            (ImportClass::Extension, diff)
        } else {
            let active_path = crate::overlay::active_library_path(conn, &id_interne)
                .map_err(|e| e.to_string())?
                .and_then(|p| crate::libpath::resolve(cfg.library_path.as_deref(), &p));
            let diff = match active_path {
                Some(active_path) => identity::diff_content(&fm.dir, &active_path),
                // Pas de dossier de base à comparer : on ne peut pas prouver que
                // c'est une extension → comportement historique (mise à jour).
                None => crate::identity::DiffStats {
                    added: 0,
                    overwritten: 0,
                    existing_total: 0,
                },
            };
            (classify_diff(&diff), Some(diff))
        };

        // La décision explicite de l'utilisateur (§4.4) prime sur l'auto-classement.
        let resolved = match decision {
            Some("update") => ImportClass::Update,
            Some("extension") => ImportClass::Extension,
            _ => class,
        };

        match resolved {
            ImportClass::Update => { /* poursuit vers le chemin UPDATE_REPLACE ci-dessous */ }
            ImportClass::Extension => {
                // Range comme couche à part — ne touche jamais la base (§4.4).
                let name_layer = layer_name(fm, archive_name);
                let (_, resources_extracted) = layers::store_layer(
                    conn,
                    library,
                    &id_interne,
                    fm.kind,
                    &name_layer,
                    &fm.dir,
                    copy,
                    diff.as_ref().unwrap_or(&crate::identity::DiffStats {
                        added: 0,
                        overwritten: 0,
                        existing_total: 0,
                    }),
                    archive_name,
                    res_mode,
                )?;
                // Couche active par défaut : composer tout de suite pour qu'elle
                // apparaisse en jeu (§4.4). Best-effort, comme auto_activate.
                let _ = crate::compose::recompose(conn, cfg, &id_interne);
                return Ok(ImportedMod {
                    id_interne,
                    kind: kind_str,
                    display_name: Some(name),
                    outcome: "EXTENSION".into(),
                    version_label: ui.version,
                    conflict: None,
                    added_count: diff.map(|d| d.added),
                    overwritten_count: diff.map(|d| d.overwritten),
                    existing_total: diff.map(|d| d.existing_total),
                    resources_extracted,
                });
            }
            ImportClass::Ambiguous => {
                if block_ambiguous {
                    // Rien écrit : on attend le choix de l'utilisateur (§4.4).
                    return Ok(ImportedMod {
                        id_interne,
                        kind: kind_str,
                        display_name: Some(name),
                        outcome: "AMBIGUOUS".into(),
                        version_label: ui.version,
                        conflict: None,
                        added_count: diff.map(|d| d.added),
                        overwritten_count: diff.map(|d| d.overwritten),
                        existing_total: diff.map(|d| d.existing_total),
                        resources_extracted: 0,
                    });
                }
                // Import en masse : défaut sûr = extension (jamais destructif).
                let name_layer = layer_name(fm, archive_name);
                let (_, resources_extracted) = layers::store_layer(
                    conn,
                    library,
                    &id_interne,
                    fm.kind,
                    &name_layer,
                    &fm.dir,
                    copy,
                    diff.as_ref().unwrap_or(&crate::identity::DiffStats {
                        added: 0,
                        overwritten: 0,
                        existing_total: 0,
                    }),
                    archive_name,
                    res_mode,
                )?;
                let _ = crate::compose::recompose(conn, cfg, &id_interne);
                return Ok(ImportedMod {
                    id_interne,
                    kind: kind_str,
                    display_name: Some(name),
                    outcome: "EXTENSION".into(),
                    version_label: ui.version,
                    conflict: None,
                    added_count: diff.map(|d| d.added),
                    overwritten_count: diff.map(|d| d.overwritten),
                    existing_total: diff.map(|d| d.existing_total),
                    resources_extracted,
                });
            }
        }
    }

    let conflict = if is_update {
        None
    } else {
        crate::overlay::find_fuzzy(conn, &kind_str, &brand, &name, &id_interne)
            .map_err(|e| e.to_string())?
            .into_iter()
            .next()
            .map(|m| FuzzyConflict {
                existing_id: m.id_interne,
                existing_name: m.display_name,
            })
    };

    // --- Rangement bibliothèque ---
    let version_folder = version_folder_name(ui.version.as_deref(), &now);
    let dest = unique_dir(
        &library
            .join(fm.kind.content_folder())
            .join(&id_interne)
            .join(&version_folder),
    );
    // Copie (préserve la source) ou déplacement adaptatif (rename / copie+suppr).
    // Fichiers annexes (§4.5.2) redirigés vers le dossier ressources du mod,
    // jamais dans le contenu de jeu, selon le réglage global.
    let resources_dest = crate::resources::resources_dir(library, fm.kind, &id_interne);
    let resources_extracted = crate::resources::file_mod(
        &fm.dir,
        &dest,
        &resources_dest,
        res_mode,
        !copy,
        crate::resources::Source::ModFolder,
    )?;
    let library_path = crate::libpath::to_relative(Some(library), &dest);

    // --- Écriture overlay ---
    crate::overlay::upsert_mod(
        conn,
        &id_interne,
        &kind_str,
        ui.brand.as_deref(),
        Some(&name),
        &id_hash,
        ui.year,
        &now,
    )
    .map_err(|e| e.to_string())?;

    // Source de pack (§4.4) — uniquement quand l'import regroupe plusieurs mods.
    if pack.is_some() {
        crate::overlay::set_source(conn, &id_interne, pack, None).map_err(|e| e.to_string())?;
    }

    let version_id = Uuid::new_v4().to_string();
    crate::overlay::insert_version(
        conn,
        &version_id,
        &id_interne,
        ui.version.as_deref(),
        ui.author.as_deref(),
        &now,
        &library_path,
        Some(archive_name),
        &signature,
        &csp,
        &skins,
        &layouts,
        &ui.tags,
        published_at.as_deref(),
    )
    .map_err(|e| e.to_string())?;

    // Archive/dossier source conservé (§10/§11), s'il y en a un pour cet import.
    if let Some(kept) = kept_archive {
        crate::overlay::set_kept_archive(conn, &version_id, kept).map_err(|e| e.to_string())?;
    }

    // Taille sur disque (§9.4) : calculée maintenant, le dossier final venant
    // d'être créé par la copie/le déplacement ci-dessus.
    let size_bytes = inspect::dir_size_bytes(&dest) as i64;
    crate::overlay::update_version_size(conn, &version_id, size_bytes).map_err(|e| e.to_string())?;

    crate::overlay::set_active_version(conn, &id_interne, &version_id).map_err(|e| e.to_string())?;

    // Harmonisation des tags + extraction specs/pays (§5.4), stockée en overlay.
    let class = ui.class.clone().unwrap_or_default();
    let h = harmonize::compute(rules, fm.kind, &ui.tags, &name, &class, ui.country.as_deref());
    harmonize::store(conn, &id_interne, &h, ui.country.as_deref()).map_err(|e| e.to_string())?;

    let outcome = if is_update { "UPDATE_REPLACE" } else { "IMPORT" };
    // Détails structurés (§ i18n) : rendus localisés côté front via `history.<key>`.
    let details = match (&is_update, &ui.version) {
        (true, Some(v)) => serde_json::json!({ "key": "updated", "version": v }).to_string(),
        (true, None) => serde_json::json!({ "key": "updatedNoVersion" }).to_string(),
        (false, _) => serde_json::json!({ "key": "imported", "archive": archive_name }).to_string(),
    };
    crate::overlay::add_history(conn, &id_interne, &now, outcome, &details).map_err(|e| e.to_string())?;

    Ok(ImportedMod {
        id_interne,
        kind: kind_str,
        display_name: Some(name),
        outcome: outcome.to_string(),
        version_label: ui.version,
        conflict,
        added_count: None,
        overwritten_count: None,
        existing_total: None,
        resources_extracted,
    })
}

/// Nom de couche lisible : id du dossier entrant s'il diffère de l'archive,
/// sinon nom de l'archive (assaini). Évite d'écraser une couche existante.
fn layer_name(fm: &modscan::FoundMod, archive_name: &str) -> String {
    let dir = fm
        .dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let base = if dir.is_empty() { archive_name.to_string() } else { dir };
    sanitize(&base)
}

/// Nom de dossier de version lisible hors de l'app : label assaini, sinon date.
fn version_folder_name(version: Option<&str>, now_rfc3339: &str) -> String {
    match version.map(sanitize).filter(|s| !s.is_empty()) {
        Some(v) => format!("v{v}"),
        None => {
            // "2026-06-26T22:40:13+02:00" -> "2026-06-26_2240"
            let compact: String = now_rfc3339
                .chars()
                .take(16)
                .map(|c| if c == 'T' { '_' } else { c })
                .filter(|c| *c != ':')
                .collect();
            format!("import-{compact}")
        }
    }
}

fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| if r#"\/:*?"<>|"#.contains(c) { '_' } else { c })
        .collect::<String>()
        .trim()
        .to_string()
}

/// Garantit un chemin libre : ajoute un suffixe court si déjà pris.
pub(crate) fn unique_dir(base: &Path) -> PathBuf {
    if !base.exists() {
        return base.to_path_buf();
    }
    let suffix = &Uuid::new_v4().to_string()[..8];
    let mut p = base.as_os_str().to_owned();
    p.push("-");
    p.push(suffix);
    PathBuf::from(p)
}

fn make_temp_dir() -> std::io::Result<PathBuf> {
    let dir = std::env::temp_dir().join(format!("pitbox-import-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use std::process::Command;

    /// Import d'un dossier hors du lot qui l'encadre en production : contexte
    /// de progression silencieux, item unique. Reproduit ce que fait
    /// `import_folders` autour de lui, copie de la source comprise — sans quoi
    /// les tests qui vérifient `kept_archive_path` passeraient à côté.
    fn import_folder_for_test(
        conn: &Connection,
        cfg: &AppConfig,
        rules: &Rules,
        dir: &Path,
        copy: bool,
        decisions: &[ImportDecision],
    ) -> ArchiveResult {
        let ctx = ImportCtx::silent();
        let kept = keep_source(cfg, dir, &source_label(dir));
        import_one_folder(&ctx, conn, cfg, rules, 0, dir, copy, decisions, kept.as_deref())
    }

    /// Crée un dossier voiture synthétique <root>/<id>/ui/ui_car.json + un .kn5.
    fn make_fake_car(root: &Path, id: &str) {
        let ui = root.join(id).join("ui");
        std::fs::create_dir_all(&ui).unwrap();
        std::fs::write(
            ui.join("ui_car.json"),
            r#"{"name":"My Test Car","brand":"TestBrand","tags":["gt3","turbo","italy"],"class":"race","year":2020,"version":"1.0","author":"Tester"}"#,
        )
        .unwrap();
        std::fs::write(root.join(id).join("model.kn5"), b"FAKE_KN5_DATA").unwrap();
    }

    /// Voiture synthétique <root>/<id> avec `ui/ui_car.json` + les fichiers
    /// relatifs listés (contenu = leur nom, pour varier les tailles/signatures).
    fn make_car_with_files(root: &Path, id: &str, files: &[&str]) {
        let base = root.join(id);
        std::fs::create_dir_all(base.join("ui")).unwrap();
        std::fs::write(
            base.join("ui").join("ui_car.json"),
            br#"{"name":"Spa Test","brand":"B","tags":[]}"#,
        )
        .unwrap();
        for f in files {
            let p = base.join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, f.as_bytes()).unwrap();
        }
    }

    /// Zippe <src>/* dans <zip> via 7-Zip.
    fn zip_dir(sevenzip: &Path, src: &Path, zip: &Path) {
        let status = Command::new(sevenzip)
            .current_dir(src)
            .arg("a")
            .arg(zip)
            .arg("*")
            .status()
            .unwrap();
        assert!(status.success(), "7z a a échoué");
    }

    /// Règle (§7.3/§4.5.2) : une voiture enfermée dans un zip imbriqué, avec sa
    /// notice posée à côté, est importée comme une voiture — et la notice lui
    /// est rattachée.
    ///
    /// Bug réel : rien n'étant reconnu à la racine, l'archive entière partait en
    /// « autre mod ». La voiture n'entrait jamais en bibliothèque, et le `.zip`
    /// brut se retrouvait lié à la racine du dossier du jeu. Packaging courant
    /// (notice + archive), pas un cas tordu.
    #[test]
    fn car_locked_inside_a_nested_zip_is_imported_with_its_readme() {
        let Some(sevenzip) = crate::detect::find_7zip() else {
            eprintln!("7-Zip introuvable — test ignoré");
            return;
        };
        let base = crate::testutil::temp_dir("nested-zip");

        // inner.zip contient le dossier de la voiture, rien d'autre.
        let car_src = base.join("carsrc");
        make_fake_car(&car_src, "nested_car");
        let inner = base.join("inner.zip");
        zip_dir(&sevenzip, &car_src, &inner);

        // outer.zip = notice + inner.zip. Rien de reconnaissable à la racine.
        let outer_src = base.join("outersrc");
        std::fs::create_dir_all(&outer_src).unwrap();
        std::fs::write(outer_src.join("readme.txt"), b"notice du mod").unwrap();
        std::fs::copy(&inner, outer_src.join("inner.zip")).unwrap();
        let outer = base.join("outer.zip");
        zip_dir(&sevenzip, &outer_src, &outer);

        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let db = crate::overlay::Db(std::sync::Mutex::new(
            crate::overlay::open(&base.join("overlay.sqlite")).unwrap(),
        ));
        let cfg = AppConfig {
            sevenzip_exe: Some(sevenzip.clone()),
            library_path: Some(library.clone()),
            ..Default::default()
        };
        let rules = crate::rules::default_rules();

        let res = import_archives(
            &ImportCtx::silent(),
            &db,
            &cfg,
            &rules,
            &[outer.to_string_lossy().into_owned()],
            &[],
        )
        .unwrap();

        assert_eq!(res[0].mods.len(), 1, "la voiture est reconnue, pas rangée en vrac");
        assert_eq!(res[0].mods[0].id_interne, "nested_car");
        assert_eq!(res[0].mods[0].outcome, "IMPORT");
        assert!(
            res[0].others.is_empty(),
            "plus de zip brut en « autre mod » : {:?}",
            res[0].others
        );
        // La notice suit la voiture, dans ses ressources (§4.5.2) — c'est un
        // document à lire, pas un ajout au jeu. Vérifié sur le disque : le
        // rangement d'une annexe à côté du mod n'incrémente aucun compteur du
        // rapport, seul l'emplacement fait foi.
        let res_dir = crate::resources::resources_dir(&library, ModKind::Car, "nested_car");
        assert!(
            res_dir.join("readme.txt").is_file(),
            "notice rangée dans les ressources de la voiture, pas laissée seule ({})",
            res_dir.display()
        );
    }

    #[test]
    fn mod_folder_contents_are_never_extracted_whatever_the_mode() {
        // Règle d'or (§4.5.1) : rien ne sort du dossier du mod, quel que soit le
        // réglage d'extraction. Bug réel — `logo.png`, `body_shadow.png` et
        // `tyre_*_shadow.png`, de vrais assets AC vivant à la racine du dossier
        // voiture, ont été sortis de 23 mods par un tri fondé sur l'extension.
        for mode in ["info_only", "all"] {
            let base = crate::testutil::temp_dir("import-golden");
            let library = base.join("library");
            std::fs::create_dir_all(&library).unwrap();
            let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
            let mut cfg = AppConfig {
                library_path: Some(library.clone()),
                ..Default::default()
            };
            cfg.prefs.resource_extraction_mode = mode.into();
            let rules = crate::rules::default_rules();

            let src = base.join("src");
            let files = [
                "model.kn5",
                "logo.png",
                "body_shadow.png",
                "tyre_0_shadow.png",
                "changelog.txt",
                "presentation.pdf",
                "livery_template.psd",
            ];
            make_car_with_files(&src, "annex_car", &files);
            let r = import_folder_for_test(&conn, &cfg, &rules, &src, true, &[]);
            assert_eq!(r.mods[0].outcome, "IMPORT");
            assert_eq!(r.mods[0].resources_extracted, 0, "rien extrait du mod ({mode})");

            let versions = crate::overlay::get_versions(&conn, "annex_car").unwrap();
            let content_dir = library.join(&versions[0].library_path);
            for f in files {
                assert!(content_dir.join(f).is_file(), "{f} conservé dans le mod ({mode})");
            }
            assert!(
                !library.join("resources").join("cars").join("annex_car").exists(),
                "aucun dossier ressources créé ({mode})"
            );
        }
    }

    #[test]
    fn documents_beside_the_mod_folder_are_extracted_to_resources() {
        // L'autre moitié de la règle (§4.5.1) : un PDF de présentation livré à la
        // racine de l'archive, à côté du dossier du mod, n'est pas du contenu
        // de jeu — il est rangé en ressources et jamais déployé dans AC.
        let base = crate::testutil::temp_dir("import-beside");
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            library_path: Some(library.clone()),
            ..Default::default()
        };
        let rules = crate::rules::default_rules();

        // Archive « à la RSS » : la voiture sous content/cars/, un PDF à côté.
        let src = base.join("RSS_Pack");
        make_fake_car(&src.join("content").join("cars"), "beside_car");
        std::fs::write(src.join("Read Me - Beside.pdf"), b"%PDF").unwrap();

        let r = import_folder_for_test(&conn, &cfg, &rules, &src, true, &[]);
        assert!(r.error.is_none(), "erreur inattendue: {:?}", r.error);
        assert_eq!(r.mods.len(), 1, "la voiture est reconnue");
        assert!(r.others.is_empty(), "une annexe ne devient pas un « autre mod »");
        assert_eq!(r.extras, 0, "ni un ajout au jeu : elle n'a rien à faire dans AC");
        assert!(
            library
                .join("resources")
                .join("cars")
                .join("beside_car")
                .join("Read Me - Beside.pdf")
                .is_file(),
            "PDF rangé dans les ressources de la voiture qu'''il accompagne"
        );
    }

    #[test]
    fn ancillary_extraction_mode_none_drops_files_beside_the_mod() {
        // §4.5.2, mode « Aucun » : rien n'est extrait vers la bibliothèque, mais
        // l'annexe ne finit pas non plus dans le contenu de jeu (règle absolue,
        // indépendante du réglage). Ne vaut que pour ce qui est livré à côté du
        // mod — dans le mod, l'annexe reste simplement en place.
        let base = crate::testutil::temp_dir("import-none");
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        let mut cfg = AppConfig {
            library_path: Some(library.clone()),
            ..Default::default()
        };
        cfg.prefs.resource_extraction_mode = "none".into();
        let rules = crate::rules::default_rules();

        let src = base.join("NonePack");
        make_fake_car(&src.join("content").join("cars"), "annex_car2");
        std::fs::write(src.join("changelog.txt"), b"notes").unwrap();

        let r = import_folder_for_test(&conn, &cfg, &rules, &src, true, &[]);
        assert!(r.error.is_none(), "erreur inattendue: {:?}", r.error);
        assert!(
            r.others.iter().all(|o| o.resources_extracted == 0),
            "rien extrait en mode Aucun"
        );
        assert!(
            !library.join("resources").exists(),
            "aucun dossier ressources en mode Aucun"
        );
        // Copie (pas déplacement) : la source garde son annexe intacte.
        assert!(src.join("changelog.txt").is_file());
    }

    #[test]
    fn extension_detected_not_replaced() {
        // Bug Spa (§4.4) : un import qui ajoute surtout des chemins nouveaux et
        // n'écrase que peu de fichiers est une COUCHE, pas une mise à jour — la
        // base ne doit jamais être remplacée/perdue.
        let base = crate::testutil::temp_dir("import");
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            library_path: Some(library.clone()),
            ..Default::default()
        };
        let rules = crate::rules::default_rules();

        // Base : 10 fichiers (dont model.kn5 pour la signature).
        let src1 = base.join("src1");
        make_car_with_files(
            &src1,
            "spa",
            &[
                "model.kn5",
                "data/a.ini",
                "data/b.ini",
                "data/c.ini",
                "data/d.ini",
                "data/e.ini",
                "data/f.ini",
                "data/g.ini",
                "data/h.ini",
            ],
        );
        let r1 = import_folder_for_test(&conn, &cfg, &rules, &src1, true, &[]);
        assert_eq!(r1.mods[0].outcome, "IMPORT");
        let base_version = crate::overlay::get_versions(&conn, "spa").unwrap();
        assert_eq!(base_version.len(), 1);
        let base_path = base_version[0].library_path.clone();

        // Extension : réécrit seulement ui_car.json (1/10) et ajoute 3 chemins
        // nouveaux (dont 2 .kn5 → signature différente, pas un doublon).
        let src2 = base.join("src2");
        make_car_with_files(&src2, "spa", &["new/layout1.kn5", "new/layout2.kn5", "new/extra.ini"]);
        let r2 = import_folder_for_test(&conn, &cfg, &rules, &src2, true, &[]);
        assert_eq!(r2.mods[0].outcome, "EXTENSION", "couche, pas mise à jour");
        assert_eq!(r2.mods[0].overwritten_count, Some(1));
        assert_eq!(r2.mods[0].added_count, Some(3));

        // La base est intacte : toujours une seule version, dossier + model.kn5 présents.
        assert_eq!(
            crate::overlay::get_versions(&conn, "spa").unwrap().len(),
            1,
            "aucune version ajoutée"
        );
        assert!(
            library.join(&base_path).join("model.kn5").is_file(),
            "contenu de base préservé"
        );
        // La couche est rangée à part.
        let layers = crate::overlay::list_layers(&conn, "spa").unwrap();
        assert_eq!(layers.len(), 1);
        assert!(library
            .join(&layers[0].library_path)
            .join("new")
            .join("layout1.kn5")
            .is_file());
    }

    #[test]
    fn stock_never_replaced() {
        // Règle absolue (§4.4) : un contenu de base Kunos ne reçoit jamais de
        // remplacement — tout import dessus est une couche, quelle que soit la
        // proportion de fichiers écrasés.
        let base = crate::testutil::temp_dir("import");
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            library_path: Some(library.clone()),
            ..Default::default()
        };
        let rules = crate::rules::default_rules();

        // Contenu de base indexé (is_stock=1), sans version bibliothèque.
        let now = chrono::Local::now().to_rfc3339();
        crate::overlay::upsert_stock_mod(&conn, "ks_spa", "Car", Some("Kunos"), Some("Spa"), &now).unwrap();

        // Import « version complète améliorée » par-dessus : recouvrement total.
        let src = base.join("src");
        make_car_with_files(&src, "ks_spa", &["model.kn5", "data/surfaces.ini"]);
        let r = import_folder_for_test(&conn, &cfg, &rules, &src, true, &[]);
        assert_eq!(r.mods[0].outcome, "EXTENSION", "le stock ne peut jamais être remplacé");
        assert_eq!(
            crate::overlay::get_versions(&conn, "ks_spa").unwrap().len(),
            0,
            "aucune version : base intacte"
        );
        assert_eq!(crate::overlay::list_layers(&conn, "ks_spa").unwrap().len(), 1);
        assert!(
            crate::overlay::get_mod(&conn, "ks_spa").unwrap().unwrap().is_stock,
            "reste contenu de base"
        );
    }

    #[test]
    fn ambiguous_blocks_then_resolves() {
        // Cas ambigu (§4.4) : import unitaire → rien écrit, on attend le choix ;
        // reprise avec la décision "update" → vraie mise à jour.
        let base = crate::testutil::temp_dir("import");
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            library_path: Some(library.clone()),
            ..Default::default()
        };
        let rules = crate::rules::default_rules();

        // Base : 5 fichiers.
        let src1 = base.join("src1");
        make_car_with_files(&src1, "amb", &["model.kn5", "data/a.ini", "data/b.ini", "data/c.ini"]);
        import_folder_for_test(&conn, &cfg, &rules, &src1, true, &[]);

        // Entrant : écrase 2/5 (ui_car.json + data/a.ini), signature changée
        // (model2.kn5), 1 chemin nouveau → coverage 0.4 = bande ambiguë.
        let src2 = base.join("src2");
        make_car_with_files(&src2, "amb", &["model2.kn5", "data/a.ini", "new/y.ini"]);
        let r = import_folder_for_test(&conn, &cfg, &rules, &src2, true, &[]);
        assert_eq!(r.mods[0].outcome, "AMBIGUOUS");
        assert_eq!(r.mods[0].overwritten_count, Some(2));
        assert_eq!(r.mods[0].existing_total, Some(5));
        assert_eq!(
            crate::overlay::get_versions(&conn, "amb").unwrap().len(),
            1,
            "rien écrit tant qu'on n'a pas décidé"
        );
        assert_eq!(crate::overlay::list_layers(&conn, "amb").unwrap().len(), 0);

        // Reprise avec la décision "update".
        let decisions = vec![ImportDecision {
            id: "amb".into(),
            decision: "update".into(),
        }];
        let r2 = import_folder_for_test(&conn, &cfg, &rules, &src2, true, &decisions);
        assert_eq!(r2.mods[0].outcome, "UPDATE_REPLACE");
        assert_eq!(
            crate::overlay::get_versions(&conn, "amb").unwrap().len(),
            2,
            "nouvelle version après décision"
        );
    }

    /// Règle (§4.4) : une reprise après arbitrage ne rejoue que le mod tranché.
    /// Rejouer ses voisins les faisait revenir en « doublon » — donc sans dégât,
    /// mais au prix du lot entier, et la barre annonçait un travail qui n'en
    /// était pas un.
    #[test]
    fn resuming_an_ambiguous_decision_touches_only_the_decided_mod() {
        let base = crate::testutil::temp_dir("import-resume");
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            library_path: Some(library.clone()),
            ..Default::default()
        };
        let rules = crate::rules::default_rules();

        // Un pack de deux voitures : une qui deviendra ambiguë, une voisine.
        let src1 = base.join("src1");
        make_car_with_files(&src1, "amb", &["model.kn5", "data/a.ini", "data/b.ini", "data/c.ini"]);
        make_fake_car(&src1, "neighbour");
        import_folder_for_test(&conn, &cfg, &rules, &src1, true, &[]);

        // Second import du même pack : « amb » change assez pour être ambiguë,
        // la voisine est à l'identique.
        let src2 = base.join("src2");
        make_car_with_files(&src2, "amb", &["model2.kn5", "data/a.ini", "new/y.ini"]);
        make_fake_car(&src2, "neighbour");
        let r = import_folder_for_test(&conn, &cfg, &rules, &src2, true, &[]);
        assert_eq!(r.mods.len(), 2, "les deux voitures sont vues au premier passage");

        // Reprise : seule « amb » est tranchée.
        let decisions = vec![ImportDecision {
            id: "amb".into(),
            decision: "update".into(),
        }];
        let r2 = import_folder_for_test(&conn, &cfg, &rules, &src2, true, &decisions);
        assert_eq!(r2.mods.len(), 1, "la voisine n'est pas rejouée : {:?}", r2.mods);
        assert_eq!(r2.mods[0].id_interne, "amb");
        assert_eq!(r2.mods[0].outcome, "UPDATE_REPLACE");
        assert_eq!(
            crate::overlay::get_versions(&conn, "amb").unwrap().len(),
            2,
            "nouvelle version après décision"
        );
        assert_eq!(
            crate::overlay::get_versions(&conn, "neighbour").unwrap().len(),
            1,
            "la voisine n'a pas bougé"
        );
    }

    /// Règle : l'avancement du rangement d'un item est croissant et reste dans
    /// [0,1] — c'est ce qui garantit que la barre globale, qui l'agrège, ne
    /// recule jamais (§4.2bis).
    #[test]
    fn filing_ratio_grows_monotonically_within_bounds() {
        assert!(filing_ratio(0, 0) > 0.0 && filing_ratio(0, 0) < 1.0, "item sans mod");
        let mut previous = SCAN_SHARE;
        for done in 1..=5 {
            let r = filing_ratio(done, 5);
            assert!(r > previous, "croissant à {done}/5 : {r} <= {previous}");
            assert!(r <= 1.0, "borné à 1 : {r}");
            previous = r;
        }
        assert!(
            filing_ratio(5, 5) < 1.0,
            "le dernier mod ne finit pas l'item : skins, apps et restes viennent après"
        );
    }

    /// Règle : la queue d'un item (§4.2bis) reprend exactement là où le dernier
    /// mod s'est arrêté et va jusqu'au bout, bande après bande. Un trou entre
    /// deux bandes ferait sauter la barre ; un chevauchement la ferait reculer.
    #[test]
    fn tail_bands_chain_from_the_last_mod_to_the_end_of_the_item() {
        // La séquence qu'un item parcourt réellement : dernier mod rangé, puis
        // 4 livrées, une app, 3 restes.
        let steps = [
            filing_ratio(3, 3),
            tail_ratio(TAIL_START, TAIL_SUBS_END, 0, 4),
            tail_ratio(TAIL_START, TAIL_SUBS_END, 4, 4),
            tail_ratio(TAIL_SUBS_END, TAIL_APPS_END, 1, 1),
            tail_ratio(TAIL_APPS_END, 1.0, 0, 3),
            tail_ratio(TAIL_APPS_END, 1.0, 3, 3),
        ];
        for pair in steps.windows(2) {
            assert!(pair[1] >= pair[0], "la barre ne recule jamais : {steps:?}");
        }
        assert_eq!(steps[0], TAIL_START, "la queue reprend là où le dernier mod s'arrête");
        assert_eq!(steps[steps.len() - 1], 1.0, "le balayage des restes ferme l'item");
        // Rien à traiter dans une bande : elle est derrière nous, pas devant.
        assert_eq!(tail_ratio(TAIL_APPS_END, 1.0, 0, 0), 1.0, "aucun reste = bande finie");
    }

    /// Règle : un lot qui ne peut pas tenir sur le volume de la bibliothèque est
    /// refusé AVANT d'écrire quoi que ce soit (§4.2bis) — un import mort à
    /// mi-parcours laisse un temporaire et une bibliothèque à nettoyer à la main.
    #[test]
    fn disk_space_check_refuses_a_batch_that_cannot_fit() {
        let base = crate::testutil::temp_dir("diskspace");
        let cfg = AppConfig {
            library_path: Some(base.to_path_buf()),
            ..Default::default()
        };
        assert!(check_disk_space(&cfg, &[1024], 1.5).is_ok(), "1 Ko tient toujours");
        let huge = u64::MAX / 4;
        assert_eq!(
            check_disk_space(&cfg, &[huge], 1.5),
            Err(crate::errors::NOT_ENOUGH_DISK_SPACE.to_string()),
            "4 exaoctets ne tiennent nulle part"
        );
    }

    /// Règle : une bibliothèque non configurée ne doit pas faire échouer le
    /// contrôle d'espace — il n'y a alors aucun volume à interroger, et refuser
    /// sur une information absente bloquerait un import parfaitement valide.
    #[test]
    fn disk_space_check_stays_silent_without_a_library() {
        let cfg = AppConfig::default();
        assert!(check_disk_space(&cfg, &[u64::MAX / 4], 1.5).is_ok(), "rien à vérifier");
    }

    /// Règle : une annulation demandée avant le lancement n'importe rien
    /// (§4.2bis). L'arrêt est constaté entre deux items, jamais au milieu du
    /// rangement d'un mod — donc ici, aucun item n'est entamé.
    #[test]
    fn cancelled_batch_imports_nothing() {
        let base = crate::testutil::temp_dir("import-cancel");
        let src = base.join("src");
        make_fake_car(&src, "never_imported");
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let db_path = base.join("overlay.sqlite");
        let db = crate::overlay::Db(std::sync::Mutex::new(crate::overlay::open(&db_path).unwrap()));
        let cfg = AppConfig {
            library_path: Some(library.clone()),
            ..Default::default()
        };
        let rules = crate::rules::default_rules();

        let out = import_folders(
            &ImportCtx::silent_cancelled(),
            &db,
            &cfg,
            &rules,
            &[src.to_string_lossy().into_owned()],
            true,
            &[],
        )
        .unwrap();
        assert!(out.is_empty(), "aucun item traité");
        assert!(
            crate::overlay::list_mods(&db.0.lock().unwrap()).unwrap().is_empty(),
            "rien n'a été écrit en overlay"
        );
    }

    #[test]
    fn full_import_pipeline() {
        // Nécessite 7-Zip ; sinon le test est ignoré silencieusement.
        let Some(sevenzip) = crate::detect::find_7zip() else {
            eprintln!("7-Zip introuvable — test ignoré");
            return;
        };

        let base = crate::testutil::temp_dir("import");
        let src = base.join("src");
        std::fs::create_dir_all(&src).unwrap();
        make_fake_car(&src, "test_car");
        let zip = base.join("test_car.zip");
        zip_dir(&sevenzip, &src, &zip);

        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let db_path = base.join("overlay.sqlite");
        let db = crate::overlay::Db(std::sync::Mutex::new(crate::overlay::open(&db_path).unwrap()));

        let cfg = AppConfig {
            sevenzip_exe: Some(sevenzip.clone()),
            library_path: Some(library.clone()),
            ..Default::default()
        };

        let rules = crate::rules::default_rules();

        // --- 1er import : NOUVEAU ---
        let zip_str = zip.to_string_lossy().into_owned();
        let res = import_archives(
            &ImportCtx::silent(),
            &db,
            &cfg,
            &rules,
            std::slice::from_ref(&zip_str),
            &[],
        )
        .unwrap();
        assert_eq!(res.len(), 1);
        assert!(res[0].error.is_none(), "erreur: {:?}", res[0].error);
        assert_eq!(res[0].mods.len(), 1);
        assert_eq!(res[0].mods[0].outcome, "IMPORT");
        assert_eq!(res[0].mods[0].id_interne, "test_car");

        // Fichier rangé dans la bibliothèque + ui_car.json préservé.
        let conn = db.0.lock().unwrap();
        let mods = crate::overlay::list_mods(&conn).unwrap();
        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0].display_name.as_deref(), Some("My Test Car"));
        assert_eq!(mods[0].version_count, 1);
        assert_eq!(mods[0].tags_from_mod, vec!["gt3", "turbo", "italy"]);

        // Harmonisation (§5.4) : gt3 -> #gt3 (catégorie), turbo -> aspiration,
        // italy -> pays ; les tags techniques/pays sont retirés du vocabulaire.
        assert_eq!(mods[0].category.as_deref(), Some("#gt3"));
        assert!(mods[0].tags_from_rule.contains(&"#gt3".to_string()));
        assert_eq!(mods[0].aspiration.as_deref(), Some("TURBO"));
        assert_eq!(mods[0].country.as_deref(), Some("Italy"));
        assert!(!mods[0].tags_from_rule.contains(&"turbo".to_string()));
        assert!(!mods[0].tags_from_rule.contains(&"italy".to_string()));
        let versions = crate::overlay::get_versions(&conn, "test_car").unwrap();
        let lib_ui = library.join(&versions[0].library_path).join("ui").join("ui_car.json");
        assert!(lib_ui.is_file(), "ui_car.json doit exister dans la bibliothèque");
        drop(conn); // libéré avant le prochain run_import, qui reprend le verrou lui-même.

        // --- 2e import de la MÊME archive : DOUBLON (pas de réimport) ---
        let res_dup = import_archives(
            &ImportCtx::silent(),
            &db,
            &cfg,
            &rules,
            std::slice::from_ref(&zip_str),
            &[],
        )
        .unwrap();
        assert_eq!(res_dup[0].mods[0].outcome, "DUPLICATE");
        assert_eq!(
            crate::overlay::list_mods(&db.0.lock().unwrap()).unwrap()[0].version_count,
            1,
            "réimport à l'identique → toujours une seule version"
        );

        // --- 3e import avec contenu modifié : MISE À JOUR (nouvelle version) ---
        std::fs::write(src.join("test_car").join("model.kn5"), b"DIFFERENT_KN5_CONTENT_XXL").unwrap();
        let zip2 = base.join("test_car_v2.zip");
        zip_dir(&sevenzip, &src, &zip2);
        let res2 = import_archives(
            &ImportCtx::silent(),
            &db,
            &cfg,
            &rules,
            &[zip2.to_string_lossy().into_owned()],
            &[],
        )
        .unwrap();
        assert_eq!(res2[0].mods[0].outcome, "UPDATE_REPLACE");
        let mods2 = crate::overlay::list_mods(&db.0.lock().unwrap()).unwrap();
        assert_eq!(mods2.len(), 1, "toujours un seul mod logique");
        assert_eq!(mods2[0].version_count, 2, "deux versions coexistent");
    }

    #[test]
    fn app_activated_by_default_on_import() {
        // Bug réel : une app importée restait inactive tant qu'on n'allait pas
        // cliquer « Activer » sur l'écran Apps — contrairement aux mods
        // voiture/circuit (`auto_activate`) et aux mods « autres »
        // (`activate_other` appelé juste après l'import), §4.2/§12bis.4.
        let base = crate::testutil::temp_dir("import-app-autoactivate");
        let library = base.join("library");
        let ac = base.join("ac");
        std::fs::create_dir_all(&library).unwrap();
        std::fs::create_dir_all(&ac).unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            library_path: Some(library.clone()),
            ac_install_path: Some(ac.clone()),
            ..Default::default()
        };
        let rules = crate::rules::default_rules();

        let src = base.join("MyApp");
        let app = src.join("apps").join("lua").join("MyApp");
        std::fs::create_dir_all(&app).unwrap();
        std::fs::write(app.join("MyApp.lua"), b"-- app").unwrap();

        let r = import_folder_for_test(&conn, &cfg, &rules, &src, true, &[]);
        assert!(r.error.is_none(), "erreur inattendue: {:?}", r.error);
        assert_eq!(r.apps.len(), 1);
        assert!(
            crate::activation::is_junction(&ac.join("apps").join("lua").join("MyApp")),
            "app activée dès l'import, sans action manuelle"
        );
    }

    #[test]
    fn unrecognized_folder_becomes_other_mod() {
        // Type non reconnu (ni voiture, circuit, skin, son, app) : jamais
        // perdu, rangé comme « autre mod » et activé par défaut (§7.3).
        let base = crate::testutil::temp_dir("import");
        let library = base.join("library");
        let ac = base.join("ac");
        std::fs::create_dir_all(&library).unwrap();
        std::fs::create_dir_all(ac.join("extension").join("config")).unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            library_path: Some(library.clone()),
            ac_install_path: Some(ac.clone()),
            ..Default::default()
        };
        let rules = crate::rules::default_rules();

        let src = base.join("MyShaderPack");
        let leaf = src.join("extension").join("config").join("new_thing");
        std::fs::create_dir_all(&leaf).unwrap();
        std::fs::write(leaf.join("settings.ini"), b"x").unwrap();

        let r = import_folder_for_test(&conn, &cfg, &rules, &src, true, &[]);
        assert!(r.error.is_none(), "erreur inattendue: {:?}", r.error);
        assert_eq!(r.mods.len(), 0);
        assert_eq!(r.others.len(), 1);
        assert_eq!(r.others[0].id, "MyShaderPack");
        assert!(library.join("others").join("MyShaderPack").exists());
        assert!(
            crate::activation::is_junction(&ac.join("extension").join("config").join("new_thing")),
            "activé par défaut comme les autres types (§4.2)"
        );
    }

    #[test]
    fn app_and_sibling_content_override_both_survive_import() {
        // Bug réel (mods style CMRT) : une app livrée avec, à côté, un dossier
        // qui vise `extension/...` (ou `content/...`) directement — avant ce
        // correctif, dès que l'app était reconnue, tout le reste du dossier
        // disparaissait silencieusement au tri (tout-ou-rien §7.3).
        let base = crate::testutil::temp_dir("import-leftover-dir");
        let library = base.join("library");
        let ac = base.join("ac");
        std::fs::create_dir_all(&library).unwrap();
        std::fs::create_dir_all(ac.join("extension").join("config")).unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            library_path: Some(library.clone()),
            ac_install_path: Some(ac.clone()),
            ..Default::default()
        };
        let rules = crate::rules::default_rules();

        let src = base.join("MyApp_Pack");
        let app = src.join("apps").join("lua").join("MyApp");
        std::fs::create_dir_all(&app).unwrap();
        std::fs::write(app.join("MyApp.lua"), b"-- app").unwrap();

        let leaf = src.join("extension").join("config").join("new_thing");
        std::fs::create_dir_all(&leaf).unwrap();
        std::fs::write(leaf.join("settings.ini"), b"x").unwrap();

        let r = import_folder_for_test(&conn, &cfg, &rules, &src, true, &[]);
        assert!(r.error.is_none(), "erreur inattendue: {:?}", r.error);
        assert_eq!(r.apps.len(), 1, "l'app est bien reconnue");
        assert_eq!(
            r.others.len(),
            1,
            "le dossier extension/ à côté de l'app ne doit plus être perdu"
        );
        assert!(library.join("apps").join("MyApp").join("MyApp.lua").is_file());
        assert!(
            crate::activation::is_junction(&ac.join("extension").join("config").join("new_thing")),
            "reste importé et activé comme autre mod, chemin préservé (extension/config/new_thing)"
        );
    }

    #[test]
    fn fonts_and_drivers_are_extras_like_the_rest() {
        // §4.5.3 : `content/fonts` et `content/driver` avaient leur propre
        // mécanisme — copie globale dans AC, jamais désactivée, écrasement par
        // défaut. Il était déjà court-circuité par le balayage des restes, et
        // contredisait la règle d'or n°5. Ils suivent désormais le sort commun :
        // suivis, et retirés avec le mod qui les a apportés.
        let base = crate::testutil::temp_dir("import-fonts");
        let library = base.join("library");
        let ac = base.join("ac");
        std::fs::create_dir_all(&library).unwrap();
        std::fs::create_dir_all(ac.join("content").join("cars")).unwrap();
        // Font déjà présente, que personne ne réclamera : ni touchée, ni retirée.
        let foreign = ac.join("content").join("fonts").join("kunos.txt");
        std::fs::create_dir_all(foreign.parent().unwrap()).unwrap();
        std::fs::write(&foreign, b"KUNOS").unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            library_path: Some(library.clone()),
            ac_install_path: Some(ac.clone()),
            ..Default::default()
        };
        let rules = crate::rules::default_rules();

        let src = base.join("FontPack");
        make_fake_car(&src.join("content").join("cars"), "font_car");
        for (rel, name, body) in [
            ("content/fonts", "rss-arial.txt", &b"MOD"[..]),
            ("content/driver", "rss_driver.kn5", &b"MOD"[..]),
            // Même nom que la font déjà là, et plus récente : la remplace,
            // après sauvegarde de l'originale (§4.5.4).
            ("content/fonts", "kunos.txt", &b"MOD"[..]),
        ] {
            let d = src.join(rel);
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join(name), body).unwrap();
        }

        let r = import_folder_for_test(&conn, &cfg, &rules, &src, true, &[]);
        assert!(r.error.is_none(), "erreur inattendue: {:?}", r.error);
        assert_eq!(r.extras, 2, "fonts et driver rattachés à la voiture");

        assert!(ac.join("content").join("fonts").join("rss-arial.txt").is_file());
        assert!(ac.join("content").join("driver").join("rss_driver.kn5").is_file());
        // La font homonyme est plus récente côté mod : elle prend la place,
        // mais l'originale est sauvegardée (§4.5.4), pas perdue.
        assert_eq!(std::fs::read(&foreign).unwrap(), b"MOD");
        assert!(
            crate::gamebackup::is_replaced(&conn, &foreign),
            "le remplacement est tracé, pas silencieux"
        );

        crate::maintenance::delete_broken(&conn, &cfg, "font_car").unwrap();
        assert!(
            !ac.join("content").join("fonts").join("rss-arial.txt").exists(),
            "l'ajout pur est retiré avec le mod qui l'a apporté"
        );
        assert!(!ac.join("content").join("driver").join("rss_driver.kn5").exists());
        assert_eq!(
            std::fs::read(&foreign).unwrap(),
            b"KUNOS",
            "et la font d'origine revient à la suppression du mod"
        );
    }

    #[test]
    fn leftovers_become_extras_of_the_mod_they_came_with() {
        // §4.5.3 : ce qu'une archive livre à côté du dossier du mod lui
        // appartient — configs CSP, shaders, pilote, textures d'équipe. Stocké
        // brut avec son chemin relatif à AC, posé à l'activation, et retiré
        // avec le mod. Avant, tout ça devenait des « autres mods » anonymes qui
        // survivaient à la suppression de la voiture.
        let base = crate::testutil::temp_dir("import-sat");
        let library = base.join("library");
        let ac = base.join("ac");
        std::fs::create_dir_all(&library).unwrap();
        std::fs::create_dir_all(ac.join("content").join("cars")).unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            library_path: Some(library.clone()),
            ac_install_path: Some(ac.clone()),
            ..Default::default()
        };
        let rules = crate::rules::default_rules();

        let src = base.join("RSS_Pack");
        make_fake_car(&src.join("content").join("cars"), "rss_test_v8");
        for (rel, name) in [
            ("extension/config/cars/rss/rss_test_v8", "car.ini"),
            ("system/shaders", "shader.fxo"),
            ("content/driver", "driver.kn5"),
        ] {
            let d = src.join(rel);
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join(name), name.as_bytes()).unwrap();
        }

        let r = import_folder_for_test(&conn, &cfg, &rules, &src, true, &[]);
        assert!(r.error.is_none(), "erreur inattendue: {:?}", r.error);
        assert_eq!(r.mods.len(), 1, "la voiture est reconnue");
        assert_eq!(r.extras, 3, "les trois restes lui sont rattachés");
        assert!(r.others.is_empty(), "aucun « autre mod » anonyme créé");

        // Stockés bruts, chemin relatif à AC conservé.
        let sat = crate::extras::dir(&library, ModKind::Car, "rss_test_v8");
        for rel in [
            "extension/config/cars/rss/rss_test_v8/car.ini",
            "system/shaders/shader.fxo",
            "content/driver/driver.kn5",
        ] {
            let mut p = sat.clone();
            for seg in rel.split('/') {
                p = p.join(seg);
            }
            assert!(p.is_file(), "{rel} stocké en ajout au jeu");
        }

        // Posés dans AC dès l'import (activation par défaut, §4.2).
        assert!(ac.join("system").join("shaders").join("shader.fxo").is_file());
        assert!(ac.join("content").join("driver").join("driver.kn5").is_file());

        // Et retirés avec le mod — c'est tout l'intérêt du rattachement.
        crate::maintenance::delete_broken(&conn, &cfg, "rss_test_v8").unwrap();
        assert!(
            !ac.join("system").join("shaders").join("shader.fxo").exists(),
            "ajout retiré d'AC à la suppression du mod"
        );
        assert!(!ac.join("content").join("driver").join("driver.kn5").exists());
        assert!(!sat.exists(), "arbre des ajouts supprimé avec le mod");
        assert!(
            ac.join("content").is_dir(),
            "un dossier AC préexistant n'est jamais emporté"
        );
    }

    #[test]
    fn every_unattached_leftover_of_one_archive_gets_its_own_id_and_survives() {
        // Bug réel (RSS GT-M Lanzo) : tous les restes d'une même archive
        // tombaient sur le même id « autre mod » — `other_id` voyait dans
        // `<archive>.rar__<label>` une extension `rar__<label>` et ne gardait
        // que `<archive>`. Le premier reste était importé, les suivants
        // rejetés par `other_exists`, et leurs fichiers — déjà déplacés dans le
        // dossier temporaire d'emballage — disparaissaient à son nettoyage.
        //
        // Pack multi-voitures : rien ne rattache ces restes à une voiture
        // précise (§4.5.3), ils restent donc des « autres mods » — le cas où
        // l'unicité des ids compte encore.
        let base = crate::testutil::temp_dir("import-leftover-ids");
        let library = base.join("library");
        let ac = base.join("ac");
        std::fs::create_dir_all(&library).unwrap();
        std::fs::create_dir_all(ac.join("content").join("cars")).unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            library_path: Some(library.clone()),
            ac_install_path: Some(ac.clone()),
            ..Default::default()
        };
        let rules = crate::rules::default_rules();

        let src = base.join("RSS_Pack.rar");
        make_fake_car(&src.join("content").join("cars"), "rss_test_a");
        make_fake_car(&src.join("content").join("cars"), "rss_test_b");
        for (rel, name) in [
            ("extension/config/shared", "shared.ini"),
            ("system/shaders", "shader.fxo"),
            ("content/texture/crew", "brand.dds"),
            ("content/driver", "driver.kn5"),
        ] {
            let d = src.join(rel);
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join(name), name.as_bytes()).unwrap();
        }

        let r = import_folder_for_test(&conn, &cfg, &rules, &src, true, &[]);
        assert!(r.error.is_none(), "erreur inattendue: {:?}", r.error);
        assert_eq!(r.mods.len(), 2, "les deux voitures sont reconnues");
        assert_eq!(r.extras, 0, "pack ambigu : aucun rattachement automatique");
        assert_eq!(
            r.others.len(),
            4,
            "les quatre restes sont importés, aucun écrasé par collision d'id"
        );

        let ids: Vec<&str> = r.others.iter().map(|o| o.id.as_str()).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 4, "quatre ids distincts, obtenus: {ids:?}");

        for rel in [
            "extension/config/shared/shared.ini",
            "system/shaders/shader.fxo",
            "content/texture/crew/brand.dds",
            "content/driver/driver.kn5",
        ] {
            let found = r.others.iter().any(|o| {
                let mut p = library.join("others").join(&o.id);
                for seg in rel.split('/') {
                    p = p.join(seg);
                }
                p.is_file()
            });
            assert!(found, "{rel} conservé en bibliothèque");
        }
    }

    #[test]
    fn leftover_keeps_its_path_relative_to_the_archive_root() {
        // Bug réel : l'emballage d'un reste ne conservait que son nom de
        // fichier, or la pose rejoue le chemin depuis la racine d'AC.
        // `content/driver` atterrissait donc à `AC\driver` au lieu d'
        // `AC\content\driver` — hors de portée du jeu.
        let base = crate::testutil::temp_dir("import-leftover-relpath");
        let library = base.join("library");
        let ac = base.join("ac");
        std::fs::create_dir_all(&library).unwrap();
        std::fs::create_dir_all(ac.join("content").join("cars")).unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            library_path: Some(library.clone()),
            ac_install_path: Some(ac.clone()),
            ..Default::default()
        };
        let rules = crate::rules::default_rules();

        let src = base.join("WithDriver");
        make_fake_car(&src.join("content").join("cars"), "some_car");
        let d = src.join("content").join("driver");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("pro.kn5"), b"driver-model").unwrap();

        let r = import_folder_for_test(&conn, &cfg, &rules, &src, true, &[]);
        assert!(r.error.is_none(), "erreur inattendue: {:?}", r.error);
        assert_eq!(r.extras, 1, "le dossier driver/ est rattaché à la voiture");

        let stored = crate::extras::dir(&library, ModKind::Car, "some_car")
            .join("content")
            .join("driver")
            .join("pro.kn5");
        assert!(stored.is_file(), "chemin relatif conservé en bibliothèque: {stored:?}");

        assert!(
            ac.join("content").join("driver").join("pro.kn5").is_file(),
            "posé sous content/driver, là où AC le lit"
        );
        assert!(
            !ac.join("driver").exists(),
            "jamais à la racine d'AC (le bug d'origine)"
        );
    }

    #[test]
    fn nested_zip_next_to_app_is_extracted_and_imported_as_other_mod() {
        // Bug réel (CMRT_Complete_hud) : un zip séparé à la racine du mod,
        // sibling du dossier `apps/`, qui vise `content/gui/...` etc. N'était
        // jamais scanné (modscan ne descend jamais dans un fichier) ni perdu
        // via le fallback « autre mod » (qui ne se déclenchait pas puisque
        // l'app, elle, était bien reconnue) — silencieusement jeté au
        // nettoyage du dossier temporaire.
        let Some(sevenzip) = crate::detect::find_7zip() else {
            eprintln!("7-Zip introuvable — test ignoré");
            return;
        };

        let base = crate::testutil::temp_dir("import-nested-zip");
        let library = base.join("library");
        let ac = base.join("ac");
        std::fs::create_dir_all(&library).unwrap();
        std::fs::create_dir_all(ac.join("extension").join("config")).unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            sevenzip_exe: Some(sevenzip.clone()),
            library_path: Some(library.clone()),
            ac_install_path: Some(ac.clone()),
            ..Default::default()
        };
        let rules = crate::rules::default_rules();

        let src = base.join("CMRT_HUD");
        let app = src.join("apps").join("lua").join("CMRT_HUD");
        std::fs::create_dir_all(&app).unwrap();
        std::fs::write(app.join("CMRT_HUD.lua"), b"-- hud").unwrap();

        // Contenu du zip imbriqué : sa racine contient directement `extension/`,
        // comme une vraie install AC (le zip "flag/fuel/starting lights" de CMRT
        // vise réellement `content/gui/...`, ici simplifié en `extension/...`).
        let nested_src = base.join("nested_src");
        let nested_leaf = nested_src.join("extension").join("config").join("new_hud_tweak");
        std::fs::create_dir_all(&nested_leaf).unwrap();
        std::fs::write(nested_leaf.join("settings.ini"), b"x").unwrap();
        let nested_zip = src.join("CMRT_flag_fuel_and_starting_lights_replacement.zip");
        zip_dir(&sevenzip, &nested_src, &nested_zip);

        let r = import_folder_for_test(&conn, &cfg, &rules, &src, true, &[]);
        assert!(r.error.is_none(), "erreur inattendue: {:?}", r.error);
        assert_eq!(r.apps.len(), 1, "l'app est bien reconnue");
        assert_eq!(
            r.others.len(),
            1,
            "le zip imbriqué doit être extrait et importé comme autre mod, pas perdu"
        );
        assert!(r.others[0]
            .id
            .contains("CMRT_flag_fuel_and_starting_lights_replacement"));
        assert!(
            crate::activation::is_junction(&ac.join("extension").join("config").join("new_hud_tweak")),
            "activé par défaut, chemin du zip imbriqué préservé"
        );
    }

    #[test]
    fn folder_import_copy_and_move() {
        let base = crate::testutil::temp_dir("import");
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            library_path: Some(library.clone()),
            ..Default::default()
        };
        let rules = crate::rules::default_rules();

        // Copie : la source est préservée.
        let src_copy = base.join("src_copy");
        make_fake_car(&src_copy, "copy_car");
        let r = import_folder_for_test(&conn, &cfg, &rules, &src_copy, true, &[]);
        assert_eq!(r.mods.len(), 1);
        assert_eq!(r.mods[0].outcome, "IMPORT");
        assert!(
            src_copy.join("copy_car").join("ui").join("ui_car.json").is_file(),
            "source préservée (copie)"
        );
        assert!(
            library.join("cars").join("copy_car").exists(),
            "rangé dans la bibliothèque"
        );

        // Déplacement : la source est retirée.
        let src_move = base.join("src_move");
        make_fake_car(&src_move, "move_car");
        let r2 = import_folder_for_test(&conn, &cfg, &rules, &src_move, false, &[]);
        assert_eq!(r2.mods.len(), 1);
        assert!(!src_move.join("move_car").exists(), "source retirée (déplacement)");
        assert!(
            library.join("cars").join("move_car").exists(),
            "rangé dans la bibliothèque"
        );
    }

    #[test]
    fn pack_source_on_multi_car_folder() {
        let base = crate::testutil::temp_dir("import");
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            library_path: Some(library.clone()),
            ..Default::default()
        };
        let rules = crate::rules::default_rules();

        // Dossier « pack » contenant deux voitures.
        let pack = base.join("ferrari_pack");
        make_fake_car(&pack, "ferrari_a");
        make_fake_car(&pack, "ferrari_b");
        let r = import_folder_for_test(&conn, &cfg, &rules, &pack, true, &[]);
        assert_eq!(r.mods.len(), 2);
        // Les deux mods partagent la même source de pack (§4.4).
        for id in ["ferrari_a", "ferrari_b"] {
            let m = crate::overlay::get_mod(&conn, id).unwrap().unwrap();
            assert_eq!(
                m.source_pack.as_deref(),
                Some("ferrari_pack"),
                "{id} doit pointer le pack"
            );
        }

        // Un import mono-voiture ne crée pas de pack.
        let solo = base.join("solo");
        make_fake_car(&solo, "solo_car");
        import_folder_for_test(&conn, &cfg, &rules, &solo, true, &[]);
        let m = crate::overlay::get_mod(&conn, "solo_car").unwrap().unwrap();
        assert_eq!(m.source_pack, None, "mono-voiture → pas de pack");
    }

    #[test]
    fn bulk_analyze_and_execute() {
        let base = crate::testutil::temp_dir("import");
        let parent = base.join("catalog");
        std::fs::create_dir_all(&parent).unwrap();
        make_fake_car(&parent, "bulk_car"); // sous-dossier = voiture
                                            // Sous-dossier sans structure AC → ignoré.
        let notes = parent.join("notes");
        std::fs::create_dir_all(&notes).unwrap();
        std::fs::write(notes.join("readme.txt"), b"hi").unwrap();

        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            library_path: Some(library.clone()),
            ..Default::default()
        };
        let rules = crate::rules::default_rules();

        // Analyse : 1 nouveau (bulk_car), 1 ignoré (notes). Aucune écriture.
        let entries = analyze_bulk(&conn, &cfg, &parent).unwrap();
        assert_eq!(entries.len(), 2);
        let car = entries.iter().find(|e| e.subfolder == "bulk_car").unwrap();
        assert!(!car.ignored);
        assert_eq!(car.mods[0].status, "new");
        assert!(entries.iter().find(|e| e.subfolder == "notes").unwrap().ignored);
        assert!(!library.join("cars").join("bulk_car").exists(), "analyse n'écrit rien");

        // Exécution (copie).
        let item = BulkExecItem {
            path: car.path.clone(),
            skip_ids: vec![],
            replace_ids: vec![],
        };
        let r = exec_one(&ImportCtx::silent(), &conn, &cfg, &rules, 0, &item, true);
        assert_eq!(r.mods.len(), 1);
        assert!(library.join("cars").join("bulk_car").exists());

        // Reprenable : ré-analyse → bulk_car devient un doublon (même contenu).
        let entries2 = analyze_bulk(&conn, &cfg, &parent).unwrap();
        let car2 = entries2.iter().find(|e| e.subfolder == "bulk_car").unwrap();
        assert_eq!(car2.mods[0].status, "duplicate");
    }

    #[test]
    fn published_at_estimated_on_import() {
        let base = crate::testutil::temp_dir("import");
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            library_path: Some(library.clone()),
            ..Default::default()
        };
        let rules = crate::rules::default_rules();

        let src = base.join("src_pub");
        make_fake_car(&src, "pub_car");
        let r = import_folder_for_test(&conn, &cfg, &rules, &src, true, &[]);
        assert_eq!(r.mods.len(), 1);

        // Date de publication estimée (§6.2) depuis la date de modification des
        // fichiers, remontée à la fois sur la version et sur le mod (version active).
        let versions = crate::overlay::get_versions(&conn, "pub_car").unwrap();
        assert!(
            versions[0].published_at.is_some(),
            "date de publication absente sur la version"
        );
        let m = crate::overlay::get_mod(&conn, "pub_car").unwrap().unwrap();
        assert!(
            m.published_at.is_some(),
            "date de publication absente sur le mod (version active)"
        );
    }

    #[test]
    fn fuzzy_conflict_and_replace() {
        let Some(sevenzip) = crate::detect::find_7zip() else {
            return;
        };
        let base = crate::testutil::temp_dir("import");
        let rules = crate::rules::default_rules();

        // Deux voitures avec des id de dossier différents mais même brand+name.
        let zip_for = |id: &str| -> String {
            let src = base.join(format!("src_{id}"));
            std::fs::create_dir_all(&src).unwrap();
            make_fake_car(&src, id);
            let zip = base.join(format!("{id}.zip"));
            zip_dir(&sevenzip, &src, &zip);
            zip.to_string_lossy().into_owned()
        };
        let zip_a = zip_for("car_a");
        let zip_b = zip_for("car_b");

        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let db = crate::overlay::Db(std::sync::Mutex::new(
            crate::overlay::open(&base.join("overlay.sqlite")).unwrap(),
        ));
        let cfg = AppConfig {
            sevenzip_exe: Some(sevenzip),
            library_path: Some(library.clone()),
            ..Default::default()
        };

        // 1er : pas de conflit (rien d'existant).
        let r1 = import_archives(&ImportCtx::silent(), &db, &cfg, &rules, &[zip_a], &[]).unwrap();
        assert!(r1[0].mods[0].conflict.is_none());

        // 2e : conflit flou vers car_a.
        let r2 = import_archives(&ImportCtx::silent(), &db, &cfg, &rules, &[zip_b], &[]).unwrap();
        let conflict = r2[0].mods[0].conflict.as_ref().expect("conflit attendu");
        assert_eq!(conflict.existing_id, "car_a");

        // Résolution "replace" : car_a disparaît, seul car_b reste.
        let conn = db.0.lock().unwrap();
        resolve_conflict(&conn, &cfg, "car_b", "car_a", "replace").unwrap();
        let mods = crate::overlay::list_mods(&conn).unwrap();
        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0].id_interne, "car_b");
        assert!(!library.join("cars").join("car_a").exists(), "fichiers car_a supprimés");
    }

    #[test]
    fn resolve_conflict_replace_removes_hardlink_deployment_of_old_mod() {
        // §2 : si l'ancien mod (fuzzy conflict) était actif par hardlinks, la
        // résolution "replace" doit retirer son déploiement dans content/, pas
        // seulement ses fichiers de bibliothèque (sinon copie orpheline vivante).
        let base = crate::testutil::temp_dir("import");
        let ac = base.join("ac");
        let library = base.join("library");
        std::fs::create_dir_all(ac.join("content").join("cars")).unwrap();
        std::fs::create_dir_all(&library).unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            ac_install_path: Some(ac.clone()),
            library_path: Some(library.clone()),
            ..Default::default()
        };
        let now = chrono::Local::now().to_rfc3339();

        let old_dir = library.join("cars").join("car_a").join("v1");
        std::fs::create_dir_all(&old_dir).unwrap();
        std::fs::write(old_dir.join("f.txt"), "old").unwrap();
        crate::overlay::upsert_mod(&conn, "car_a", "Car", Some("B"), Some("Same"), "h", None, &now).unwrap();
        crate::overlay::insert_version(
            &conn,
            "v1",
            "car_a",
            Some("1.0"),
            None,
            &now,
            &old_dir.to_string_lossy(),
            None,
            "sig",
            &[],
            &[],
            &[],
            &[],
            None,
        )
        .unwrap();
        crate::overlay::set_active_version(&conn, "car_a", "v1").unwrap();
        crate::activation::activate(&conn, &cfg, "car_a", None).unwrap();
        let link = ac.join("content").join("cars").join("car_a");
        assert!(
            crate::deploy::is_deployed(&link),
            "précondition : car_a actif par hardlinks"
        );

        // "car_b" existe juste en overlay (le conflit ne dépend que de l'id passé).
        crate::overlay::upsert_mod(&conn, "car_b", "Car", Some("B"), Some("Same"), "h2", None, &now).unwrap();

        resolve_conflict(&conn, &cfg, "car_b", "car_a", "replace").unwrap();

        assert!(
            !link.exists(),
            "déploiement content/ de l'ancien mod retiré, pas laissé orphelin"
        );
    }

    #[test]
    fn keep_source_archive_pref_off_leaves_kept_path_empty() {
        // §10/§11 : réglage désactivé par défaut — aucune copie, aucun chemin
        // enregistré (comportement historique, pas d'espace disque en plus).
        let base = crate::testutil::temp_dir("import");
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            library_path: Some(library.clone()),
            ..Default::default()
        };
        let rules = crate::rules::default_rules();

        let src = base.join("src");
        make_fake_car(&src, "nokeep_car");
        let r = import_folder_for_test(&conn, &cfg, &rules, &src, true, &[]);
        assert_eq!(r.mods[0].outcome, "IMPORT");

        let versions = crate::overlay::get_versions(&conn, "nokeep_car").unwrap();
        assert!(versions[0].kept_archive_path.is_none());
        assert!(!library.join("_source_archives").exists());
    }

    #[test]
    fn keep_source_archive_pref_on_copies_folder_and_records_path() {
        // §10/§11 : réglage activé — le dossier source est copié à part dans la
        // bibliothèque et son chemin enregistré sur la version, pour permettre
        // plus tard « Réinstaller depuis l'archive source » (maintenance.rs).
        let base = crate::testutil::temp_dir("import");
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        let mut cfg = AppConfig {
            library_path: Some(library.clone()),
            ..Default::default()
        };
        cfg.prefs.keep_source_archive = true;
        let rules = crate::rules::default_rules();

        let src = base.join("src");
        make_fake_car(&src, "keep_car");
        let r = import_folder_for_test(&conn, &cfg, &rules, &src, true, &[]);
        assert_eq!(r.mods[0].outcome, "IMPORT");

        let versions = crate::overlay::get_versions(&conn, "keep_car").unwrap();
        let kept = versions[0]
            .kept_archive_path
            .as_ref()
            .expect("archive source conservée");
        let kept_path = library.join(kept);
        // La copie porte sur `dir` (le dossier passé à l'import, ici `src`, qui
        // peut contenir plusieurs mods) — le mod retrouvé est donc un niveau
        // plus bas, comme au premier import (même logique de descente).
        assert!(kept_path.is_dir(), "dossier source copié tel quel");
        assert!(kept_path.join("keep_car").join("ui").join("ui_car.json").is_file());
        assert!(
            kept_path.starts_with(&library),
            "copie rangée dans la bibliothèque, pas ailleurs"
        );
        // Source d'origine toujours présente : `copy=true` préserve la source.
        assert!(
            src.join("keep_car").join("model.kn5").is_file(),
            "source d'origine intacte (copie)"
        );
    }

    /// Règle (§10/§11) : une copie de source qu'aucune version ne réclame est
    /// retirée. `keep_source` copie avant de savoir si la copie servira — un
    /// import de dossier en mode déplacement consomme sa source pendant le
    /// rangement, il n'y a pas d'autre ordre possible. Sans ce nettoyage,
    /// réimporter la même archive de 2 Go en laissait 2 Go de plus à chaque
    /// fois, sans que rien ne les référence.
    #[test]
    fn duplicate_import_leaves_no_orphan_source_copy() {
        let base = crate::testutil::temp_dir("import");
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        let mut cfg = AppConfig {
            library_path: Some(library.clone()),
            ..Default::default()
        };
        cfg.prefs.keep_source_archive = true;
        let rules = crate::rules::default_rules();

        let src = base.join("src");
        make_fake_car(&src, "dup_car");
        let first = import_folder_for_test(&conn, &cfg, &rules, &src, true, &[]);
        assert_eq!(first.mods[0].outcome, "IMPORT");
        let kept = crate::overlay::get_versions(&conn, "dup_car").unwrap()[0]
            .kept_archive_path
            .clone()
            .expect("archive source conservée au premier import");

        // Même source à l'identique : doublon, aucune version stockée.
        let second = import_folder_for_test(&conn, &cfg, &rules, &src, true, &[]);
        assert_eq!(second.mods[0].outcome, "DUPLICATE");

        let copies: Vec<_> = std::fs::read_dir(library.join("_source_archives"))
            .unwrap()
            .flatten()
            .collect();
        assert_eq!(copies.len(), 1, "la copie du doublon a été retirée");
        assert!(
            library.join(&kept).exists(),
            "celle du premier import, elle, est toujours là"
        );
    }

    /// Règle (§7.3/§10) : une archive dont rien n'est reconnu devient un « autre
    /// mod », qui ne stocke aucune version — sa copie de source n'a donc rien à
    /// faire dans `_source_archives/`.
    #[test]
    fn unrecognized_content_leaves_no_orphan_source_copy() {
        let base = crate::testutil::temp_dir("import");
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        let mut cfg = AppConfig {
            library_path: Some(library.clone()),
            ..Default::default()
        };
        cfg.prefs.keep_source_archive = true;
        let rules = crate::rules::default_rules();

        let src = base.join("MysteryPack");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("readme.txt"), b"rien de reconnaissable").unwrap();

        let r = import_folder_for_test(&conn, &cfg, &rules, &src, true, &[]);
        assert_eq!(r.others.len(), 1, "rangé en « autre mod », jamais perdu");
        let archives = library.join("_source_archives");
        assert!(
            !archives.exists() || std::fs::read_dir(&archives).unwrap().next().is_none(),
            "aucune copie de source laissée derrière"
        );
    }
}
