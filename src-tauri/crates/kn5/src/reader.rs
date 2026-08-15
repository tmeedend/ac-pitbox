//! Bounds-checked cursor over the raw file.
//!
//! Written by hand rather than with `binrw` (suggested by spec §5.2): the
//! layout is riddled with version-dependent fields and every single count has
//! to be validated against a cap *and* against the bytes that remain before
//! anything is allocated. Expressing that in `binrw` attributes costs more
//! than the hundred lines below, for one more dependency.
//!
//! All integers are little-endian (§3).

use crate::error::{Kn5Error, Result};

pub(crate) struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub(crate) fn position(&self) -> usize {
        self.pos
    }

    pub(crate) fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    /// The single place where the cursor advances — every other method goes
    /// through it, so there is exactly one bounds check to get right.
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(n).ok_or(Kn5Error::UnexpectedEof {
            offset: self.pos,
            needed: n,
            available: self.remaining(),
        })?;
        let slice = self.data.get(self.pos..end).ok_or(Kn5Error::UnexpectedEof {
            offset: self.pos,
            needed: n,
            available: self.remaining(),
        })?;
        self.pos = end;
        Ok(slice)
    }

    pub(crate) fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        let slice = self.take(N)?;
        let mut out = [0u8; N];
        out.copy_from_slice(slice);
        Ok(out)
    }

    pub(crate) fn u8(&mut self) -> Result<u8> {
        Ok(self.array::<1>()?[0])
    }

    /// AC stores booleans as one byte; anything non-zero is true.
    pub(crate) fn bool(&mut self) -> Result<bool> {
        Ok(self.u8()? != 0)
    }

    pub(crate) fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_le_bytes(self.array::<4>()?))
    }

    pub(crate) fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.array::<4>()?))
    }

    pub(crate) fn f32(&mut self) -> Result<f32> {
        Ok(f32::from_le_bytes(self.array::<4>()?))
    }

    pub(crate) fn f32s<const N: usize>(&mut self) -> Result<[f32; N]> {
        let mut out = [0f32; N];
        for slot in out.iter_mut() {
            *slot = self.f32()?;
        }
        Ok(out)
    }

    pub(crate) fn bytes(&mut self, n: usize) -> Result<Vec<u8>> {
        Ok(self.take(n)?.to_vec())
    }

    /// Reads an `i32` count and refuses to let it drive an allocation unless
    /// it is plausible. Three checks, in order of cheapness:
    ///
    /// 1. not negative (the format signs its counts);
    /// 2. not above the configured cap;
    /// 3. not larger than the remaining bytes can hold, given the smallest
    ///    possible size of one entry.
    ///
    /// Check 3 is the one that really matters: it ties every allocation to the
    /// actual file size, so a 2 KB file can never make us reserve a gigabyte,
    /// whatever the caps are set to. Pass `min_item_size = 0` when an entry
    /// has no meaningful minimum.
    pub(crate) fn count(&mut self, field: &'static str, limit: usize, min_item_size: usize) -> Result<usize> {
        let raw = self.i32()?;
        if raw < 0 {
            return Err(Kn5Error::NegativeCount { field, value: raw });
        }
        let value = raw as usize;
        if value > limit {
            return Err(Kn5Error::LimitExceeded { field, value, limit });
        }
        if let Some(affordable) = self.remaining().checked_div(min_item_size) {
            if value > affordable {
                return Err(Kn5Error::LimitExceeded {
                    field,
                    value,
                    limit: affordable,
                });
            }
        }
        Ok(value)
    }

    /// Length-prefixed string: `i32` byte length, then that many bytes (§3).
    ///
    /// Decoded lossily on purpose. The spec says UTF-8, and Kunos files are,
    /// but mod authors occasionally ship names encoded in the Windows ANSI
    /// codepage. Refusing the file over one accented character in a material
    /// name would cost a whole car preview; a replacement character in a name
    /// costs nothing, since the names we actually key on (texture filenames,
    /// sampler slots) are ASCII.
    pub(crate) fn string(&mut self, field: &'static str, max_bytes: usize) -> Result<String> {
        let len = self.count(field, max_bytes, 1)?;
        let raw = self.take(len)?;
        Ok(String::from_utf8_lossy(raw).into_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A read that runs past the end must report where it started, not panic.
    #[test]
    fn take_past_end_reports_offset() {
        let mut r = Reader::new(&[1, 2, 3]);
        assert_eq!(r.u8().ok(), Some(1), "first byte read");
        match r.i32() {
            Err(Kn5Error::UnexpectedEof {
                offset,
                needed,
                available,
            }) => {
                assert_eq!(
                    (offset, needed, available),
                    (1, 4, 2),
                    "EOF located at the failing read"
                );
            }
            other => panic!("expected UnexpectedEof, got {other:?}"),
        }
    }

    // Counts are signed in the format: a negative one is corruption, never a
    // huge unsigned value.
    #[test]
    fn negative_count_is_rejected() {
        let bytes = (-1i32).to_le_bytes();
        let mut r = Reader::new(&bytes);
        assert!(
            matches!(r.count("field", 100, 0), Err(Kn5Error::NegativeCount { .. })),
            "negative count rejected instead of wrapping to a huge usize"
        );
    }

    // The guard that actually protects memory: a count larger than the file
    // could possibly hold is refused before allocating.
    #[test]
    fn count_larger_than_remaining_bytes_is_rejected() {
        // Claims 1000 entries of >= 12 bytes each, but only 4 bytes follow.
        let mut bytes = 1000i32.to_le_bytes().to_vec();
        bytes.extend_from_slice(&[0; 4]);
        let mut r = Reader::new(&bytes);
        assert!(
            matches!(r.count("field", usize::MAX, 12), Err(Kn5Error::LimitExceeded { .. })),
            "implausible count rejected even with no configured cap"
        );
    }

    // Mod authors sometimes ship Windows-ANSI names; a bad byte must not fail
    // the whole file.
    #[test]
    fn string_with_invalid_utf8_is_decoded_lossily() {
        let mut bytes = 3i32.to_le_bytes().to_vec();
        bytes.extend_from_slice(&[b'a', 0xE9, b'b']);
        let mut r = Reader::new(&bytes);
        let s = r.string("name", 4096).expect("lossy decode succeeds");
        assert!(s.starts_with('a') && s.ends_with('b'), "surrounding ASCII preserved");
    }
}
