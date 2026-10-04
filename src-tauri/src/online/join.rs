//! Joining a server through Content Manager (`docs/online-join-research.md`).
//!
//! `acmanager://race/online` and not `race/online/join`: the latter only opens
//! CM's own server page, where the car has to be picked again. `race/online`
//! takes the car and starts the game at once, through the same pipeline as
//! Quick Drive (CSP's automatic downloads included). What it leaves out, and
//! CM's native join does, is `[REMOTE] __FEATURES` — without which CSP sends no
//! Steam ticket and an AssettoServer refuses us — so it is written back into
//! `race.ini` right after CM, like the player's skin offline (`raceini.rs`).
//!
//! A booking server is the exception: `race/online` does not book a slot, so
//! it goes through CM's page, where booking lives.

use std::path::Path;

use rusqlite::Connection;
use serde::Deserialize;

use crate::config::AppConfig;
use crate::http::encode_query_value;
use crate::modscan::ModKind;

/// What the screen sends to join.
#[derive(Debug, Clone, Deserialize)]
pub struct JoinRequest {
    pub ip: String,
    pub port: u16,
    pub http_port: u16,
    pub car_id: String,
    /// The track folder, to put it in the game if it is only in the library.
    pub track_id: String,
    /// `folder-layout`, the id CM resolves itself (`TrackRef::kunos_id`).
    pub track_kunos_id: String,
    pub password: Option<String>,
    pub booking: bool,
    /// The server's other cars, laid in the game with the one driven: the
    /// game loads them all. On a booking server, every car (the one driven is
    /// picked in CM).
    #[serde(default)]
    pub other_cars: Vec<String>,
    /// Layers of this car or track to deactivate for the session
    /// (`session_layers.rs`), as the panel let the user choose.
    #[serde(default)]
    pub set_aside: Vec<String>,
}

/// The URI handed to Content Manager. Every value is percent-encoded: a
/// password is free text, and an `&` in it would start a new parameter.
pub fn join_uri(request: &JoinRequest) -> String {
    let mut params = vec![("ip", request.ip.clone()), ("httpPort", request.http_port.to_string())];
    let route = if request.booking {
        "race/online/join"
    } else {
        params.extend([
            ("port", request.port.to_string()),
            ("car", request.car_id.clone()),
            ("track", request.track_kunos_id.clone()),
        ]);
        "race/online"
    };
    if let Some(password) = request.password.as_deref().filter(|p| !p.is_empty()) {
        params.push(("plainPassword", password.to_string()));
    }
    let query: Vec<String> = params
        .iter()
        .map(|(key, value)| format!("{key}={}", encode_query_value(value)))
        .collect();
    format!("acmanager://{route}?{}", query.join("&"))
}

/// The network half of a join, run without the SQLite lock: Steam has to be
/// there, and the server's features are read at the last moment — they are
/// the server's, not what the panel read a minute before.
///
/// A server that does not answer is joined without features: a vanilla one
/// has none, and CM will say better than us if it is down.
pub fn prepare(request: &JoinRequest) -> Result<Vec<String>, String> {
    if !crate::launch::steam_running() {
        return Err(crate::errors::STEAM_NOT_RUNNING.into());
    }
    let steam_id = super::active_steam_id().ok_or(crate::errors::STEAM_NOT_SIGNED_IN)?;
    if request.booking {
        return Ok(Vec::new());
    }
    Ok(
        match super::server::fetch_entry_list(&request.ip, request.http_port, steam_id) {
            Ok(entries) => entries.features,
            Err(e) => {
                log::warn!(
                    "online: no entry list from {}:{} ({e}), joining without features",
                    request.ip,
                    request.http_port
                );
                Vec::new()
            }
        },
    )
}

/// The layers of `set_aside` that belong to the car or the track joined —
/// the only ones a join may touch. The ids come from the screen: anything
/// else is refused and logged rather than trusted.
fn own_layers(conn: &Connection, request: &JoinRequest) -> Vec<String> {
    request
        .set_aside
        .iter()
        .filter(|id| match crate::overlay::get_layer(conn, id) {
            Ok(Some(layer)) if layer.parent_id == request.car_id || layer.parent_id == request.track_id => true,
            other => {
                log::warn!("online: layer {id} is not on this car or track ({other:?}), left as it is");
                false
            }
        })
        .cloned()
        .collect()
}

/// Puts the car and the track in the game if needed, sets aside the layers
/// asked for, then hands the join to Content Manager. `features` come from
/// `prepare`; `config_dir` holds the list of layers to give back.
pub fn join(
    conn: &Connection,
    cfg: &AppConfig,
    config_dir: &Path,
    request: &JoinRequest,
    features: Vec<String>,
) -> Result<(), String> {
    let cm = crate::launch::cm_exe(cfg)?;

    // Same net as an offline session (ESPACE R5), then the same activation:
    // a car only in the library would be "missing" to CM. On a booking
    // server the car is picked in CM's page, so only the track is ours.
    if !request.booking {
        crate::skeleton::guard_mod(conn, &request.car_id)?;
        crate::launch::ensure_available(conn, cfg, ModKind::Car, &request.car_id)?;
    }
    // Every other car of the server too: one missing keeps the player out.
    for car in &request.other_cars {
        crate::skeleton::guard_mod(conn, car)?;
        crate::launch::ensure_available(conn, cfg, ModKind::Car, car)?;
    }
    crate::skeleton::guard_mod(conn, &request.track_id)?;
    crate::launch::ensure_available(conn, cfg, ModKind::Track, &request.track_id)?;

    // After the activation, which lays the layers in too: set aside now,
    // they would otherwise come back with it.
    super::session_layers::set_aside(conn, cfg, config_dir, &own_layers(conn, request))?;

    let uri = join_uri(request);
    if let Err(e) = crate::launch::spawn_cm(cm, Some(std::ffi::OsStr::new(&uri))) {
        // No game will start, so no game will end to give the layers back.
        super::session_layers::restore(conn, cfg, config_dir);
        return Err(format!("starting Content Manager: {e}"));
    }
    if !features.is_empty() {
        crate::raceini::spawn_remote_features_patcher(request.car_id.clone(), features);
    }

    // "Already driven" marker (§6), as for an offline session — the car only
    // when we know it, not on a booking server.
    let now = chrono::Local::now().to_rfc3339();
    let driven = if request.booking {
        vec![&request.track_id]
    } else {
        vec![&request.car_id, &request.track_id]
    };
    for id in driven {
        if let Err(e) = crate::overlay::mark_launched(conn, id, &now) {
            log::warn!("online: cannot mark {id} as driven — {e}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> JoinRequest {
        JoinRequest {
            ip: "5.9.255.37".into(),
            port: 9690,
            http_port: 8071,
            car_id: "ks_toyota_celica_st185".into(),
            track_id: "ks_nordschleife".into(),
            track_kunos_id: "ks_nordschleife-touristenfahrten".into(),
            password: None,
            booking: false,
            other_cars: Vec::new(),
            set_aside: Vec::new(),
        }
    }

    /// Rule (online-join-research.md): the join that connected in the game,
    /// parameter for parameter.
    #[test]
    fn the_uri_is_the_one_measured_in_game() {
        assert_eq!(
            join_uri(&request()),
            "acmanager://race/online?ip=5.9.255.37&httpPort=8071&port=9690\
             &car=ks_toyota_celica_st185&track=ks_nordschleife-touristenfahrten"
        );
    }

    /// Rule: a password is free text — encoded, so it cannot add parameters.
    #[test]
    fn a_password_cannot_break_the_uri() {
        let uri = join_uri(&JoinRequest {
            password: Some("a&car=other".into()),
            ..request()
        });
        assert!(uri.ends_with("&plainPassword=a%26car%3Dother"), "{uri}");
        assert_eq!(uri.matches("car=").count(), 1, "no second car parameter");
    }

    /// Rule: a booking server goes to CM's page, which books; the car is
    /// chosen there.
    #[test]
    fn a_booking_server_opens_cm_page() {
        let uri = join_uri(&JoinRequest {
            booking: true,
            ..request()
        });
        assert_eq!(uri, "acmanager://race/online/join?ip=5.9.255.37&httpPort=8071");
    }
}
