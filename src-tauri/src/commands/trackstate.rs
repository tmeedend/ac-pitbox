//! États de piste offerts à l'écran de session (§2.2) — voir `quickdrive.rs`,
//! qui porte la table et l'envoie au preset.

use serde::Serialize;

/// Un état tel que l'écran le lit : un **nom** et ses quatre valeurs.
///
/// Le `start` sert d'identifiant parce que c'est le seul des quatre que le
/// réglage retient (`RaceSetup::grip`) — mais l'écran n'a plus à connaître la
/// liste, il la reçoit. C'est ce qui permettra au lot 4 d'y ajouter des états
/// d'une autre provenance sans toucher à l'écran.
#[derive(Debug, Clone, Serialize)]
pub struct TrackStateOption {
    /// `"builtin"` (table du jeu) ou `"cm"` (preset utilisateur). **Le nom seul
    /// ne distingue pas** : rien n'empêche d'appeler son preset « Green »
    /// (§4.6), et c'est le couple origine + nom qu'une session retient.
    pub origin: String,
    pub start: u32,
    pub name: String,
    pub transfer: u32,
    pub randomness: u32,
    pub lap_gain: u32,
    pub description: String,
    /// `true` pour l'entrée « Auto », qui n'est pas un état mais le drapeau
    /// `WeatherDefined` : ses quatre valeurs sont celles de Green, en repli.
    pub weather_defined: bool,
}

/// Les sept entrées du jeu, puis les presets utilisateur de Content Manager
/// (§4.1).
///
/// **Les natifs ne sont jamais remplacés ni masqués.** Ils viennent du fichier
/// du jeu, ce sont les noms que tout le monde emploie, et quelqu'un qui s'est
/// fabriqué une piste verte humide veut quand même pouvoir choisir `Optimum`.
/// Le second groupe est simplement absent quand le dossier est vide ou
/// introuvable — aucun message, c'est le cas nominal.
///
/// Appelé à **l'ouverture de l'écran** et non au démarrage de l'app (§4.3) : le
/// scénario réel est de créer un preset dans CM puis de revenir dans Pit Box,
/// et une lecture au démarrage obligerait à relancer.
#[tauri::command]
pub fn track_states() -> Vec<TrackStateOption> {
    let mut out = vec![crate::quickdrive::weather_state_option()];
    out.extend(crate::quickdrive::TRACK_STATES.iter().map(|s| TrackStateOption {
        origin: "builtin".into(),
        start: s.start,
        name: s.name.to_string(),
        transfer: s.transfer,
        randomness: s.randomness,
        lap_gain: s.lap_gain,
        description: s.description.to_string(),
        weather_defined: false,
    }));
    out.extend(crate::trackstates::cm_states());
    out
}
