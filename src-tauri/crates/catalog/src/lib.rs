//! The rules catalogue of Pit Box in two layers (REGLES§2): what the
//! application ships, and the user's decisions on top of it.
//!
//! Shared by the application, which merges the layers to classify the library,
//! and by `rules-tool`, which promotes a developer's decisions into the
//! catalogue: ONE implementation of each merge, so what the tool writes is
//! exactly what the application showed. No I/O here — the callers read and
//! write the files.
//!
//! - `taxonomy`: the index tables (families, country aliases and tags), keyed
//!   by their natural key.
//! - `rules`: the list rules of the Rules screen, keyed by stable ids.

pub mod rules;
pub mod taxonomy;
