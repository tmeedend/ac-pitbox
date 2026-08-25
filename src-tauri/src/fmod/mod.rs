//! Playing a car's real engine event through the FMOD Studio engine that ships
//! with Assetto Corsa.
//!
//! Why this exists: picking "the idle" out of a bank by analysing the signal is
//! a problem we cannot solve — 40 acceptable choices out of 91, and
//! `docs/fsb5-format.md` explains why no amount of threshold tuning fixes that.
//! The answer is already in the bank's FMOD event graph, and the game ships the
//! engine that reads it. **Do not try to improve the heuristic instead.**
//!
//! The in-house FSB5 decoder is not replaced by any of this: it still feeds the
//! mod sheet (codec, sample count, rate, duration) and it is still the only
//! path that works with no game installed. See
//! `docs/SPEC-engine-sound-fmod.md` §4.1.
//!
//! Layout, and why it is split this way: `guids` and `params` are pure and
//! portable, so the recognition rules that decide what actually gets played are
//! unit-tested without a DLL anywhere in sight. `sys` is the Windows-only FFI,
//! and holds every fact about the ABI that FMOD's own headers get wrong.

// Lot 1 is the bindings; lot 2 gives them an owning thread and lot 3 a command
// to call it. Until then nothing outside the tests calls in here, and the
// crate-wide `-D warnings` would turn that into a build failure. **Remove this
// once the thread of §4.3 exists** — after that, anything unused really is.
#![allow(dead_code)]

pub mod guids;
pub mod params;
#[cfg(windows)]
pub mod sys;
