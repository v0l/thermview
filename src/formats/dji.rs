use crate::Error;
use crate::bytes::{Bytes, jpeg_segments};
use crate::image::{Format, Params, ThermalImage};

const SIZES: [(usize, usize); 3] = [(640, 512), (320, 256), (160, 120)];

pub fn parse(data: &[u8]) -> Result<ThermalImage, Error> {
    let segments = jpeg_segments(data);
    let raw: Vec<u8> =
        segments.iter().filter(|s| s.marker == 0xE3).flat_map(|s| s.payload).copied().collect();
    let calibration = segments.iter().rev().find(|s| s.marker == 0xE4).map(|s| s.payload);
    if raw.is_empty() {
        return Err(Error::new("no APP3 thermal data"));
    }
    let (width, height) =
        SIZES.into_iter().find(|(w, h)| raw.len() >= w * h * 2).ok_or_else(|| {
            Error::new(format!(
                "{} bytes of APP3 data is not a raw frame of a known DJI camera",
                raw.len()
            ))
        })?;
    let celsius = raw[..width * height * 2]
        .chunks_exact(2)
        .map(|p| (u16::from_le_bytes([p[0], p[1]]) as f64 / 64.0 - 273.15) as f32)
        .collect();

    let mut params =
        Params { emissivity: 0.95, distance: 5.0, humidity: 70.0, reflected: 23.0, air: 20.0 };
    if let Some(cal) = calibration.filter(|c| c.len() >= 20) {
        let b = Bytes::le(cal);
        params.distance = (b.f32(0)? as f64).clamp(1.0, 25.0);
        params.humidity = (b.f32(4)? as f64).clamp(20.0, 100.0);
        params.emissivity = (b.f32(8)? as f64).clamp(0.1, 1.0);
        params.reflected = (b.f32(12)? as f64).clamp(-40.0, 500.0);
    }
    if let Some(d) = crate::exif::subject_distance(data) {
        params.distance = d;
    }

    let mut image = ThermalImage::new(Format::Dji, width, height, celsius);
    image.params = params;
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segment(marker: u8, payload: &[u8]) -> Vec<u8> {
        let mut s = vec![0xFF, marker];
        s.extend(((payload.len() + 2) as u16).to_be_bytes());
        s.extend(payload);
        s
    }

    #[test]
    fn app3_chunks_join_into_one_frame_and_app4_carries_calibration() {
        let counts: Vec<u8> =
            (0..160 * 120).flat_map(|i| ((i % 7) as u16 + 19000).to_le_bytes()).collect();
        let mut data = vec![0xFF, 0xD8];
        for chunk in counts.chunks(30000) {
            data.extend(segment(0xE3, chunk));
        }
        let cal: Vec<u8> =
            [3.0f32, 55.0, 0.9, 25.0, 0.0].iter().flat_map(|f| f.to_le_bytes()).collect();
        data.extend(segment(0xE4, &cal));
        data.extend([0xFF, 0xD9]);
        let img = crate::formats::parse(&data, None).unwrap();
        assert_eq!((img.format, img.width, img.height), (Format::Dji, 160, 120));
        assert!((img.celsius[0] - (19000.0 / 64.0 - 273.15)).abs() < 1e-3);
        assert!((img.params.distance - 3.0).abs() < 1e-6);
        assert!((img.params.emissivity - 0.9).abs() < 1e-6);
    }
}
