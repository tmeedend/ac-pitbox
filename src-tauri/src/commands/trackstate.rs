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

#[tauri::command]
pub fn track_states() -> Vec<TrackStateOption> {
    let mut out = vec![crate::quickdrive::weather_state_option()];
    out.extend(crate::quickdrive::TRACK_STATES.iter().map(|s| TrackStateOption {
        start: s.start,
        name: s.name.to_string(),
        transfer: s.transfer,
        randomness: s.randomness,
        lap_gain: s.lap_gain,
        description: s.description.to_string(),
        weather_defined: false,
    }));
    out
}
