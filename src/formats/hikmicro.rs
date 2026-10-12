use crate::Error;
use crate::bytes::{Bytes, find};
use crate::image::{Format, ThermalImage};

const MARK: &[u8] = &[0xFF, 0xD9, b'H', b'D', b'R', b'I'];

pub fn hdri_start(data: &[u8]) -> Option<usize> {
    find(data, MARK, 0).map(|i| i + 2)
}

pub fn parse(data: &[u8]) -> Result<ThermalImage, Error> {
    let start = hdri_start(data).ok_or_else(|| Error::new("no HDRI thermal block"))?;
    let b = Bytes::le(&data[start..]);
    let magic = b.u16(4)?;
    if magic != 0x1028 {
        return Err(Error::new(format!("unknown HDRI magic 0x{magic:x}")));
    }
    let width = b.u32(12)? as usize;
    let height = b.u32(16)? as usize;
    if width == 0 || height == 0 || width > 4096 || height > 4096 {
        return Err(Error::new(format!("invalid HDRI size {width}x{height}")));
    }
    let pixels = b
        .data
        .get(44..44 + width * height * 2)
        .ok_or_else(|| Error::new("HDRI pixel data truncated"))?;
    let celsius =
        pixels.chunks_exact(2).map(|p| (u16::from_le_bytes([p[0], p[1]]) as f64 / 100.0) as f32);
    Ok(ThermalImage::new(Format::Hikmicro, width, height, celsius.collect()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic(width: u32, height: u32, centi: &[u16]) -> Vec<u8> {
        let mut d = vec![0xFF, 0xD8, 0xFF, 0xD9, 0xFF, 0xD8, 0x00, 0xFF, 0xD9];
        let mut hdri = vec![0u8; 44];
        hdri[0..4].copy_from_slice(b"HDRI");
        hdri[4..6].copy_from_slice(&0x1028u16.to_le_bytes());
        hdri[12..16].copy_from_slice(&width.to_le_bytes());
        hdri[16..20].copy_from_slice(&height.to_le_bytes());
        d.extend(hdri);
        for c in centi {
            d.extend(c.to_le_bytes());
        }
        d
    }

    #[test]
    fn hdri_after_the_second_jpeg_is_centi_celsius() {
        let data = synthetic(2, 2, &[2150, 3000, 1999, 4567]);
        let img = crate::formats::parse(&data, Some("x.jpg")).unwrap();
        assert_eq!((img.format, img.width, img.height), (Format::Hikmicro, 2, 2));
        assert!((img.at(1, 1).unwrap() - 45.67).abs() < 1e-4);
        assert_eq!((img.stats.min.x, img.stats.min.y), (0, 1));
    }

    #[test]
    fn oversized_dimensions_are_refused() {
        assert!(crate::formats::parse(&synthetic(5000, 1, &[]), None).is_err());
    }
}
