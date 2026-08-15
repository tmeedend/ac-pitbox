//! Conversion of a parsed [`kn5::Kn5Model`] into a glTF preview.
//!
//! Split from the `kn5` crate because this half does touch the filesystem
//! (skin overrides live on disk next to the model) and pulls in image codecs,
//! neither of which belong in a parser (spec §5.1).
//!
//! Lot 2 covers the texture pipeline; the glTF writer itself lands in lot 3.

mod texture;

pub use texture::{
    prepare_textures, PreparedTexture, TextureOptions, TextureOrigin, TextureRole, TextureSet, TextureWarning,
};
