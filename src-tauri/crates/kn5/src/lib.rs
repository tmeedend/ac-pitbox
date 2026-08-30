//! KN5 parsing — the proprietary 3D model format used by Assetto Corsa.
//!
//! Pure parsing only: no Tauri, no filesystem, no network. The caller hands
//! over the whole file as bytes and gets a [`Kn5Model`] back. Keeping I/O out
//! is what makes this crate testable from `kn5-tool` without launching the app
//! (see `docs/SPEC-preview-3d-kn5.md` §5.1).
//!
//! The format is undocumented by Kunos and was reverse-engineered by the
//! community; the layout implemented here is the one described in §3 of that
//! spec, written from scratch rather than transcribed from any third-party
//! codebase (§2 — licensing).
//!
//! # Untrusted input
//!
//! A KN5 comes from a mod the user downloaded, so it is hostile input by
//! default. Every allocation derived from a field of the file goes through
//! [`reader::Reader::count`], which rejects negative values, values above a
//! configurable cap ([`Limits`]) and values that could not possibly fit in the
//! bytes that remain. Node recursion is depth-limited. There is no `unwrap`,
//! no `panic!` and no indexing by a value read from the file.

mod error;
mod knh;
mod ksanim;
mod limits;
mod model;
mod parse;
mod reader;
mod texture;

pub use error::{Kn5Error, Result};
pub use knh::{parse_hierarchy, parse_hierarchy_with_limits, Kn5Hierarchy};
pub use ksanim::{parse_animation, parse_animation_with_limits, Kn5AnimatedNode, Kn5Animation};
pub use limits::Limits;
pub use model::{
    Kn5Bone, Kn5Material, Kn5MaterialProperty, Kn5Mesh, Kn5Model, Kn5Node, Kn5NodeKind, Kn5Sampler, Kn5SkinBinding,
    Kn5SkinnedMesh, Kn5Texture, Kn5Vertex,
};
pub use parse::{parse, parse_with_limits};
pub use texture::ImageFormat;
