//! Multiplayer servers (`docs/SPEC-play-online.md`): the public list, one
//! server's details, and joining through Content Manager.
//!
//! Everything here talks to the network, and WinHTTP blocks: the command
//! facades run it in `spawn_blocking`, and take the SQLite lock only for the
//! short read that judges servers against the library — never across a
//! request.

mod installed;
pub mod join;
pub mod lobby;
pub mod server;
mod steam;

pub use installed::Installed;
pub use steam::active_steam_id;
