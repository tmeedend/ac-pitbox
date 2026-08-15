//! Image format sniffing for embedded texture blobs.
//!
//! The name stored in the file ends in `.dds` far more often than the blob
//! actually is one — mod authors rename PNGs and JPEGs freely, and AC loads
//! them anyway. So the extension is never trusted; the magic bytes decide
//! (spec §3.2).

/// Container format of an embedded texture blob.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    /// DirectDraw Surface — by far the common case, usually BC1/BC3/BC7.
    Dds,
    Png,
    Jpeg,
    /// Recognised by neither magic. Kept as a distinct case rather than an
    /// error: the converter will report it and move on.
    Unknown,
}

impl ImageFormat {
    pub fn sniff(data: &[u8]) -> Self {
        if data.starts_with(b"DDS ") {
            Self::Dds
        } else if data.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
            Self::Png
        } else if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
            Self::Jpeg
        } else {
            Self::Unknown
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dds => "dds",
            Self::Png => "png",
            Self::Jpeg => "jpeg",
            Self::Unknown => "unknown",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Rule: the blob decides, not the filename (§3.2).
    #[test]
    fn sniffs_container_from_magic_bytes() {
        assert_eq!(ImageFormat::sniff(b"DDS \x7c\x00\x00\x00"), ImageFormat::Dds, "dds");
        assert_eq!(
            ImageFormat::sniff(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0]),
            ImageFormat::Png,
            "png"
        );
        assert_eq!(ImageFormat::sniff(&[0xFF, 0xD8, 0xFF, 0xE0]), ImageFormat::Jpeg, "jpeg");
        assert_eq!(ImageFormat::sniff(b"nope"), ImageFormat::Unknown, "unrecognised");
        assert_eq!(
            ImageFormat::sniff(&[]),
            ImageFormat::Unknown,
            "empty blob is not a panic"
        );
    }
}
