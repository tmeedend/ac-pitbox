//! Presets d'état de piste **créés par l'utilisateur dans Content Manager**
//! (§4), lus et jamais écrits.
//!
//! ## Ce que le relevé a donné
//!
//! Le dossier est `%LocalAppData%\AcTools Content Manager\Presets\Track States`
//! — même racine que les grilles (`cmimport.rs`), un sous-dossier de plus. Il
//! existe sur la machine de référence (créé par CM) et il y est **vide** : le
//! cas nominal de ce lot est donc « aucun preset », et il ne produit ni message
//! ni erreur.
//!
//! **Le format est relevé sur un preset réel**, enregistré depuis CM pour
//! l'occasion. Un `.cmpreset` de ce dossier est exactement l'objet d'état, JSON
//! brut sans en-tête — comme un `.cmpreset` de `Race Grids` est exactement
//! l'objet `RaceGrid` :
//!
//! ```json
//! {"s":0.89,"t":0.8,"r":0.03,"g":50,"d":"Old tarmac. Bad grip won't get better soon.","w":false}
//! ```
//!
//! C'est exactement ce que `quickdrive::build_track_properties` écrit déjà :
//! **ce module en est l'inverse**, et c'est la meilleure garantie qu'ils
//! restent d'accord. `s`, `t` et `r` sont des pourcentages divisés par cent,
//! `g` est le `LAP_GAIN` brut.
//!
//! **L'échantillon est un duplicata de l'état natif `Old`**, ce qui en fait un
//! témoin : ses quatre valeurs doivent relire 89 / 80 / 3 / 50, les nombres que
//! `cfg/templates/tracks.ini` donne à cette entrée. L'échelle n'est donc pas
//! seulement supposée cohérente, elle est vérifiée contre des valeurs connues.
//!
//! L'identité d'un état est son **nom de fichier** : c'est ainsi que CM
//! lui-même le désigne (`TrackPropertiesPresetFilename`), y compris pour les
//! natifs — `Optimum.cmpreset`, qui n'existe pourtant pas sur disque, les
//! natifs étant virtuels.
//!
//! ## Tolérance
//!
//! Un fichier illisible, mal formé ou sans valeurs exploitables est **ignoré en
//! silence** : c'est un enrichissement optionnel, il ne doit jamais empêcher de
//! régler ni de lancer une session.

use std::path::Path;

use serde_json::Value;

use crate::commands::trackstate::TrackStateOption;

/// Le sous-dossier de CM. `None` quand CM n'est pas installé — un
/// non-résultat, pas une panne.
fn dir() -> Option<std::path::PathBuf> {
    let d = crate::cmimport::presets_dir()?.join("Track States");
    d.is_dir().then_some(d)
}

/// Un pourcentage écrit divisé par cent (`0.95` → 95).
///
/// **Bornage volontairement large.** L'instruction proposait la plage de
/// l'éditeur de CM (grip initial 85-100, lap gain 0-700), en demandant de la
/// confirmer par le relevé : il ne l'a pas confirmée, faute d'échantillon. Un
/// plancher non confirmé à 85 réécrirait en silence un état que l'utilisateur
/// a composé lui-même à 70 — précisément ce que ce lot est censé lui rendre.
/// On s'en tient donc à ce qui a un sens physique : un pourcentage tient entre
/// 0 et 100. Si CM ne descend jamais sous 85, aucun fichier réel n'atteindra ce
/// bornage, et il n'aura rien coûté.
fn percent(v: Option<&Value>) -> Option<u32> {
    let f = v?.as_f64()?;
    if !f.is_finite() {
        return None;
    }
    Some((f * 100.0).round().clamp(0.0, 100.0) as u32)
}

fn lap_gain(v: Option<&Value>) -> Option<u32> {
    let f = v?.as_f64()?;
    if !f.is_finite() {
        return None;
    }
    // Au moins 1 : « zéro tour pour gagner un point de grip » ne veut rien dire,
    // et le plafond n'est pas borné faute de l'avoir relevé.
    Some(f.round().max(1.0) as u32)
}

fn text(v: Option<&Value>) -> Option<String> {
    match v? {
        Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
        _ => None,
    }
}

/// Convertit un objet d'état en entrée de la liste. `None` dès qu'une des
/// quatre valeurs manque : un état à trois nombres n'est pas un état.
fn convert(obj: &Value, name: &str) -> Option<TrackStateOption> {
    Some(TrackStateOption {
        start: percent(obj.get("s"))?,
        name: name.to_string(),
        transfer: percent(obj.get("t"))?,
        randomness: percent(obj.get("r"))?,
        lap_gain: lap_gain(obj.get("g"))?,
        description: text(obj.get("d")).unwrap_or_default(),
        // `w` est le drapeau `WeatherDefined` du natif « Auto ». Un preset
        // utilisateur qui le porterait est repris tel quel — c'est son choix.
        weather_defined: obj.get("w").and_then(Value::as_bool).unwrap_or(false),
        origin: "cm".into(),
    })
}

fn read_dir(root: &Path) -> Vec<TrackStateOption> {
    let mut out: Vec<TrackStateOption> = walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .map(|e| e.into_path())
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("cmpreset")))
        .filter_map(|p| {
            // Le **nom de fichier** est l'identité, c'est ainsi que CM lui-même
            // désigne un état (`TrackPropertiesPresetFilename`).
            let name = p.file_stem()?.to_string_lossy().to_string();
            let text = std::fs::read_to_string(&p).ok()?;
            let root: Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()?;
            convert(&root, &name)
        })
        .collect();
    out.sort_by_key(|s| s.name.to_lowercase());
    out
}

/// Les presets utilisateur, ordre alphabétique. Liste vide quand le dossier est
/// absent ou vide — le cas nominal.
pub fn cm_states() -> Vec<TrackStateOption> {
    dir().map(|d| read_dir(&d)).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Règle protégée : ce module est l'**inverse exact** de ce que
    /// `build_track_properties` écrit. Les trois premiers sont des pourcentages
    /// divisés par cent, le quatrième est brut — se tromper d'échelle sur un
    /// seul des quatre donnerait un état plausible et faux.
    #[test]
    fn a_state_reads_back_at_the_scale_content_manager_writes() {
        let obj: Value =
            serde_json::from_str(r#"{"s":0.95,"t":0.9,"r":0.02,"g":132,"d":"A clean track.","w":false}"#).unwrap();
        let st = convert(&obj, "Wet morning").unwrap();
        assert_eq!((st.start, st.transfer, st.randomness), (95, 90, 2), "pourcentages ×100");
        assert_eq!(st.lap_gain, 132, "lap gain brut");
        assert_eq!(st.description, "A clean track.");
        assert_eq!(st.name, "Wet morning", "le nom vient du fichier");
        assert_eq!(st.origin, "cm");
    }

    /// Règle protégée : le fichier **réel**, recopié verbatim depuis
    /// `Presets\Track States\grip-example-for-pitbox.cmpreset`.
    ///
    /// C'est un duplicata de l'état natif `Old`, donc il sert de **témoin** :
    /// ses quatre valeurs doivent relire exactement ce que
    /// `cfg/templates/tracks.ini` donne à cette entrée. Une erreur d'échelle sur
    /// un seul des quatre donnerait un état plausible et faux — c'est le genre
    /// de défaut qui ne se voit qu'au volant.
    #[test]
    fn the_reference_file_reads_back_as_the_builtin_it_duplicates() {
        const REAL: &str =
            r#"{"s":0.89,"t":0.8,"r":0.03,"g":50,"d":"Old tarmac. Bad grip won't get better soon.","w":false}"#;
        let obj: Value = serde_json::from_str(REAL).unwrap();
        let st = convert(&obj, "grip-example-for-pitbox").unwrap();
        let old = crate::quickdrive::TRACK_STATES
            .iter()
            .find(|s| s.name == "Old")
            .expect("l'état natif Old");
        assert_eq!(
            (st.start, st.transfer, st.randomness, st.lap_gain),
            (old.start, old.transfer, old.randomness, old.lap_gain),
            "relu à l'identique de l'état qu'il duplique"
        );
        assert_eq!(st.description, old.description);
        assert_eq!(st.name, "grip-example-for-pitbox", "le nom vient du fichier");
    }

    /// Règle protégée : l'arrondi, et non la troncature.
    ///
    /// `0.29 * 100.0` vaut `28.999999999999996` en flottant : un `as u32` seul
    /// rendrait **28**. Le fichier de référence n'expose pas le défaut (`0.8`
    /// tombe du bon côté), donc rien n'aurait signalé sa réintroduction — un
    /// état relu un point plus glissant qu'il n'a été composé.
    #[test]
    fn a_percentage_is_rounded_not_truncated() {
        let obj: Value = serde_json::from_str(r#"{"s":0.29,"t":0.71,"r":0.07,"g":40,"d":"","w":false}"#).unwrap();
        let st = convert(&obj, "x").unwrap();
        assert_eq!((st.start, st.transfer, st.randomness), (29, 71, 7));
    }

    /// Règle protégée : un fichier cassé est ignoré, jamais fatal — les autres
    /// doivent arriver. Un enrichissement optionnel n'empêche pas de lancer une
    /// session.
    #[test]
    fn a_broken_file_is_skipped_and_the_others_still_arrive() {
        let dir = crate::testutil::temp_dir("track-states");
        std::fs::write(
            dir.join("Damp.cmpreset"),
            r#"{"s":0.7,"t":0.5,"r":0.05,"g":90,"d":"","w":false}"#,
        )
        .unwrap();
        std::fs::write(dir.join("broken.cmpreset"), "{not json").unwrap();
        // Trois nombres sur quatre : pas un état.
        std::fs::write(dir.join("partial.cmpreset"), r#"{"s":0.9,"t":0.8,"r":0.01}"#).unwrap();
        let states = read_dir(&dir);
        assert_eq!(states.len(), 1, "seul le fichier complet est retenu");
        assert_eq!(states[0].name, "Damp");
        assert_eq!(
            states[0].start, 70,
            "pas de plancher à 85 : c'est son état, pas le nôtre"
        );
    }

    /// Lit le **vrai** dossier de Content Manager de la machine, et affiche ce
    /// qui en sort. Ignoré par défaut, comme tout ce qui dépend d'un état
    /// extérieur au dépôt :
    ///
    /// ```text
    /// cargo test --lib trackstates::tests::what_content_manager_really_holds -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "dépend du dossier Content Manager de la machine"]
    fn what_content_manager_really_holds() {
        match dir() {
            None => println!("(pas de dossier Track States)"),
            Some(d) => {
                println!("dossier : {}", d.display());
                for s in read_dir(&d) {
                    println!(
                        "  {:<28} s={} t={} r={} g={} w={} d={:?}",
                        s.name, s.start, s.transfer, s.randomness, s.lap_gain, s.weather_defined, s.description
                    );
                }
            }
        }
    }

    /// Règle protégée : ordre alphabétique, insensible à la casse — une liste
    /// qui dépend de l'ordre du système de fichiers se réorganise toute seule
    /// d'une machine à l'autre.
    #[test]
    fn user_presets_come_out_in_alphabetical_order() {
        let dir = crate::testutil::temp_dir("track-states-order");
        for n in ["zebra", "Alpha", "monsoon"] {
            std::fs::write(
                dir.join(format!("{n}.cmpreset")),
                r#"{"s":0.9,"t":0.8,"r":0.01,"g":100,"d":"","w":false}"#,
            )
            .unwrap();
        }
        let names: Vec<_> = read_dir(&dir).into_iter().map(|s| s.name).collect();
        assert_eq!(names, vec!["Alpha", "monsoon", "zebra"]);
    }
}
