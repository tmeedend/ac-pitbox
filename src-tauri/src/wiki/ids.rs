//! Wikidata identifiers, in one place (§4.4).
//!
//! The spec asks for these to be **verified against Wikidata before being
//! wired**, and never scattered through the code. Every one below was read off
//! the live API — the label in the comment is the label the API returned, and
//! the "seen on" note says which real item it was confirmed against. None of
//! them is remembered or guessed.
//!
//! Two of the checks changed the design rather than confirming it, and both are
//! recorded where they bite: `PART_OF` vs `SUBCLASS_OF` below, and the
//! `TRACK_TYPES` list, whose comment explains why the type filter carries the
//! whole track strategy.

// --- Properties -------------------------------------------------------------

/// `instance of` / « nature de l'élément ». The type filter of §4.1.2 and
/// §4.2.2 reads this and nothing else.
pub const INSTANCE_OF: &str = "P31";

/// `subclass of` / « sous-classe de ».
///
/// **Not a parent for cars.** Measured on the AE86 (Q1377219): it carries
/// `P361 → Q2626308` (Toyota Sprinter Trueno, the generic model — the right
/// "article général" of §5.3) *and* `P279 → Q5333841`, which is **"sport
/// compact"**, a classification. Climbing P279 blindly would offer an article
/// about a car category as if it were the mod's model. Hence
/// `parent_of_type`: a P279 target is only accepted when its own P31 is in the
/// same allowlist as the entity itself.
pub const SUBCLASS_OF: &str = "P279";

/// `part of` / « partie de ». The relation §5.3 names: a generation to its
/// model, a track configuration to its circuit. Tried before `SUBCLASS_OF`.
/// Seen on Q1377219 → Q2626308.
pub const PART_OF: &str = "P361";

/// `coordinate location` / « coordonnées géographiques » (globe-coordinate).
/// What the geographic search of §4.2 matches on. Seen on Q152207
/// (Nürburgring): 50.3355555, 6.9475.
pub const COORDINATE_LOCATION: &str = "P625";

/// `manufacturer` / « fabricant ». The brand half of the car score (§4.1.4).
/// Seen on all four cars measured: MX-5 → Q35996, Supra and AE86 → Q53268,
/// M3 → Q26678.
pub const MANUFACTURER: &str = "P176";

/// `start time` / « date de début » and `end time` / « date de fin ».
///
/// The production period of §4.1.4, and the **only** source for it that exists
/// in practice. `P571` ("inception") was the obvious candidate and is absent
/// from every car item measured — MX-5, Supra, M3 and the Sprinter Trueno all
/// lack it. Only the *generation* item carries a period: Q1377219 has
/// P580 = 1986, P582 = 1989. The year is therefore a bonus when present and
/// never a penalty when missing (see `matchcar::score`).
pub const START_TIME: &str = "P580";
pub const END_TIME: &str = "P582";

// --- Classes: cars ----------------------------------------------------------

/// `car model` / « modèle d'automobile ».
///
/// The single class every real car item measured carries: Mazda MX-5
/// (Q127110), Toyota Supra (Q647404), BMW M3 (Q796579) and Toyota AE86
/// (Q1377219) are all exactly `P31 = Q3231690`.
pub const CAR_MODEL: &str = "Q3231690";

/// `car model series` / « série de modèles d'automobile ». A group of related
/// models — the shape a model family takes when it is not one model.
pub const CAR_MODEL_SERIES: &str = "Q59773381";

/// `racing automobile model` / « modèle de voiture de course ».
///
/// **A separate class from `CAR_MODEL`, and the library is full of them.**
/// Ferrari SF15-T (Q18918144), Audi R18 (Q693616) and Chevrolet Corvette C7.R
/// (Q16957322) all carry this one and none of them carries `Q3231690` — so a
/// filter that only knew about road cars rejected every Formula One car,
/// prototype and GT racer in the library while their articles sat there.
/// Found because a calibration report claimed the SF15-T had no article.
pub const RACING_CAR_MODEL: &str = "Q90834785";

/// What §4.1.3 accepts for a car. Every entry added here is a new way to match
/// the wrong thing, so each one is measured on a real item rather than
/// imagined.
pub const CAR_TYPES: [&str; 3] = [CAR_MODEL, CAR_MODEL_SERIES, RACING_CAR_MODEL];

// --- Classes: tracks, roads and passes --------------------------------------

/// `motorsport racing track` / « circuit de sport mécanique ». The class real
/// circuits carry: Nürburgring (Q152207), Spa (Q172851) and Suzuka (Q174170)
/// are all `P31 = Q2338524`.
pub const MOTORSPORT_RACING_TRACK: &str = "Q2338524";

/// `race track` / « circuit ». Superclass of `MOTORSPORT_RACING_TRACK`
/// (Q2338524 has `P279 → Q1777138`), kept for the items typed one level up.
pub const RACE_TRACK: &str = "Q1777138";

/// `road` / « route », `street` / « rue ».
pub const ROAD: &str = "Q34442";
pub const STREET: &str = "Q79007";

/// `controlled-access highway` / « autoroute », and the two classes the Shuto
/// Expressway (Q1369525) actually carries: `highway system` (Q25631158) and
/// `urban motorway` (Q1367934, itself `P279 → Q46622`). The spec names
/// "autoroute"; the real item names neither of the obvious ids, which is
/// exactly why they were looked up.
pub const CONTROLLED_ACCESS_HIGHWAY: &str = "Q46622";
pub const HIGHWAY_SYSTEM: &str = "Q25631158";
pub const URBAN_MOTORWAY: &str = "Q1367934";

/// `mountain pass` / « col routier ». Seen on the Stelvio (Q1334855).
pub const MOUNTAIN_PASS: &str = "Q133056";

/// What a purpose-built circuit is. **Tried alone first** (see `TRACK_TYPES`).
pub const CIRCUIT_TYPES: [&str; 2] = [MOTORSPORT_RACING_TRACK, RACE_TRACK];

/// Roads, passes and expressways — the Shutoko and touge half of the library.
///
/// Kept **apart** from `CIRCUIT_TYPES`, and that separation was bought with a
/// calibration run: a street is a valid answer for Shutoko and a catastrophe
/// for Zandvoort, because every circuit on earth has streets at its own
/// coordinates. The run matched the Nordschleife against `Kurt-Bosch-Straße`
/// (7 mm of score apart), Vallelunga against two Roman streets and Zandvoort
/// against `Duinweg` — four of the fourteen ambiguities, all of them this.
pub const ROUTE_TYPES: [&str; 6] = [
    ROAD,
    STREET,
    CONTROLLED_ACCESS_HIGHWAY,
    HIGHWAY_SYSTEM,
    URBAN_MOTORWAY,
    MOUNTAIN_PASS,
];

/// The allowlist of §4.2.2, wider than the cars' on purpose.
///
/// **This list carries the whole track strategy.** Measured five metres from
/// the Nordschleife's coordinates: the twenty nearest items are nineteen Grand
/// Prix editions (`Q18608583`, "recurring sporting event"), a village, a
/// castle, a stream and a closed amusement park — and the Nürburgring itself
/// is not among them, because some twenty items share the exact same
/// coordinate and the order between equals is arbitrary. Without the filter,
/// "the nearest one" (§4.2.3) matches the Nordschleife to the 1997 Luxembourg
/// Grand Prix.
///
/// Mountains are **not** in the list. The Initial D passes are the tempting
/// case — Mount Haruna, the real Akina, is a `stratovolcano` and a `mountain
/// range`, so it is missed — but widening a list to catch one family of mods
/// is how a decorative feature starts matching a circuit to the mountain it
/// sits on. The calibration report says how many tracks this actually costs;
/// that is the moment to reconsider, with the number in hand.
pub const TRACK_TYPES: [&str; 8] = [
    MOTORSPORT_RACING_TRACK,
    RACE_TRACK,
    ROAD,
    STREET,
    CONTROLLED_ACCESS_HIGHWAY,
    HIGHWAY_SYSTEM,
    URBAN_MOTORWAY,
    MOUNTAIN_PASS,
];
