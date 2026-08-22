//! Ajouts au jeu d'un mod (§4.5.3) : ce qu'une archive livre **à côté** du
//! dossier du mod mais qui lui appartient — configs CSP
//! (`extension/config/cars/rss/<id>/…`), shaders (`system/shaders/…`),
//! textures d'équipe (`content/texture/…`), modèle de pilote
//! (`content/driver/…`). AC les lit hors de `content/<type>/<id>`, ils ne
//! peuvent donc pas voyager dans le dossier du mod.
//!
//! **Stockés bruts, avec leur chemin relatif à la racine d'AC**, dans un arbre
//! dédié (`<lib>/extras/<type>/<id>/…`) — jamais dans la version, qui est
//! déployée telle quelle dans `content/`. Deux propriétés en découlent :
//!
//! - **L'import ne jette rien.** Ce qui n'est pas classé est conservé tel quel,
//!   donc l'*interprétation* (où poser, qui arbitre un fichier partagé) reste
//!   recalculable depuis la bibliothèque à tout moment. Aucune règle des
//!   versions précédentes à mémoriser, aucune archive à conserver : c'est
//!   l'entrée qui est préservée, pas la décision.
//! - **L'ajout vit et meurt avec son mod.** Posé à l'activation, retiré à
//!   la désactivation, supprimé avec lui — c'est ce que le passage par « autre
//!   mod » ne donnait pas : les fichiers d'une voiture désinstallée restaient
//!   dans AC, rattachés à une entrée anonyme que plus rien ne reliait au mod.
//!
//! Au **niveau du mod**, pas de la version (comme `resources/`, §4.5.2) : les
//! configs CSP d'une mise à jour remplacent celles de la précédente, ce qui est
//! le comportement voulu, et les couches (§4.3) partagent le même arbre.
//!
//! Pose **fichier par fichier** (hardlink), jamais par jonction de dossier :
//! plusieurs mods visent les mêmes arbres (`extension/textures/common/rss/…`
//! est livré à l'identique par chaque voiture RSS), et une jonction de dossier
//! en donnerait la propriété exclusive au premier arrivé.
//!
//! **Fichiers partagés** : chaque mod *réclame* les chemins d'AC dont il a
//! besoin (`extra_links`), et deux règles suffisent.
//!
//! - *Compteur de références* — un fichier n'est retiré d'AC que lorsque plus
//!   aucun mod ne le réclame. Désactiver une voiture RSS n'emporte pas les
//!   textures communes dont douze autres dépendent, et il n'y a plus de course
//!   à la propriété : le premier arrivé ne gagne rien.
//! - *Arbitrage par date* — l'exemplaire à la **date de modification la plus
//!   récente** gagne, un mod plus récent corrigeant en général des bugs de
//!   celui d'avant. La date traverse la chaîne intacte : 7-Zip restitue celle
//!   stockée dans l'archive, `std::fs::copy` la conserve sous Windows, et un
//!   hardlink partage l'entrée MFT. À égalité (archives repackées par un tiers,
//!   qui perdent les dates), c'est le dernier mod installé.
//!
//! Un fichier que **personne ne réclame** — contenu Kunos, ou mod installé hors
//! de l'app — relève du même arbitrage : un exemplaire plus récent le remplace,
//! mais seulement après que l'original a été mis à l'abri (`gamebackup`, §4.5.4),
//! et il revient dès que plus aucun mod ne réclame le chemin. Un exemplaire
//! plus ancien ou de même date ne prend jamais la place de ce qui tourne déjà.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use walkdir::WalkDir;

use crate::config::AppConfig;
use crate::modscan::ModKind;
use crate::overlay;

/// Ce qui peut **posséder** des ajouts au jeu : une voiture, un circuit, ou une
/// app (§12bis.4). Donne le segment d'arbre sous `extras/` — et le même sous
/// `resources/`, les deux arbres étant rangés à l'identique.
///
/// Une app en a besoin pour la même raison qu'une voiture : ce qu'une archive
/// livre **à côté** de son dossier lui appartient quand même. Sans ce type, le
/// balayage des restes (§7.3) ne connaissait que voitures et circuits, donc le
/// `READ ME.pdf` livré à côté d'une app n'avait aucun propriétaire possible et
/// devenait un « autre mod » à nom absurde
/// (`_RSS_Settings….rar__READ ME - RSS Settings Application.pdf`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnerKind {
    Car,
    Track,
    App,
    /// L'**archive** elle-même, quand elle livre plusieurs mods (§4.4). Le
    /// `MANUAL.pdf` d'un pack de vingt voitures n'appartient à aucune d'elles :
    /// il documente le pack. Sans ce propriétaire, tout ce qui entourait un
    /// pack tombait en « autre mod » anonyme — y compris ses `content/fonts`,
    /// qui survivaient à la suppression de toutes ses voitures.
    Pack,
}

impl OwnerKind {
    /// Segment de bibliothèque : `<lib>/extras/<category>/<id>`.
    pub fn category(self) -> &'static str {
        match self {
            OwnerKind::Car => "cars",
            OwnerKind::Track => "tracks",
            OwnerKind::App => "apps",
            OwnerKind::Pack => "packs",
        }
    }

    /// Relit ce que [`Self::category`] a écrit dans `extra_links.kind`. Accepte
    /// le singulier et n'importe quelle casse, parce que ce type est persisté
    /// en base et comparé à plusieurs endroits, et qu'un écart y est
    /// **silencieux** : un circuit relu comme voiture cherche simplement dans
    /// le mauvais arbre de bibliothèque et ne trouve rien. Bug réel : les
    /// ajouts au jeu d'un circuit étaient posés puis immédiatement effacés,
    /// parce que `"tracks"` était comparé à `"Track"`.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "car" | "cars" => Some(OwnerKind::Car),
            "track" | "tracks" => Some(OwnerKind::Track),
            "app" | "apps" => Some(OwnerKind::App),
            "pack" | "packs" => Some(OwnerKind::Pack),
            _ => None,
        }
    }
}

impl From<ModKind> for OwnerKind {
    fn from(k: ModKind) -> Self {
        match k {
            ModKind::Car => OwnerKind::Car,
            ModKind::Track => OwnerKind::Track,
        }
    }
}

/// Arbre des ajouts au jeu d'un mod : `<lib>/extras/<type>/<id>`.
pub fn dir(library: &Path, owner: OwnerKind, id: &str) -> PathBuf {
    library.join("extras").join(owner.category()).join(id)
}

/// Racine depuis laquelle les chemins de cet arbre sont relatifs à AC —
/// emballage de l'auteur traversé ([`crate::acpath::effective_root`]).
///
/// L'import applique déjà cette règle au balayage (§7.3), mais un arbre rangé
/// **avant** ce correctif garde l'emballage figé, et rien ne le répare : les
/// ajouts au jeu se recalculent depuis le disque, jamais depuis l'archive.
/// D'où la traversée ici aussi — même choix que `others::relative_files`.
///
/// Cas réel : une archive dont le `.zip` porte un nom dupliqué
/// (`Track - … SchleifeTrack - … Schleife.zip`) et contient un dossier du même
/// nom, avec `content/` **et** `extension/` dedans. Le circuit s'installait —
/// `modscan` descend l'emballage — mais les huit images de fond livrées à côté
/// restaient en bibliothèque : deux moitiés du même import qui ne s'accordaient
/// pas sur la racine.
fn root_of(sat: &Path) -> PathBuf {
    crate::acpath::effective_root(sat)
}

/// Range un reste dans les ajouts au jeu du mod, à `rel` (son chemin relatif à la
/// racine de l'archive, donc à la racine d'AC). Fusionne avec l'existant : une
/// mise à jour du mod remplace ses propres fichiers, sans effacer les autres.
pub fn store(sat_dir: &Path, rel: &Path, src: &Path, copy: bool) -> Result<(), String> {
    let dest = sat_dir.join(rel);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("ajout au jeu : {e}"))?;
    }
    if src.is_dir() {
        if copy {
            crate::archive::copy_dir(src, &dest)
        } else {
            crate::archive::move_dir(src, &dest)
        }
        .map_err(|e| format!("ajout au jeu : {e}"))
    } else {
        if !copy && std::fs::rename(src, &dest).is_ok() {
            return Ok(());
        }
        std::fs::copy(src, &dest)
            .map(|_| ())
            .map_err(|e| format!("ajout au jeu : {e}"))
    }
}

/// Supprime l'arbre des ajouts au jeu d'un mod (suppression du mod).
pub fn remove_tree(library: &Path, owner: OwnerKind, id: &str) {
    let d = dir(library, owner, id);
    if d.exists() {
        if let Err(e) = std::fs::remove_dir_all(&d) {
            log::warn!("remove extras tree {}: {e}", d.display());
        }
    }
}

/// Aligne les ajouts au jeu d'un **pack** sur l'état de ses membres (§4.4) :
/// posés dès qu'au moins un est actif, retirés quand aucun ne l'est.
///
/// Un pack ne s'active pas lui-même — c'est une métadonnée partagée, pas une
/// entité déployable. Mais ce qu'il livre (`content/fonts`, une config CSP
/// commune) n'a de sens dans le jeu que tant qu'au moins une de ses voitures y
/// est. D'où cette synchronisation, appelée après chaque activation,
/// désactivation ou suppression d'un membre.
///
/// Best-effort : un pack dont les ajouts ne peuvent pas être posés ne doit pas
/// empêcher la voiture de rouler.
pub fn sync_pack(conn: &Connection, cfg: &AppConfig, pack: &str) {
    let members = overlay::list_pack_ids(conn, pack).unwrap_or_default();
    let any_active = members.iter().any(|id| {
        overlay::get_mod(conn, id)
            .ok()
            .flatten()
            .and_then(|m| ModKind::from_kind(&m.kind))
            .is_some_and(|k| crate::activation::is_mod_active(cfg, k, id))
    });
    let outcome = if any_active {
        deploy(conn, cfg, OwnerKind::Pack, pack).map(|_| ())
    } else {
        undeploy(conn, cfg, pack)
    };
    if let Err(e) = outcome {
        log::warn!("sync_pack {pack}: {e}");
    }
}

/// Exemplaire d'un fichier partagé proposé par un mod.
struct Claim {
    mod_id: String,
    src: PathBuf,
    /// Date de modification du fichier **dans l'archive de l'auteur** : 7-Zip
    /// restitue la date stockée, `std::fs::copy` la conserve sous Windows, et
    /// un hardlink partage l'entrée MFT. Elle traverse donc toute la chaîne
    /// intacte, et distingue deux versions d'un même fichier RSS.
    mtime: std::time::SystemTime,
    /// Départage deux exemplaires de même date : le dernier mod installé gagne.
    claimed_at: String,
}

/// Ce que l'arbitrage a trouvé pour un chemin d'AC. La distinction entre les
/// deux derniers cas est ce qui décide d'une **suppression** : seule l'absence
/// de réclamant en base l'autorise. Ne pas savoir résoudre une réclamation
/// (bibliothèque déplacée, exemplaire disparu, `kind` illisible) n'est jamais
/// une raison d'effacer — c'est la réclamation qui décide, pas notre capacité
/// à la suivre.
enum Arbitration {
    /// Un mod réclame le chemin et son exemplaire a été retrouvé.
    Winner(Claim),
    /// Plus aucun mod ne réclame le chemin : il doit partir.
    Unclaimed,
    /// Des mods le réclament encore, mais aucun exemplaire n'a pu être lu.
    Unresolvable,
}

/// Qui, parmi les mods qui réclament ce fichier, fournit l'exemplaire à poser.
/// **La date de modification la plus récente gagne** — un mod plus récent
/// corrige en général des bugs de celui d'avant. À égalité (archives repackées
/// par un tiers, qui perdent les dates), c'est le dernier mod installé.
fn best_claim(conn: &Connection, cfg: &AppConfig, ac_path: &Path) -> Arbitration {
    let (Some(library), Some(ac)) = (cfg.library_path.as_ref(), cfg.ac_install_path.as_ref()) else {
        return Arbitration::Unresolvable;
    };
    let Ok(rel) = ac_path.strip_prefix(ac) else {
        return Arbitration::Unresolvable;
    };
    let rows = match overlay::extra_claimants(conn, &ac_path.to_string_lossy()) {
        Ok(rows) => rows,
        Err(e) => {
            log::warn!("extra_claimants {}: {e}", ac_path.display());
            return Arbitration::Unresolvable;
        }
    };
    if rows.is_empty() {
        return Arbitration::Unclaimed;
    }
    let best = rows
        .into_iter()
        .filter_map(|(mod_id, kind, claimed_at)| {
            // `kind` a été écrit par `set_extra_links` sous la forme
            // `OwnerKind::category()` ("cars"/"tracks"/"apps") : le relire
            // autrement enverrait la recherche dans le mauvais arbre de
            // bibliothèque. Un type inconnu est **ignoré**, jamais traité comme
            // une absence de réclamant : c'est la réclamation qui décide d'une
            // suppression, pas notre capacité à la suivre (§4.5.4, règle 4).
            let Some(kind) = OwnerKind::parse(&kind) else {
                log::warn!("extras claim {mod_id}: unknown kind {kind:?}, ignored");
                return None;
            };
            // `root_of` et non `dir` : l'exemplaire vit sous la racine
            // traversée, comme à la pose. Sans ça, l'arbre d'un mod emballé
            // n'est jamais résolu, donc jamais désigné fournisseur — le fichier
            // est posé mais la fiche le dit non posé.
            let src = root_of(&dir(library, kind, &mod_id)).join(rel);
            let mtime = std::fs::metadata(&src)
                .and_then(|m| m.modified())
                .inspect_err(|e| log::warn!("extras claim {mod_id}: {}: {e}", src.display()))
                .ok()?;
            Some(Claim {
                mod_id,
                src,
                mtime,
                claimed_at,
            })
        })
        .max_by(|a, b| a.mtime.cmp(&b.mtime).then_with(|| a.claimed_at.cmp(&b.claimed_at)));
    match best {
        Some(c) => Arbitration::Winner(c),
        None => Arbitration::Unresolvable,
    }
}

/// Aligne le fichier posé dans AC sur l'exemplaire qui doit gagner. Sans
/// réclamant, le fichier est retiré : plus aucun mod n'en dépend.
///
/// Le fournisseur courant est lu en base, jamais déduit de la taille et de la
/// date du fichier posé : c'est précisément dans le cas qu'on veut arbitrer —
/// deux exemplaires de même date — que cette déduction se trompe.
fn sync(conn: &Connection, cfg: &AppConfig, ac_path: &Path) {
    let key = ac_path.to_string_lossy().into_owned();
    let best = match best_claim(conn, cfg, ac_path) {
        Arbitration::Winner(c) => c,
        Arbitration::Unclaimed => {
            // Plus aucun réclamant. Si ce chemin était un fichier du jeu qu'un
            // mod avait remplacé, l'original revient (§4.5.4) ; sinon le
            // fichier part.
            if crate::gamebackup::is_replaced(conn, ac_path) {
                crate::gamebackup::restore(conn, ac_path);
            } else if ac_path.is_file() {
                if let Err(e) = std::fs::remove_file(ac_path) {
                    log::warn!("extras remove {}: {e}", ac_path.display());
                }
            }
            return;
        }
        // Encore réclamé, mais introuvable en bibliothèque : on laisse en place
        // ce qui tourne. Effacer ici, c'était retirer d'AC un fichier qu'un mod
        // actif venait de poser.
        Arbitration::Unresolvable => {
            log::warn!(
                "extras sync {}: still claimed but no copy found, left alone",
                ac_path.display()
            );
            return;
        }
    };
    let current = overlay::extra_provider(conn, &key).unwrap_or(None);
    if current.as_deref() == Some(best.mod_id.as_str()) && ac_path.is_file() {
        return;
    }
    if ac_path.exists() {
        if let Err(e) = std::fs::remove_file(ac_path) {
            log::warn!("extras replace {}: {e}", ac_path.display());
            return;
        }
    }
    match crate::deploy::link_or_copy(&best.src, ac_path) {
        Ok(()) => {
            if let Err(e) = overlay::set_extra_provider(conn, &key, &best.mod_id) {
                log::warn!("set_extra_provider {}: {e}", ac_path.display());
            }
        }
        Err(e) => log::warn!("extras replace {} <- {}: {e}", ac_path.display(), best.mod_id),
    }
}

/// Pose les ajouts au jeu du mod dans AC et mémorise exactement ce qu'il réclame
/// — c'est cette liste, et elle seule, qui sera retirée à la désactivation.
/// Best-effort : un fichier qui ne peut pas être posé est signalé, jamais forcé.
pub fn deploy(conn: &Connection, cfg: &AppConfig, owner: OwnerKind, mod_id: &str) -> Result<usize, String> {
    let (Some(library), Some(ac)) = (cfg.library_path.as_ref(), cfg.ac_install_path.as_ref()) else {
        return Ok(0);
    };
    let sat = dir(library, owner, mod_id);
    if !sat.is_dir() {
        return Ok(0);
    }

    let sat = root_of(&sat);
    let mut files: Vec<PathBuf> = Vec::new();
    let mut created_dirs: BTreeSet<PathBuf> = BTreeSet::new();
    for entry in WalkDir::new(&sat).into_iter().flatten() {
        if !entry.file_type().is_file() {
            continue;
        }
        let src = entry.path();
        let Ok(rel) = src.strip_prefix(&sat) else { continue };
        // Chemin qui n'en est pas un : dossier d'emballage de l'auteur pris
        // pour un chemin de jeu (§4.5.3). Conservé en bibliothèque et listé
        // dans l'onglet, mais jamais posé — sinon `Track Installation/` et
        // consorts se déversent à la racine de l'install.
        if !crate::acpath::is_ac_relative(rel) {
            log::warn!("extras {mod_id}: {} is not an AC path, not deployed", rel.display());
            continue;
        }
        let target = ac.join(rel);

        // Fichier déjà présent : trois cas, et un seul est un refus.
        if target.exists() {
            let claimed = overlay::extra_claimants(conn, &target.to_string_lossy())
                .map(|c| !c.is_empty())
                .unwrap_or(false);
            // 1. Réclamé par un autre mod : fichier partagé, on s'y ajoute et
            //    l'arbitrage par date (`sync`) tranche.
            // 2. Déjà remplacé par nous : l'original est à l'abri, même chose.
            // 3. Fichier du jeu intact : le **même arbitrage par date**
            //    s'applique. Un exemplaire plus récent le remplace, après
            //    sauvegarde (§4.5.4) ; un exemplaire plus ancien ou de même date
            //    ne prend pas la place de ce qui tourne déjà. Sans cette
            //    comparaison, le dernier mod installé écraserait une font mise
            //    à jour par un autre outil, ce que rien ne justifie.
            if !claimed && !crate::gamebackup::is_replaced(conn, &target) {
                // **Sauf autorisation explicite** (§4.6ter). L'arbitrage par
                // date protège les poses automatiques ; il n'a aucune autorité
                // contre une décision prise en connaissance de cause. Cas réel,
                // le patch « Hide Pit Crew » de LA Canyons : ses `pitcrew.kn5`
                // datent de 2020, ceux de l'install Kunos portent la date du
                // téléchargement Steam — donc plus récents, donc le patch
                // n'arrivait jamais, alors que l'utilisateur venait de lire
                // « remplace 2 fichiers du jeu de base » et de répondre oui.
                //
                // La sauvegarde de l'original reste obligatoire : c'est elle
                // qui rend l'opération sûre (§4.5.4), pas la comparaison de
                // dates.
                let forced = overlay::is_forced_extra(conn, mod_id, &target.to_string_lossy());
                if !forced && !crate::gamebackup::is_newer(src, &target) {
                    log::warn!(
                        "extras {mod_id}: {} exists and is not older, left alone",
                        target.display()
                    );
                    continue;
                }
                // `protect` refuse s'il n'a pas pu sécuriser l'original — et
                // alors on ne touche à rien.
                if !crate::gamebackup::protect(conn, cfg, &target) {
                    log::warn!(
                        "extras {mod_id}: {} could not be backed up, left alone",
                        target.display()
                    );
                    continue;
                }
            }
            files.push(target);
            continue;
        }

        // Dossiers qu'il faut créer pour poser ce fichier — mémorisés avant de
        // les créer, sinon on ne saurait plus, au retrait, lesquels étaient
        // déjà là. « Dossier vide » ne suffit pas comme critère : un dossier
        // d'AC préexistant peut le devenir.
        let mut cur = target.parent();
        while let Some(d) = cur {
            if d == ac || !d.starts_with(ac) || d.exists() {
                break;
            }
            created_dirs.insert(d.to_path_buf());
            cur = d.parent();
        }
        match crate::deploy::link_or_copy(src, &target) {
            Ok(()) => files.push(target),
            Err(e) => log::warn!("extras deploy {} -> {}: {e}", mod_id, target.display()),
        }
    }

    let placed = files.len();
    let mut entries: Vec<(String, bool)> = files
        .iter()
        .map(|f| (f.to_string_lossy().into_owned(), false))
        .collect();
    entries.extend(
        created_dirs
            .into_iter()
            .map(|d| (d.to_string_lossy().into_owned(), true)),
    );
    // Enregistré **avant** l'arbitrage : `sync` lit les réclamations en base,
    // ce mod doit donc déjà y figurer pour pouvoir gagner.
    overlay::set_extra_links(conn, mod_id, owner.category(), &entries).map_err(|e| e.to_string())?;
    for f in &files {
        sync(conn, cfg, f);
    }
    Ok(placed)
}

/// Le fichier posé dans AC est-il **encore celui que nous y avons mis** ?
///
/// La question se pose parce que certains chemins sont partagés avec un outil
/// externe : `extension/config/tracks/loaded/` est la cible de synchro du
/// téléchargeur de configs de Content Manager (§4.5.3, [`crate::acpath`]). Si
/// CM a repris le chemin entre-temps, ce qui est là ne nous appartient plus, et
/// la règle d'or n°5 vaut dans les deux sens : on ne supprime pas un fichier
/// qu'on n'a pas posé.
///
/// Le critère est **taille + date de modification**, et il couvre les deux
/// formes que prend la pose (`deploy::link_or_copy`) :
/// - **hardlink** (cas normal) — les deux chemins partagent l'entrée MFT, donc
///   taille et date sont identiques par construction, et le restent si un outil
///   réécrit le fichier *en place* (c'est alors bien toujours notre entrée) ;
/// - **copie physique** (repli quand bibliothèque et jeu sont sur deux volumes)
///   — `std::fs::copy` conserve la date sous Windows, c'est déjà le critère
///   d'arbitrage des exemplaires (§4.5.4).
///
/// Ce qu'il détecte, c'est la **recréation** du fichier par quelqu'un d'autre :
/// contenu différent, date de téléchargement fraîche. L'index de fichier NTFS
/// serait plus direct, mais `MetadataExt::file_index` est encore instable en
/// Rust stable (`windows_by_handle`) — et il donnerait la même réponse dans
/// tous les cas réalistes.
///
/// Source illisible = « pas à nous » : dans le doute on laisse en place, comme
/// partout ailleurs ici.
fn is_still_ours(src: &Path, deployed: &Path) -> bool {
    let (Ok(a), Ok(b)) = (std::fs::metadata(src), std::fs::metadata(deployed)) else {
        return false;
    };
    a.len() == b.len() && a.modified().ok() == b.modified().ok()
}

/// Retire la réclamation du mod sur ses ajouts au jeu, puis réaligne chaque
/// fichier : encore réclamé par un autre mod, il **reste** (et repasse à
/// l'exemplaire du meilleur réclamant restant) ; plus réclamé du tout, il est
/// retiré. C'est le compteur de références des fichiers partagés (§4.5.4) —
/// désactiver une voiture RSS n'emporte pas les textures communes dont douze
/// autres dépendent. Puis les dossiers créés pour l'occasion sont élagués, du
/// plus profond au plus superficiel ; `remove_dir` échoue sur un dossier non
/// vide, second garde-fou.
pub fn undeploy(conn: &Connection, cfg: &AppConfig, mod_id: &str) -> Result<(), String> {
    let links = overlay::get_extra_links(conn, mod_id).map_err(|e| e.to_string())?;
    // Garde-fou : on n'efface jamais hors du dossier d'AC, même si la base dit
    // le contraire (bibliothèque déplacée, chemin d'AC changé depuis la pose).
    let ac = cfg.ac_install_path.as_ref();
    let inside_ac = |p: &Path| ac.is_some_and(|ac| p.starts_with(ac));

    // Arbre des ajouts de ce mod, relevé **avant** d'effacer la réclamation :
    // c'est elle qui porte le type, et sans lui on ne sait plus où chercher
    // l'exemplaire de référence pour vérifier l'identité de ce qui est posé.
    let sat = links
        .first()
        .and_then(|(_, _, kind)| OwnerKind::parse(kind))
        .zip(cfg.library_path.as_ref())
        // Même racine qu'à la pose, emballage traversé : sinon la vérification
        // d'identité chercherait l'exemplaire à un chemin qui n'existe pas, le
        // lirait comme « plus à nous » et ne retirerait plus jamais rien.
        .map(|(owner, library)| root_of(&dir(library, owner, mod_id)));

    // La réclamation part d'abord : `sync` compte ce qui reste en base, ce mod
    // ne doit plus y figurer.
    overlay::set_extra_links(conn, mod_id, "", &[]).map_err(|e| e.to_string())?;
    for (p, _, _) in links.iter().filter(|(_, is_dir, _)| !is_dir) {
        let p = Path::new(p);
        if !inside_ac(p) {
            log::warn!("extras undeploy {}: outside AC, skipped", p.display());
            continue;
        }
        // Le fichier posé n'est plus le nôtre : un outil externe a repris le
        // chemin depuis. Cas concret, `extension/config/tracks/loaded/` — le
        // téléchargeur de configs de CM y écrit aussi (§4.5.3). On ne touche à
        // rien : ce qui est là appartient désormais à quelqu'un d'autre, et le
        // supprimer casserait l'install de l'utilisateur, pas la nôtre.
        if let (Some(sat), Some(ac)) = (sat.as_ref(), ac) {
            if let Ok(rel) = p.strip_prefix(ac) {
                if p.is_file() && !is_still_ours(&sat.join(rel), p) {
                    log::warn!(
                        "extras undeploy {}: replaced by another tool since deploy, left alone",
                        p.display()
                    );
                    continue;
                }
            }
        }
        sync(conn, cfg, p);
    }
    let mut dirs: Vec<&Path> = links
        .iter()
        .filter(|(_, is_dir, _)| *is_dir)
        .map(|(p, _, _)| Path::new(p.as_str()))
        .filter(|p| inside_ac(p))
        .collect();
    // Du plus profond au plus superficiel, sinon un parent encore peuplé de ses
    // propres sous-dossiers ne pourrait jamais être retiré.
    dirs.sort_by_key(|p| std::cmp::Reverse(p.components().count()));
    for d in dirs {
        let _ = std::fs::remove_dir(d);
    }
    Ok(())
}

/// Une entrée de l'onglet « Ajouts au jeu » de la fiche (§4.5.5).
#[derive(Debug, Clone, serde::Serialize)]
pub struct ExtraFile {
    /// Chemin relatif à la racine d'AC — c'est *l'*information utile : elle dit
    /// où le fichier atterrit dans le jeu (`extension/config/cars/…`).
    pub rel_path: String,
    pub size_bytes: u64,
    /// Actuellement posé dans AC par ce mod. Faux = un autre mod fournit le
    /// même fichier (partagé), ou le mod est inactif.
    pub deployed: bool,
    /// Mod qui fournit l'exemplaire posé, quand ce n'est pas celui-ci.
    pub provided_by: Option<String>,
    /// Ce chemin était un fichier du jeu : l'original est sauvegardé et sera
    /// restauré (§4.5.4). Signalé sur la fiche — une modification réversible mais
    /// invisible reste un piège.
    pub replaces_game_file: bool,
    /// Le chemin n'en est pas un du point de vue d'AC (dossier d'emballage de
    /// l'auteur) : conservé en bibliothèque, jamais posé dans le jeu. Signalé
    /// plutôt que masqué — un fichier listé qui n'arrive jamais dans le jeu
    /// sans qu'on dise pourquoi est plus déroutant qu'un fichier absent.
    pub off_game_path: bool,
    /// Le chemin est dans une zone qu'un outil externe synchronise
    /// ([`crate::acpath::is_externally_managed`]) — typiquement
    /// `extension/config/tracks/loaded/`, que le téléchargeur de configs de
    /// Content Manager réécrit. L'app pose quand même : arbitrer les choix de
    /// l'auteur n'est pas son rôle. Mais un ajout que CM remplacera sans
    /// prévenir ne doit pas avoir l'air stable, donc on le dit.
    pub externally_managed: bool,
    /// Un fichier étranger occupe déjà ce chemin dans AC : ni posé par nous, ni
    /// par un autre mod, ni un fichier du jeu qu'on a remplacé. L'exemplaire du
    /// mod a perdu l'arbitrage par date et attend (§4.5.4).
    ///
    /// C'est le cas le plus fréquent en zone auto-gérée, et il était totalement
    /// muet : les configs du dépôt CSP sont remises à jour en continu quand une
    /// archive porte la date de son packaging, donc CM gagne presque toujours —
    /// et rien à l'écran ne disait pourquoi le fichier du mod n'arrivait pas.
    pub held_by_foreign_file: bool,
}

/// Liste ce qu'un mod installe hors de `content/<type>/<id>`, **lu en direct
/// sur disque** comme le bloc Ressources (§4.5.5) : un mod importé avant que
/// l'app ne suive ces fichiers n'a rien à réimporter pour que l'onglet se
/// remplisse. L'état de pose, lui, vient de la base.
pub fn list(conn: &Connection, cfg: &AppConfig, owner: OwnerKind, mod_id: &str) -> Vec<ExtraFile> {
    let (Some(library), Some(ac)) = (cfg.library_path.as_ref(), cfg.ac_install_path.as_ref()) else {
        return Vec::new();
    };
    let sat = dir(library, owner, mod_id);
    if !sat.is_dir() {
        return Vec::new();
    }
    let sat = root_of(&sat);
    let mut out: Vec<ExtraFile> = WalkDir::new(&sat)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| {
            let rel = e.path().strip_prefix(&sat).ok()?.to_path_buf();
            let target = ac.join(&rel);
            let provider = overlay::extra_provider(conn, &target.to_string_lossy()).unwrap_or(None);
            let off_game_path = !crate::acpath::is_ac_relative(&rel);
            let replaces_game_file = crate::gamebackup::is_replaced(conn, &target);
            Some(ExtraFile {
                rel_path: rel.to_string_lossy().replace('\\', "/"),
                size_bytes: e.metadata().map(|m| m.len()).unwrap_or(0),
                deployed: provider.as_deref() == Some(mod_id),
                // Personne ne fournit ce chemin et pourtant il est occupé : ce
                // qui est là vient d'ailleurs (CM, une install manuelle). Sans
                // intérêt sur un chemin qu'on ne pose jamais de toute façon.
                held_by_foreign_file: !off_game_path && provider.is_none() && !replaces_game_file && target.is_file(),
                provided_by: provider.filter(|p| p != mod_id),
                replaces_game_file,
                off_game_path,
                externally_managed: crate::acpath::is_externally_managed(&rel),
            })
        })
        .collect();
    out.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(p: &Path, body: &[u8]) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, body).unwrap();
    }

    fn cfg_for(base: &Path) -> AppConfig {
        AppConfig {
            library_path: Some(base.join("library")),
            ac_install_path: Some(base.join("ac")),
            ..Default::default()
        }
    }

    /// Date de modification explicite — c'est le critère d'arbitrage, il faut
    /// pouvoir le poser plutôt que dépendre de l'ordre d'écriture des tests.
    fn set_mtime(p: &Path, secs_since_epoch: u64) {
        let t = std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs_since_epoch);
        std::fs::File::options()
            .write(true)
            .open(p)
            .unwrap()
            .set_modified(t)
            .unwrap();
    }

    #[test]
    fn a_wrapper_folder_is_kept_but_never_deployed() {
        // Bug réel : le dossier d'emballage de l'auteur (`Ferrari F2002
        // V1.4/`, `Track Installation/`, `Optional - No ambient sounds/`)
        // était pris pour un chemin de jeu et déversé à la racine de l'install.
        // Il reste rangé en bibliothèque et listé — l'import ne jette rien
        // (§4.5.3) — mais rien ne descend dans le jeu.
        let base = crate::testutil::temp_dir("sat-wrapper");
        let cfg = cfg_for(&base);
        let library = cfg.library_path.clone().unwrap();
        let ac = cfg.ac_install_path.clone().unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();

        let sat = dir(&library, OwnerKind::Car, "ferrari_f2002");
        write(&sat.join("Ferrari F2002 V1.4").join("READ ME.txt"), b"notice");
        let good = Path::new("content").join("driver").join("driver_501.kn5");
        write(&sat.join(&good), b"pilote");

        let n = deploy(&conn, &cfg, OwnerKind::Car, "ferrari_f2002").unwrap();
        assert_eq!(n, 1, "seul le vrai chemin de jeu est posé");
        assert!(ac.join(&good).is_file(), "le pilote arrive bien dans content/driver");
        assert!(
            !ac.join("Ferrari F2002 V1.4").exists(),
            "rien ne se déverse à la racine de l'install"
        );

        // Listé quand même, et dit pour ce qu'il est.
        let listed = list(&conn, &cfg, OwnerKind::Car, "ferrari_f2002");
        let wrapper = listed
            .iter()
            .find(|f| f.rel_path.starts_with("Ferrari F2002"))
            .expect("l'emballage reste visible dans l'onglet");
        assert!(wrapper.off_game_path, "signalé hors chemin de jeu");
        assert!(!wrapper.deployed, "et non posé");
        let driver = listed.iter().find(|f| f.rel_path.contains("driver_501")).unwrap();
        assert!(!driver.off_game_path, "le vrai chemin n'est pas signalé");
        assert!(driver.deployed);
    }

    #[test]
    fn shared_file_survives_until_the_last_mod_stops_claiming_it() {
        // §4.5.4 — compteur de références. Douze voitures RSS livrent le même
        // `extension/textures/common/rss/…` : en désactiver une ne doit pas
        // emporter le fichier dont les onze autres dépendent. Et l'arbitrage
        // par date décide de l'exemplaire posé, dans les deux sens : quand le
        // plus récent s'en va, on repasse à celui qui reste.
        let base = crate::testutil::temp_dir("sat-shared");
        let cfg = cfg_for(&base);
        let library = cfg.library_path.clone().unwrap();
        let ac = cfg.ac_install_path.clone().unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();

        let rel = Path::new("extension").join("textures").join("common").join("rss.dds");
        let old_car = dir(&library, OwnerKind::Car, "rss_old");
        let new_car = dir(&library, OwnerKind::Car, "rss_new");
        write(&old_car.join(&rel), b"ANCIENNE");
        write(&new_car.join(&rel), b"NOUVELLE");
        set_mtime(&old_car.join(&rel), 1_000_000);
        set_mtime(&new_car.join(&rel), 2_000_000);
        let target = ac.join(&rel);

        // La plus ancienne d'abord : elle pose le fichier.
        deploy(&conn, &cfg, OwnerKind::Car, "rss_old").unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"ANCIENNE");

        // La plus récente ensuite : son exemplaire gagne, sans dépendre de
        // l'ordre d'installation.
        deploy(&conn, &cfg, OwnerKind::Car, "rss_new").unwrap();
        assert_eq!(
            std::fs::read(&target).unwrap(),
            b"NOUVELLE",
            "la date de modification la plus récente gagne"
        );

        // La plus récente s'en va : le fichier reste, à l'exemplaire restant.
        undeploy(&conn, &cfg, "rss_new").unwrap();
        assert!(target.is_file(), "encore réclamé par rss_old : jamais retiré");
        assert_eq!(
            std::fs::read(&target).unwrap(),
            b"ANCIENNE",
            "on repasse à l'exemplaire du réclamant restant"
        );

        // Plus personne : le fichier part, et les dossiers créés avec lui.
        undeploy(&conn, &cfg, "rss_old").unwrap();
        assert!(!target.exists(), "plus aucun réclamant : retiré");
        assert!(!ac.join("extension").exists(), "dossiers créés pour l'occasion élagués");
    }

    #[test]
    fn a_track_deploys_its_extras_like_a_car() {
        // Bug réel (bahrain_international_circuit) : les ajouts au jeu d'un
        // circuit étaient posés puis **immédiatement retirés** par l'arbitrage.
        // `set_extra_links` écrit le type sous la forme `content_folder()`
        // ("tracks") et `best_claim` le relisait en le comparant à "Track" :
        // tout circuit était donc cherché dans `extras/cars/`, où il n'y a
        // rien — donc « aucun réclamant », donc suppression. Les voitures
        // passaient par hasard, "cars" tombant dans la même branche par défaut.
        let base = crate::testutil::temp_dir("sat-track");
        let cfg = cfg_for(&base);
        let library = cfg.library_path.clone().unwrap();
        let ac = cfg.ac_install_path.clone().unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();

        let sat = dir(&library, OwnerKind::Track, "bahrain_international_circuit");
        let ini = Path::new("extension")
            .join("config")
            .join("tracks")
            .join("loaded")
            .join("bahrain_international_circuit.ini");
        let vao = Path::new("extension")
            .join("vao-patches")
            .join("bahrain_international_circuit.vao-patch");
        write(&sat.join(&ini), b"[TRACK]");
        write(&sat.join(&vao), b"vao");

        let n = deploy(&conn, &cfg, OwnerKind::Track, "bahrain_international_circuit").unwrap();
        assert_eq!(n, 2, "les deux ajouts du circuit sont posés");
        assert!(ac.join(&ini).is_file(), "la config CSP survit à l'arbitrage");
        assert!(ac.join(&vao).is_file(), "le vao-patch survit à l'arbitrage");

        let listed = list(&conn, &cfg, OwnerKind::Track, "bahrain_international_circuit");
        assert!(listed.iter().all(|f| f.deployed), "et la fiche les dit posés");
    }

    #[test]
    fn an_unresolvable_claim_never_removes_what_is_deployed() {
        // Garde-fou : c'est l'absence de **réclamation en base** qui autorise
        // une suppression, jamais notre incapacité à retrouver l'exemplaire.
        // Sans lui, n'importe quel décrochage de la bibliothèque (dossier
        // déplacé, type illisible) vidait d'AC les fichiers d'un mod actif.
        let base = crate::testutil::temp_dir("sat-unresolvable");
        let cfg = cfg_for(&base);
        let library = cfg.library_path.clone().unwrap();
        let ac = cfg.ac_install_path.clone().unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();

        let rel = Path::new("extension").join("config").join("mine.ini");
        let sat = dir(&library, OwnerKind::Car, "car_a");
        write(&sat.join(&rel), b"MINE");
        deploy(&conn, &cfg, OwnerKind::Car, "car_a").unwrap();
        let target = ac.join(&rel);
        assert!(target.is_file(), "posé");

        // L'exemplaire disparaît de la bibliothèque, la réclamation reste.
        std::fs::remove_dir_all(&sat).unwrap();
        sync(&conn, &cfg, &target);
        assert!(target.is_file(), "encore réclamé : on laisse tourner ce qui est posé");
    }

    #[test]
    fn extras_wrapped_by_their_author_still_reach_the_game() {
        // Cas réel (Aspertsham) : le `.zip` porte un nom dupliqué et contient un
        // dossier du même nom, avec `content/` et `extension/` dedans. Le
        // circuit s'installait — `modscan` descend l'emballage — mais les huit
        // images de fond livrées à côté restaient en bibliothèque, refusées
        // comme chemin hors jeu. Deux moitiés du même import qui ne
        // s'accordaient pas sur la racine.
        //
        // Traversé à la lecture, pas seulement à l'import : les arbres rangés
        // avant le correctif gardent l'emballage figé, et les ajouts au jeu se
        // recalculent depuis le disque — rien d'autre ne les répare.
        let base = crate::testutil::temp_dir("sat-wrapped");
        let cfg = cfg_for(&base);
        let library = cfg.library_path.clone().unwrap();
        let ac = cfg.ac_install_path.clone().unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();

        let sat = dir(&library, OwnerKind::Track, "aspertsham");
        let wrapper = sat.join("Track - Aspertsham - Hargasser SchleifeTrack - Aspertsham - Hargasser Schleife");
        let rel = Path::new("extension").join("backgrounds").join("aspertsham_0.jpg");
        write(&wrapper.join(&rel), b"JPG");

        let n = deploy(&conn, &cfg, OwnerKind::Track, "aspertsham").unwrap();
        assert_eq!(n, 1, "l'image de fond est posée malgré l'emballage");
        assert!(ac.join(&rel).is_file(), "à sa vraie place dans le jeu");
        assert!(
            !ac.join("Track - Aspertsham - Hargasser SchleifeTrack - Aspertsham - Hargasser Schleife")
                .exists(),
            "et rien ne se déverse à la racine de l'install"
        );

        let listed = list(&conn, &cfg, OwnerKind::Track, "aspertsham");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].rel_path, "extension/backgrounds/aspertsham_0.jpg");
        assert!(!listed[0].off_game_path, "plus signalé hors chemin de jeu");
        assert!(listed[0].deployed);

        // Le retrait doit continuer de reconnaître son propre exemplaire : la
        // vérification d'identité part de la même racine traversée.
        undeploy(&conn, &cfg, "aspertsham").unwrap();
        assert!(!ac.join(&rel).exists(), "retiré comme n'importe quel ajout");
    }

    #[test]
    fn a_file_replaced_by_another_tool_is_never_removed() {
        // §4.5.3 : `extension/config/tracks/loaded/` est partagé avec le
        // téléchargeur de configs de CM. S'il a repris le chemin depuis notre
        // pose, le fichier qui est là ne nous appartient plus — le retirer à la
        // désactivation casserait l'install de l'utilisateur. La règle d'or n°5
        // vaut dans les deux sens : on ne supprime que ce qu'on a posé.
        let base = crate::testutil::temp_dir("sat-foreign");
        let cfg = cfg_for(&base);
        let library = cfg.library_path.clone().unwrap();
        let ac = cfg.ac_install_path.clone().unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();

        let rel = Path::new("extension")
            .join("config")
            .join("tracks")
            .join("loaded")
            .join("spa.ini");
        write(&dir(&library, OwnerKind::Track, "spa").join(&rel), b"MOD-CONFIG");
        deploy(&conn, &cfg, OwnerKind::Track, "spa").unwrap();
        let target = ac.join(&rel);
        assert_eq!(std::fs::read(&target).unwrap(), b"MOD-CONFIG", "posé");

        // CM resynchronise : le chemin est repris, notre lien est rompu.
        std::fs::remove_file(&target).unwrap();
        std::fs::write(&target, b"CM-CONFIG").unwrap();

        undeploy(&conn, &cfg, "spa").unwrap();
        assert_eq!(
            std::fs::read(&target).unwrap(),
            b"CM-CONFIG",
            "la config de CM survit à la désactivation du mod"
        );
    }

    #[test]
    fn list_flags_cm_managed_paths_and_foreign_occupants() {
        // §4.5.5 : deux informations qui manquaient et rendaient la situation
        // illisible — que le chemin est en zone auto-gérée par CM, et qu'un
        // fichier étranger l'occupe déjà. Les configs du dépôt CSP étant
        // remises à jour en continu, elles gagnent presque toujours
        // l'arbitrage par date contre une archive figée : sans ces drapeaux,
        // le fichier du mod n'arrivait jamais et rien ne disait pourquoi.
        let base = crate::testutil::temp_dir("sat-managed");
        let cfg = cfg_for(&base);
        let library = cfg.library_path.clone().unwrap();
        let ac = cfg.ac_install_path.clone().unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();

        let managed = Path::new("extension")
            .join("config")
            .join("tracks")
            .join("loaded")
            .join("spa.ini");
        let own = Path::new("extension").join("weather").join("spa_fog.ini");
        let sat = dir(&library, OwnerKind::Track, "spa");
        write(&sat.join(&managed), b"MOD");
        write(&sat.join(&own), b"MOD");
        set_mtime(&sat.join(&managed), 1_000_000);

        // CM est déjà passé, avec un exemplaire plus récent.
        write(&ac.join(&managed), b"CM");
        set_mtime(&ac.join(&managed), 2_000_000);

        let n = deploy(&conn, &cfg, OwnerKind::Track, "spa").unwrap();
        assert_eq!(n, 1, "seul le chemin libre est posé");
        assert_eq!(std::fs::read(ac.join(&managed)).unwrap(), b"CM", "CM garde la main");

        let listed = list(&conn, &cfg, OwnerKind::Track, "spa");
        let cfg_file = listed
            .iter()
            .find(|f| f.rel_path.contains("loaded/spa.ini"))
            .expect("la config reste listée");
        assert!(cfg_file.externally_managed, "signalée en zone auto-gérée");
        assert!(cfg_file.held_by_foreign_file, "et occupée par un fichier étranger");
        assert!(!cfg_file.deployed, "donc pas posée");
        assert!(cfg_file.provided_by.is_none(), "aucun autre mod en cause");

        let weather = listed.iter().find(|f| f.rel_path.contains("spa_fog")).unwrap();
        assert!(!weather.externally_managed, "hors zone auto-gérée");
        assert!(!weather.held_by_foreign_file);
        assert!(weather.deployed);
    }

    #[test]
    fn list_reports_where_each_file_lands_and_who_provides_it() {
        // §4.5.5, onglet « Ajouts au jeu » : la fiche doit dire *où* le mod
        // pose ses fichiers dans le jeu, et lesquels sont en fait fournis par
        // un autre mod (fichier partagé). Sans ça, un mod peut poser 69
        // fichiers hors de son dossier sans que rien ne le montre.
        let base = crate::testutil::temp_dir("sat-list");
        let cfg = cfg_for(&base);
        let library = cfg.library_path.clone().unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();

        let shared = Path::new("system").join("shaders").join("shared.fxo");
        let own = Path::new("extension").join("config").join("mine.ini");
        write(&dir(&library, OwnerKind::Car, "car_a").join(&shared), b"AAAA");
        write(&dir(&library, OwnerKind::Car, "car_a").join(&own), b"MINE");
        write(&dir(&library, OwnerKind::Car, "car_b").join(&shared), b"BBBB");
        // car_b, plus récent, gagnera le fichier partagé.
        set_mtime(&dir(&library, OwnerKind::Car, "car_a").join(&shared), 1_000_000);
        set_mtime(&dir(&library, OwnerKind::Car, "car_b").join(&shared), 2_000_000);

        deploy(&conn, &cfg, OwnerKind::Car, "car_a").unwrap();
        deploy(&conn, &cfg, OwnerKind::Car, "car_b").unwrap();

        let listed = list(&conn, &cfg, OwnerKind::Car, "car_a");
        assert_eq!(listed.len(), 2, "les deux fichiers de car_a sont listés");

        let mine = listed
            .iter()
            .find(|f| f.rel_path == "extension/config/mine.ini")
            .unwrap();
        assert!(mine.deployed, "fichier propre : posé par ce mod");
        assert!(mine.provided_by.is_none());
        assert_eq!(mine.size_bytes, 4);

        let sh = listed
            .iter()
            .find(|f| f.rel_path == "system/shaders/shared.fxo")
            .unwrap();
        assert!(!sh.deployed, "fichier partagé perdu à l'arbitrage");
        assert_eq!(
            sh.provided_by.as_deref(),
            Some("car_b"),
            "la fiche nomme le mod qui fournit l'exemplaire posé"
        );

        // Chemins relatifs à AC, séparateurs normalisés pour l'affichage.
        assert!(listed.iter().all(|f| !f.rel_path.contains('\\')));
    }

    #[test]
    fn equal_dates_are_settled_by_the_last_installed_mod() {
        // Archives repackées par un tiers : toutes les dates sont identiques,
        // le critère principal ne départage plus. Repli sur le dernier mod
        // installé — ici le dernier à avoir réclamé le fichier.
        let base = crate::testutil::temp_dir("sat-tie");
        let cfg = cfg_for(&base);
        let library = cfg.library_path.clone().unwrap();
        let ac = cfg.ac_install_path.clone().unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();

        let rel = Path::new("system").join("shaders").join("shared.fxo");
        for (id, body) in [("car_a", b"AAAA"), ("car_b", b"BBBB")] {
            let p = dir(&library, OwnerKind::Car, id).join(&rel);
            write(&p, body);
            set_mtime(&p, 1_500_000);
        }

        deploy(&conn, &cfg, OwnerKind::Car, "car_a").unwrap();
        assert_eq!(std::fs::read(ac.join(&rel)).unwrap(), b"AAAA");
        deploy(&conn, &cfg, OwnerKind::Car, "car_b").unwrap();
        assert_eq!(
            std::fs::read(ac.join(&rel)).unwrap(),
            b"BBBB",
            "à égalité de date, le dernier installé gagne"
        );
    }

    #[test]
    fn extras_deployed_on_activate_and_fully_removed_on_deactivate() {
        // §4.5.3 : l'ajout vit et meurt avec son mod. C'est ce que le
        // passage par « autre mod » ne donnait pas — les fichiers d'une voiture
        // désinstallée restaient dans AC, rattachés à une entrée anonyme.
        let base = crate::testutil::temp_dir("sat");
        let cfg = cfg_for(&base);
        let library = cfg.library_path.clone().unwrap();
        let ac = cfg.ac_install_path.clone().unwrap();
        std::fs::create_dir_all(ac.join("content")).unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();

        let sat = dir(&library, OwnerKind::Car, "rss_car");
        write(&sat.join("extension").join("config").join("cars").join("x.ini"), b"cfg");
        write(&sat.join("content").join("driver").join("pro.kn5"), b"model");

        let n = deploy(&conn, &cfg, OwnerKind::Car, "rss_car").unwrap();
        assert_eq!(n, 2, "les deux ajouts sont posés");
        assert!(ac.join("extension").join("config").join("cars").join("x.ini").is_file());
        assert!(ac.join("content").join("driver").join("pro.kn5").is_file());

        undeploy(&conn, &cfg, "rss_car").unwrap();
        assert!(!ac.join("extension").join("config").join("cars").join("x.ini").exists());
        assert!(!ac.join("content").join("driver").join("pro.kn5").exists());
        assert!(
            !ac.join("extension").exists(),
            "les dossiers créés pour l'occasion sont élagués"
        );
        assert!(
            ac.join("content").is_dir(),
            "un dossier AC préexistant n'est jamais emporté"
        );
        assert!(
            sat.join("content").join("driver").join("pro.kn5").is_file(),
            "la bibliothèque garde l'ajout : réactivable sans réimport"
        );
    }

    #[test]
    fn a_newer_mod_file_replaces_a_game_file_and_the_original_comes_back() {
        // §4.5.4 : la règle d'or n°5 n'interdit pas de toucher un fichier du jeu,
        // elle exige qu'il soit sauvegardé et restauré. Avant, la pose sautait
        // le fichier en silence et le mod s'installait à moitié — c'est ce qui
        // cassait les mods qui remplacent vraiment (HUD façon CMRT, shaders).
        let base = crate::testutil::temp_dir("sat-replace");
        let cfg = cfg_for(&base);
        let library = cfg.library_path.clone().unwrap();
        let ac = cfg.ac_install_path.clone().unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();

        let kunos = ac.join("system").join("shaders").join("stock.fxo");
        write(&kunos, b"KUNOS");
        set_mtime(&kunos, 1_000_000);
        let sat = dir(&library, OwnerKind::Car, "rss_car");
        write(&sat.join("system").join("shaders").join("stock.fxo"), b"MOD");
        write(&sat.join("system").join("shaders").join("new.fxo"), b"MOD");
        set_mtime(&sat.join("system").join("shaders").join("stock.fxo"), 2_000_000);

        let n = deploy(&conn, &cfg, OwnerKind::Car, "rss_car").unwrap();
        assert_eq!(n, 2, "le nouveau fichier ET le remplacement sont posés");
        assert_eq!(std::fs::read(&kunos).unwrap(), b"MOD", "le mod prend la place");
        assert!(
            crate::gamebackup::is_replaced(&conn, &kunos),
            "et le remplacement est tracé, pas silencieux"
        );

        undeploy(&conn, &cfg, "rss_car").unwrap();
        assert_eq!(
            std::fs::read(&kunos).unwrap(),
            b"KUNOS",
            "l'original du jeu revient à la désactivation"
        );
        assert!(!crate::gamebackup::is_replaced(&conn, &kunos));
        assert!(
            !ac.join("system").join("shaders").join("new.fxo").exists(),
            "l'ajout pur, lui, part"
        );
    }

    #[test]
    fn an_older_mod_file_never_displaces_what_already_runs() {
        // Même arbitrage par date que pour les fichiers partagés : un
        // exemplaire plus ancien (ou de même date) ne prend pas la place de ce
        // qui tourne déjà. Sans ça, le dernier mod installé écraserait une font
        // mise à jour par un autre outil, ce que rien ne justifie.
        let base = crate::testutil::temp_dir("sat-older");
        let cfg = cfg_for(&base);
        let library = cfg.library_path.clone().unwrap();
        let ac = cfg.ac_install_path.clone().unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();

        let existing = ac.join("content").join("fonts").join("shared.txt");
        write(&existing, b"RECENT");
        set_mtime(&existing, 2_000_000);
        let sat = dir(&library, OwnerKind::Car, "old_car");
        write(&sat.join("content").join("fonts").join("shared.txt"), b"ANCIEN");
        set_mtime(&sat.join("content").join("fonts").join("shared.txt"), 1_000_000);

        let n = deploy(&conn, &cfg, OwnerKind::Car, "old_car").unwrap();
        assert_eq!(n, 0, "rien posé : l'exemplaire du mod est plus ancien");
        assert_eq!(std::fs::read(&existing).unwrap(), b"RECENT", "intact");
        assert!(
            !crate::gamebackup::is_replaced(&conn, &existing),
            "aucune sauvegarde inutile"
        );

        undeploy(&conn, &cfg, "old_car").unwrap();
        assert_eq!(
            std::fs::read(&existing).unwrap(),
            b"RECENT",
            "la désactivation n'emporte pas ce qu'elle n'a pas posé"
        );
    }

    #[test]
    fn store_merges_into_the_existing_extras_tree() {
        // Une mise à jour du mod remplace ses propres fichiers sans effacer les
        // autres : l'arbre est au niveau du mod, partagé par les versions.
        let base = crate::testutil::temp_dir("sat-store");
        let sat = base.join("sat");
        write(&sat.join("system").join("old.fxo"), b"old");

        let src = base.join("src").join("extension");
        write(&src.join("config").join("a.ini"), b"a");
        store(&sat, Path::new("extension"), &src, true).unwrap();

        assert!(sat.join("extension").join("config").join("a.ini").is_file());
        assert!(sat.join("system").join("old.fxo").is_file(), "l'existant est conservé");
        assert!(src.join("config").join("a.ini").is_file(), "copie : source intacte");
    }
}
