//! One server, asked directly: `/INFO` for its live state, `/JSON` for its
//! entry list ("Ce qu'un serveur AC expose", levels 1 and 2, of
//! `SPEC-play-online.md`).
//!
//! The entry list is what the car choice rests on: one slot per car, free or
//! taken, each with the skin the server will impose — a requested skin is
//! ignored online, the slot's is loaded (measured, `online-join-research.md`).

use std::path::Path;

use serde::Serialize;
use serde_json::Value;

use crate::http;

use super::installed::Installed;
use super::lobby::{self, ServerSummary, SERVER_AGENT, SERVER_MAX_BODY, SERVER_TIMEOUT_MS};
use super::readiness::Level;

/// One car model of the server, its slots summed up.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CarSlots {
    pub id: String,
    pub total: u32,
    pub free: u32,
    /// The skin of the first free slot — the one the game will load if we
    /// join in this car. `None` when every slot is taken.
    pub skin: Option<String>,
    /// The car can be driven here (installed, or in the library).
    pub available: bool,
    /// How ready the car is here (`readiness.rs`).
    pub level: Level,
    /// The DLC to name when the car is blocked.
    pub dlc: Option<String>,
    /// Photo of `skin`, when the car is in the game with that livery. A car
    /// only in the library has none yet: it is laid in the game at join time.
    pub preview: Option<String>,
}

/// Someone on the server now.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Driver {
    pub name: String,
    pub car: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ServerDetail {
    pub summary: ServerSummary,
    pub cars: Vec<CarSlots>,
    pub drivers: Vec<Driver>,
    /// What the server declares it can do (`STEAM_TICKET`, `WEATHERFX_V1`…),
    /// empty on a vanilla server. Written into `race.ini` at join time.
    pub features: Vec<String>,
}

/// One slot of `/JSON`.
#[derive(Debug, Clone, PartialEq)]
struct Slot {
    model: String,
    skin: String,
    driver: String,
    connected: bool,
}

#[derive(Debug, Default, PartialEq)]
pub(super) struct EntryList {
    slots: Vec<Slot>,
    pub(super) features: Vec<String>,
}

/// Reads `/JSON`. Slots outside the entry list (`IsEntryList: false`) are left
/// out, as CM does: they are not places anyone can take.
fn parse_entry_list(root: &Value) -> EntryList {
    let text = |v: &Value| v.as_str().unwrap_or_default().to_string();
    let slots = root["Cars"]
        .as_array()
        .map(|cars| {
            cars.iter()
                .filter(|c| c["IsEntryList"].as_bool().unwrap_or(true))
                .map(|c| Slot {
                    model: text(&c["Model"]),
                    // AssettoServer appends the slot's CSP parameters after a
                    // slash (`black/ADAn`); the folder is before it.
                    skin: text(&c["Skin"]).split('/').next().unwrap_or_default().to_string(),
                    driver: text(&c["DriverName"]),
                    connected: c["IsConnected"].as_bool().unwrap_or(false),
                })
                .filter(|s| !s.model.is_empty())
                .collect()
        })
        .unwrap_or_default();
    let features = root["Features"]
        .as_array()
        .map(|f| f.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
        .unwrap_or_default();
    EntryList { slots, features }
}

/// Sums the slots per car, in the server's own order of `cars` (the order its
/// operator chose), then any model the entry list has and `cars` forgot.
///
/// A car with no slot at all is left out: an AssettoServer lists its AI
/// traffic (`traffic_jp_toyota_camry`…) among its cars, and an installed one
/// came first in the panel as a choice nobody can take (seen on a Shutoko
/// server). Only when the entry list has slots: an empty one says nothing.
fn car_slots(cars: &[String], entries: &EntryList, installed: &Installed) -> Vec<CarSlots> {
    let mut order: Vec<String> = cars.to_vec();
    for slot in &entries.slots {
        if !order.iter().any(|c| c.eq_ignore_ascii_case(&slot.model)) {
            order.push(slot.model.clone());
        }
    }
    order
        .into_iter()
        .map(|id| {
            let mine: Vec<&Slot> = entries
                .slots
                .iter()
                .filter(|s| s.model.eq_ignore_ascii_case(&id))
                .collect();
            let first_free = mine.iter().find(|s| !s.connected);
            let (level, dlc) = installed.car_level(&id);
            CarSlots {
                level,
                dlc,
                total: mine.len() as u32,
                free: mine.iter().filter(|s| !s.connected).count() as u32,
                skin: first_free.map(|s| s.skin.clone()).filter(|s| !s.is_empty()),
                available: level <= Level::OneClick,
                preview: None,
                id,
            }
        })
        .filter(|car| car.total > 0 || entries.slots.is_empty())
        .collect()
}

/// The photo of each car in the skin the server imposes, read in the game's
/// `content/cars`.
fn fill_previews(cars: &mut [CarSlots], cars_dir: &Path) {
    for car in cars.iter_mut().filter(|c| c.available) {
        if let Some(skin) = &car.skin {
            car.preview = crate::library::skin_preview(&cars_dir.join(&car.id).join("skins").join(skin));
        }
    }
}

pub(super) fn drivers(entries: &EntryList) -> Vec<Driver> {
    entries
        .slots
        .iter()
        .filter(|s| s.connected && !s.driver.is_empty())
        .map(|s| Driver {
            name: s.driver.clone(),
            car: s.model.clone(),
        })
        .collect()
}

fn get_json(ip: &str, http_port: u16, path: &str) -> Result<Value, String> {
    let url = format!("http://{ip}:{http_port}{path}");
    let response = http::get_url(&url, SERVER_AGENT, SERVER_TIMEOUT_MS, SERVER_MAX_BODY)
        .ok_or_else(|| crate::errors::SERVER_UNREACHABLE.to_string())?;
    if response.status != 200 {
        log::warn!("online: {url} answered {}", response.status);
        return Err(crate::errors::SERVER_UNREACHABLE.to_string());
    }
    serde_json::from_slice(&response.body).map_err(|e| {
        log::warn!("online: {url} is not JSON — {e}");
        crate::errors::SERVER_UNREACHABLE.to_string()
    })
}

/// The entry list alone, for the join: fresh features at the moment of
/// joining, whatever the detail panel read a minute before.
pub(super) fn fetch_entry_list(ip: &str, http_port: u16, steam_id: u64) -> Result<EntryList, String> {
    get_json(ip, http_port, &format!("/JSON|{steam_id}")).map(|root| parse_entry_list(&root))
}

/// Everything the detail panel shows, asked from the server itself. `cars_dir`
/// is the game's `content/cars`, where the imposed skins' photos are looked up.
pub fn fetch_detail(
    ip: &str,
    http_port: u16,
    steam_id: u64,
    installed: &Installed,
    cars_dir: Option<&Path>,
) -> Result<ServerDetail, String> {
    let mut info = get_json(ip, http_port, "/INFO")?;
    // The address we reached wins over the one the server reports about
    // itself: a vanilla server's `/INFO` leaves `ip` empty (measured), and
    // `parse_info` would drop it as unjoinable.
    info["ip"] = Value::from(ip);
    let mut summary = lobby::parse_info(&info).ok_or_else(|| {
        log::warn!("online: {ip}:{http_port}/INFO lacks ports or track");
        crate::errors::SERVER_UNREACHABLE.to_string()
    })?;
    installed.judge(&mut summary);
    let entries = fetch_entry_list(ip, http_port, steam_id)?;
    let mut cars = car_slots(&summary.cars, &entries, installed);
    if let Some(cars_dir) = cars_dir {
        fill_previews(&mut cars, cars_dir);
    }
    Ok(ServerDetail {
        cars,
        drivers: drivers(&entries),
        features: entries.features,
        summary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real `/JSON` of an AssettoServer, trimmed: the CSP parameters after
    /// the skin, a slot outside the entry list, one driver connected.
    fn entry_list() -> EntryList {
        parse_entry_list(&serde_json::json!({
            "Features": ["STEAM_TICKET", "WEATHERFX_V1"],
            "Cars": [
                { "Model": "ks_mazda_miata", "Skin": "00_classic_red/ADAn", "DriverName": "Léo",
                  "IsEntryList": true, "IsConnected": true },
                { "Model": "ks_mazda_miata", "Skin": "05_sunburst_yellow/ADAn", "DriverName": null,
                  "IsEntryList": true, "IsConnected": false },
                { "Model": "bmw_m3_e30", "Skin": "red", "DriverName": null,
                  "IsEntryList": true, "IsConnected": false },
                { "Model": "traffic_bus", "Skin": "", "DriverName": null,
                  "IsEntryList": false, "IsConnected": false }
            ]
        }))
    }

    /// Rule (online-join-research.md): the skin shown is the first FREE slot's,
    /// since that is the one the server imposes — CSP parameters stripped.
    #[test]
    fn the_skin_shown_is_the_first_free_slot() {
        let cars = vec!["ks_mazda_miata".to_string(), "bmw_m3_e30".to_string()];
        let slots = car_slots(&cars, &entry_list(), &Installed::default());
        assert_eq!(
            slots[0].skin.as_deref(),
            Some("05_sunburst_yellow"),
            "taken slot skipped, suffix gone"
        );
        assert_eq!((slots[0].free, slots[0].total), (1, 2), "one of two free");
        assert!(!slots[0].available, "nothing installed");
    }

    /// Rule: the photo shown is the one of the skin the server imposes — not
    /// the first livery of the folder, which the game will not load.
    #[test]
    fn the_photo_is_the_imposed_skin() {
        let base = crate::testutil::temp_dir("online-preview");
        let skins = base.join("ks_mazda_miata").join("skins");
        for skin in ["00_classic_red", "05_sunburst_yellow"] {
            std::fs::create_dir_all(skins.join(skin)).unwrap();
            std::fs::write(skins.join(skin).join("preview.jpg"), b"IMG").unwrap();
        }
        let mut installed = Installed::default();
        installed.add_car_for_tests("ks_mazda_miata", crate::online::installed::Presence::Game);
        let mut cars = car_slots(&["ks_mazda_miata".to_string()], &entry_list(), &installed);
        fill_previews(&mut cars, &base);
        let preview = cars[0].preview.as_deref().expect("a photo");
        assert!(
            preview.contains("05_sunburst_yellow"),
            "the free slot's livery: {preview}"
        );
    }

    /// Rule: a car the server lists without a single slot (AI traffic) is no
    /// choice — it is not offered, installed or not.
    #[test]
    fn a_car_without_slots_is_not_offered() {
        let cars = vec!["traffic_jp_toyota_camry".to_string(), "ks_mazda_miata".to_string()];
        let slots = car_slots(&cars, &entry_list(), &Installed::default());
        assert!(
            slots.iter().all(|s| s.id != "traffic_jp_toyota_camry"),
            "traffic car left out: {slots:?}"
        );
        let unknown = car_slots(&cars, &EntryList::default(), &Installed::default());
        assert_eq!(unknown.len(), 2, "an empty entry list hides nothing");
    }

    /// Rule: slots outside the entry list are nobody's place (CM filters them).
    #[test]
    fn slots_outside_the_entry_list_are_ignored() {
        let slots = car_slots(&[], &entry_list(), &Installed::default());
        assert!(slots.iter().all(|s| s.id != "traffic_bus"), "AI traffic slot left out");
        assert_eq!(slots.len(), 2, "models the list forgot are still shown");
    }

    /// Rule: the features are what the join writes into race.ini, and the
    /// connected drivers are listed with their car.
    #[test]
    fn features_and_drivers_are_read() {
        let entries = entry_list();
        assert_eq!(entries.features, vec!["STEAM_TICKET", "WEATHERFX_V1"]);
        assert_eq!(
            drivers(&entries),
            vec![Driver {
                name: "Léo".into(),
                car: "ks_mazda_miata".into()
            }]
        );
    }

    /// Rule: a vanilla server declares no features, and that is not an error.
    #[test]
    fn a_vanilla_server_has_no_features() {
        let entries = parse_entry_list(&serde_json::json!({ "Cars": [] }));
        assert!(entries.features.is_empty());
    }
}
