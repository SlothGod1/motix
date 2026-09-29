//! Bounds-checked reading from an in-memory byte slice. Every read returns an
//! error instead of panicking when the data is too short.

use crate::ProbeError;

pub(crate) struct Bytes<'a> {
    data: &'a [u8],
    pos: usize,
}

const SHORT: ProbeError = ProbeError::Malformed("a header is truncated");

impl<'a> Bytes<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub(crate) fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    pub(crate) fn take(&mut self, n: usize) -> Result<&'a [u8], ProbeError> {
        let end = self.pos.checked_add(n).ok_or(SHORT)?;
        let s = self.data.get(self.pos..end).ok_or(SHORT)?;
        self.pos = end;
        Ok(s)
    }

    pub(crate) fn skip(&mut self, n: usize) -> Result<(), ProbeError> {
        self.take(n).map(|_| ())
    }

    pub(crate) fn rest(&mut self) -> &'a [u8] {
        let s = &self.data[self.pos..];
        self.pos = self.data.len();
        s
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], ProbeError> {
        let s = self.take(N)?;
        let mut a = [0_u8; N];
        a.copy_from_slice(s);
        Ok(a)
    }

    pub(crate) fn u8(&mut self) -> Result<u8, ProbeError> {
        Ok(self.array::<1>()?[0])
    }
    pub(crate) fn be_u16(&mut self) -> Result<u16, ProbeError> {
        Ok(u16::from_be_bytes(self.array()?))
    }
    pub(crate) fn be_u32(&mut self) -> Result<u32, ProbeError> {
        Ok(u32::from_be_bytes(self.array()?))
    }
    pub(crate) fn be_i32(&mut self) -> Result<i32, ProbeError> {
        Ok(i32::from_be_bytes(self.array()?))
    }
    pub(crate) fn be_u64(&mut self) -> Result<u64, ProbeError> {
        Ok(u64::from_be_bytes(self.array()?))
    }
    pub(crate) fn le_u16(&mut self) -> Result<u16, ProbeError> {
        Ok(u16::from_le_bytes(self.array()?))
    }
    pub(crate) fn le_u32(&mut self) -> Result<u32, ProbeError> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    pub(crate) fn fourcc(&mut self) -> Result<[u8; 4], ProbeError> {
        self.array()
    }
}

/// Reads exactly `n` bytes (bounded by [`crate::MAX_HEADER_BYTES`]) from a reader.
pub(crate) fn read_vec<R: std::io::Read>(r: &mut R, n: u64) -> Result<Vec<u8>, ProbeError> {
    if n > crate::MAX_HEADER_BYTES {
        return Err(ProbeError::TooLarge);
    }
    let n = usize::try_from(n).map_err(|_| ProbeError::TooLarge)?;
    let mut v = Vec::new();
    v.try_reserve_exact(n).map_err(|_| ProbeError::TooLarge)?;
    v.resize(n, 0);
    r.read_exact(&mut v)?;
    Ok(v)
}
