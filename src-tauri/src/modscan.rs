//! Détection de type et descente récursive — porté de `archives.py`
//! (`isCar` / `isTrack` / `isCarSound` / `recursiveMoveModsToValidModDir`).
//!
//! Gère les archives à racine décalée, les mods imbriqués et plusieurs mods
//! dans une même archive.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum ModKind {
    Car,
    Track,
}

impl ModKind {
    /// Relit la colonne `mods.kind`, qui porte `"Car"` / `"Track"` (le `Debug`
    /// de cet enum). Tolérante à la casse pour la même raison que partout
    /// ailleurs : un type relu de travers envoie la recherche dans le mauvais
    /// arbre, en silence.
    pub fn from_kind(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "car" | "cars" => Some(ModKind::Car),
            "track" | "tracks" => Some(ModKind::Track),
            _ => None,
        }
    }

    /// Segment de dossier dans `content/` : "cars" | "tracks".
    pub fn content_folder(self) -> &'static str {
        match self {
            ModKind::Car => "cars",
            ModKind::Track => "tracks",
        }
    }
}

#[derive(Debug, Clone)]
pub struct FoundMod {
    pub kind: ModKind,
    /// Dossier qui deviendra `content/<type>s/<id>` (contient `ui/`, etc.).
    pub dir: PathBuf,
}

/// `<dir>/ui/ui_car.json` présent.
pub fn is_car(dir: &Path) -> bool {
    dir.is_dir() && dir.join("ui").join("ui_car.json").is_file()
}

/// `ui_track.json` à la racine `ui/` ou dans un sous-dossier de layout.
pub fn is_track(dir: &Path) -> bool {
    let ui = dir.join("ui");
    if !ui.is_dir() {
        return false;
    }
    if ui.join("ui_track.json").is_file() {
        return true;
    }
    if let Ok(entries) = std::fs::read_dir(&ui) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() && p.join("ui_track.json").is_file() {
                return true;
            }
        }
    }
    false
}

/// Mod son : présence de `*.bank` + `GUIDs.txt`. Détecté mais hors périmètre
/// car/track de L1 (cf. §14.5) — on évite simplement d'y descendre.
pub fn is_car_sound(dir: &Path) -> bool {
    if !dir.is_dir() || !dir.join("GUIDs.txt").is_file() {
        return false;
    }
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .any(|e| e.path().extension().is_some_and(|ext| ext.eq_ignore_ascii_case("bank")))
        })
        .unwrap_or(false)
}

/// Le dossier d'emballage d'un pack de son, quand il y en a un : celui qui
/// contient le dossier de son **et rien d'autre qu'des fichiers**.
///
/// Sert à rattacher au son ce qui est livré à côté de lui — un `ReadMe.txt`,
/// une notice. Sans lui, ces fichiers n'ont aucun propriétaire possible et
/// tombent en « autre mod » : bug réel, le `ReadMe.txt` d'un mod de son GT40
/// est devenu un mod à lui tout seul, dont l'unique fichier a ensuite été
/// classé annexe et déplacé dans ses ressources. Deux lignes de rapport et une
/// entrée vide en bibliothèque, pour un fichier texte.
///
/// **Deux garde-fous**, sans lesquels on avalerait ce qui ne nous appartient
/// pas :
///
/// - jamais sous `content/cars/<id>/` ou `content/tracks/<id>/` — là, les
///   voisins du `sfx/` sont le contenu de la voiture (`data.acd`, `ui/`,
///   `skins/`), pas des annexes du son ;
/// - jamais si le dossier porte un **autre sous-dossier** : il livre alors
///   plusieurs choses, et rien ne dit que ce qui l'entoure revient au son.
fn sound_pack_root(dir: &Path) -> Option<PathBuf> {
    let parent = dir.parent()?;
    let grandparent_name = parent
        .parent()
        .and_then(|g| g.file_name())
        .map(|n| n.to_string_lossy().to_ascii_lowercase());
    if matches!(grandparent_name.as_deref(), Some("cars") | Some("tracks")) {
        return None;
    }
    let only_child = std::fs::read_dir(parent)
        .ok()?
        .flatten()
        .all(|e| !e.path().is_dir() || e.path() == dir);
    only_child.then(|| parent.to_path_buf())
}

/// Le dossier contient au moins un sous-dossier.
fn has_subdir(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .map(|mut e| e.any(|x| x.map(|x| x.path().is_dir()).unwrap_or(false)))
        .unwrap_or(false)
}

/// Dossier de skin « feuille » : porte `ui_skin.json`, ou n'a pas de sous-dossier
/// (les fichiers de livrée sont à plat). Sert à distinguer un skin d'un dossier
/// de voiture (qui, lui, contient des dossiers de skins).
fn is_skin_leaf(dir: &Path) -> bool {
    dir.join("ui_skin.json").is_file() || !has_subdir(dir)
}

/// Vrai si les enfants de `skins/` sont des **dossiers de voitures** (forme
/// `skins/<voiture>/<skin>`) plutôt que des skins (forme `<voiture>/skins/<skin>`).
fn skins_are_per_car_folders(skins_dir: &Path) -> bool {
    let children: Vec<PathBuf> = std::fs::read_dir(skins_dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    // Un enfant qui ressemble à un skin → forme classique (skins directs).
    if children.iter().any(|c| is_skin_leaf(c)) {
        return false;
    }
    // Sinon, des enfants contenant des sous-dossiers = dossiers de voitures.
    children.iter().any(|c| has_subdir(c))
}

/// Type d'un sous-élément rattaché détecté à l'import.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubKind {
    Skin,
    Sound,
}

#[derive(Debug, Clone)]
pub struct FoundSub {
    pub kind: SubKind,
    /// Voiture cible (nom du dossier) à laquelle rattacher le sous-élément.
    pub parent_id: String,
    /// Pour un SKIN : dossier dont les **enfants directs sont les skins**. Pour
    /// un SON : dossier contenant `GUIDs.txt` + `.bank`.
    pub dir: PathBuf,
    /// Dossier "racine" du pack dans la source (celui qui contient `skins/`),
    /// quand il désigne sans ambiguïté une seule cible — `None` pour la forme
    /// multi-voitures (`skins/<voiture>/<skin>`, une racine par voiture, pas de
    /// dossier commun). Sert à retrouver des fichiers voisins de `skins/` (ex.
    /// `ext_config.ini` d'un pack de skins de circuit, §8).
    pub extra_root: Option<PathBuf>,
}

/// Descend récursivement à partir de `root` et collecte les voitures/circuits.
/// Un dossier reconnu comme mod n'est pas exploré plus profond.
pub fn scan(root: &Path) -> Vec<FoundMod> {
    let mut found = Vec::new();
    descend(root, &mut found);
    found
}

/// Descend et collecte les **sous-éléments** (packs de skins, mods de son) qui
/// ne sont pas des mods de premier niveau (§12bis.2). Disjoint de `scan` : une
/// vraie voiture/circuit (avec `ui/`) est ignorée ici.
pub fn scan_subs(root: &Path) -> Vec<FoundSub> {
    let mut found = Vec::new();
    descend_subs(root, &mut found);
    found
}

/// App Python OU Lua/CSP d'AC (§12bis.4) : dossier `<nom>` contenant `<nom>.py`
/// (script principal, convention AC `apps/python/<App>/<App>.py`) ou
/// `<nom>.lua` (convention CSP `apps/lua/<App>/<App>.lua`, même schéma de
/// nommage) — les deux sont des scripts d'app autonomes, seul le sous-dossier
/// `apps/<langue>/` où AC va les chercher diffère (`apps.rs::app_lang`).
pub fn is_app(dir: &Path) -> bool {
    let Some(name) = dir.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    dir.join(format!("{name}.py")).is_file() || dir.join(format!("{name}.lua")).is_file()
}

#[derive(Debug, Clone)]
pub struct FoundApp {
    pub name: String,
    pub dir: PathBuf,
}

/// Descend et collecte les **apps** (type autonome, §12bis.4). Disjoint des
/// voitures/circuits.
pub fn scan_apps(root: &Path) -> Vec<FoundApp> {
    let mut found = Vec::new();
    descend_apps(root, &mut found);
    found
}

fn descend_apps(dir: &Path, out: &mut Vec<FoundApp>) {
    if is_car(dir) || is_track(dir) {
        return; // mod de 1er niveau : pas une app, et inutile d'y descendre
    }
    if is_app(dir) {
        let name = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        out.push(FoundApp {
            name,
            dir: dir.to_path_buf(),
        });
        return;
    }
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                descend_apps(&p, out);
            }
        }
    }
}

fn descend_subs(dir: &Path, out: &mut Vec<FoundSub>) {
    // Vrai mod de premier niveau : géré par `scan`, pas ici.
    if is_car(dir) || is_track(dir) {
        return;
    }
    let dir_name = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();

    // Pack de skins : deux arborescences possibles (§12bis.2), plus la
    // convention CM pour les skins de circuit `skins/cm_skins/<skin>` (regroupe
    // les livrées sous un sous-dossier dédié, à distinguer d'un dossier de
    // voiture/circuit cible qui aurait la même forme à un niveau) — sinon
    // `skins_are_per_car_folders` la confond avec `skins/<voiture>/<skin>` et
    // route le pack vers un parent inexistant nommé "cm_skins".
    let skins = dir.join("skins");
    if skins.is_dir() && has_subdir(&skins) {
        let cm_skins = skins.join("cm_skins");
        if cm_skins.is_dir() {
            out.push(FoundSub {
                kind: SubKind::Skin,
                parent_id: dir_name,
                dir: cm_skins,
                extra_root: Some(dir.to_path_buf()),
            });
            return;
        }
        if skins_are_per_car_folders(&skins) {
            // Forme `skins/<voiture>/<skin>` : chaque enfant = une voiture cible,
            // pas de dossier racine commun (extra_root ambigu, on l'omet).
            for e in std::fs::read_dir(&skins).into_iter().flatten().flatten() {
                let car = e.path();
                if car.is_dir() && has_subdir(&car) {
                    let parent = e.file_name().to_string_lossy().into_owned();
                    out.push(FoundSub {
                        kind: SubKind::Skin,
                        parent_id: parent,
                        dir: car,
                        extra_root: None,
                    });
                }
            }
        } else {
            // Forme `<voiture>/skins/<skin>` : le dossier courant est la voiture.
            out.push(FoundSub {
                kind: SubKind::Skin,
                parent_id: dir_name,
                dir: skins,
                extra_root: Some(dir.to_path_buf()),
            });
        }
        return;
    }

    if is_car_sound(dir) {
        out.push(FoundSub {
            kind: SubKind::Sound,
            parent_id: dir_name,
            dir: dir.to_path_buf(),
            extra_root: sound_pack_root(dir),
        });
        return;
    }
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                descend_subs(&p, out);
            }
        }
    }
}

fn descend(dir: &Path, out: &mut Vec<FoundMod>) {
    if is_track(dir) {
        out.push(FoundMod {
            kind: ModKind::Track,
            dir: dir.to_path_buf(),
        });
        return;
    }
    if is_car(dir) {
        out.push(FoundMod {
            kind: ModKind::Car,
            dir: dir.to_path_buf(),
        });
        return;
    }
    if is_car_sound(dir) {
        // Hors périmètre L1 : on ne descend pas dedans, on ne l'importe pas.
        return;
    }
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                descend(&p, out);
            }
        }
    }
}

/// Does `dir` hold a file with this extension, directly at its root?
fn has_ext_at_root(dir: &Path, ext: &str) -> bool {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .any(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case(ext)))
        })
        .unwrap_or(false)
}

/// `models.ini` / `models_<layout>.ini` at the root of `dir`.
fn has_models_ini(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries.flatten().any(|e| {
                e.file_name().to_str().is_some_and(|n| {
                    n.to_ascii_lowercase().starts_with("models") && n.to_ascii_lowercase().ends_with(".ini")
                })
            })
        })
        .unwrap_or(false)
}

/// Can AC load this folder on its own?
///
/// [`is_car`] and [`is_track`] answer "shaped like a mod", which is not the
/// same question: both only look at `ui/`, and a folder meant to be **dropped
/// onto** an existing mod carries the very same `ui/` — the author copies it to
/// ship a new `preview.png`. What separates the two is geometry. A car needs a
/// `.kn5` at its root; a track needs one too, or a `models*.ini` naming the
/// ones it loads. A folder that has neither cannot be driven: it is a
/// **fragment**, and importing it as a mod creates a phantom entry that the
/// game can never load (see `fragment.rs`).
///
/// Measured over the whole reference corpus, **without a single exception**:
/// 103 library track versions, the 121 tracks of the AC install and 123 library
/// car versions all carry their geometry. The predicate therefore never demotes
/// a real mod, which is what makes it safe to act on rather than merely warn.
pub fn has_geometry(kind: ModKind, dir: &Path) -> bool {
    if has_ext_at_root(dir, "kn5") {
        return true;
    }
    match kind {
        // A track may keep its models out of the root and name them in
        // `models_<layout>.ini`, which some authors put in the layout folder
        // rather than at the root — both forms exist in the corpus.
        ModKind::Track => {
            has_models_ini(dir)
                || std::fs::read_dir(dir)
                    .map(|entries| {
                        entries
                            .flatten()
                            .any(|e| e.path().is_dir() && has_models_ini(&e.path()))
                    })
                    .unwrap_or(false)
        }
        ModKind::Car => false,
    }
}
