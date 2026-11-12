//! Indexed FLIR CSQ sequences. Keep encoded frames in memory and decode only the
//! requested frame, rather than retaining a temperature raster for every frame.
use crate::Error;
use crate::bytes::{Bytes, find};
use crate::image::{Format, ThermalImage};
use std::ops::Range;

pub struct Video {
    bytes: Vec<u8>,
    frames: Vec<Range<usize>>,
    pub fps: f64,
    pub estimated_fps: bool,
    pub current: usize,
    pub playing: bool,
    pub auto_range: bool,
    pub last_tick: Option<f64>,
}

impl Video {
    pub fn open(bytes: Vec<u8>) -> Result<Self, Error> {
        let mut frames = Vec::new();
        let mut at = 0;
        let mut fps = None;
        while let Some(start) = find(&bytes, b"FFF\0", at) {
            let data = &bytes[start..];
            let records = crate::formats::records(data)
                .map_err(|e| Error::new(format!("CSQ frame {}: {e}", frames.len() + 1)))?;
            if !records.iter().any(|r| r.kind == 1) {
                return Err(Error::new(format!(
                    "CSQ frame {} has no thermal data",
                    frames.len() + 1
                )));
            }
            if frames.is_empty() {
                fps = records.iter().find(|r| r.kind == 32).and_then(|r| {
                    let b = Bytes::le(&data[r.offset..r.offset + r.length]);
                    let b = b.order(b.u16(0).ok()? < 0x100);
                    b.u16(0x464).ok().filter(|v| *v > 0 && *v <= 1000).map(f64::from)
                });
            }
            let end = records
                .iter()
                .map(|r| r.offset + r.length)
                .max()
                .ok_or_else(|| Error::new("CSQ contains an empty frame"))?;
            at = start + end;
            frames.push(start..at);
        }
        if frames.is_empty() {
            return Err(Error::new("CSQ contains no FFF thermal frames"));
        }
        Ok(Self {
            bytes,
            frames,
            fps: fps.unwrap_or(30.0),
            estimated_fps: fps.is_none(),
            current: 0,
            playing: false,
            auto_range: true,
            last_tick: None,
        })
    }

    pub fn len(&self) -> usize {
        self.frames.len()
    }
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    pub fn frame(&self, index: usize) -> Result<ThermalImage, Error> {
        let range = self.frames.get(index).ok_or_else(|| Error::new("frame out of range"))?;
        crate::formats::parse_fff(&self.bytes[range.clone()], Format::FlirCsq)
            .map_err(|e| Error::new(format!("CSQ frame {}: {e}", index + 1)))
    }
}
