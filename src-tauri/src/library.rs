//! Vue bibliothèque (§6) : assemble les lignes overlay avec la vignette de
//! preview et l'état actif/inactif pour la galerie et le tableau.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde::Serialize;

use crate::cm_stats::{self, CmUsage};
use crate::config::AppConfig;
use crate::inspect;
use crate::modscan::ModKind;
use crate::overlay::{self, HistoryRow, ModRow, VersionRow};
use crate::uijson::{self, NativeSpecs};

#[derive(Debug, Clone, Serialize)]
pub struct ModCard {
    #[serde(flatten)]
    pub base: ModRow,
    /// Chemin absolu d'une preview (à passer à convertFileSrc côté front).
    /// Voiture : skin ; circuit : photo illustratrice (fond).
    pub preview: Option<String>,
    /// Tracé du circuit à superposer à la photo (circuits uniquement, §6.1).
    pub outline: Option<String>,
    /// Junction présente dans content/ (détection fine = L3).
    pub active: bool,
    /// Distance parcourue (km) d'après CM, si connue (§6).
    pub distance_km: Option<f64>,
    /// « Déjà essayé » : lancé par l'app OU km CM > 0 (§6).
    pub tried: bool,
    /// Poids natif (voitures), lu à la volée dans ui_car.json — colonne §6.2.
    pub weight: Option<String>,
    /// Puissance native (voitures), lue dans le **même** `NativeSpecs` que le
    /// poids — la fiche est déjà ouverte et analysée une fois par carte, donc
    /// ce champ ne coûte aucune lecture de plus. Les deux ensemble portent le
    /// rapport poids/puissance du filtre `Performance` (CIBLE§3.4), qui a besoin
    /// d'être calculable **par carte, côté front**, sans aller-retour backend
    /// à chaque cran du curseur de tolérance.
    pub bhp: Option<String>,
    /// Effective description: the user's own text (§5bis.3) when there is one,
    /// otherwise the `ui_*.json` one. Same arbitration as the detail view, but
    /// carried by the card so the library's description filter (§6.1) stays
    /// purely client-side, with no backend round-trip per keystroke.
    pub description: Option<String>,
    /// Badge/logo de la marque (`ui/badge.png`, voitures), à la place des initiales.
    pub badge: Option<String>,
    /// Mod cassé (fichiers de la version active manquants/invalides, §6.4) —
    /// même détection que l'écran Maintenance (§10), remontée ici comme
    /// signalement visuel sur la carte bibliothèque.
    pub broken: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModDetail {
    #[serde(flatten)]
    pub card: ModCard,
    pub versions: Vec<VersionRow>,
    pub history: Vec<HistoryRow>,
    /// Fiche technique native (voitures uniquement), lue de ui_car.json. Only
    /// its description is still read from here; the sheet is `tech`.
    pub specs: Option<NativeSpecs>,
    /// The tech sheet as shown (FICHE§6.3), cars only — from the base, never
    /// from the files.
    pub tech: Option<crate::techsheet::TechSheet>,
    /// Détail circuit (description + layouts illustrés), circuits uniquement.
    pub track: Option<uijson::TrackDetail>,
    /// Nom du DLC Kunos d'origine (contenu de base uniquement, §11) —
    /// `None` pour le jeu de base ou un mod importé (le bloc Source/Origine
    /// y affiche alors l'archive ou « Jeu de base »).
    pub stock_pack: Option<String>,
}

fn is_active(cfg: &AppConfig, m: &ModRow) -> bool {
    // Contenu de base Kunos : toujours un vrai dossier (jamais de déploiement),
    // chargé par AC en permanence — donc toujours « actif ».
    if m.is_stock {
        return true;
    }
    if cfg.ac_install_path.is_none() {
        return false;
    }
    // « Actif » = déploiement géré par l'app présent (symlink hérité ou
    // hardlinks, §2) — pas un vrai dossier installé hors app.
    crate::activation::is_mod_active(cfg, ModKind::from_column(&m.kind), &m.id_interne)
}

/// The card image of a mod in the showcase: the one frozen when its files
/// went (ESPACE§3.3). The skin previews it was chosen from are gone, and a
/// regenerated grid thumbnail could no longer be found without the `.kn5` it
/// is keyed on. `own` is the mod's own folder (`entity_dir`).
fn showcase_image(own: Option<&Path>) -> Option<String> {
    own.and_then(crate::skeleton::image_of)
        .map(|p| p.to_string_lossy().into_owned())
}

/// The image a mod's card shows, as the backend picks it — also what the
/// showcase freezes when the screen did not name one (ESPACE§3.3).
pub(crate) fn preview_for(conn: &Connection, cfg: &AppConfig, m: &ModRow) -> Option<String> {
    preview_in(m, &entity_dirs(conn, cfg, m))
}

/// `preview_for` on a composition stack already resolved (`entity_dirs`).
fn preview_in(m: &ModRow, stack: &[PathBuf]) -> Option<String> {
    // Version active en bibliothèque, sinon content/ (contenu de base Kunos) —
    // c'est ce qui fait apparaître la vignette du stock, comme l'écran de session.
    // Couches actives d'abord (§4.3) : ce que l'app montre doit être ce que le
    // jeu voit, sinon une couche qui remplace un `preview.png` reste invisible.
    match ModKind::from_column(&m.kind) {
        ModKind::Car => layered(stack, |d| inspect::preview_path(ModKind::Car, d)),
        // Circuit : la photo illustratrice (fond), repli sur le tracé si absente.
        // Le repli se fait **pile épuisée**, pas couche par couche : une couche
        // qui n'apporte qu'un tracé ne doit pas priver la carte de la photo de
        // la base.
        ModKind::Track => layered(stack, inspect::track_preview).or_else(|| layered(stack, inspect::track_outline)),
    }
}

/// Tracé d'un circuit à superposer à la photo (None pour une voiture).
fn outline_in(m: &ModRow, stack: &[PathBuf]) -> Option<String> {
    if m.kind != "Track" {
        return None;
    }
    layered(stack, inspect::track_outline)
}

/// Native spec sheet of a car (weight §6.2, description §6.1), read on the fly
/// from ui_car.json - "native" data, never harmonized by the rule engine
/// (§6). One read for both fields: two separate helpers would reopen and
/// reparse the very same file for every card of the list.
fn car_specs_in(m: &ModRow, stack: &[PathBuf]) -> Option<NativeSpecs> {
    if m.kind != "Car" {
        return None;
    }
    layered(stack, uijson::read_car_specs)
}

/// Effective description of a card: the user's own text wins over the file
/// (§5bis.3), exactly as on the detail view (`detail`, below). `native` is the
/// sheet already read by `car_specs_in` - nothing to reread for a car; a track
/// only opens its `ui_track.json` when no user text spares it, and through the
/// light reader (not the image scan of `read_track_detail`).
fn description_in(m: &ModRow, stack: &[PathBuf], native: Option<&NativeSpecs>) -> Option<String> {
    if let Some(user) = &m.description_user {
        return Some(user.clone());
    }
    match ModKind::from_column(&m.kind) {
        ModKind::Car => native.and_then(|s| s.description.clone()),
        ModKind::Track => layered(stack, uijson::read_track_description),
    }
}

/// Badge/logo de la marque (voitures uniquement), lu à la volée dans `ui/badge.png`.
fn badge_in(m: &ModRow, stack: &[PathBuf]) -> Option<String> {
    if m.kind != "Car" {
        return None;
    }
    layered(stack, inspect::brand_badge)
}

/// `(car id, brand, badge path)` of every car that has both - what the brand
/// logos are elected from (`logos::elect`).
pub fn car_badges(conn: &Connection, cfg: &AppConfig) -> rusqlite::Result<Vec<(String, String, String)>> {
    Ok(badges_from_disk(branded_cars(conn, cfg)?))
}

/// `car_badges`, the base lock held only while the base is read - same reason
/// as `list_cards_shared`: the badges are looked for on the disk, one car at a
/// time, and the brand logos are asked for at every start.
pub fn car_badges_shared(db: &overlay::Db, cfg: &AppConfig) -> Result<Vec<(String, String, String)>, String> {
    let cars = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        branded_cars(&conn, cfg).map_err(|e| e.to_string())?
    };
    Ok(badges_from_disk(cars))
}

/// Every car with a brand, and its composition stack - the base side of
/// `car_badges`.
fn branded_cars(conn: &Connection, cfg: &AppConfig) -> rusqlite::Result<Vec<(ModRow, String, Vec<PathBuf>)>> {
    Ok(overlay::list_mods(conn)?
        .into_iter()
        .filter(|m| m.kind == "Car")
        .filter_map(|m| {
            let brand = m.brand.clone().filter(|b| !b.trim().is_empty())?;
            let stack = entity_dirs(conn, cfg, &m);
            Some((m, brand, stack))
        })
        .collect())
}

fn badges_from_disk(cars: Vec<(ModRow, String, Vec<PathBuf>)>) -> Vec<(String, String, String)> {
    cars.into_iter()
        .filter_map(|(m, brand, stack)| Some((m.id_interne.clone(), brand, badge_in(&m, &stack)?)))
        .collect()
}

/// What a card needs from the base: where its files are, and whether it is
/// broken. Everything else on a card comes from the files (`card_from_disk`),
/// read with no connection at hand - which is what lets `list_cards_shared`
/// release the lock before the slow part.
struct CardBase {
    m: ModRow,
    /// The composition stack (§4.3), resolved once for the whole card: each
    /// reader used to resolve it again, up to six base queries per card.
    stack: Vec<PathBuf>,
    /// The mod's own folder alone, the last of `stack` when there is one.
    own: Option<PathBuf>,
    /// Read on the base side: `broken_reason` asks the base for the version's
    /// path, and its few file checks cost little.
    broken: bool,
}

fn card_base(conn: &Connection, cfg: &AppConfig, m: ModRow) -> CardBase {
    let own = entity_dir(conn, cfg, &m);
    let mut stack = layer_dirs(conn, cfg, &m);
    stack.extend(own.clone());
    let broken = crate::maintenance::broken_reason(conn, cfg, &m).is_some();
    CardBase { m, stack, own, broken }
}

fn card_from_disk(cfg: &AppConfig, base: CardBase) -> ModCard {
    let CardBase { m, stack, own, broken } = base;
    let preview = if m.showcase {
        showcase_image(own.as_deref())
    } else {
        preview_in(&m, &stack)
    };
    let outline = outline_in(&m, &stack);
    let active = is_active(cfg, &m);
    let native = car_specs_in(&m, &stack);
    let weight = native.as_ref().and_then(|s| s.weight.clone());
    let bhp = native.as_ref().and_then(|s| s.bhp.clone());
    let description = description_in(&m, &stack, native.as_ref());
    let badge = badge_in(&m, &stack);
    ModCard {
        base: m,
        preview,
        outline,
        active,
        distance_km: None,
        tried: false,
        weight,
        bhp,
        description,
        badge,
        broken,
    }
}

fn to_card(conn: &Connection, cfg: &AppConfig, m: ModRow) -> ModCard {
    card_from_disk(cfg, card_base(conn, cfg, m))
}

/// Renseigne la distance CM et le marqueur « essayé » (§6) sur une carte.
fn fill_usage(card: &mut ModCard, cm: &CmUsage, launched: &HashSet<String>) {
    let id = &card.base.id_interne;
    card.distance_km = cm.km(id);
    card.tried = launched.contains(id) || card.distance_km.is_some_and(|k| k > 0.0);
}

pub fn list_cards(conn: &Connection, cfg: &AppConfig) -> rusqlite::Result<Vec<ModCard>> {
    let (bases, launched) = card_bases(conn, cfg)?;
    Ok(cards_from_disk(cfg, bases, &launched))
}

/// The same list as `list_cards`, the base lock held **only while the base is
/// read** - not while the files are.
///
/// A card reads several files besides the base (preview, `ui_car.json`, badge),
/// and the whole list used to hold the lock for all of it: 0.37 s with a warm
/// disk cache and 2.4 s on a first start (385 mods). Any command needing the
/// base meanwhile - the session column asks for its car's detail at startup -
/// waited all that time, on the thread that drives the window. The base part
/// is a few tens of milliseconds, read in one go, so the list is one snapshot.
pub fn list_cards_shared(db: &overlay::Db, cfg: &AppConfig) -> Result<Vec<ModCard>, String> {
    let (bases, launched) = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        card_bases(&conn, cfg).map_err(|e| e.to_string())?
    };
    Ok(cards_from_disk(cfg, bases, &launched))
}

/// Every mod's `CardBase`, and the ids launched from Pit Box (§6).
fn card_bases(conn: &Connection, cfg: &AppConfig) -> rusqlite::Result<(Vec<CardBase>, HashSet<String>)> {
    let launched = overlay::launched_ids(conn)?;
    let bases = overlay::list_mods(conn)?
        .into_iter()
        .map(|m| card_base(conn, cfg, m))
        .collect();
    Ok((bases, launched))
}

fn cards_from_disk(cfg: &AppConfig, bases: Vec<CardBase>, launched: &HashSet<String>) -> Vec<ModCard> {
    let cm = cm_stats::read();
    bases
        .into_iter()
        .map(|b| {
            let mut card = card_from_disk(cfg, b);
            fill_usage(&mut card, &cm, launched);
            card
        })
        .collect()
}

/// Skin d'une voiture avec sa miniature (SESSION§1).
#[derive(Debug, Clone, Serialize)]
pub struct SkinItem {
    pub id: String,
    pub name: String,
    pub preview: Option<String>,
    /// `livery.png` (couleurs/motif du skin seul, sans la voiture) — convention
    /// AC reprise par CM pour son propre sélecteur de skin. Bien plus lisible
    /// que `preview` (photo de la voiture entière) une fois écrasé à 20px
    /// dans un menu déroulant (SESSION§1) ; utilisé aussi en vignette dans la
    /// grille de skins de la fiche détail (§6.3).
    pub livery: Option<String>,
    /// Pilote, numéro et pays **déclarés par la livrée**, tels que le jeu les
    /// emploie pour l'IA qui la porte (§4.2).
    ///
    /// C'est ce que vaut une cellule `Auto` du plateau : ne pas les lire
    /// obligeait à afficher un `Auto` creux là où le jeu, lui, sait très bien
    /// quel nom il va mettre. Mesuré sur 400 livrées réelles : `skinname` et
    /// `number` sont présents sur les 400, `country` sur 396 et `drivername`
    /// sur 393 — la donnée est là, il suffisait de la prendre.
    pub driver: Option<String>,
    pub number: Option<String>,
    pub country: Option<String>,
}

/// Ce qu'une livrée déclare dans son `ui_skin.json`.
#[derive(Debug, Clone, Default)]
pub(crate) struct SkinInfo {
    pub name: Option<String>,
    pub driver: Option<String>,
    pub number: Option<String>,
    pub country: Option<String>,
}

/// Une valeur de `ui_skin.json`, vide traitée comme absente. Le numéro peut
/// être écrit en nombre **ou** en chaîne selon l'auteur : les deux se lisent.
fn skin_field(v: &serde_json::Value, key: &str) -> Option<String> {
    match v.get(key)? {
        serde_json::Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

pub(crate) fn read_skin_info(skin_dir: &Path) -> SkinInfo {
    let Some(v) = crate::uijson::read_ui_json(&skin_dir.join("ui_skin.json")) else {
        return SkinInfo::default();
    };
    SkinInfo {
        name: skin_field(&v, "skinname").or_else(|| skin_field(&v, "name")),
        driver: skin_field(&v, "drivername"),
        number: skin_field(&v, "number"),
        country: skin_field(&v, "country"),
    }
}

/// Nom lisible d'une livrée.
///
/// `pub(crate)` parce que l'inventaire (§4) doit afficher **le même** nom que
/// le sélecteur de la fiche : un `chp_unit_118` d'un côté et un « Unit 118 » de
/// l'autre, pour la même livrée, est une divergence qu'aucun typage ne signale.
pub(crate) fn read_skin_name(skin_dir: &Path) -> Option<String> {
    read_skin_info(skin_dir).name
}

/// The photo of a livery: `preview.jpg`, else `preview.png`, as an absolute
/// path for `convertFileSrc`.
pub(crate) fn skin_preview(skin_dir: &Path) -> Option<String> {
    ["preview.jpg", "preview.png"]
        .iter()
        .map(|n| skin_dir.join(n))
        .find(|p| p.is_file())
        .map(|p| p.to_string_lossy().into_owned())
}

/// Lit les skins d'un dossier `skins/` donné (sous-dossiers + miniature + nom).
fn read_skins_dir(skins_dir: &Path) -> Vec<SkinItem> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(skins_dir) {
        for e in entries.flatten() {
            let p = e.path();
            if !p.is_dir() {
                continue;
            }
            let id = e.file_name().to_string_lossy().into_owned();
            let preview = skin_preview(&p);
            let livery = p.join("livery.png");
            let livery = livery.is_file().then(|| livery.to_string_lossy().into_owned());
            let info = read_skin_info(&p);
            let name = info.name.clone().unwrap_or_else(|| id.clone());
            out.push(SkinItem {
                id,
                name,
                preview,
                livery,
                driver: info.driver,
                number: info.number,
                country: info.country,
            });
        }
    }
    out.sort_by_key(|a| a.id.to_lowercase());
    out
}

/// Skins d'une voiture pour la fiche détail (§6.3). Pour un **mod géré**, on lit
/// la version active en bibliothèque (disponible même inactif). Pour une
/// **voiture de base Kunos** (`is_stock`, sans version bibliothèque), on lit
/// directement `content/cars/<id>/skins` — là où vivent ses skins (y compris
/// ceux projetés par junction, §8.3).
pub fn list_mod_skins(conn: &Connection, cfg: &AppConfig, mod_id: &str) -> Vec<SkinItem> {
    let Some(m) = overlay::get_mod(conn, mod_id).ok().flatten() else {
        return Vec::new();
    };
    if m.showcase {
        // The skins folder went with the files (ESPACE§3.1): their names are
        // in the base, and the fiche still lists them (ESPACE§6) — with no
        // image, and nothing a skin would declare for the game.
        let names = m
            .active_version_id
            .as_ref()
            .and_then(|vid| overlay::get_version(conn, vid).ok().flatten())
            .map(|v| v.skins)
            .unwrap_or_default();
        return names
            .into_iter()
            .map(|id| SkinItem {
                name: id.clone(),
                id,
                preview: None,
                livery: None,
                driver: None,
                number: None,
                country: None,
            })
            .collect();
    }
    if !m.is_stock {
        if let Some(lib) = m
            .active_version_id
            .as_ref()
            .and_then(|vid| overlay::get_version_path(conn, vid).ok().flatten())
            .and_then(|stored| crate::libpath::resolve(cfg.library_path.as_deref(), &stored))
        {
            return read_skins_dir(&lib.join("skins"));
        }
    }
    // Voiture de base (ou mod sans version) : skins installés dans content/.
    if let Some(ac) = &cfg.ac_install_path {
        return read_skins_dir(&ac.join("content").join("cars").join(mod_id).join("skins"));
    }
    Vec::new()
}

/// Dossiers météo installés (`content/weather/*`).
pub fn list_weather(cfg: &AppConfig) -> Vec<String> {
    let Some(ac) = &cfg.ac_install_path else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(ac.join("content").join("weather")) {
        for e in entries.flatten() {
            if e.path().is_dir() {
                if let Some(n) = e.file_name().to_str() {
                    out.push(n.to_string());
                }
            }
        }
    }
    out.sort();
    out
}

/// Dossier de l'entité contenant `ui/`, tel que le jeu le voit :
/// - version active en bibliothèque pour un mod géré (intacte, jamais composée) ;
/// - sinon `content/<type>s/<id>` (contenu de base Kunos, ou mod géré/contenu
///   de base **composé** avec ses couches actives, §4.3/§4.4 : depuis la
///   bascule hardlinks, `content/<id>` EST directement le résultat composé —
///   plus de dossier `<lib>/composed/<type>s/<id>` intermédiaire à consulter,
///   contrairement à l'ancien mécanisme par junction).
fn entity_dir(conn: &Connection, cfg: &AppConfig, m: &ModRow) -> Option<PathBuf> {
    if !m.is_stock {
        if let Some(vid) = &m.active_version_id {
            if let Ok(Some(p)) = overlay::get_version_path(conn, vid) {
                if let Some(resolved) = crate::libpath::resolve(cfg.library_path.as_deref(), &p) {
                    return Some(resolved);
                }
            }
        }
    }
    cfg.ac_install_path.as_ref().map(|ac| {
        ac.join("content")
            .join(ModKind::from_column(&m.kind).content_folder())
            .join(&m.id_interne)
    })
}

/// Pile de composition d'un mod (§4.3), **du plus prioritaire au moins** : ses
/// couches actives par priorité décroissante, puis la version de base.
///
/// C'est la même superposition que celle que `deploy::compose_tree` pose dans
/// `content/`. Elle est refaite ici parce que le résultat composé n'existe sur
/// le disque que tant que le mod est **actif** : la fiche d'un mod désactivé
/// doit dire la même chose que celle du même mod activé, et la lire dans
/// `content/` ne le permettrait pas.
///
/// Une couche **inactive** est exclue, exactement comme à la composition : la
/// désactiver doit faire réapparaître la base, à l'écran comme dans le jeu.
fn entity_dirs(conn: &Connection, cfg: &AppConfig, m: &ModRow) -> Vec<PathBuf> {
    let mut dirs = layer_dirs(conn, cfg, m);
    dirs.extend(entity_dir(conn, cfg, m));
    dirs
}

/// The active layers' folders of a mod, the one that wins first - the top of
/// its composition stack, without the mod's own folder.
///
/// A layer in the showcase keeps its host's skeleton (ESPACE§5.4). It is read
/// with a host in the showcase, whose own skeleton is read the same way: the
/// layout a layer brings stays on the fiche. With a host that has its files it
/// is not — what it would add cannot be raced.
fn layer_dirs(conn: &Connection, cfg: &AppConfig, m: &ModRow) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = overlay::list_layers(conn, &m.id_interne, ModKind::from_column(&m.kind).into())
        .unwrap_or_default()
        .into_iter()
        .filter(|l| l.is_active && (m.showcase || !l.is_skeleton()))
        .filter_map(|l| crate::libpath::resolve(cfg.library_path.as_deref(), &l.library_path))
        .collect();
    // `list_layers` trie par priorité **croissante** et c'est la plus haute qui
    // gagne (§4.3) : la pile de lecture est donc l'inverse de la liste.
    dirs.reverse();
    dirs
}

/// Layouts of a track with their names and images, read on the same
/// composition stack as the detail sheet — so the Online page shows the
/// picture the game will show, active layers included.
pub(crate) fn track_layouts_detail(conn: &Connection, cfg: &AppConfig, m: &ModRow) -> uijson::TrackDetail {
    uijson::read_track_detail(&entity_dirs(conn, cfg, m))
}

/// Applique un lecteur à la pile de composition et retient la première réponse.
///
/// Résolution **fichier par fichier**, jamais dossier par dossier : une couche
/// qui n'apporte qu'un `preview.png` ne doit pas masquer le `ui_car.json` de la
/// base. Chaque appelant passe donc le lecteur du seul fichier qui l'intéresse.
fn layered<T>(dirs: &[PathBuf], read: impl Fn(&Path) -> Option<T>) -> Option<T> {
    dirs.iter().find_map(|d| read(d))
}

/// Dossier réel d'un mod (voiture/circuit, géré ou contenu de base), pour
/// « Ouvrir le dossier » dans l'explorateur — même résolution que la fiche
/// détail (`entity_dir`), exposée publiquement pour la commande dédiée.
pub fn folder_path(conn: &Connection, cfg: &AppConfig, id: &str) -> Result<PathBuf, String> {
    let m = overlay::get_mod(conn, id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("mod introuvable : {id}"))?;
    entity_dir(conn, cfg, &m).ok_or_else(|| format!("dossier introuvable pour « {id} »"))
}

/// Fonctionnalités CSP effectivement détectées pour un mod (§6) : config
/// propre au mod + config CSP "chargée" séparément par CSP (hors du mod, cf.
/// `inspect::csp_features_loaded` — c'est notamment ce qui manquait pour le
/// contenu de base). Calculé à la demande (pas mis en cache) : sert à griser
/// les réglages météo/saison non supportés sur l'écran de session.
pub fn mod_csp_features(conn: &Connection, cfg: &AppConfig, id: &str) -> Result<Vec<String>, String> {
    let m = overlay::get_mod(conn, id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("mod introuvable : {id}"))?;
    let kind = ModKind::from_column(&m.kind);
    // Union sur la pile de composition, pas premier arrivé : une couche AJOUTE
    // ses extensions CSP à celles de la base (une config météo posée sur un
    // circuit qui n'en avait pas), elle ne les remplace pas.
    let mut feats: Vec<String> = entity_dirs(conn, cfg, &m)
        .iter()
        .flat_map(|d| inspect::csp_features(d))
        .collect();
    if let Some(ac) = &cfg.ac_install_path {
        feats.extend(inspect::csp_features_loaded(ac, kind, id));
    }
    feats.sort();
    feats.dedup();
    Ok(feats)
}

pub fn detail(conn: &Connection, cfg: &AppConfig, id: &str) -> rusqlite::Result<Option<ModDetail>> {
    let Some(m) = overlay::get_mod(conn, id)? else {
        return Ok(None);
    };
    let versions = overlay::get_versions(conn, id)?;
    let history = overlay::get_history(conn, id)?;
    // Dossier de l'entité : version active en bibliothèque, sinon content/ (stock).
    let entity_dirs = entity_dirs(conn, cfg, &m);
    // Description saisie par l'utilisateur (§5bis.3) : contrairement au nom
    // (arbitré en SQL, voir `MOD_SELECT`), la description native n'est pas en
    // base — elle se relit dans le `ui_*.json` à chaque affichage. L'arbitrage
    // se fait donc ici, une fois pour les deux formes de fiche.
    let described = m.description_user.clone();
    // Fiche technique native lue à la demande (voitures).
    let specs = if m.kind == "Car" {
        let native = layered(&entity_dirs, uijson::read_car_specs);
        // `or_else` et pas seulement `map` : un mod sans `ui_car.json` lisible
        // n'a pas de fiche native, mais peut très bien porter une description
        // écrite à la main — la perdre serait perdre la seule chose qu'on ait.
        match (&described, native) {
            (Some(_), Some(mut s)) => {
                s.description = described.clone();
                Some(s)
            }
            (Some(_), None) => Some(uijson::NativeSpecs {
                description: described.clone(),
                ..Default::default()
            }),
            (None, native) => native,
        }
    } else {
        None
    };
    // Détail circuit (description + layouts illustrés).
    let track = if m.kind == "Track" {
        let mut t = uijson::read_track_detail(&entity_dirs);
        if described.is_some() {
            t.description = described.clone();
        }
        Some(t)
    } else {
        None
    };
    let tech = if m.kind == "Car" {
        Some(crate::techsheet::effective(conn, id)?)
    } else {
        None
    };
    let mut card = to_card(conn, cfg, m);
    fill_usage(&mut card, &cm_stats::read(), &overlay::launched_ids(conn)?);
    let stock_pack = card
        .base
        .is_stock
        .then(|| crate::kunos_dates::pack_name(ModKind::from_column(&card.base.kind), id))
        .flatten();
    Ok(Some(ModDetail {
        card,
        versions,
        history,
        specs,
        tech,
        track,
        stock_pack,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Règle : le nom et la description saisis par l'utilisateur (§5bis.3)
    /// survivent à une mise à jour du mod. C'est TOUTE la raison d'être de ces
    /// deux colonnes séparées — les champs dérivés du `ui_*.json`, eux, sont
    /// réécrits à chaque réimport/réindex, et une saisie qui y vivrait
    /// disparaîtrait à la première mise à jour publiée par l'auteur.
    #[test]
    fn user_name_and_description_survive_a_mod_update() {
        let base = crate::testutil::temp_dir("override");
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let now = chrono::Local::now().to_rfc3339();
        overlay::upsert_mod(
            &conn,
            "car_a",
            "Car",
            Some("B"),
            Some("Nom d'origine"),
            "h1",
            None,
            &now,
        )
        .unwrap();

        overlay::set_mod_field(&conn, "car_a", "display_name_user", Some("Mon nom à moi")).unwrap();
        overlay::set_mod_field(&conn, "car_a", "description_user", Some("Ma description")).unwrap();

        // Mise à jour du mod : l'auteur publie un nouveau nom dans son ui_car.json.
        overlay::upsert_mod(
            &conn,
            "car_a",
            "Car",
            Some("B"),
            Some("Nom v2 de l'auteur"),
            "h2",
            None,
            &now,
        )
        .unwrap();
        // Et un réindex passe derrière, qui relit lui aussi le fichier.
        overlay::update_mod_reindexed_fields(&conn, "car_a", Some("B"), Some("Nom v2 de l'auteur"), None).unwrap();

        let m = overlay::get_mod(&conn, "car_a").unwrap().unwrap();
        assert_eq!(
            m.display_name.as_deref(),
            Some("Mon nom à moi"),
            "le nom saisi l'emporte encore après la mise à jour"
        );
        assert_eq!(
            m.display_name_user.as_deref(),
            Some("Mon nom à moi"),
            "la saisie brute reste lisible pour proposer d'y renoncer"
        );
        assert_eq!(m.description_user.as_deref(), Some("Ma description"));

        // Renoncer à la surcharge redonne le nom de l'auteur, pas l'ancien.
        overlay::set_mod_field(&conn, "car_a", "display_name_user", None).unwrap();
        let m = overlay::get_mod(&conn, "car_a").unwrap().unwrap();
        assert_eq!(
            m.display_name.as_deref(),
            Some("Nom v2 de l'auteur"),
            "sans surcharge, on retombe sur le nom courant du fichier"
        );
    }

    #[test]
    fn stock_mod_always_active() {
        // Le contenu de base Kunos est un vrai dossier (jamais une junction) :
        // il doit être considéré actif même sans dossier AC configuré.
        let base = crate::testutil::temp_dir("active");
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let now = chrono::Local::now().to_rfc3339();
        overlay::upsert_stock_mod(&conn, "ks_test_track", "Track", None, Some("Test"), &now, false).unwrap();
        let m = overlay::get_mod(&conn, "ks_test_track").unwrap().unwrap();
        assert!(is_active(&AppConfig::default(), &m));
    }

    #[test]
    fn managed_mod_active_when_deployed_via_hardlinks() {
        // §2 : is_active doit reconnaître le nouveau mécanisme de déploiement
        // (hardlinks), pas seulement l'ancien symlink.
        let base = crate::testutil::temp_dir("active-hl");
        let ac = base.join("ac");
        let lib = base.join("library");
        std::fs::create_dir_all(ac.join("content").join("cars")).unwrap();
        let carv = lib.join("cars").join("hl_car").join("v1");
        std::fs::create_dir_all(&carv).unwrap();
        std::fs::write(carv.join("f.txt"), "x").unwrap();

        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let now = chrono::Local::now().to_rfc3339();
        overlay::upsert_mod(&conn, "hl_car", "Car", Some("B"), Some("Test"), "h", None, &now).unwrap();
        overlay::insert_version(
            &conn,
            "v1",
            "hl_car",
            Some("1.0"),
            None,
            &now,
            &carv.to_string_lossy(),
            None,
            "sig",
            &[],
            &[],
            &[],
            &[],
            None,
        )
        .unwrap();
        overlay::set_active_version(&conn, "hl_car", "v1").unwrap();
        let cfg = AppConfig {
            ac_install_path: Some(ac),
            library_path: Some(lib),
            ..Default::default()
        };

        let m = overlay::get_mod(&conn, "hl_car").unwrap().unwrap();
        assert!(!is_active(&cfg, &m), "pas encore activé");

        crate::activation::activate(&conn, &cfg, "hl_car", None).unwrap();
        let m = overlay::get_mod(&conn, "hl_car").unwrap().unwrap();
        assert!(is_active(&cfg, &m), "déployé par hardlinks = actif");
    }

    #[test]
    fn entity_dir_ignores_stale_pre_hardlink_composed_leftover() {
        // Bug réel : `<lib>/composed/<type>s/<id>` est un reliquat de l'ancien
        // mécanisme par junction (avant la bascule hardlinks, §4.3) — plus
        // jamais écrit ni lu. Sur une bibliothèque utilisée avant la bascule,
        // ce dossier peut encore traîner sur le disque avec un contenu périmé
        // (ex. un layout apporté par une couche depuis désactivée) ; il ne
        // doit plus jamais être préféré au vrai contenu déployé dans content/.
        let base = crate::testutil::temp_dir("stale-composed");
        let ac = base.join("ac");
        let lib = base.join("library");
        let link = ac.join("content").join("tracks").join("spa");
        std::fs::create_dir_all(link.join("ui")).unwrap();
        std::fs::write(link.join("ui").join("ui_track.json"), br#"{"name":"Spa restored"}"#).unwrap();

        // Reliquat périmé : contient encore le layout "2022" d'une couche
        // pourtant désactivée depuis.
        let stale = lib.join("composed").join("tracks").join("spa");
        std::fs::create_dir_all(stale.join("ui").join("2022")).unwrap();
        std::fs::write(stale.join("ui").join("2022").join("ui_track.json"), "{}").unwrap();

        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        overlay::upsert_stock_mod(&conn, "spa", "Track", Some("Kunos"), Some("Spa"), "now", false).unwrap();
        let cfg = AppConfig {
            ac_install_path: Some(ac),
            library_path: Some(lib),
            ..Default::default()
        };
        let m = overlay::get_mod(&conn, "spa").unwrap().unwrap();

        let dir = entity_dir(&conn, &cfg, &m).unwrap();
        assert_eq!(
            dir, link,
            "doit résoudre vers content/, jamais vers l'ancien dossier composé périmé"
        );
        assert!(
            !dir.join("ui").join("2022").is_dir(),
            "le layout périmé du reliquat ne doit pas apparaître"
        );
    }

    /// Rule (§6.1): the description carried by a card - the one the library
    /// filters on - is the one the USER typed (§5bis.3) as soon as there is
    /// one, otherwise the `ui_*.json` one. Holds for both kinds: a car reads
    /// `ui_car.json`, a track `ui_track.json`.
    #[test]
    fn card_description_prefers_user_text_over_the_ui_json() {
        let base = crate::testutil::temp_dir("desc");
        let ac = base.join("ac");
        let car = ac.join("content").join("cars").join("ks_ferrari");
        std::fs::create_dir_all(car.join("ui")).unwrap();
        std::fs::write(
            car.join("ui").join("ui_car.json"),
            br#"{"name":"488","brand":"Ferrari","description":"Written by the modder."}"#,
        )
        .unwrap();
        let track = ac.join("content").join("tracks").join("spa");
        std::fs::create_dir_all(track.join("ui")).unwrap();
        std::fs::write(
            track.join("ui").join("ui_track.json"),
            br#"{"name":"Spa","description":"Ardennes forest."}"#,
        )
        .unwrap();

        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let now = chrono::Local::now().to_rfc3339();
        overlay::upsert_stock_mod(&conn, "ks_ferrari", "Car", Some("Ferrari"), Some("488"), &now, false).unwrap();
        overlay::upsert_stock_mod(&conn, "spa", "Track", Some("Kunos"), Some("Spa"), &now, false).unwrap();
        let cfg = AppConfig {
            ac_install_path: Some(ac),
            ..Default::default()
        };
        let described = |id: &str| {
            let m = overlay::get_mod(&conn, id).unwrap().unwrap();
            to_card(&conn, &cfg, m).description
        };

        assert_eq!(
            described("ks_ferrari").as_deref(),
            Some("Written by the modder."),
            "with no user text, the card carries the mod file's own description"
        );
        assert_eq!(
            described("spa").as_deref(),
            Some("Ardennes forest."),
            "same for a track, read from ui_track.json"
        );

        overlay::set_mod_field(&conn, "ks_ferrari", "description_user", Some("My own words")).unwrap();
        overlay::set_mod_field(&conn, "spa", "description_user", Some("My favourite track")).unwrap();

        assert_eq!(
            described("ks_ferrari").as_deref(),
            Some("My own words"),
            "user text replaces the file description, never the other way round"
        );
        assert_eq!(
            described("spa").as_deref(),
            Some("My favourite track"),
            "same for a track"
        );
    }

    #[test]
    fn stock_car_skins_read_from_content() {
        let base = crate::testutil::temp_dir("lib");
        let ac = base.join("ac");
        // Voiture de base avec un skin installé dans content/.
        let skin = ac
            .join("content")
            .join("cars")
            .join("ks_ferrari")
            .join("skins")
            .join("rosso");
        std::fs::create_dir_all(&skin).unwrap();
        std::fs::write(skin.join("preview.jpg"), b"IMG").unwrap();
        std::fs::write(skin.join("livery.png"), b"LIVERY").unwrap();

        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let now = chrono::Local::now().to_rfc3339();
        overlay::upsert_stock_mod(&conn, "ks_ferrari", "Car", Some("Ferrari"), Some("488"), &now, false).unwrap();

        let cfg = AppConfig {
            ac_install_path: Some(ac.clone()),
            ..Default::default()
        };
        let skins = list_mod_skins(&conn, &cfg, "ks_ferrari");
        assert_eq!(skins.len(), 1, "skin de la voiture de base lu dans content/");
        assert_eq!(skins[0].id, "rosso");
        assert!(skins[0].preview.is_some(), "preview.jpg détecté");
        assert!(skins[0].livery.is_some(), "livery.png détecté");
    }

    #[test]
    fn skin_without_livery_leaves_it_none() {
        let base = crate::testutil::temp_dir("lib");
        let ac = base.join("ac");
        let skin = ac
            .join("content")
            .join("cars")
            .join("ks_ferrari")
            .join("skins")
            .join("rosso");
        std::fs::create_dir_all(&skin).unwrap();
        std::fs::write(skin.join("preview.jpg"), b"IMG").unwrap();
        // Pas de livery.png : convention pas garantie sur tous les skins.

        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let now = chrono::Local::now().to_rfc3339();
        overlay::upsert_stock_mod(&conn, "ks_ferrari", "Car", Some("Ferrari"), Some("488"), &now, false).unwrap();

        let cfg = AppConfig {
            ac_install_path: Some(ac.clone()),
            ..Default::default()
        };
        let skins = list_mod_skins(&conn, &cfg, "ks_ferrari");
        assert_eq!(skins[0].livery, None, "pas de livery.png -> None, jamais une erreur");
    }

    #[test]
    fn broken_mod_flagged_on_card() {
        // Mod dont la version active pointe vers un dossier bibliothèque
        // disparu (§6.4) : list_cards doit remonter broken=true, la même
        // détection que l'écran Maintenance (§10).
        let base = crate::testutil::temp_dir("broken-card");
        std::fs::create_dir_all(&base).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let now = chrono::Local::now().to_rfc3339();

        overlay::upsert_mod(&conn, "ghost", "Car", Some("B"), Some("Ghost"), "h", None, &now).unwrap();
        overlay::insert_version(
            &conn,
            "v1",
            "ghost",
            Some("1.0"),
            None,
            &now,
            &base.join("nope").to_string_lossy(),
            None,
            "sig",
            &[],
            &[],
            &[],
            &[],
            None,
        )
        .unwrap();
        overlay::set_active_version(&conn, "ghost", "v1").unwrap();

        let cards = list_cards(&conn, &AppConfig::default()).unwrap();
        let ghost = cards.iter().find(|c| c.base.id_interne == "ghost").unwrap();
        assert!(ghost.broken);
    }

    #[test]
    fn stock_mod_never_flagged_broken() {
        // Le contenu de base n'a pas de version bibliothèque à proprement
        // parler (lecture directe dans content/) — ne doit jamais être signalé
        // cassé, même sans dossier AC configuré.
        let base = crate::testutil::temp_dir("broken-stock");
        std::fs::create_dir_all(&base).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let now = chrono::Local::now().to_rfc3339();
        overlay::upsert_stock_mod(&conn, "ks_test_track", "Track", None, Some("Test"), &now, false).unwrap();

        let cards = list_cards(&conn, &AppConfig::default()).unwrap();
        let stock = cards.iter().find(|c| c.base.id_interne == "ks_test_track").unwrap();
        assert!(!stock.broken);
    }

    /// The listing behind `list_library` reads the base first, then the files
    /// with the lock released: it must build exactly the cards of `list_cards`.
    #[test]
    fn listing_on_the_shared_base_matches_listing_on_a_connection() {
        let base = crate::testutil::temp_dir("cards-shared");
        let (conn, cfg, _) = track_with_layer(&base);
        let now = chrono::Local::now().to_rfc3339();
        overlay::upsert_stock_mod(&conn, "ks_test_car", "Car", None, Some("Test"), &now, false).unwrap();

        let expected = serde_json::to_value(list_cards(&conn, &cfg).unwrap()).unwrap();
        let db = overlay::Db(std::sync::Mutex::new(conn));
        let shared = serde_json::to_value(list_cards_shared(&db, &cfg).unwrap()).unwrap();
        assert_eq!(shared.as_array().map(Vec::len), Some(2), "both mods listed");
        assert_eq!(shared, expected, "same cards, field for field");
    }

    /// The brand logos are elected from the cars that have both a brand and a
    /// badge (TAXO§4) - and the listing that releases the base lock before
    /// looking for badges finds exactly the same ones.
    #[test]
    fn badges_on_the_shared_base_match_badges_on_a_connection() {
        let base = crate::testutil::temp_dir("badges-shared");
        let ac = base.join("ac");
        let cars = ac.join("content").join("cars");
        for id in ["ks_badged", "ks_unbranded"] {
            std::fs::create_dir_all(cars.join(id).join("ui")).unwrap();
            std::fs::write(cars.join(id).join("ui").join("badge.png"), b"x").unwrap();
        }
        std::fs::create_dir_all(cars.join("ks_no_badge").join("ui")).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let now = chrono::Local::now().to_rfc3339();
        overlay::upsert_stock_mod(&conn, "ks_badged", "Car", Some("Ferrari"), Some("A"), &now, false).unwrap();
        overlay::upsert_stock_mod(&conn, "ks_unbranded", "Car", None, Some("B"), &now, false).unwrap();
        overlay::upsert_stock_mod(&conn, "ks_no_badge", "Car", Some("Ferrari"), Some("C"), &now, false).unwrap();
        let cfg = AppConfig {
            ac_install_path: Some(ac),
            ..Default::default()
        };

        let expected = car_badges(&conn, &cfg).unwrap();
        assert_eq!(
            expected
                .iter()
                .map(|(id, brand, _)| (id.as_str(), brand.as_str()))
                .collect::<Vec<_>>(),
            vec![("ks_badged", "Ferrari")],
            "only the car with a brand and a badge"
        );
        let db = overlay::Db(std::sync::Mutex::new(conn));
        assert_eq!(car_badges_shared(&db, &cfg).unwrap(), expected, "same badges");
    }

    /// Monte un circuit géré : version de base en bibliothèque + une couche.
    /// Renvoie (conn, cfg, id de la couche).
    fn track_with_layer(base: &Path) -> (Connection, AppConfig, String) {
        let library = base.join("library");
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let now = chrono::Local::now().to_rfc3339();

        // Base : deux layouts, chacun avec sa photo.
        let ver = library.join("tracks").join("smm").join("v1");
        for l in ["a_race", "b_free"] {
            let ui = ver.join("ui").join(l);
            std::fs::create_dir_all(&ui).unwrap();
            std::fs::write(ui.join("ui_track.json"), br#"{"name":"Base","description":"d"}"#).unwrap();
            std::fs::write(ui.join("preview.png"), b"OLD").unwrap();
        }
        overlay::upsert_mod(&conn, "smm", "Track", None, Some("SMM"), "h", None, &now).unwrap();
        overlay::insert_version(
            &conn,
            "v1",
            "smm",
            Some("1"),
            None,
            &now,
            &ver.to_string_lossy(),
            None,
            "sig",
            &[],
            &[],
            &[],
            &[],
            None,
        )
        .unwrap();
        overlay::set_active_version(&conn, "smm", "v1").unwrap();

        // Couche : remplace la photo d'UN layout, en ajoute un troisième.
        let layer = library.join("layers").join("smm").join("overhaul");
        let a = layer.join("ui").join("a_race");
        std::fs::create_dir_all(&a).unwrap();
        std::fs::write(a.join("preview.png"), b"NEW").unwrap();
        let c = layer.join("ui").join("c_extra");
        std::fs::create_dir_all(&c).unwrap();
        std::fs::write(c.join("ui_track.json"), r#"{"name":"Ajouté par la couche"}"#).unwrap();
        overlay::insert_layer(
            &conn,
            "L1",
            "smm",
            "Track",
            "overhaul",
            &layer.to_string_lossy(),
            Some("overhaul.zip"),
            1,
            1,
            0,
            &now,
        )
        .unwrap();

        let cfg = AppConfig {
            library_path: Some(library),
            ..Default::default()
        };
        (conn, cfg, "L1".to_string())
    }

    #[test]
    fn an_active_layer_shows_through_on_the_detail_view() {
        // Règle (§4.3) : ce que l'app affiche est le **résultat composé**, pas
        // la version de base. Bug réel : une couche remplaçait le `preview.png`
        // d'un circuit, le jeu voyait bien la nouvelle image, et la fiche
        // continuait d'afficher l'ancienne — même après redémarrage. Tout se
        // lisait dans le dossier de la version de base, où une couche n'est par
        // construction jamais écrite.
        let base = crate::testutil::temp_dir("layered-read");
        let (conn, cfg, layer_id) = track_with_layer(&base);

        let t = detail(&conn, &cfg, "smm").unwrap().unwrap().track.unwrap();
        let a = t
            .layouts
            .iter()
            .find(|l| l.id == "a_race")
            .expect("layout de base présent");
        assert_eq!(
            std::fs::read(a.preview.as_ref().unwrap()).unwrap(),
            b"NEW",
            "la photo de la couche l'emporte sur celle de la base"
        );
        let b = t
            .layouts
            .iter()
            .find(|l| l.id == "b_free")
            .expect("layout non touché présent");
        assert_eq!(
            std::fs::read(b.preview.as_ref().unwrap()).unwrap(),
            b"OLD",
            "un layout que la couche ne touche pas garde la photo de la base"
        );
        assert_eq!(
            a.name, "Base",
            "la couche n'apporte pas de ui_track.json ici : celui de la base reste"
        );
        assert!(
            t.layouts.iter().any(|l| l.id == "c_extra"),
            "un layout ajouté par la couche apparaît"
        );

        // Désactiver la couche doit tout rendre à la base, à l'écran comme en jeu.
        overlay::set_layer_active(&conn, &layer_id, false).unwrap();
        let t = detail(&conn, &cfg, "smm").unwrap().unwrap().track.unwrap();
        let a = t.layouts.iter().find(|l| l.id == "a_race").unwrap();
        assert_eq!(
            std::fs::read(a.preview.as_ref().unwrap()).unwrap(),
            b"OLD",
            "couche désactivée : la base réapparaît"
        );
        assert!(
            !t.layouts.iter().any(|l| l.id == "c_extra"),
            "le layout apporté par la couche disparaît avec elle"
        );
    }

    #[test]
    fn the_library_card_shows_the_layers_preview() {
        // Même règle, sur la vignette de la bibliothèque : c'est là qu'on
        // regarde en premier, et c'est là que l'écart se voyait.
        let base = crate::testutil::temp_dir("layered-card");
        let (conn, cfg, _) = track_with_layer(&base);
        let m = overlay::get_mod(&conn, "smm").unwrap().unwrap();
        let preview = preview_for(&conn, &cfg, &m).expect("une vignette est trouvée");
        assert_eq!(
            std::fs::read(&preview).unwrap(),
            b"NEW",
            "la carte montre ce que le jeu montre"
        );
    }
}
