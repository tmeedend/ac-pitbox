//! Import des presets de grille de Content Manager (§6).
//!
//! **Import, jamais synchronisation.** On lit une fois, on convertit en objet
//! Pit Box, et on ne dépend plus jamais du fichier. Une passerelle vivante
//! rendrait l'app dépendante du format de CM — ce que la spec refuse déjà pour
//! la base en ligne d'AcTools. CM garde ses fichiers, Pit Box a ses copies.
//!
//! ## Le format, relevé et non déduit
//!
//! Les presets vivent sous `%LocalAppData%\AcTools Content Manager\Presets\`,
//! et deux sous-dossiers nous intéressent :
//!
//! - `Race Grids\` — un `.cmpreset` y est **exactement** l'objet `RaceGrid`,
//!   JSON brut sans en-tête ;
//! - `Quick Drive\` — un preset de session, dont `ModeData` est du JSON **en
//!   chaîne**, qui contient lui-même `RaceGridSerialized`, du JSON en chaîne
//!   une seconde fois. La grille d'une session s'importe donc aussi, ce qui
//!   double la matière sans une ligne de plus.
//!
//! Les deux sont parcourus **récursivement** : CM range ses presets dans une
//! arborescence de dossiers (§5.2 explique pourquoi Pit Box ne la reprend pas).
//!
//! ## Ce qui n'est pas importable, et pourquoi
//!
//! Seul `ModeId == "manual"` donne une grille. Les autres modes de CM
//! (`same_car`, `similar_p_w_ratio`, `same_subclass_only`…) ne décrivent pas un
//! plateau mais une **règle de tirage** : leurs `CarIds` sont des candidats, pas
//! des lignes. En faire une grille inventerait un plateau que l'utilisateur n'a
//! jamais composé. Ils sont donc comptés et nommés dans le rapport, pas
//! convertis — et Pit Box sait déjà exprimer ces règles, en mieux, avec ses
//! jetons de filtre.

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

/// Ce qu'un fichier de preset a donné.
#[derive(Debug, Clone, Serialize)]
pub struct CmGrid {
    /// Nom du fichier sans extension — celui que CM affiche.
    pub name: String,
    /// D'où il vient, pour le rapport : le chemin relatif au dossier Presets.
    pub source: String,
    /// Une ligne par adversaire, dans l'ordre du fichier.
    pub opponents: Vec<CmOpponent>,
    pub ai_level_min: u32,
    pub ai_level_max: u32,
    pub aggression: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct CmOpponent {
    pub car_id: String,
    pub car_skin: Option<String>,
    /// `None` = `Auto` — le `-1` de CM.
    pub ai_level: Option<u32>,
    pub driver_name: Option<String>,
    pub nationality: Option<String>,
    pub ballast: u32,
    pub restrictor: u32,
}

/// Un preset lu mais non convertible, avec la raison — le rapport le nomme
/// plutôt que de le taire (§6.3).
#[derive(Debug, Clone, Serialize)]
pub struct CmSkipped {
    pub name: String,
    pub source: String,
    /// Clé i18n : `errors.cmNotAGrid` (mode de tirage) ou `errors.cmUnreadable`.
    pub reason: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct CmScan {
    pub grids: Vec<CmGrid>,
    pub skipped: Vec<CmSkipped>,
    /// Dossier réellement parcouru, `None` quand Content Manager n'est pas
    /// installé — ce qui n'est pas une erreur, juste un non-résultat.
    pub root: Option<String>,
}

/// `%LocalAppData%\AcTools Content Manager\Presets`.
pub fn presets_dir() -> Option<PathBuf> {
    let dir = dirs::data_local_dir()?.join("AcTools Content Manager").join("Presets");
    dir.is_dir().then_some(dir)
}

/// Tous les `.cmpreset` d'un sous-dossier, récursivement. Un dossier absent
/// rend une liste vide : CM peut très bien n'avoir jamais enregistré de grille.
fn cmpresets(root: &Path) -> Vec<PathBuf> {
    if !root.is_dir() {
        return Vec::new();
    }
    walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .map(|e| e.into_path())
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("cmpreset")))
        .collect()
}

/// Un nombre du preset, sous les **trois** formes que CM y met.
///
/// Les tableaux par ligne sont en chaînes (`"74"`) et les valeurs globales en
/// flottants (`95.0`) — deux écritures dans le même fichier, ce qu'un
/// `as_i64()` seul laisse passer en silence : il rend `None` sur `95.0`, donc
/// la fourchette de difficulté d'un preset importé retombait sur le défaut au
/// lieu d'être lue. Trouvé par le test sur le preset de référence.
fn cell_number(v: Option<&Value>) -> Option<i64> {
    match v? {
        Value::String(s) => s.trim().parse::<f64>().ok().map(|f| f.round() as i64),
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f.round() as i64)),
        _ => None,
    }
}

/// `-1` est le `Auto` de CM, et il ne se confond pas avec `0` : le preset de
/// référence montre les deux côte à côte dans `AiAggressions`.
fn optional_level(v: Option<&Value>) -> Option<u32> {
    match cell_number(v)? {
        n if n < 0 => None,
        n => Some(n.clamp(0, 100) as u32),
    }
}

fn cell_text(v: Option<&Value>) -> Option<String> {
    match v? {
        Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
        _ => None,
    }
}

fn at<'a>(grid: &'a Value, key: &str, i: usize) -> Option<&'a Value> {
    grid.get(key)?.as_array()?.get(i)
}

/// Convertit un objet `RaceGrid` en grille Pit Box. `None` quand ce n'est pas
/// un plateau explicite (voir l'en-tête du module).
fn convert(grid: &Value, name: &str, source: &str) -> Option<CmGrid> {
    if grid.get("ModeId").and_then(Value::as_str) != Some("manual") {
        return None;
    }
    let car_ids = grid.get("CarIds")?.as_array()?;
    if car_ids.is_empty() {
        return None;
    }
    let opponents = car_ids
        .iter()
        .enumerate()
        .filter_map(|(i, id)| {
            Some(CmOpponent {
                car_id: id.as_str()?.to_string(),
                car_skin: cell_text(at(grid, "SkinIds", i)),
                ai_level: optional_level(at(grid, "AiLevels", i)),
                driver_name: cell_text(at(grid, "Names", i)),
                nationality: cell_text(at(grid, "Nationalities", i)),
                ballast: cell_number(at(grid, "Ballasts", i)).unwrap_or(0).clamp(0, 100) as u32,
                restrictor: cell_number(at(grid, "Restrictors", i)).unwrap_or(0).clamp(0, 100) as u32,
            })
        })
        .collect::<Vec<_>>();
    if opponents.is_empty() {
        return None;
    }
    // `AiLevel` est le HAUT de la fourchette et `AiLevelMin` le bas — relevé sur
    // un preset réel (95 avec un minimum de 85), pas déduit du nom.
    let max = cell_number(grid.get("AiLevel")).unwrap_or(100).clamp(0, 100) as u32;
    let min = cell_number(grid.get("AiLevelMin")).unwrap_or(70).clamp(0, 100) as u32;
    Some(CmGrid {
        name: name.to_string(),
        source: source.to_string(),
        opponents,
        ai_level_min: min.min(max),
        ai_level_max: max.max(min),
        aggression: cell_number(grid.get("AiAggression")).unwrap_or(0).clamp(0, 100) as u32,
    })
}

/// Extrait l'objet `RaceGrid` d'un fichier, quel que soit son emplacement.
///
/// Un preset de `Race Grids\` **est** cet objet ; un preset de `Quick Drive\`
/// l'enfouit sous deux couches de JSON-dans-une-chaîne. On tente donc le
/// déballage d'abord, et on retombe sur le fichier lui-même.
fn race_grid_of(root: &Value) -> Option<Value> {
    if let Some(mode_data) = root.get("ModeData").and_then(Value::as_str) {
        let mode: Value = serde_json::from_str(mode_data).ok()?;
        let serialized = mode.get("RaceGridSerialized")?.as_str()?;
        return serde_json::from_str(serialized).ok();
    }
    root.get("ModeId").is_some().then(|| root.clone())
}

/// Parcourt les presets de Content Manager et rend tout ce qui a pu être lu.
///
/// **L'import aboutit toujours** (§6.3) : un fichier illisible est nommé dans
/// le rapport, il n'interrompt pas le reste. Un utilisateur qui a cinquante
/// presets et un fichier corrompu doit récupérer les quarante-neuf autres.
pub fn scan() -> CmScan {
    let Some(root) = presets_dir() else {
        return CmScan::default();
    };
    let mut out = CmScan {
        root: Some(root.display().to_string()),
        ..Default::default()
    };
    for sub in ["Race Grids", "Quick Drive"] {
        for path in cmpresets(&root.join(sub)) {
            let name = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "?".into());
            let source = path.strip_prefix(&root).unwrap_or(&path).display().to_string();
            let parsed = std::fs::read_to_string(&path)
                .ok()
                .and_then(|text| serde_json::from_str::<Value>(text.trim_start_matches('\u{feff}')).ok());
            let Some(grid) = parsed.as_ref().and_then(race_grid_of) else {
                out.skipped.push(CmSkipped {
                    name,
                    source,
                    reason: "errors.cmUnreadable".into(),
                });
                continue;
            };
            match convert(&grid, &name, &source) {
                Some(g) => out.grids.push(g),
                None => out.skipped.push(CmSkipped {
                    name,
                    source,
                    reason: "errors.cmNotAGrid".into(),
                }),
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Le preset de référence, recopié verbatim depuis
    /// `Presets\Race Grids\pitbox-example.cmpreset`. C'est lui qui a donné les
    /// noms des six tableaux, le fait que leurs valeurs sont des chaînes, et
    /// les deux écritures de `Auto`.
    const REFERENCE: &str = r#"{"ModeId":"manual","FilterValue":"","RandomSkinsFilter":"Lot","SequentialSkins":true,"CarIds":["spear_lamborghini_lp640_veilside","lotus_evora_gte_carbon","lotus_evora_s_s2","lotus_exos_125","ks_lotus_25"],"AiLevels":["74","100","74","-1","70"],"AiAggressions":["67","100","0","-1","-1"],"Ballasts":["0","0","100","0","0"],"Restrictors":["0","0","0","100","0"],"PlayerBallast":5.0,"PlayerRestrictor":10.0,"Names":["Bob","Billy","Joe","Martin",null],"Nationalities":["Argentina","Brunei Darussalam",null,"Bhutan",null],"ShuffleCandidates":false,"VarietyLimitation":0,"OpponentsNumber":2,"StartingPosition":3,"AiLevel":95.0,"AiLevelMin":85.0,"AiLevelArrangeRandom":0.0,"AiLevelArrangeReverse":false,"AiLevelArrangePowerRatio":false,"AiAggression":0.0,"AiAggressionMin":0.0,"AiAggressionArrangeRandom":0.0,"AiAggressionArrangeReverse":false}"#;

    /// Règle protégée : les six tableaux par ligne se relisent dans le bon
    /// ordre, `-1` devient `Auto` sur un nombre et `null` sur un texte.
    #[test]
    fn the_reference_preset_converts_line_by_line() {
        let v: Value = serde_json::from_str(REFERENCE).unwrap();
        let g = convert(&v, "pitbox-example", "Race Grids/pitbox-example.cmpreset").unwrap();
        assert_eq!(g.opponents.len(), 5, "une ligne par CarId");
        assert_eq!(g.opponents[0].ai_level, Some(74));
        assert_eq!(g.opponents[3].ai_level, None, "-1 vaut Auto");
        assert_eq!(g.opponents[0].driver_name.as_deref(), Some("Bob"));
        assert_eq!(g.opponents[4].driver_name, None, "null vaut Auto");
        assert_eq!(g.opponents[1].nationality.as_deref(), Some("Brunei Darussalam"));
        assert_eq!(g.opponents[2].nationality, None);
        assert_eq!(g.opponents[2].ballast, 100);
        assert_eq!(g.opponents[3].restrictor, 100);
        assert_eq!(g.opponents[0].ballast, 0, "0 est un zéro, pas un Auto");
        // `AiLevel` est le haut, `AiLevelMin` le bas.
        assert_eq!((g.ai_level_min, g.ai_level_max), (85, 95));
    }

    /// Règle protégée : un mode de tirage n'est pas un plateau. Ses `CarIds`
    /// sont des candidats, et en faire une grille inventerait un plateau que
    /// personne n'a composé.
    #[test]
    fn a_draw_mode_is_not_a_grid() {
        let v: Value = serde_json::json!({
            "ModeId": "similar_p_w_ratio",
            "CarIds": ["ks_praga_r1"],
            "OpponentsNumber": 11,
        });
        assert!(convert(&v, "n", "s").is_none());
    }

    /// Règle protégée : la grille d'un preset de session se lit aussi, sous
    /// ses deux couches de JSON-dans-une-chaîne. C'est ce qui double la matière
    /// d'un import sans une ligne de conversion de plus.
    #[test]
    fn a_session_preset_yields_the_grid_it_carries() {
        let quick_drive = serde_json::json!({
            "Mode": "/Pages/Drive/QuickDrive_Race.xaml",
            "ModeData": serde_json::json!({ "RaceGridSerialized": REFERENCE }).to_string(),
        });
        let grid = race_grid_of(&quick_drive).unwrap();
        assert_eq!(convert(&grid, "n", "s").unwrap().opponents.len(), 5);
    }

    /// Règle protégée : un fichier illisible n'interrompt rien. Un utilisateur
    /// qui a cinquante presets et un fichier corrompu récupère les
    /// quarante-neuf autres.
    #[test]
    fn a_broken_file_is_reported_not_thrown() {
        let dir = crate::testutil::temp_dir("cm-import");
        let root = dir.join("Race Grids");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("good.cmpreset"), REFERENCE).unwrap();
        std::fs::write(root.join("broken.cmpreset"), "{not json").unwrap();
        let files = cmpresets(&root);
        assert_eq!(files.len(), 2, "les deux fichiers sont vus");
        let ok = files
            .iter()
            .filter(|p| {
                std::fs::read_to_string(p)
                    .ok()
                    .and_then(|t| serde_json::from_str::<Value>(&t).ok())
                    .is_some()
            })
            .count();
        assert_eq!(ok, 1, "un seul se relit, et ça suffit à l'import");
    }

    /// Règle protégée : CM range ses presets dans une arborescence, donc le
    /// parcours est récursif — sinon un utilisateur organisé n'importerait rien.
    #[test]
    fn presets_are_found_inside_subfolders() {
        let dir = crate::testutil::temp_dir("cm-import-tree");
        let deep = dir.join("Race Grids").join("GT3").join("2016");
        std::fs::create_dir_all(&deep).unwrap();
        std::fs::write(deep.join("field.cmpreset"), REFERENCE).unwrap();
        assert_eq!(cmpresets(&dir.join("Race Grids")).len(), 1);
    }
}
