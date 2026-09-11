//! Wikidata property identifiers, in one place (§4.4).
//!
//! The spec asks for these to be **verified against Wikidata before being
//! wired**, and never scattered through the code. Both below were read off a
//! live entity rather than remembered: `Q1377219` (Toyota AE86) carries
//! `P31 = Q3231690`, `P279 = Q5333841` and `P361 = Q2626308`, each reached at
//! `claims.<property>[0].mainsnak.datavalue.value.id`.
//!
//! The class identifiers and `P31` itself belong to the matching work (§4),
//! which is not implemented yet; they join this file when they do, so that the
//! answer to "what does P279 mean here" stays in one place.

/// "part of" — a track configuration to its circuit, a car generation to its
/// model. The relation §5.3 names explicitly, and the one tried first.
pub const PART_OF: &str = "P361";

/// "subclass of" — tried when "part of" says nothing. Car generations are
/// modelled both ways on Wikidata depending on who wrote the item, and §5.3
/// accepts the generation-to-model climb whichever way it is spelled.
pub const SUBCLASS_OF: &str = "P279";
