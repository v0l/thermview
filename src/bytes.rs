use crate::Error;

#[derive(Clone, Copy)]
pub struct Bytes<'a> {
    pub data: &'a [u8],
    pub le: bool,
}

impl<'a> Bytes<'a> {
    pub fn le(data: &'a [u8]) -> Self {
        Bytes { data, le: true }
    }

    pub fn be(data: &'a [u8]) -> Self {
        Bytes { data, le: false }
    }

    pub fn order(self, le: bool) -> Self {
        Bytes { le, ..self }
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    fn take<const N: usize>(&self, at: usize) -> Result<[u8; N], Error> {
        at.checked_add(N)
            .and_then(|end| self.data.get(at..end))
            .map(|s| s.try_into().unwrap())
            .ok_or_else(|| Error::new(format!("read of {N} bytes at {at} runs past the end")))
    }

    pub fn u8(&self, at: usize) -> Result<u8, Error> {
        Ok(self.take::<1>(at)?[0])
    }

    pub fn u16(&self, at: usize) -> Result<u16, Error> {
        let b = self.take(at)?;
        Ok(if self.le { u16::from_le_bytes(b) } else { u16::from_be_bytes(b) })
    }

    pub fn u32(&self, at: usize) -> Result<u32, Error> {
        let b = self.take(at)?;
        Ok(if self.le { u32::from_le_bytes(b) } else { u32::from_be_bytes(b) })
    }

    pub fn i32(&self, at: usize) -> Result<i32, Error> {
        let b = self.take(at)?;
        Ok(if self.le { i32::from_le_bytes(b) } else { i32::from_be_bytes(b) })
    }

    pub fn f32(&self, at: usize) -> Result<f32, Error> {
        let b = self.take(at)?;
        Ok(if self.le { f32::from_le_bytes(b) } else { f32::from_be_bytes(b) })
    }

    pub fn starts(&self, at: usize, prefix: &[u8]) -> bool {
        self.data.get(at..).is_some_and(|s| s.starts_with(prefix))
    }
}

pub fn find(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    haystack.get(from..)?.windows(needle.len()).position(|w| w == needle).map(|p| p + from)
}

pub struct Segment<'a> {
    pub marker: u8,
    pub payload: &'a [u8],
}

pub fn jpeg_segments(data: &[u8]) -> Vec<Segment<'_>> {
    let mut out = Vec::new();
    if !data.starts_with(&[0xFF, 0xD8]) {
        return out;
    }
    let mut at = 2;
    while at + 4 <= data.len() {
        if data[at] != 0xFF {
            at += 1;
            continue;
        }
        let marker = data[at + 1];
        if marker == 0xFF {
            at += 1;
            continue;
        }
        if marker == 0xD9 || marker == 0xDA {
            break;
        }
        let len = u16::from_be_bytes([data[at + 2], data[at + 3]]) as usize;
        if len < 2 {
            break;
        }
        let end = (at + 2 + len).min(data.len());
        out.push(Segment { marker, payload: &data[at + 4..end] });
        at += 2 + len;
    }
    out
}

pub fn is_jpeg(data: &[u8]) -> bool {
    data.starts_with(&[0xFF, 0xD8])
}
