//! Parsing errors.
//!
//! These are technical diagnostics, not user-facing text: the application maps
//! them to an i18n key of its own before they reach the UI (`CLAUDE.md` —
//! backend errors destined for the user are keys, not sentences).

use thiserror::Error;

pub type Result<T> = std::result::Result<T, Kn5Error>;

#[derive(Debug, Error)]
pub enum Kn5Error {
    /// Magic bytes are not `sc6969`. Also what an encrypted (CSP-protected)
    /// KN5 looks like from here — telling the two apart is the caller's job
    /// (spec §4.5); the parser never tries to decrypt anything.
    #[error("not a KN5 file (bad magic)")]
    NotAKn5File,

    /// Ran past the end of the buffer. `offset` is where the read started, so
    /// a truncated file points straight at the section that got cut.
    #[error("unexpected end of file at offset {offset}: needed {needed} bytes, {available} left")]
    UnexpectedEof {
        offset: usize,
        needed: usize,
        available: usize,
    },

    /// A length or count field was negative — the format stores them as `i32`,
    /// so this is either corruption or a hostile file.
    #[error("negative count for `{field}`: {value}")]
    NegativeCount { field: &'static str, value: i32 },

    /// A count is positive but implausible: either above the configured cap,
    /// or larger than what the remaining bytes could hold.
    #[error("`{field}` count {value} exceeds limit {limit}")]
    LimitExceeded {
        field: &'static str,
        value: usize,
        limit: usize,
    },

    /// The node tree nests deeper than [`crate::Limits::max_depth`]. Without
    /// this guard a crafted file overflows the stack, which no `Result` can
    /// catch.
    #[error("node tree deeper than {limit} levels")]
    DepthLimitExceeded { limit: usize },

    /// Node type byte outside the known 1/2/3 (dummy / mesh / skinned mesh).
    /// Fatal because the node body layout depends on it — we cannot skip past
    /// a node whose size we do not know.
    #[error("unknown node type {0} for node `{1}`")]
    UnknownNodeType(i32, String),

    /// A mesh points at a material index that does not exist.
    #[error("mesh `{node}` references material {id}, only {count} declared")]
    MaterialIdOutOfRange { node: String, id: i64, count: usize },
}
