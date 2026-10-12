use crate::Error;
use crate::bytes::Bytes;
use crate::image::{Format, Params, ThermalImage};

pub fn parse(data: &[u8]) -> Result<ThermalImage, Error> {
    let b = Bytes::le(data);
    let header = b.u16(2)? as usize;
    if header < 0x2E || header > data.len() {
        return Err(Error::new("header extends past the end of the file"));
    }
    let tail = (data[header - 2], data[header - 1]) == (0xAC, 0xCA);
    let magic = (data[0], data[1]);
    let c20x = magic == (0xCA, 0xAC) && tail;
    let p200 = magic == (0x04, 0xA0);
    if !c20x && !p200 && !tail && magic != (0xBA, 0xAB) {
        return Err(Error::new(format!("unknown magic 0x{:02x}{:02x}", magic.0, magic.1)));
    }

    let coarse_len = b.u32(4)? as usize;
    let height = b.u16(8)? as usize;
    let width = b.u16(10)? as usize;
    let flag0 = b.u16(12)?;
    let emissivity = b.u16(30)? as f64 / 10000.0;
    let distance = b.u32(42)? as f64 / 10000.0;
    if width * height != coarse_len || width == 0 {
        return Err(Error::new(format!("resolution {width}x{height} does not match {coarse_len}")));
    }
    let scale = if c20x && flag0 == 0 { 16.0 } else { 10.0 };

    let fine = header + width * height;
    let end = fine + width * height * 2;
    let raw = data.get(fine..end).ok_or_else(|| Error::new("temperature data truncated"))?;
    let celsius = raw
        .chunks_exact(2)
        .map(|p| (u16::from_le_bytes([p[0], p[1]]) as f64 / scale - 273.15) as f32)
        .collect();

    let mut image = ThermalImage::new(Format::Irg, width, height, celsius);
    image.params = Params {
        emissivity,
        distance,
        reflected: b.u16(36)? as f64 / 1000.0,
        air: b.u16(38)? as f64 / 1000.0,
        humidity: b.u8(0x1D)? as f64,
    };
    image.atm_trans = b.u16(0x2A)? as f64 / 10000.0;
    Ok(image)
}

#[cfg(test)]
pub fn synthetic(magic: [u8; 2], flag0: u16, width: u16, height: u16, kelvin: &[u16]) -> Vec<u8> {
    let header = 0x80usize;
    let mut d = vec![0u8; header];
    d[0..2].copy_from_slice(&magic);
    d[2..4].copy_from_slice(&(header as u16).to_le_bytes());
    d[4..8].copy_from_slice(&(width as u32 * height as u32).to_le_bytes());
    d[8..10].copy_from_slice(&height.to_le_bytes());
    d[10..12].copy_from_slice(&width.to_le_bytes());
    d[12..14].copy_from_slice(&flag0.to_le_bytes());
    d[30..32].copy_from_slice(&9500u16.to_le_bytes());
    d[42..46].copy_from_slice(&15000u32.to_le_bytes());
    d[header - 2] = 0xAC;
    d[header - 1] = 0xCA;
    d.extend(std::iter::repeat_n(0u8, width as usize * height as usize));
    for k in kelvin {
        d.extend(k.to_le_bytes());
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c210_counts_sixteenths_of_a_kelvin() {
        let k = |c: f64| ((c + 273.15) * 16.0).round() as u16;
        let data = synthetic(
            [0xCA, 0xAC],
            0,
            3,
            2,
            &[k(20.0), k(25.0), k(30.0), k(-5.0), k(0.0), k(60.0)],
        );
        let img = crate::formats::parse(&data, None).unwrap();
        assert_eq!((img.format, img.width, img.height), (Format::Irg, 3, 2));
        assert!((img.at(1, 0).unwrap() - 25.0).abs() < 0.05);
        assert_eq!((img.stats.max.x, img.stats.max.y), (2, 1));
        assert_eq!((img.stats.min.x, img.stats.min.y), (0, 1));
        assert!((img.params.emissivity - 0.95).abs() < 1e-9);
        assert!((img.params.distance - 1.5).abs() < 1e-9);
    }

    #[test]
    fn autel_and_other_variants_count_tenths() {
        let data = synthetic([0xBA, 0xAB], 0, 1, 1, &[2981]);
        let img = crate::formats::parse(&data, None).unwrap();
        assert!((img.celsius[0] - 24.95).abs() < 1e-3);
        let data = synthetic([0xCA, 0xAC], 1, 1, 1, &[2981]);
        assert!((crate::formats::parse(&data, None).unwrap().celsius[0] - 24.95).abs() < 1e-3);
    }

    #[test]
    fn a_short_file_is_an_error_not_a_panic() {
        let data = synthetic([0xCA, 0xAC], 0, 4, 4, &[3000; 4]);
        assert!(crate::formats::parse(&data, None).is_err());
    }
}
