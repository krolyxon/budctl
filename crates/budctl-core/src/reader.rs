//! A cursor over bytes from the buds. Every read returns a `Result`, so a short or malformed
//! message becomes an [`Error::Protocol`] instead of a panic.

use crate::error::{Error, Result};

pub(crate) struct Reader<'a> {
    bytes: &'a [u8],
    /// What is being parsed, for error messages ("battery reply", "SDP response"…).
    what: &'static str,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(bytes: &'a [u8], what: &'static str) -> Self {
        Self { bytes, what }
    }

    pub(crate) fn u8(&mut self) -> Result<u8> {
        let [byte] = self.array::<1>()?;
        Ok(byte)
    }

    pub(crate) fn i8(&mut self) -> Result<i8> {
        let [byte] = self.array::<1>()?;
        Ok(i8::from_le_bytes([byte]))
    }

    pub(crate) fn u16_le(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    pub(crate) fn u16_be(&mut self) -> Result<u16> {
        Ok(u16::from_be_bytes(self.array()?))
    }

    pub(crate) fn u32_be(&mut self) -> Result<u32> {
        Ok(u32::from_be_bytes(self.array()?))
    }

    /// The next `n` bytes.
    pub(crate) fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let (head, tail) = self.bytes.split_at_checked(n).ok_or_else(|| self.too_short())?;
        self.bytes = tail;
        Ok(head)
    }

    /// Everything not read yet.
    pub(crate) fn rest(&mut self) -> &'a [u8] {
        std::mem::take(&mut self.bytes)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        let (head, tail) = self.bytes.split_first_chunk::<N>().ok_or_else(|| self.too_short())?;
        self.bytes = tail;
        Ok(*head)
    }

    fn too_short(&self) -> Error {
        Error::Protocol(format!("{} is too short", self.what))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_in_order_and_reports_short_input() {
        let mut r = Reader::new(&[0x01, 0x34, 0x12, 0x12, 0x34, 0xFF], "test");
        assert_eq!(r.u8().unwrap(), 0x01);
        assert_eq!(r.u16_le().unwrap(), 0x1234);
        assert_eq!(r.u16_be().unwrap(), 0x1234);
        assert_eq!(r.i8().unwrap(), -1);
        assert!(r.is_empty());
        assert!(matches!(r.u8(), Err(Error::Protocol(_))));
    }

    #[test]
    fn take_and_rest() {
        let mut r = Reader::new(&[1, 2, 3, 4], "test");
        assert_eq!(r.take(2).unwrap(), &[1, 2]);
        assert!(r.take(3).is_err());
        assert_eq!(r.rest(), &[3, 4]);
        assert_eq!(r.rest(), &[] as &[u8]);
    }
}
