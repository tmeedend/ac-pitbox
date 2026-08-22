//! Dossiers proposés par l'auteur (§4.6ter) : ce qu'une archive livre **à côté**
//! du mod sans que le disque dise quoi en faire.
//!
//! Le balayage des restes (§7.3) sait classer deux choses : ce qui est un
//! chemin de jeu (ajout au jeu, §4.5.3) et ce qui est un document isolé
//! (annexe, §4.5.2). Entre les deux, il restait un fourre-tout où le *chemin*
//! décidait tout seul — et il décidait mal, parce que ce qui s'y trouve n'est
//! presque jamais un chemin :
//!
//! - `2K Skins/`, `No Dust Skins/` (Ferrari F2002) — des livrées de meilleure
//!   qualité qui remplacent celles de la voiture ;
//! - `Optional Textures/` (VRC Pageau) — deux `.dds` à poser dans la voiture ;
//! - `MODS/<variante>/` (LA Canyons) — la convention JSGME, un sous-dossier par
//!   option, dont un patch qui masque le personnel des stands **sur toutes les
//!   pistes** ;
//! - `Wallpapers/`, `Templates/`, `CM Previews Template/` — de la matière qui
//!   n'a rien à faire dans le jeu.
//!
//! Aucune règle ne les sépare depuis le disque, et c'est bien le problème :
//! l'information est dans la notice, en prose, ou dans l'intention de
//! l'utilisateur. Le §4.6 tranche ce cas-là depuis toujours — **information de
//! préférence → demander**. Ce module est la mise en œuvre de cette réponse.
//!
//! **Rangé, jamais posé, jamais perdu.** Un dossier proposé attend dans
//! `<lib>/pending/<id>/`, sa ligne en base survit à un redémarrage, et rien de
//! lui n'entre dans le jeu tant que personne n'a tranché. Ne rien décider reste
//! une réponse valable.
//!
//! **Ce qui est écarté est supprimé.** C'est l'amendement à « l'import ne jette
//! rien » (§4.5.3) : la règle protégeait la *recalculabilité* de la décision,
//! pas les octets. Un dossier que l'utilisateur a explicitement écarté n'a plus
//! de décision à recalculer — il laisse une ligne au journal d'import
//! (`userDiscarded`), donc l'information reste, et la matière part.

use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde::Serialize;

use crate::config::AppConfig;
use crate::extras::OwnerKind;
use crate::modscan::ModKind;
use crate::overlay::{self, PendingFolderRow};
use crate::resources::{self, ExtractionMode};

/// Formes reconnues. Elles ne décident de rien : elles choisissent la
/// proposition **pré-remplie** et ce qu'on montre à l'utilisateur pour qu'il
/// tranche vite. Le nom est une clé i18n côté front.
pub const SHAPE_JSGME: &str = "jsgme";
pub const SHAPE_GAME_TREE: &str = "gameTree";
pub const SHAPE_SKIN_VARIANT: &str = "skinVariant";
pub const SHAPE_DOCUMENTS: &str = "documents";
pub const SHAPE_UNKNOWN: &str = "unknown";

/// Sorts possibles d'un dossier proposé.
pub const ACTION_DISCARD: &str = "discard";
pub const ACTION_RESOURCES: &str = "resources";
pub const ACTION_GAME: &str = "game";
pub const ACTION_LAYER: &str = "layer";
pub const ACTION_OTHER: &str = "other";

/// Un dossier proposé, tel que l'écran d'arbitrage le montre. Taille et nombre
/// de fichiers sont **lus en direct sur disque**, comme les ressources et les
/// ajouts au jeu (§4.5.5) : une ligne en base qui mentirait sur le poids ferait
/// prendre une décision sur une information périmée.
#[derive(Debug, Clone, Serialize)]
pub struct PendingFolder {
    pub id: String,
    pub archive: String,
    /// Chemin dans l'archive — c'est ce qui identifie le dossier pour
    /// l'utilisateur, bien mieux que l'id interne.
    pub rel_path: String,
    pub owner_id: Option<String>,
    /// "cars" | "tracks" | "apps".
    pub owner_kind: Option<String>,
    pub shape: String,
    /// Titre donné par l'auteur (première ligne d'un `description.jsgme`).
    pub title: Option<String>,
    /// Le reste de ce que l'auteur a écrit.
    pub description: Option<String>,
    /// Nom du document d'explication trouvé dans le dossier, s'il y en a un.
    pub readme: Option<String>,
    /// Contenu dont les livrées recouvrent celles de ce mod.
    pub skin_target: Option<String>,
    /// Fichiers du **jeu de base** que ce dossier remplacerait s'il était posé
    /// (§4.6bis) : la mesure de son rayon d'action, et le seul chiffre qui dit
    /// « ceci ne concerne pas que ce mod ».
    pub replaced: usize,
    pub file_count: usize,
    pub size_bytes: u64,
    /// Action pré-remplie. Jamais appliquée toute seule.
    pub suggestion: String,
    /// Actions qui ont un sens pour ce dossier-ci. Proposer « poser comme
    /// couche » sans propriétaire, ou « installer dans le jeu » sur un dossier
    /// de fonds d'écran, c'est offrir un bouton qui ne peut que décevoir.
    pub actions: Vec<String>,
}

/// Ce que la détection a reconnu, avant rangement.
#[derive(Debug, Clone, Default)]
pub struct Detected {
    pub shape: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub readme: Option<String>,
    pub skin_target: Option<String>,
}

// --- Détection --------------------------------------------------------------

/// Documents d'explication qu'un auteur pose à côté de ce qu'il propose. Sert à
/// **montrer** la notice, jamais à classer : c'est le §4.6bis qui le dit — pour
/// les dossiers que rien ne permet d'interpréter, la réponse honnête est de
/// rendre la notice lisible, pas de deviner d'après elle.
fn readme_in(dir: &Path) -> Option<String> {
    let mut best: Option<String> = None;
    for e in std::fs::read_dir(dir).ok()?.flatten() {
        let p = e.path();
        if !p.is_file() {
            continue;
        }
        let name = e.file_name().to_string_lossy().into_owned();
        let ext = p.extension()?.to_string_lossy().to_ascii_lowercase();
        if !matches!(ext.as_str(), "txt" | "pdf" | "md" | "nfo" | "rtf" | "doc" | "docx") {
            continue;
        }
        // Un seul suffit ; le premier dans l'ordre du disque fait l'affaire.
        if best.is_none() {
            best = Some(name);
        }
    }
    best
}

/// Titre + description d'une variante JSGME. Le fichier est une convention
/// vieille de vingt ans (Generic Mod Enabler) : première ligne le nom, le reste
/// l'explication. C'est **le seul cas** où l'auteur nous donne un libellé
/// exploitable tel quel — inutile de le paraphraser.
fn jsgme_description(dir: &Path) -> Option<(String, Option<String>)> {
    let raw = std::fs::read_to_string(dir.join("description.jsgme")).ok()?;
    let mut lines = raw.lines();
    let title = lines.find(|l| !l.trim().is_empty())?.trim().to_string();
    let rest = lines.collect::<Vec<_>>().join("\n").trim().to_string();
    Some((title, (!rest.is_empty()).then_some(rest)))
}

/// Le dossier porte-t-il un arbre de jeu ? On regarde ses **enfants directs**
/// seulement : plus profond, on retrouverait le `content/` d'un dossier de mod
/// et on prendrait un pack de skins pour une livraison de jeu.
fn holds_game_tree(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .any(|e| e.path().is_dir() && crate::acpath::leads_into_game(Path::new(&e.file_name())))
}

/// Noms des sous-dossiers de `<dir>/skins`, s'il y en a un.
fn skin_names(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir.join("skins"))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_ascii_lowercase())
        .collect()
}

/// Le contenu dont ce dossier recouvre les livrées, s'il y en a un.
///
/// Signal **structurel**, jamais un nom lu : les livrées proposées portent les
/// mêmes noms que celles déjà livrées par le mod. C'est ce qui distingue
/// `2K Skins/skins/a_2002_ferrari_01_michael_t` — un remplacement de qualité
/// pour la F2002 — d'un pack de skins pour une voiture qu'on n'a pas encore.
fn skin_target(conn: &Connection, cfg: &AppConfig, dir: &Path, owners: &[(String, OwnerKind)]) -> Option<String> {
    let offered = skin_names(dir);
    if offered.is_empty() {
        return None;
    }
    owners.iter().find_map(|(id, kind)| {
        if matches!(kind, OwnerKind::App) {
            return None;
        }
        let existing = skin_names(&crate::submods::parent_content_dir(conn, cfg, id)?);
        offered.iter().any(|s| existing.contains(s)).then(|| id.clone())
    })
}

/// Le contenu dont ce pack de skins recouvre les livrées, s'il y en a un —
/// c'est-à-dire : ce « pack » est-il en réalité une **variante** que l'auteur
/// propose ?
///
/// Sert à l'import (§4.6ter) pour ne **pas** consommer un tel dossier comme
/// pack de skins. Un pack, ça s'ajoute à une voiture ; une variante, ça la
/// recouvre — et l'un n'est pas l'autre. Cas réel, la Ferrari F2002 :
/// `2K Skins/skins/<skin>` importé comme pack se rattachait à une voiture
/// nommée « 2K Skins », qui n'existe pas.
///
/// Le test est **volontairement étroit** : il exige que les livrées proposées
/// recouvrent celles d'un contenu de la même source. Sans ce recoupement, un
/// pack dont la voiture cible n'est pas encore installée reste un pack — il
/// dort en bibliothèque et `repair_projections` le branchera quand la voiture
/// arrivera, ce qui est un usage parfaitement légitime qu'on ne casse pas.
pub fn offered_liveries_target(
    conn: &Connection,
    cfg: &AppConfig,
    sub: &crate::modscan::FoundSub,
    owners: &[(String, OwnerKind)],
) -> Option<String> {
    if !matches!(sub.kind, crate::modscan::SubKind::Skin) {
        return None;
    }
    // La forme multi-voitures (`skins/<voiture>/<skin>`) n'a pas de racine
    // commune : rien à interroger, et son `parent_id` vient d'un vrai nom de
    // dossier de voiture. Jamais une variante.
    let root = sub.extra_root.as_ref()?;
    // Une cible qui existe déjà n'a rien d'ambigu : c'est un pack pour elle.
    if overlay::get_mod(conn, &sub.parent_id).ok().flatten().is_some() {
        return None;
    }
    skin_target(conn, cfg, root, owners)
}

/// Tous les fichiers du dossier sont-ils inconnus d'AC ? Les extensions listées
/// sont celles qu'un moteur de jeu ne lit jamais — un dossier qui n'en contient
/// que se range en ressources sans autre question.
///
/// **Les images n'en font pas partie**, à dessein et pour la raison de §4.5.2 :
/// rien ne distingue une capture de présentation d'un asset AC. Un dossier de
/// fonds d'écran retombe donc en `Unknown`, ce qui est honnête — c'est
/// l'utilisateur qui sait que ce sont des fonds d'écran.
fn only_documents(dir: &Path) -> bool {
    let mut seen = false;
    for e in walkdir::WalkDir::new(dir).into_iter().flatten() {
        if !e.file_type().is_file() {
            continue;
        }
        seen = true;
        let ok = e
            .path()
            .extension()
            .map(|x| x.to_string_lossy().to_ascii_lowercase())
            .is_some_and(|x| {
                matches!(
                    x.as_str(),
                    "txt" | "pdf" | "md" | "doc" | "docx" | "rtf" | "nfo" | "html" | "url" | "psd" | "xcf" | "ai"
                )
            });
        if !ok {
            return false;
        }
    }
    seen
}

/// Reconnaît ce qu'est un dossier proposé, pour choisir quoi montrer et quoi
/// pré-remplir. Ne décide jamais du sort du dossier.
pub fn detect(conn: &Connection, cfg: &AppConfig, dir: &Path, owners: &[(String, OwnerKind)]) -> Detected {
    let readme = readme_in(dir);
    if let Some((title, description)) = jsgme_description(dir) {
        return Detected {
            shape: SHAPE_JSGME.into(),
            title: Some(title),
            description,
            readme,
            skin_target: None,
        };
    }
    if let Some(target) = skin_target(conn, cfg, dir, owners) {
        return Detected {
            shape: SHAPE_SKIN_VARIANT.into(),
            skin_target: Some(target),
            readme,
            ..Default::default()
        };
    }
    if holds_game_tree(dir) {
        return Detected {
            shape: SHAPE_GAME_TREE.into(),
            readme,
            ..Default::default()
        };
    }
    if only_documents(dir) {
        return Detected {
            shape: SHAPE_DOCUMENTS.into(),
            readme,
            ..Default::default()
        };
    }
    Detected {
        shape: SHAPE_UNKNOWN.into(),
        readme,
        ..Default::default()
    }
}

// --- Mise en attente --------------------------------------------------------

/// Arbre d'attente d'un dossier proposé.
fn dir_for(library: &Path, id: &str) -> PathBuf {
    library.join("pending").join(id)
}

/// Range un dossier proposé en attente. Renvoie `None` si le rangement échoue —
/// l'appelant retombe alors sur le classement d'avant (ajout au jeu / autre
/// mod), qui ne perd rien non plus.
#[allow(clippy::too_many_arguments)]
pub fn park(
    conn: &Connection,
    library: &Path,
    archive: &str,
    rel: &Path,
    src: &Path,
    owner: Option<(&str, OwnerKind)>,
    detected: Detected,
    copy: bool,
    replaced: usize,
) -> Option<String> {
    let id = format!("{archive}__{}", rel.to_string_lossy());
    let dest = dir_for(library, &sanitize_id(&id));
    // Reimport de la meme archive : la source est fraiche, on repart de zero
    // plutot que de fusionner deux etats dont un peut etre a moitie resolu.
    if dest.exists() {
        if let Err(e) = std::fs::remove_dir_all(&dest) {
            log::warn!("park {}: clear previous: {e}", dest.display());
            return None;
        }
    }
    let placed = if copy {
        crate::archive::copy_dir(src, &dest)
    } else {
        crate::archive::move_dir(src, &dest)
    };
    if let Err(e) = placed {
        log::warn!("park {}: {e}", rel.display());
        return None;
    }
    let row = PendingFolderRow {
        id: sanitize_id(&id),
        archive: archive.to_string(),
        rel_path: rel.to_string_lossy().replace('\\', "/"),
        library_path: crate::libpath::to_relative(Some(library), &dest),
        owner_id: owner.map(|(id, _)| id.to_string()),
        owner_kind: owner.map(|(_, k)| k.category().to_string()),
        shape: detected.shape,
        title: detected.title,
        description: detected.description,
        readme: detected.readme,
        skin_target: detected.skin_target,
        replaced,
        found_at: chrono::Local::now().to_rfc3339(),
    };
    if let Err(e) = overlay::insert_pending_folder(conn, &row) {
        log::warn!("park {}: overlay: {e}", rel.display());
        return None;
    }
    Some(row.id)
}

/// Les séparateurs d'un chemin d'archive ne peuvent pas rester dans un nom de
/// dossier de bibliothèque. Même précaution que `others::other_id`, dont un
/// découpage trop naïf avait déjà coûté des fichiers.
fn sanitize_id(raw: &str) -> String {
    raw.chars()
        .map(|c| {
            if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '_'
            } else {
                c
            }
        })
        .collect()
}

// --- Lecture ----------------------------------------------------------------

/// Poids et nombre de fichiers, lus sur disque.
fn weigh(dir: &Path) -> (usize, u64) {
    walkdir::WalkDir::new(dir)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file())
        .fold((0, 0), |(n, b), e| {
            (n + 1, b + e.metadata().map(|m| m.len()).unwrap_or(0))
        })
}

/// Actions qui ont un sens pour ce dossier, la proposition pré-remplie en tête.
fn actions_for(row: &PendingFolderRow) -> (String, Vec<String>) {
    let owned = row.owner_id.is_some();
    // « Poser comme couche » compose par-dessus la version du mod (§4.3) : il
    // faut un mod de contenu, une app n'en a pas.
    let layerable = owned && row.owner_kind.as_deref() != Some("apps");

    let mut actions: Vec<String> = Vec::new();
    if holds_game_shape(row) {
        actions.push(ACTION_GAME.into());
    }
    if layerable {
        actions.push(ACTION_LAYER.into());
    }
    if owned {
        actions.push(ACTION_RESOURCES.into());
    } else {
        actions.push(ACTION_OTHER.into());
    }
    actions.push(ACTION_DISCARD.into());

    let suggestion = match row.shape.as_str() {
        SHAPE_SKIN_VARIANT if layerable => ACTION_LAYER,
        SHAPE_GAME_TREE => ACTION_GAME,
        SHAPE_JSGME if actions.iter().any(|a| a == ACTION_GAME) => ACTION_GAME,
        SHAPE_DOCUMENTS if owned => ACTION_RESOURCES,
        _ => actions.first().map(String::as_str).unwrap_or(ACTION_DISCARD),
    };
    (suggestion.to_string(), actions)
}

/// Le dossier en attente porte-t-il un arbre de jeu ? Relu du `shape` plutôt
/// que du disque : c'est la détection d'import qui fait foi, et la relire ici
/// ferait dépendre les boutons proposés d'un état qui a pu changer.
fn holds_game_shape(row: &PendingFolderRow) -> bool {
    matches!(row.shape.as_str(), SHAPE_GAME_TREE) || row.shape == SHAPE_JSGME
}

fn to_card(cfg: &AppConfig, row: PendingFolderRow) -> PendingFolder {
    let (file_count, size_bytes) = crate::libpath::resolve(cfg.library_path.as_deref(), &row.library_path)
        .map(|d| weigh(&d))
        .unwrap_or((0, 0));
    let (suggestion, actions) = actions_for(&row);
    PendingFolder {
        id: row.id,
        archive: row.archive,
        rel_path: row.rel_path,
        owner_id: row.owner_id,
        owner_kind: row.owner_kind,
        shape: row.shape,
        title: row.title,
        description: row.description,
        readme: row.readme,
        skin_target: row.skin_target,
        replaced: row.replaced,
        file_count,
        size_bytes,
        suggestion,
        actions,
    }
}

pub fn list(conn: &Connection, cfg: &AppConfig) -> Result<Vec<PendingFolder>, String> {
    Ok(overlay::list_pending_folders(conn)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|r| to_card(cfg, r))
        .collect())
}

/// Contenu texte de la notice d'un dossier proposé.
///
/// §4.6bis le dit déjà : pour les dossiers que rien ne permet d'interpréter, la
/// réponse honnête n'est pas de deviner, c'est de rendre la notice **lisible**.
/// Ici elle se lit sans quitter l'écran où la décision se prend — le va-et-vient
/// vers l'explorateur est précisément ce qui fait cliquer au hasard.
///
/// Garde-fou anti-traversée et plafond de taille hérités de la
/// prévisualisation des ressources (§4.5.2) : le nom vient du front, donc il
/// est traité comme tel.
pub fn read_document(conn: &Connection, cfg: &AppConfig, id: &str, name: &str) -> Result<String, String> {
    let row = overlay::get_pending_folder(conn, id)
        .map_err(|e| e.to_string())?
        .ok_or(crate::errors::PENDING_NOT_FOUND)?;
    let dir = crate::libpath::resolve(cfg.library_path.as_deref(), &row.library_path)
        .ok_or(crate::errors::LIBRARY_NOT_CONFIGURED)?;
    let bytes = resources::read_resource(&dir, name)?;
    // `from_utf8_lossy` : ces notices sont souvent en Latin-1 ou en UTF-16 avec
    // BOM. Rendre un caractère de remplacement vaut mieux que refuser d'afficher
    // la seule chose qui explique quoi faire.
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

// --- Résolution -------------------------------------------------------------

/// Applique le sort choisi par l'utilisateur. La ligne et le dossier d'attente
/// disparaissent dans tous les cas : soit le contenu a été rangé ailleurs, soit
/// il a été écarté.
pub fn resolve(conn: &Connection, cfg: &AppConfig, id: &str, action: &str) -> Result<(), String> {
    let row = overlay::get_pending_folder(conn, id)
        .map_err(|e| e.to_string())?
        .ok_or(crate::errors::PENDING_NOT_FOUND)?;
    let library = cfg.library_path.as_ref().ok_or(crate::errors::LIBRARY_NOT_CONFIGURED)?;
    let dir = crate::libpath::resolve(Some(library), &row.library_path).ok_or(crate::errors::LIBRARY_NOT_CONFIGURED)?;
    if !dir.is_dir() {
        // Le dossier a disparu sous nos pieds : la ligne n'a plus d'objet.
        let _ = overlay::delete_pending_folder(conn, id);
        return Err(crate::errors::PENDING_NOT_FOUND.into());
    }

    match action {
        ACTION_DISCARD => discard(conn, &row, &dir)?,
        ACTION_RESOURCES => into_resources(&row, library, &dir)?,
        ACTION_GAME => into_game(conn, cfg, &row, library, &dir)?,
        ACTION_LAYER => into_layer(conn, cfg, &row, library, &dir)?,
        ACTION_OTHER => into_other(conn, cfg, &row, library, &dir)?,
        _ => return Err(crate::errors::PENDING_UNKNOWN_ACTION.into()),
    }

    let _ = std::fs::remove_dir_all(&dir);
    overlay::delete_pending_folder(conn, id).map_err(|e| e.to_string())
}

/// Écarté : supprimé, et **dit**. C'est le seul endroit de l'app où de la
/// matière importée disparaît, d'où la ligne de journal — sans elle, un
/// « il me manque quelque chose » six mois plus tard serait indiagnosticable.
fn discard(conn: &Connection, row: &PendingFolderRow, dir: &Path) -> Result<(), String> {
    std::fs::remove_dir_all(dir).map_err(|e| e.to_string())?;
    overlay::record_decision(
        conn,
        row.owner_id.as_deref(),
        &row.archive,
        "userDiscarded",
        &row.rel_path,
        None,
    );
    Ok(())
}

/// Dossier ressources du propriétaire, ou celui d'une entrée « autre mod »
/// portant l'id du dossier proposé quand il n'a pas de propriétaire.
fn resources_dir_of(row: &PendingFolderRow, library: &Path) -> PathBuf {
    match (&row.owner_id, &row.owner_kind) {
        (Some(id), Some(kind)) => resources::resources_dir_for(library, kind, &[id]),
        _ => resources::resources_dir_for(library, "others", &[&row.id]),
    }
}

fn into_resources(row: &PendingFolderRow, library: &Path, dir: &Path) -> Result<(), String> {
    // Sous son propre nom : deux dossiers proposés du même mod (« 2K Skins » et
    // « No Dust Skins ») ne doivent pas se mélanger une fois rangés.
    let name = leaf_name(&row.rel_path);
    let dest = crate::importer::unique_dir(&resources_dir_of(row, library).join(name));
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    crate::archive::move_dir(dir, &dest).map_err(|e| e.to_string())
}

/// Sort les documents d'information posés **à la racine** du dossier proposé
/// vers les ressources, avant que le reste ne parte en couche ou dans le jeu.
///
/// Même règle qu'à l'import (§4.5.2) et pour la même raison : à la racine de ce
/// que l'auteur a livré à côté du mod, un document est une notice. Ce qui est
/// plus profond ne bouge pas — un `.txt` dans `skins/<livrée>/` fait partie de
/// la livrée. Best-effort : une notice qu'on n'arrive pas à ranger part avec le
/// reste, ce qui est laid mais pas destructeur.
fn lift_ancillaries(res_dir: &Path, dir: &Path, mode: ExtractionMode) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if !p.is_file() || crate::resources::route_beside_root(&p, mode) != resources::Route::Resources {
            continue;
        }
        let rel = Path::new(&e.file_name()).to_path_buf();
        if let Err(err) = crate::extras::store(res_dir, &rel, &p, false) {
            log::warn!("lift_ancillaries {}: {err}", rel.display());
        }
    }
}

/// Dernier segment du chemin d'archive — le nom que l'auteur a donné.
fn leaf_name(rel_path: &str) -> String {
    rel_path
        .rsplit('/')
        .find(|s| !s.is_empty())
        .unwrap_or(rel_path)
        .to_string()
}

/// Installé dans le jeu. C'est **la réponse de l'utilisateur** qui autorise à
/// chercher la racine de jeu à l'intérieur du dossier : sans elle, l'app
/// n'avait que le chemin d'archive pour trancher, et devait donc refuser
/// (§4.5.3). Avec elle, il n'y a plus rien à deviner.
fn into_game(
    conn: &Connection,
    cfg: &AppConfig,
    row: &PendingFolderRow,
    library: &Path,
    dir: &Path,
) -> Result<(), String> {
    let root = crate::acpath::effective_root(dir);
    match (&row.owner_id, &row.owner_kind) {
        (Some(owner), Some(kind)) => {
            let owner_kind = OwnerKind::parse(kind).ok_or(crate::errors::PENDING_UNKNOWN_ACTION)?;
            let sat = crate::extras::dir(library, owner_kind, owner);
            for e in std::fs::read_dir(&root).map_err(|e| e.to_string())?.flatten() {
                let rel = Path::new(&e.file_name()).to_path_buf();
                crate::extras::store(&sat, &rel, &e.path(), false)?;
            }
            crate::extras::deploy(conn, cfg, owner_kind, owner).map(|_| ())
        }
        // Sans propriétaire, « installer dans le jeu » et « autre mod » sont le
        // même geste — à ceci près qu'on part de la racine de jeu et non du
        // dossier d'emballage.
        _ => activate_as_other(conn, cfg, library, &row.id, &root),
    }
}

fn into_other(
    conn: &Connection,
    cfg: &AppConfig,
    row: &PendingFolderRow,
    library: &Path,
    dir: &Path,
) -> Result<(), String> {
    activate_as_other(conn, cfg, library, &row.id, dir)
}

fn activate_as_other(conn: &Connection, cfg: &AppConfig, library: &Path, id: &str, src: &Path) -> Result<(), String> {
    let mode = ExtractionMode::parse(&cfg.prefs.resource_extraction_mode);
    let other =
        crate::others::import_other(conn, library, id, src, false, mode).ok_or(crate::errors::PENDING_ALREADY_KNOWN)?;
    crate::others::activate_other(conn, cfg, &other.id).map(|_| ())
}

/// Posé comme **couche** (§4.3) sur le mod : composé par-dessus sa version, sans
/// jamais la toucher. C'est la seule façon non destructive de répondre à ce que
/// l'auteur demande quand il propose des livrées de meilleure qualité ou des
/// textures alternatives — « copiez ceci dans le dossier de la voiture ».
/// Retirer la couche remet la voiture d'origine.
fn into_layer(
    conn: &Connection,
    cfg: &AppConfig,
    row: &PendingFolderRow,
    library: &Path,
    dir: &Path,
) -> Result<(), String> {
    let owner = row.owner_id.as_deref().ok_or(crate::errors::PENDING_UNKNOWN_ACTION)?;
    let kind = match row.owner_kind.as_deref() {
        Some("tracks") => ModKind::Track,
        Some("cars") => ModKind::Car,
        _ => return Err(crate::errors::PENDING_UNKNOWN_ACTION.into()),
    };
    let mode = ExtractionMode::parse(&cfg.prefs.resource_extraction_mode);
    // La notice sort **avant** : `store_layer` range en `Source::ModFolder`,
    // donc il n'extrait rien (règle d'or n°3) — et il a raison, une couche
    // importée est un dossier de mod. Ici ce n'en est pas un : c'est ce que
    // l'auteur a posé à côté, et son `READ ME.txt` finirait composé dans le
    // dossier de la voiture. On le range donc en ressources d'abord.
    lift_ancillaries(&resources_dir_of(row, library), dir, mode);
    // Compté contre la version active : c'est ce que la fiche affichera pour
    // dire ce que la couche apporte et ce qu'elle recouvre.
    let diff = crate::submods::parent_content_dir(conn, cfg, owner)
        .filter(|p| p.is_dir())
        .map(|base| crate::identity::diff_content(dir, &base))
        .unwrap_or_default();
    let mode = ExtractionMode::parse(&cfg.prefs.resource_extraction_mode);
    crate::layers::store_layer(
        conn,
        library,
        owner,
        kind,
        &leaf_name(&row.rel_path),
        dir,
        false,
        &diff,
        &row.archive,
        mode,
    )?;
    crate::compose::recompose(conn, cfg, owner)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(p: &Path, body: &[u8]) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, body).unwrap();
    }

    #[test]
    fn a_jsgme_variant_carries_the_title_its_author_wrote() {
        // Le `description.jsgme` est la seule notice d'un format exploitable
        // tel quel : première ligne le nom, le reste l'explication. Cas réel,
        // LA Canyons.
        let base = crate::testutil::temp_dir("pending-jsgme");
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig::default();
        let dir = base.join("Hide Pit Crew");
        write(&dir.join("content").join("objects3D").join("pitcrew.kn5"), b"x");
        write(
            &dir.join("description.jsgme"),
            b"Hide Pit Crew\n\nHides the Pit Crew for more immersion\nBackup files included",
        );

        let d = detect(&conn, &cfg, &dir, &[]);
        assert_eq!(d.shape, SHAPE_JSGME);
        assert_eq!(d.title.as_deref(), Some("Hide Pit Crew"), "le nom vient de l'auteur");
        assert!(
            d.description.as_deref().is_some_and(|s| s.contains("immersion")),
            "et son explication aussi"
        );
    }

    #[test]
    fn offered_liveries_are_recognised_by_the_names_they_overwrite() {
        // Cas réel (Ferrari F2002) : `2K Skins/skins/<skin>` porte exactement
        // les noms de livrées de la voiture livrée à côté. Signal structurel —
        // le nom du dossier n'est jamais lu, il n'a rien d'un id de voiture.
        let base = crate::testutil::temp_dir("pending-skins");
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let ac = base.join("ac");
        let cfg = AppConfig {
            ac_install_path: Some(ac.clone()),
            ..Default::default()
        };
        // La voiture, telle que `parent_content_dir` la retrouve faute de mod
        // géré : sous `content/cars/<id>`.
        let car = ac.join("content").join("cars").join("ferrari_f2002");
        write(&car.join("skins").join("a_2002_michael").join("skin.dds"), b"x");
        write(&car.join("skins").join("b_2002_rubens").join("skin.dds"), b"x");

        let offered = base.join("2K Skins");
        write(&offered.join("skins").join("a_2002_michael").join("skin.dds"), b"HD");
        write(&offered.join("READ ME.txt"), b"2K liveries");

        let owners = vec![("ferrari_f2002".to_string(), OwnerKind::Car)];
        let d = detect(&conn, &cfg, &offered, &owners);
        assert_eq!(d.shape, SHAPE_SKIN_VARIANT);
        assert_eq!(d.skin_target.as_deref(), Some("ferrari_f2002"));
        assert_eq!(d.readme.as_deref(), Some("READ ME.txt"), "la notice est montrée");

        // Aucun recoupement : ce n'est pas une variante, c'est un pack pour
        // autre chose — et on ne le fait pas passer pour ce qu'il n'est pas.
        let unrelated = base.join("Some Pack");
        write(&unrelated.join("skins").join("zz_unknown").join("skin.dds"), b"x");
        assert_eq!(detect(&conn, &cfg, &unrelated, &owners).skin_target, None);
    }

    #[test]
    fn a_folder_of_documents_is_told_apart_from_one_that_holds_a_game_tree() {
        let base = crate::testutil::temp_dir("pending-shapes");
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig::default();

        let docs = base.join("Templates");
        write(&docs.join("body.psd"), b"x");
        write(&docs.join("READ ME.txt"), b"x");
        assert_eq!(detect(&conn, &cfg, &docs, &[]).shape, SHAPE_DOCUMENTS);

        let tree = base.join("Optional Install");
        write(&tree.join("content").join("cars").join("x").join("y.ini"), b"x");
        assert_eq!(detect(&conn, &cfg, &tree, &[]).shape, SHAPE_GAME_TREE);

        // Des images : rien ne distingue une capture d'un asset AC (§4.5.2), on
        // ne tranche donc pas — et c'est l'utilisateur qui sait.
        let shots = base.join("Wallpapers");
        write(&shots.join("01.jpg"), b"x");
        assert_eq!(detect(&conn, &cfg, &shots, &[]).shape, SHAPE_UNKNOWN);
    }

    #[test]
    fn a_discarded_folder_leaves_the_disk_but_not_the_journal() {
        // L'amendement à « l'import ne jette rien » (§4.5.3) : ce que
        // l'utilisateur écarte est supprimé, et **dit**. L'information reste,
        // la matière part.
        let base = crate::testutil::temp_dir("pending-discard");
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            library_path: Some(library.clone()),
            ..Default::default()
        };

        let src = base.join("src").join("Wallpapers");
        write(&src.join("01.jpg"), b"x");
        let id = park(
            &conn,
            &library,
            "Pack.7z",
            Path::new("Wallpapers"),
            &src,
            Some(("some_car", OwnerKind::Car)),
            Detected {
                shape: SHAPE_UNKNOWN.into(),
                ..Default::default()
            },
            true,
            0,
        )
        .expect("mis en attente");

        let parked = dir_for(&library, &id);
        assert!(parked.join("01.jpg").is_file(), "rangé, pas posé");
        assert_eq!(list(&conn, &cfg).unwrap().len(), 1);

        resolve(&conn, &cfg, &id, ACTION_DISCARD).unwrap();
        assert!(!parked.exists(), "la matière part");
        assert!(list(&conn, &cfg).unwrap().is_empty());
        let journal = overlay::decisions_for_mod(&conn, "some_car").unwrap();
        assert!(
            journal
                .iter()
                .any(|d| d.kind == "userDiscarded" && d.subject == "Wallpapers"),
            "et l'information reste"
        );
    }

    #[test]
    fn keeping_a_folder_as_resources_files_it_under_its_own_name() {
        // Deux dossiers proposés du même mod (« 2K Skins », « No Dust Skins »)
        // ne doivent pas se mélanger une fois rangés.
        let base = crate::testutil::temp_dir("pending-resources");
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            library_path: Some(library.clone()),
            ..Default::default()
        };

        for name in ["2K Skins", "No Dust Skins"] {
            let src = base.join("src").join(name);
            write(&src.join("READ ME.txt"), name.as_bytes());
            let id = park(
                &conn,
                &library,
                "F2002.7z",
                Path::new(name),
                &src,
                Some(("ferrari_f2002", OwnerKind::Car)),
                Detected {
                    shape: SHAPE_DOCUMENTS.into(),
                    ..Default::default()
                },
                true,
                0,
            )
            .expect("mis en attente");
            resolve(&conn, &cfg, &id, ACTION_RESOURCES).unwrap();
        }

        let res = resources::resources_dir_for(&library, "cars", &["ferrari_f2002"]);
        assert_eq!(
            std::fs::read(res.join("2K Skins").join("READ ME.txt")).unwrap(),
            b"2K Skins"
        );
        assert_eq!(
            std::fs::read(res.join("No Dust Skins").join("READ ME.txt")).unwrap(),
            b"No Dust Skins"
        );
    }
}
