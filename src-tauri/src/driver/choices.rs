//! What a car lets the user choose for its driver: the suits, gloves and
//! helmets that will actually show on its mannequin (§4.6ter).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::Serialize;

use super::bodies::era_of;
use super::{body_file, diffuse_textures, outfit_of, GLOVES_DIR, HELMET_DIR, SUIT_DIR};

/// Un dossier de garde-robe, tel qu'il s'offre au choix.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WardrobeOption {
    /// Valeur telle que `skin.ini` l'écrit : `plain/red`.
    pub id: String,
    /// Ce qu'on affiche. Les noms de dossier AC ne se traduisent pas.
    pub label: String,
    /// La vignette qu'AC range à côté des `.dds`, quand il y en a une —
    /// 173 des 176 dossiers de casque en ont, d'où l'intérêt d'un menu
    /// illustré plutôt qu'une liste de noms.
    pub thumbnail: Option<String>,
}

/// Ce qu'une voiture donnée permet de choisir.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DriverChoices {
    /// Le mannequin sur lequel ces listes ont été calculées : celui de la
    /// voiture, ou celui que l'utilisateur lui a substitué.
    pub model: String,
    /// `true` quand ce mannequin n'est pas celui que la voiture nomme — le
    /// mode « corps substitué » de l'écran Pilote (PILOTE§10), qui ne vaut que dans
    /// l'aperçu.
    pub substituted: bool,
    /// Époque de la boîte à casques du mannequin, clé de la table [`super::bodies::ERAS`].
    /// `None` = mannequin qui nomme ses images autrement, donc aucun casque du
    /// jeu ne s'y pose (PILOTE§11.1).
    pub era: Option<&'static str>,
    pub suits: Vec<WardrobeOption>,
    pub gloves: Vec<WardrobeOption>,
    pub helmets: Vec<WardrobeOption>,
}

/// Les tenues qui marcheront réellement sur le mannequin de cette voiture.
///
/// **Une seule règle de compatibilité, et elle couvre les trois listes** : un
/// dossier est retenu s'il contient un fichier que le mannequin utilise comme
/// `txDiffuse`. Mesuré sur le parc, ça donne exactement ce qu'il faut :
///
/// - les combinaisons et les gants passent partout, parce que les cinq
///   mannequins Kunos réclament tous `2016_Suit_DIFF.dds` et
///   `2016_Gloves_DIFF.dds` — 53 et 67 dossiers, universels ;
/// - les casques se filtrent tout seuls par époque, `HELMET_2012` contre
///   `HELMET_1985`, `HELMET_1975`, `HELMET_1969` — 176 dossiers, dont 100
///   pour les voitures modernes ;
/// - les dossiers `_nm`, qui ne portent que des cartes de normales partagées,
///   tombent d'eux-mêmes : une normale n'est pas une `txDiffuse`, donc ce
///   n'est pas un choix de tenue.
///
/// Aucune liste codée en dur, donc, et un mannequin de mod inconnu est traité
/// comme les autres.
pub fn choices(ac_root: &Path, car_dir: &Path, car_id: &str, body: Option<&str>) -> Option<DriverChoices> {
    let declared = outfit_of(car_dir, car_id, None)?.model;
    // Le corps substitué commande les trois listes (PILOTE§1.3) : c'est lui qui
    // porte les noms de texture, donc lui qui décide de ce qui s'y pose.
    let model = body
        .map(str::trim)
        .filter(|m| !m.is_empty())
        .unwrap_or(&declared)
        .to_string();
    let diffuse = diffuse_textures(&body_file(ac_root, &model))?;

    let textures = ac_root.join("content").join("texture");
    Some(DriverChoices {
        suits: wardrobe_options(&textures.join(SUIT_DIR), &diffuse),
        gloves: wardrobe_options(&textures.join(GLOVES_DIR), &diffuse),
        helmets: wardrobe_options(&textures.join(HELMET_DIR), &diffuse),
        era: era_of(&diffuse),
        substituted: model != declared,
        model,
    })
}

/// Parcourt un dossier de garde-robe et retient les feuilles utilisables.
fn wardrobe_options(kind_dir: &Path, diffuse: &BTreeSet<String>) -> Vec<WardrobeOption> {
    let mut out = Vec::new();
    collect_wardrobe(kind_dir, kind_dir, diffuse, 0, &mut out);
    out.sort_by_key(|option| option.id.to_lowercase());
    out
}

/// Profondeur maximale explorée sous un dossier de garde-robe. AC range en
/// `<famille>/<couleur>` ; une de plus laisse la place à un mod fantaisiste,
/// sans transformer le scan en parcours de disque.
const WARDROBE_DEPTH: usize = 3;

fn collect_wardrobe(root: &Path, dir: &Path, diffuse: &BTreeSet<String>, depth: usize, out: &mut Vec<WardrobeOption>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut files: Vec<PathBuf> = Vec::new();
    let mut folders: Vec<PathBuf> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        match entry.file_type() {
            Ok(kind) if kind.is_dir() => folders.push(path),
            Ok(_) => files.push(path),
            Err(_) => {}
        }
    }

    let matches = |path: &Path| {
        path.file_name()
            .map(|n| n.to_string_lossy().to_lowercase())
            .is_some_and(|n| diffuse.contains(&n))
    };
    if files.iter().any(|f| matches(f)) {
        if let Ok(rel) = dir.strip_prefix(root) {
            let id = rel.to_string_lossy().replace('\\', "/");
            out.push(WardrobeOption {
                label: id.replace('/', " · "),
                thumbnail: thumbnail_of(&files, &matches).map(|p| p.to_string_lossy().into_owned()),
                id,
            });
        }
    }
    if depth < WARDROBE_DEPTH {
        for folder in folders {
            collect_wardrobe(root, &folder, diffuse, depth + 1, out);
        }
    }
}

/// La vignette d'un dossier : l'image qui porte le nom d'une de ses textures
/// utiles, à défaut n'importe laquelle.
fn thumbnail_of(files: &[PathBuf], matches: &dyn Fn(&Path) -> bool) -> Option<PathBuf> {
    let is_image = |p: &PathBuf| {
        p.extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("jpg") || e.eq_ignore_ascii_case("png"))
    };
    let stems: BTreeSet<String> = files
        .iter()
        .filter(|p| matches(p))
        .filter_map(|p| p.file_stem())
        .map(|s| s.to_string_lossy().to_lowercase())
        .collect();
    files
        .iter()
        .filter(|p| is_image(p))
        .find(|p| {
            p.file_stem()
                .map(|s| s.to_string_lossy().to_lowercase())
                .is_some_and(|s| stems.contains(&s))
        })
        .or_else(|| files.iter().find(|p| is_image(p)))
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ce que chaque mannequin de l'install peut porter, par la règle de
    /// [`choices`] — la mesure qui a servi à la fixer.
    ///
    /// Attendu, mesuré indépendamment avant d'écrire le code : 53 combinaisons
    /// et 69 paires de gants pour **tous** les mannequins Kunos, et des
    /// casques filtrés par époque — 100 en 2012, 44 en 1975, 21 en 1969, 11 en
    /// 1985. Un écart ici veut dire que la règle a changé de sens.
    ///
    /// ```text
    /// PITBOX_AC_ROOT="D:\...\assettocorsa" cargo test --lib driver -- --ignored --nocapture what_each
    /// ```
    #[test]
    #[ignore = "needs a real Assetto Corsa install; measurement, not a check"]
    fn what_each_mannequin_can_wear() {
        let Ok(ac_root) = std::env::var("PITBOX_AC_ROOT") else {
            eprintln!("PITBOX_AC_ROOT unset, skipping");
            return;
        };
        let root = PathBuf::from(ac_root);
        let textures = root.join("content").join("texture");
        let mut mannequins: Vec<PathBuf> = std::fs::read_dir(root.join("content").join("driver"))
            .expect("read content/driver")
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("kn5")))
            .collect();
        mannequins.sort();

        eprintln!(
            "
{:28} {:>6} {:>7} {:>8}  vignettes",
            "mannequin", "suits", "gloves", "helmets"
        );
        for mannequin in &mannequins {
            let Some(diffuse) = diffuse_textures(mannequin) else {
                eprintln!(
                    "  {:26} illisible",
                    mannequin.file_stem().unwrap_or_default().to_string_lossy()
                );
                continue;
            };
            let suits = wardrobe_options(&textures.join(SUIT_DIR), &diffuse);
            let gloves = wardrobe_options(&textures.join(GLOVES_DIR), &diffuse);
            let helmets = wardrobe_options(&textures.join(HELMET_DIR), &diffuse);
            let with_thumb = suits
                .iter()
                .chain(&gloves)
                .chain(&helmets)
                .filter(|o| o.thumbnail.is_some())
                .count();
            let total = suits.len() + gloves.len() + helmets.len();
            eprintln!(
                "  {:26} {:>6} {:>7} {:>8}  {with_thumb}/{total}",
                mannequin.file_stem().unwrap_or_default().to_string_lossy(),
                suits.len(),
                gloves.len(),
                helmets.len()
            );
        }
    }
}
