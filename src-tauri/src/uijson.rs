//! Lecture **strictement en lecture seule** des `ui_car.json` / `ui_track.json`.
//!
//! Règle d'or (§3.0) : ces fichiers ne sont JAMAIS réécrits. On les lit comme
//! une entrée du pipeline, jamais comme une sortie.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

/// Champs natifs exploités à l'import (L1). Les specs/courbes (fiche technique)
/// relèvent de L2 et seront lues plus tard.
/// `class` et `country` sont lus dès maintenant mais consommés en L2
/// (extraction classe race/street et pays).
#[derive(Debug, Clone, Default, Serialize)] // On ajoute Serialize et Deserialize ici
#[allow(dead_code)]
pub struct UiInfo {
    pub name: Option<String>,
    pub brand: Option<String>,
    pub year: Option<i64>,
    pub version: Option<String>,
    pub author: Option<String>,
    pub class: Option<String>,
    pub country: Option<String>,
    /// Tags bruts lus dans le fichier (origine « fichier mod », lecture seule).
    pub tags: Vec<String>,
}

/// Lit un fichier texte en tolérant les mods dont l'encodage n'est pas de
/// l'UTF-8 strict (Windows-1252/Latin-1, octets invalides…) : on retente en
/// lossy si la lecture stricte échoue, plutôt que d'abandonner silencieusement.
fn read_text_lossy(path: &Path) -> Option<String> {
    match fs::read_to_string(path) {
        Ok(s) => Some(s),
        Err(_) => {
            let bytes = fs::read(path).ok()?;
            Some(String::from_utf8_lossy(&bytes).into_owned())
        }
    }
}

fn read_json(path: &Path) -> Option<Value> {
    let text = read_text_lossy(path)?;
    // Tolère un BOM UTF-8 en tête (fréquent sur les mods).
    let text = text.trim_start_matches('\u{feff}');
    // Nettoie les JSON malformés fréquents chez les moddeurs AC (retours à la
    // ligne bruts collés dans une chaîne — ex. description — invalident tout
    // le fichier pour un parseur JSON strict ; voir clean_assetto_json).
    let text = clean_assetto_json(text);
    serde_json::from_str(&text).ok()
}

/// Convertit une valeur JSON en chaîne (gère nombre ou chaîne).
fn as_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => {
            let t = s.trim();
            if t.is_empty() {
                None
            } else {
                Some(t.to_string())
            }
        }
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn parse(path: &Path) -> Option<UiInfo> {
    let v = read_json(path)?;

    let year = v.get("year").and_then(|y| match y {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => s.trim().parse::<i64>().ok(),
        _ => None,
    });

    let tags = v
        .get("tags")
        .and_then(|t| t.as_array())
        .map(|arr| arr.iter().filter_map(as_string).collect())
        .unwrap_or_default();

    Some(UiInfo {
        name: v.get("name").and_then(as_string),
        brand: v.get("brand").and_then(as_string),
        year,
        version: v.get("version").and_then(as_string),
        author: v.get("author").and_then(as_string),
        class: v.get("class").and_then(as_string),
        country: v.get("country").and_then(as_string),
        tags,
    })
}

fn clean_assetto_json(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut in_string = false;
    let chars = input.chars().peekable();

    for c in chars {
        if c == '"' {
            in_string = !in_string;
            result.push(c);
        } else if in_string {
            // À l'intérieur d'une chaîne (Description, tags, etc.)
            match c {
                '\n' | '\r' => result.push(' '),
                '\u{a0}' => result.push(' '),            // Espace insécable
                _ if c.is_control() => result.push(' '), // Caractère invisible crash-test
                _ => result.push(c),
            }
        } else {
            // À l'extérieur d'une chaîne (Structure JSON)
            match c {
                // On garde les sauts de ligne et espaces standards hors-chaîne,
                // mais on vire TOUS les caractères de contrôle bizarres cachés
                _ if c.is_control() && c != '\n' && c != '\r' && c != '\t' => {}
                _ => result.push(c),
            }
        }
    }

    // Nettoyage final des virgules traînantes
    let re_trailing_comma = regex::Regex::new(r",\s*([\]}])").unwrap();
    re_trailing_comma.replace_all(&result, "$1").into_owned()
}

/// Chemin du `ui_car.json` d'un dossier voiture.
pub fn car_ui_path(car_dir: &Path) -> PathBuf {
    car_dir.join("ui").join("ui_car.json")
}

/// Chemin du `ui_track.json` d'un circuit : à la racine `ui/`, sinon dans le
/// premier sous-dossier de layout qui en contient un.
pub fn track_ui_path(track_dir: &Path) -> Option<PathBuf> {
    let ui = track_dir.join("ui");
    let root = ui.join("ui_track.json");
    if root.is_file() {
        return Some(root);
    }
    let entries = std::fs::read_dir(&ui).ok()?;
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            let layout_json = p.join("ui_track.json");
            if layout_json.is_file() {
                return Some(layout_json);
            }
        }
    }
    None
}

pub fn read_car(car_dir: &Path) -> Option<UiInfo> {
    parse(&car_ui_path(car_dir))
}

pub fn read_track(track_dir: &Path) -> Option<UiInfo> {
    parse(&track_ui_path(track_dir)?)
}

/// Nom lisible d'un showroom (`content/showroom/<id>/ui/ui_showroom.json`) —
/// `None` si le fichier est absent ou muet, l'appelant retombe alors sur l'id
/// du dossier. Passe par le lecteur tolérant : ces fichiers Kunos sont indentés
/// à la tabulation et truffés d'espaces en fin de ligne.
pub fn read_showroom_name(showroom_dir: &Path) -> Option<String> {
    let v = read_json(&showroom_dir.join("ui").join("ui_showroom.json"))?;
    as_string(v.get("name")?)
}

/// Fiche technique native d'une voiture (§5bis.1), lue directement dans
/// `ui_car.json`. `specs` est un OBJET de chaînes déjà formatées (pas de
/// parsing), les courbes sont des paires `[rpm, valeur]`. Lecture seule.
#[derive(Debug, Clone, Serialize, Default)]
pub struct NativeSpecs {
    pub bhp: Option<String>,
    pub torque: Option<String>,
    pub weight: Option<String>,
    pub topspeed: Option<String>,
    pub acceleration: Option<String>,
    pub pwratio: Option<String>,
    pub range: Option<String>,
    pub description: Option<String>,
    pub country: Option<String>,
    pub author: Option<String>,
    pub year: Option<i64>,
    pub power_curve: Vec<[f64; 2]>,
    pub torque_curve: Vec<[f64; 2]>,
}

fn curve(v: &Value, key: &str) -> Vec<[f64; 2]> {
    v.get(key)
        .and_then(|c| c.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|pt| {
                    let p = pt.as_array()?;
                    Some([p.first()?.as_f64()?, p.get(1)?.as_f64()?])
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Un layout de circuit avec ses images (§6.3 / §8.6).
#[derive(Debug, Clone, Serialize, Default)]
pub struct LayoutItem {
    /// Dossier du layout (vide si mono-layout).
    pub id: String,
    pub name: String,
    pub length: Option<String>,
    /// Image illustratrice (`preview.png`).
    pub preview: Option<String>,
    /// Tracé (`outline.png`/`map.png`).
    pub outline: Option<String>,
}

/// Fiche détaillée d'un circuit : description + layouts illustrés (§6.3).
#[derive(Debug, Clone, Serialize, Default)]
pub struct TrackDetail {
    pub description: Option<String>,
    pub layouts: Vec<LayoutItem>,
}

fn first_file(dir: &Path, names: &[&str]) -> Option<String> {
    names
        .iter()
        .map(|n| dir.join(n))
        .find(|p| p.is_file())
        .map(|p| p.to_string_lossy().into_owned())
}

const TRACK_PREVIEW: [&str; 2] = ["preview.png", "preview.jpg"];
const TRACK_OUTLINE: [&str; 3] = ["outline.png", "outline.jpg", "map.png"];

/// Lit la description et les layouts (avec images) d'un circuit. Gère le mono-
/// layout (`ui/`) et le multi-layout (`ui/<layout>/`). Lecture seule.
pub fn read_track_detail(track_dir: &Path) -> TrackDetail {
    let mut layouts = Vec::new();
    let mut description = None;

    let mut add = |dir: &Path, id: String| {
        let v = read_json(&dir.join("ui_track.json"));
        let name = v
            .as_ref()
            .and_then(|v| v.get("name").and_then(as_string))
            .unwrap_or_else(|| if id.is_empty() { "(défaut)".into() } else { id.clone() });
        if description.is_none() {
            description = v.as_ref().and_then(|v| v.get("description").and_then(as_string));
        }
        let length = v.as_ref().and_then(|v| v.get("length").and_then(as_string));
        layouts.push(LayoutItem {
            id,
            name,
            length,
            preview: first_file(dir, &TRACK_PREVIEW),
            outline: first_file(dir, &TRACK_OUTLINE),
        });
    };

    for (dir, id) in layout_dirs(track_dir) {
        add(&dir, id);
    }

    TrackDetail { description, layouts }
}

/// Dossiers `ui/` porteurs d'un `ui_track.json`, avec l'id de layout associé
/// (vide pour un circuit mono-layout, dont le fichier est à la racine `ui/`).
/// Ordre stable : racine d'abord, puis sous-dossiers triés.
fn layout_dirs(track_dir: &Path) -> Vec<(PathBuf, String)> {
    let ui = track_dir.join("ui");
    let mut out = Vec::new();
    if ui.join("ui_track.json").is_file() {
        out.push((ui.clone(), String::new()));
    }
    if let Ok(entries) = std::fs::read_dir(&ui) {
        let mut subs: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir() && p.join("ui_track.json").is_file())
            .collect();
        subs.sort();
        for p in subs {
            let lid = p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            out.push((p, lid));
        }
    }
    out
}

/// Native description of a track, **without** the image scan of
/// `read_track_detail`: the first layout that carries one (same arbitration as
/// the detail view). Called for every track of the library list (description
/// filter, §6.1), where also scanning each layout's previews would cost one
/// directory walk per track for nothing.
pub fn read_track_description(track_dir: &Path) -> Option<String> {
    layout_dirs(track_dir).into_iter().find_map(|(dir, _)| {
        read_json(&dir.join("ui_track.json"))?
            .get("description")
            .and_then(as_string)
    })
}

/// Nom d'un circuit tel qu'il doit s'afficher (§5bis.3) : la racine commune de
/// ses layouts quand il en a plusieurs, sinon le nom du seul qu'il a.
///
/// C'est ce qui remplace `read_track(dir).name`, qui rendait le nom du
/// **premier** layout — « Highlands Drift » pour un circuit qui s'appelle
/// « Highlands ». Ne lit que le champ `name` de chaque layout : pas de scan
/// des images, contrairement à `read_track_detail`.
pub fn read_track_name(track_dir: &Path) -> Option<String> {
    let names: Vec<String> = layout_dirs(track_dir)
        .iter()
        .filter_map(|(dir, _)| read_json(&dir.join("ui_track.json")))
        .filter_map(|v| v.get("name").and_then(as_string))
        .collect();
    // Repli sur le premier nom lisible : sans racine commune (des layouts qui
    // ne partagent rien), mieux vaut le comportement d'avant qu'aucun nom.
    common_layout_name(&names).or_else(|| names.first().cloned())
}

/// Racine commune des noms de layouts d'un circuit (§5bis.3).
///
/// Un circuit multi-layouts n'a pas de nom à lui : chaque `ui_track.json` de
/// layout porte le sien (« Highlands Drift », « Highlands Long », « Highlands
/// Short »…). On prenait jusqu'ici celui du **premier layout trouvé**, ce qui
/// affichait « Highlands Drift » pour tout le circuit — un nom faux, et qui
/// dépendait de l'ordre alphabétique des dossiers.
///
/// La racine se calcule **par mots entiers**, jamais caractère par caractère :
/// sur « Nordschleife » et « Nordschleife Tourist », une comparaison par
/// caractères s'arrêterait au milieu d'un mot dès que deux layouts divergent
/// (« Monza 1966 » / « Monza 1971 » donnerait « Monza 19 »). La ponctuation de
/// liaison laissée en bout (« Spa - GP » / « Spa - National » → « Spa - ») est
/// retirée, elle n'est là que pour séparer.
///
/// `None` quand les layouts n'ont aucun mot en commun : mieux vaut alors le
/// comportement d'avant (le nom du premier) qu'un nom vide — et de toute façon
/// l'utilisateur peut renommer.
pub fn common_layout_name(names: &[String]) -> Option<String> {
    let first = names.first()?;
    let reference: Vec<&str> = first.split_whitespace().collect();
    if reference.is_empty() {
        return None;
    }
    let mut shared = reference.len();
    for name in names.iter().skip(1) {
        let words: Vec<&str> = name.split_whitespace().collect();
        let mut common = 0;
        while common < shared && common < words.len() && words[common].eq_ignore_ascii_case(reference[common]) {
            common += 1;
        }
        shared = common;
        if shared == 0 {
            return None;
        }
    }
    let root = reference[..shared].join(" ");
    // `trim_end_matches` sur la ponctuation seule : un nom qui se termine
    // légitimement par un chiffre ou une lettre n'est pas touché.
    let root = root.trim_end_matches(|c: char| !c.is_alphanumeric()).trim();
    if root.is_empty() {
        None
    } else {
        Some(root.to_string())
    }
}

pub fn read_car_specs(car_dir: &Path) -> Option<NativeSpecs> {
    let v = read_json(&car_ui_path(car_dir))?;
    let specs = v.get("specs");
    let sget = |k: &str| specs.and_then(|s| s.get(k)).and_then(as_string);
    let year = v.get("year").and_then(|y| match y {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => s.trim().parse::<i64>().ok(),
        _ => None,
    });
    Some(NativeSpecs {
        bhp: sget("bhp"),
        torque: sget("torque"),
        weight: sget("weight"),
        topspeed: sget("topspeed"),
        acceleration: sget("acceleration"),
        pwratio: sget("pwratio"),
        range: sget("range"),
        description: v.get("description").and_then(as_string),
        country: v.get("country").and_then(as_string),
        author: v.get("author").and_then(as_string),
        year,
        power_curve: curve(&v, "powerCurve"),
        torque_curve: curve(&v, "torqueCurve"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reproduit un vrai `ui_car.json` de contenu de base (ex. lotus_exige_s) :
    /// la description contient des retours à la ligne bruts non échappés,
    /// invalides pour un parseur JSON strict — le fichier entier doit quand
    /// même être lu (specs + description), pas juste ignoré silencieusement.
    #[test]
    fn reads_specs_with_raw_newlines_in_description() {
        let dir = crate::testutil::temp_dir("uijson");
        std::fs::create_dir_all(dir.join("ui")).unwrap();
        std::fs::write(
            car_ui_path(&dir),
            "{\n\"name\": \"Lotus Exige S\",\n\"brand\": \"Lotus\",\n\"description\": \"Welcome to the world.\nSecond line.\nThird line.\",\n\"specs\": {\"bhp\": \"345bhp\", \"weight\": \"1176kg\"}\n}\n",
        )
        .unwrap();

        let specs = read_car_specs(&dir).expect("specs should parse despite raw newlines");
        assert!(specs.description.unwrap().contains("Welcome to the world."));
        assert_eq!(specs.bhp.as_deref(), Some("345bhp"));
        assert_eq!(specs.weight.as_deref(), Some("1176kg"));
    }

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    /// Règle : le nom d'un circuit multi-layouts est la racine commune de ses
    /// layouts, découpée sur des MOTS entiers (§5bis.3). Le cas signalé par
    /// l'utilisateur : « Highlands » affiché « Highlands Drift » parce qu'on
    /// prenait le premier layout venu.
    #[test]
    fn common_layout_name_cuts_on_whole_words() {
        assert_eq!(
            common_layout_name(&names(&[
                "Highlands Drift",
                "Highlands Long",
                "Highlands",
                "Highlands Short"
            ]))
            .as_deref(),
            Some("Highlands"),
            "racine commune, pas le nom du premier layout"
        );
        // Le piège d'une comparaison caractère par caractère : elle rendrait
        // « Monza 19 », qui n'est le nom de rien.
        assert_eq!(
            common_layout_name(&names(&["Monza 1966", "Monza 1971"])).as_deref(),
            Some("Monza"),
            "coupe sur un mot entier, jamais au milieu"
        );
        // La ponctuation de liaison ne survit pas seule en bout de racine.
        assert_eq!(
            common_layout_name(&names(&["Spa - GP", "Spa - National"])).as_deref(),
            Some("Spa"),
            "séparateur laissé en bout retiré"
        );
        // Un seul layout : son nom est déjà le nom du circuit.
        assert_eq!(
            common_layout_name(&names(&["Nordschleife"])).as_deref(),
            Some("Nordschleife"),
            "mono-layout : le nom tel quel"
        );
        // Le premier mot doit valoir pour TOUS, pas seulement pour deux d'affilée.
        assert_eq!(
            common_layout_name(&names(&["Highlands Drift", "Autre chose"])),
            None,
            "aucun mot commun : pas de racine inventée"
        );
        assert_eq!(common_layout_name(&[]), None, "aucun layout : rien à déduire");
    }

    /// Règle : la casse ne casse pas la racine — les auteurs de mods ne sont
    /// pas réguliers là-dessus (« Highlands short » à côté de « Highlands Long »).
    #[test]
    fn common_layout_name_ignores_case_differences() {
        assert_eq!(
            common_layout_name(&names(&["Highlands Long", "highlands short"])).as_deref(),
            Some("Highlands"),
            "casse du premier layout conservée pour l'affichage"
        );
    }

    /// Règle : `read_track_name` applique la racine commune sur une vraie
    /// arborescence multi-layouts — c'est ce qui remplace `read_track().name`
    /// à l'import comme au réindex.
    #[test]
    fn track_name_uses_the_common_root_of_its_layouts() {
        let dir = crate::testutil::temp_dir("trackname");
        for (layout, name) in [
            ("drift", "Highlands Drift"),
            ("long", "Highlands Long"),
            ("short", "Highlands Short"),
        ] {
            let d = dir.join("ui").join(layout);
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join("ui_track.json"), format!("{{\"name\":\"{name}\"}}")).unwrap();
        }
        assert_eq!(
            read_track_name(&dir).as_deref(),
            Some("Highlands"),
            "racine commune des 3 layouts"
        );
        // Contrôle : l'ancienne lecture rendait bien le premier layout venu,
        // c'est-à-dire le nom faux que ce test protège contre un retour en arrière.
        assert_eq!(
            read_track(&dir).and_then(|u| u.name).as_deref(),
            Some("Highlands Drift")
        );
    }
}
