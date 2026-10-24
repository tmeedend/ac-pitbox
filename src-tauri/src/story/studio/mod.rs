//! Create Your Own, the story editor (`docs/story/SPEC-story-mode.md`,
//! « Create Your Own: the editor »): the stories an author works on, and the
//! files the editor reads and writes in them.
//!
//! A story lives in a folder of the author's choosing - often a private git
//! repository - never in Pit Box's own data. What Pit Box keeps is only the
//! list of those folders, in `app_config_dir/story/studio.json`: nothing in
//! `overlay.sqlite` or `config.json` (CLAUDE.md, « Chantiers longs derrière
//! un drapeau »). Forgetting a story drops it from that list and touches none
//! of its files.
//!
//! The editor writes a file **whole** and at once, synchronously, in the
//! engine's canonical form (`story_engine::write`). Two safeguards come with
//! that: a file is only written when it still holds what the editor last read
//! (`Stamp`), so a change made meanwhile in a text editor is reloaded rather
//! than overwritten; and the engine reads every write back before it reaches
//! the disk. Every write is recorded in the story's undo history
//! (`history.rs`).
//!
//! Split by what changes together: `registry` (the list of stories),
//! `open` (a story opened and checked), `files` (what the editor writes),
//! `preview` (what it shows without writing), `package` (the permission
//! register and the build).

mod files;
mod open;
mod package;
mod preview;
mod registry;

pub use files::*;
pub use open::*;
pub use package::*;
pub use preview::*;
pub use registry::*;

#[cfg(test)]
mod tests;
