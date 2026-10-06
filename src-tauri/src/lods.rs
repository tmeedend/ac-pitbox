//! Cars that borrow their 3D models from another car (SESSION§2.5).
//!
//! A car's `lods.ini` names the `.kn5` each level of detail loads, and the
//! path is relative to the car's folder - so it can climb out of it. Variants
//! use it to ship physics alone: the "BOP" Auriel 90 of the VRC pack has no
//! model of its own, its four LODs are `../vrc_arc_auriel90/vrc_arc_auriel90_*.kn5`.
//!
//! Nothing ties the two in a mod manager's eyes: two folders, two cars, two
//! entries of the library. Real bug this exists for: the base car deleted as
//! an apparent duplicate, the variant still listed, and Assetto Corsa crashing
//! while seating the driver in a car that has no body (`DriverModel`, from
//! `CarAvatar::init3D`). Hence what this module answers, and two places that
//! ask: the launch, which lays the lender in the game or refuses with its
//! name, and the deletion, which names the cars a deletion would break.

use std::collections::HashMap;
use std::path::Path;

use rusqlite::Connection;

use crate::config::AppConfig;
use crate::modscan::ModKind;

/// What a decrypted `lods.ini` must contain for its key to be believed.
const LODS_MARKER: &str = "[LOD_";

/// The model every car carries whatever it borrows: physics, not graphics.
const COLLIDER: &str = "collider.kn5";

/// The car folders a `lods.ini` loads models from, other than its own: the
/// first segment after a leading `..`, once per folder, in order of first use.
pub fn lenders_in(lods: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in lods.lines() {
        // A comment is not a path, and `;` starts one anywhere on the line.
        let line = line.split(';').next().unwrap_or("");
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if !key.trim().eq_ignore_ascii_case("FILE") {
            continue;
        }
        let path = value.trim().replace('\\', "/");
        let mut parts = path.split('/').filter(|p| !p.is_empty() && *p != ".");
        if parts.next() != Some("..") {
            continue;
        }
        let Some(folder) = parts.next().filter(|f| *f != "..") else {
            continue;
        };
        // A file name and a folder after it: `../x.kn5` alone names no car.
        if parts.next().is_none() {
            continue;
        }
        if !out.iter().any(|o| o.eq_ignore_ascii_case(folder)) {
            out.push(folder.to_string());
        }
    }
    out
}

/// Whether `dir` carries a model of its own, the collider aside.
fn has_own_model(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries.flatten().any(|e| {
                let name = e.file_name().to_string_lossy().to_lowercase();
                name.ends_with(".kn5") && name != COLLIDER
            })
        })
        .unwrap_or(false)
}

/// The cars `car_dir` borrows its models from. Empty for the overwhelming
/// majority, which carry their own: their `data.acd` is not even opened, so
/// asking it of a whole library costs a folder listing per car.
pub fn lenders(car_dir: &Path, car_id: &str) -> Vec<String> {
    if has_own_model(car_dir) {
        return Vec::new();
    }
    crate::cardata::read(car_dir, car_id, "lods.ini", LODS_MARKER)
        .map(|text| lenders_in(&text))
        .unwrap_or_default()
}

/// A library car that takes its models from another one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Borrower {
    pub id: String,
    pub name: String,
}

/// Library cars whose models come from another car: lender id (lower-cased,
/// folder names are not case-sensitive) → the cars that borrow from it.
pub fn borrowers(conn: &Connection, cfg: &AppConfig) -> Result<HashMap<String, Vec<Borrower>>, String> {
    let mut out: HashMap<String, Vec<Borrower>> = HashMap::new();
    for m in crate::overlay::list_mods(conn).map_err(|e| e.to_string())? {
        if m.is_stock || ModKind::from_column(&m.kind) != ModKind::Car {
            continue;
        }
        let Some(dir) = crate::overlay::active_library_path(conn, &m.id_interne)
            .map_err(|e| e.to_string())?
            .and_then(|stored| crate::libpath::resolve(cfg.library_path.as_deref(), &stored))
        else {
            continue;
        };
        for lender in lenders(&dir, &m.id_interne) {
            out.entry(lender.to_lowercase()).or_default().push(Borrower {
                id: m.id_interne.clone(),
                name: m.display_name.clone().unwrap_or_else(|| m.id_interne.clone()),
            });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The real file of the VRC "BOP" Auriel 90 (decrypted from its
    /// `data.acd`): four LODs, all in the base car's folder.
    const BOP_LODS: &str = "\
[COCKPIT_HR]
DISTANCE_SWITCH=5

[LOD_0]
FILE=../vrc_arc_auriel90/vrc_arc_auriel90_a.kn5
IN=0
OUT=15

[LOD_1]
FILE=../vrc_arc_auriel90/vrc_arc_auriel90_b.kn5
IN=15

[LOD_3]
FILE=../vrc_arc_auriel90/vrc_arc_auriel90_d.kn5
IN=200
";

    /// SESSION§2.5: the folder a variant takes its models from, named once.
    #[test]
    fn a_variant_names_the_car_it_borrows_from_once() {
        assert_eq!(
            lenders_in(BOP_LODS),
            vec!["vrc_arc_auriel90".to_string()],
            "one lender, once"
        );
    }

    /// The common case: models in the car's own folder borrow nothing.
    #[test]
    fn models_of_its_own_borrow_nothing() {
        let own = "[LOD_0]\nFILE=car_a.kn5\n\n[LOD_1]\nFILE=./car_b.kn5\n";
        assert!(lenders_in(own).is_empty(), "no `..`, no lender");
    }

    /// Windows separators, spaces around `=`, a lower-case key and a trailing
    /// comment are all written by real authors.
    #[test]
    fn the_path_is_read_however_it_is_spelled() {
        let text = "[LOD_0]\nfile = ..\\base_car\\body.kn5 ; borrowed\n";
        assert_eq!(
            lenders_in(text),
            vec!["base_car".to_string()],
            "backslashes and comment"
        );
    }

    /// `../x.kn5` points at the cars folder itself, not into a car: there is
    /// no car to name, and inventing one would refuse a launch for nothing.
    #[test]
    fn a_path_that_names_no_car_is_ignored() {
        assert!(lenders_in("[LOD_0]\nFILE=../loose.kn5\n").is_empty(), "no folder");
        assert!(
            lenders_in("[LOD_0]\nFILE=../../x/y.kn5\n").is_empty(),
            "out of the cars folder"
        );
        assert!(lenders_in("; FILE=../other/x.kn5\n").is_empty(), "commented out");
    }

    /// The prefilter: a folder with its own model is not asked its `lods.ini`
    /// - and a collider alone is not a model.
    #[test]
    fn only_a_car_without_a_model_is_asked_its_lods() {
        let base = crate::testutil::temp_dir("lods_prefilter");
        let own = base.join("own");
        std::fs::create_dir_all(own.join("data")).expect("data");
        std::fs::write(own.join("own.kn5"), b"KN5").expect("kn5");
        std::fs::write(own.join("data").join("lods.ini"), BOP_LODS).expect("lods");
        assert!(lenders(&own, "own").is_empty(), "its own model wins over lods.ini");

        let variant = base.join("variant");
        std::fs::create_dir_all(variant.join("data")).expect("data");
        std::fs::write(variant.join(COLLIDER), b"KN5").expect("collider");
        std::fs::write(variant.join("data").join("lods.ini"), BOP_LODS).expect("lods");
        assert_eq!(
            lenders(&variant, "variant"),
            vec!["vrc_arc_auriel90".to_string()],
            "a collider alone does not make a body"
        );
    }
}
