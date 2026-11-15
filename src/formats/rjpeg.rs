use super::flir::{Record, records};
use crate::Error;
use crate::bytes::{Bytes, find, jpeg_segments};
use crate::image::{Format, Params, Radiometry, ThermalImage};
use crate::planck::{Conversion, Environment, Planck};
use std::sync::Arc;

const PNG_SIGNATURE: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

pub fn parse(data: &[u8]) -> Result<ThermalImage, Error> {
    let fff = flir_app1(data)?;
    parse_fff(&fff, Format::FlirJpeg)
}

pub(crate) fn parse_fff(fff: &[u8], format: Format) -> Result<ThermalImage, Error> {
    let recs = records(fff)?;
    let find_rec = |kind: u16| -> Result<&Record, Error> {
        recs.iter()
            .find(|r| r.kind == kind)
            .ok_or_else(|| Error::new(format!("no record of type {kind}")))
    };

    let cam = find_rec(32)?;
    let base = cam.offset;
    let camera = &fff[base..base + cam.length];
    let b = Bytes::le(camera);
    let b = b.order(b.u16(0)? < 0x0100);
    let f = |at: usize| -> Result<f64, Error> {
        let value = b.f32(at)? as f64;
        if !value.is_finite() {
            return Err(Error::new(format!("nonfinite CameraInfo field at {at:#x}")));
        }
        Ok(value)
    };
    let emissivity = f(0x20)?;
    let distance = f(0x24)?;
    let reflected_k = f(0x28)?;
    let air_k = f(0x2c)?;
    let stored_humidity = f(0x3c)?;
    // Match ExifTool's FFF convention: values above 2 are percentages.
    let humidity = if stored_humidity > 2.0 { stored_humidity } else { stored_humidity * 100.0 };
    let planck =
        Planck { r1: f(0x58)?, b: f(0x5c)?, f: f(0x60)?, o: b.i32(0x308)? as f64, r2: f(0x30c)? };
    let window_k = f(0x30)?;
    let environment = Environment {
        alpha1: f(0x70)?,
        alpha2: f(0x74)?,
        beta1: f(0x78)?,
        beta2: f(0x7c)?,
        x: f(0x80)?,
        window_temperature: window_k - 273.15,
        window_transmission: f(0x34)?,
    };
    if planck.r1 <= 0.0 || planck.b <= 0.0 || planck.r2 <= 0.0 {
        return Err(Error::new("invalid CameraInfo Planck calibration"));
    }
    if emissivity <= 0.0
        || emissivity > 1.0
        || distance < 0.0
        || reflected_k <= 0.0
        || air_k <= 0.0
        || window_k <= 0.0
        || !(0.0..=100.0).contains(&humidity)
        || environment.window_transmission <= 0.0
        || environment.window_transmission > 1.0
    {
        return Err(Error::new("invalid CameraInfo environmental settings"));
    }

    let raw = find_rec(1)?;
    let (width, height, counts) = raw_counts(&fff[raw.offset..raw.offset + raw.length])
        .map_err(|e| Error::new(format!("raw data: {e}")))?;

    let params = Params {
        emissivity,
        distance,
        reflected: reflected_k - 273.15,
        air: air_k - 273.15,
        humidity,
    };
    let conv = Conversion::with_environment(&params, &planck, &environment);
    let celsius = counts.iter().map(|&c| conv.celsius(c as f64) as f32).collect();
    let mut image = ThermalImage::new(format, width, height, celsius);
    image.params = params;
    image.atm_trans = environment.tau(params.distance, params.air, params.humidity);
    image.radiometry = Some(Arc::new(Radiometry { counts, planck, environment }));
    Ok(image)
}

fn flir_app1(data: &[u8]) -> Result<Vec<u8>, Error> {
    let mut chunks: Vec<(u8, &[u8])> = Vec::new();
    for s in jpeg_segments(data) {
        let p = s.payload;
        if s.marker != 0xE1 || p.len() < 8 || !p.starts_with(b"FLIR\0") {
            continue;
        }
        if chunks.iter().any(|(n, _)| *n == p[6]) {
            return Err(Error::new(format!("duplicate FLIR chunk {}", p[6])));
        }
        chunks.push((p[6], &p[8..]));
    }
    if chunks.is_empty() {
        return Err(Error::new("no FLIR APP1 data"));
    }
    chunks.sort_by_key(|(n, _)| *n);
    Ok(chunks.into_iter().flat_map(|(_, c)| c.iter().copied()).collect())
}

fn raw_counts(rec: &[u8]) -> Result<(usize, usize, Vec<u16>), Error> {
    if let Some(at) = find(rec, PNG_SIGNATURE, 0) {
        return png_counts(&rec[at..]);
    }
    if rec.get(0x20..0x22) == Some(&[0xff, 0xd8]) {
        return jpegls_counts(&rec[0x20..]);
    }
    let b = Bytes::le(rec);
    let b = b.order(b.u16(0)? < 0x0100);
    let (w, h) = (b.u16(2)? as usize, b.u16(4)? as usize);
    if w == 0 || h == 0 || w > 4096 || h > 4096 {
        return Err(Error::new(format!("invalid size {w}x{h}")));
    }
    let counts = (0..w * h).map(|i| b.u16(0x20 + i * 2)).collect::<Result<_, _>>()?;
    Ok((w, h, counts))
}

fn png_counts(png: &[u8]) -> Result<(usize, usize, Vec<u16>), Error> {
    let mut reader = png::Decoder::new(std::io::Cursor::new(png))
        .read_info()
        .map_err(|e| Error::new(e.to_string()))?;
    let info = reader.info();
    let (w, h) = (info.width as usize, info.height as usize);
    if info.bit_depth != png::BitDepth::Sixteen || info.color_type != png::ColorType::Grayscale {
        return Err(Error::new(format!(
            "expected a 16-bit grey PNG, got {:?} {:?}",
            info.bit_depth, info.color_type
        )));
    }
    let mut buf = vec![0; reader.output_buffer_size().ok_or_else(|| Error::new("PNG too large"))?];
    reader.next_frame(&mut buf).map_err(|e| Error::new(e.to_string()))?;
    let counts =
        buf[..w * h * 2].chunks_exact(2).map(|p| u16::from_le_bytes([p[0], p[1]])).collect();
    Ok((w, h, counts))
}

#[cfg(not(target_arch = "wasm32"))]
fn jpegls_counts(data: &[u8]) -> Result<(usize, usize, Vec<u16>), Error> {
    let fail = |e: charls::Error| Error::new(format!("JPEG-LS: {e}"));
    let info = charls::CharLS::default().get_frame_info(data).map_err(fail)?;
    let (w, h) = (info.width as usize, info.height as usize);
    if w == 0
        || h == 0
        || w > 4096
        || h > 4096
        || info.bits_per_sample != 16
        || info.component_count != 1
    {
        return Err(Error::new("expected a 16-bit grayscale thermal JPEG-LS frame"));
    }
    let bytes = decode_jpegls(data).map_err(fail)?;
    if bytes.len() != w * h * 2 {
        return Err(Error::new("JPEG-LS pixel count does not match frame dimensions"));
    }
    let counts = bytes.chunks_exact(2).map(|p| u16::from_ne_bytes([p[0], p[1]])).collect();
    Ok((w, h, counts))
}

// Some FLIR encoders put whole zero padding bytes before EOI. CharLS 2.4.2
// rejects those after decoding the raster. Retry only those end-of-scan errors,
// removing one zero at a time so genuine entropy bits and FF stuffing survive.
// Every accepted result must still pass the decoder's complete-raster checks.
#[cfg(not(target_arch = "wasm32"))]
fn decode_jpegls(data: &[u8]) -> Result<Vec<u8>, charls::Error> {
    let original = match charls::CharLS::default().decode(data) {
        Ok(bytes) => return Ok(bytes),
        Err(e) => e,
    };
    // CharLS C API: 6 = too_much_encoded_data, 12 = marker_start_byte_not_found.
    if !matches!(original, charls::Error::JpegLsError { code: 6 | 12 }) {
        return Err(original);
    }
    let Some(eoi) = data.windows(2).rposition(|p| p == [0xff, 0xd9]) else {
        return Err(original);
    };
    let zeros = data[..eoi].iter().rev().take_while(|&&b| b == 0).count().min(64);
    if zeros == 0 {
        return Err(original);
    }
    let mut padded = data.to_vec();
    for removed in 1..=zeros {
        padded.remove(eoi - removed);
        if let Ok(bytes) = charls::CharLS::default().decode(&padded) {
            return Ok(bytes);
        }
    }
    Err(original)
}

#[cfg(target_arch = "wasm32")]
fn jpegls_counts(_: &[u8]) -> Result<(usize, usize, Vec<u16>), Error> {
    Err(Error::new("JPEG-LS CSQ videos currently require the desktop version of ThermView"))
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod jpegls_tests {
    use super::*;

    #[test]
    fn camera_padding_before_eoi_preserves_every_sensor_count() {
        let counts: Vec<u16> = (0..48).map(|i| 14000 + i * 10).collect();
        let raw: Vec<u8> = counts.iter().flat_map(|c| c.to_ne_bytes()).collect();
        let encoded = charls::CharLS::default()
            .encode(
                charls::FrameInfo { width: 8, height: 6, bits_per_sample: 16, component_count: 1 },
                0,
                &raw,
            )
            .unwrap();
        let eoi = encoded.windows(2).rposition(|p| p == [0xff, 0xd9]).unwrap();
        for padding in [0, 1, 4, 8, 32] {
            let mut padded = encoded.clone();
            padded.splice(eoi..eoi, vec![0; padding]);
            // FFF record alignment may also add zeros after EOI.
            padded.extend([0, 0, 0]);
            let (w, h, decoded) = jpegls_counts(&padded).unwrap();
            assert_eq!((w, h), (8, 6));
            assert_eq!(decoded, counts, "{padding} padding bytes");
        }
        let mut invalid = encoded.clone();
        invalid.insert(eoi, 0x7f);
        assert!(jpegls_counts(&invalid).is_err(), "nonzero encoded data must not be discarded");
        assert!(jpegls_counts(&encoded[..eoi]).is_err(), "missing EOI must not be fabricated");
    }
}

#[cfg(test)]
mod metadata_tests {
    use super::*;

    fn fixture(le: bool) -> Vec<u8> {
        let mut data = vec![0u8; 0x80 + 0x310 + 0x26];
        data[..4].copy_from_slice(b"FFF\0");
        data[0x14..0x18].copy_from_slice(&100u32.to_le_bytes());
        data[0x18..0x1c].copy_from_slice(&0x40u32.to_le_bytes());
        data[0x1c..0x20].copy_from_slice(&2u32.to_le_bytes());
        for (at, kind, offset, length) in [(0x40, 32u16, 0x80u32, 0x310u32), (0x60, 1, 0x390, 0x26)]
        {
            data[at..at + 2].copy_from_slice(&kind.to_le_bytes());
            data[at + 12..at + 16].copy_from_slice(&offset.to_le_bytes());
            data[at + 16..at + 20].copy_from_slice(&length.to_le_bytes());
        }
        data[0x80..0x82].copy_from_slice(&if le { 2u16.to_le_bytes() } else { 2u16.to_be_bytes() });
        for (at, value) in [
            (0x20, 0.96f32),
            (0x24, 5.0),
            (0x28, 290.15),
            (0x2c, 292.15),
            (0x30, 298.15),
            (0x34, 0.85),
            (0x3c, 0.57),
            (0x58, 16002.714),
            (0x5c, 1417.8),
            (0x60, 1.0),
            (0x70, 1.239e-8),
            (0x74, 1.1095e-8),
            (0x78, 0.00318),
            (0x7c, 0.0031802),
            (0x80, 0.732),
            (0x30c, 0.011601289),
        ] {
            let bytes = if le { value.to_le_bytes() } else { value.to_be_bytes() };
            data[0x80 + at..0x84 + at].copy_from_slice(&bytes);
        }
        data[0x388..0x38c].copy_from_slice(&if le {
            (-5030i32).to_le_bytes()
        } else {
            (-5030i32).to_be_bytes()
        });
        data[0x390..0x392].copy_from_slice(&2u16.to_le_bytes());
        data[0x392..0x394].copy_from_slice(&3u16.to_le_bytes());
        data[0x394..0x396].copy_from_slice(&1u16.to_le_bytes());
        for (i, value) in [14000u16, 15000, 16000].into_iter().enumerate() {
            data[0x3b0 + i * 2..0x3b2 + i * 2].copy_from_slice(&value.to_le_bytes());
        }
        data
    }

    #[test]
    fn file_coefficients_and_window_match_external_reference_in_both_byte_orders() {
        // AlexanderProd/csq raw2temp, pinned in the validation report. Includes
        // nonzero distance and an absorbing window, so each correction matters.
        // Kelvin fields round to float32 before conversion back to Celsius.
        for le in [true, false] {
            let image = parse_fff(&fixture(le), Format::FlirCsq).unwrap();
            let expected = [3.680166, 11.523173, 18.756985];
            for (actual, expected) in image.celsius.iter().zip(expected) {
                assert!((*actual as f64 - expected).abs() < 0.0001, "{actual} vs {expected}");
            }
            assert_eq!(image.recompute(image.params).celsius, image.celsius);
            let edited = Params { distance: 8.0, ..image.params };
            let recalibrated = image.recompute(edited);
            assert_ne!(recalibrated.celsius, image.celsius);
            assert_eq!(
                recalibrated.radiometry.as_ref().unwrap().environment,
                image.radiometry.as_ref().unwrap().environment
            );
        }
    }

    #[test]
    fn zero_settings_and_percentage_humidity_are_preserved() {
        let mut data = fixture(true);
        data[0xa4..0xa8].copy_from_slice(&0f32.to_le_bytes());
        data[0xbc..0xc0].copy_from_slice(&0f32.to_le_bytes());
        // Zero Planck O is also legitimate and must not activate a fallback.
        data[0x388..0x38c].copy_from_slice(&0i32.to_le_bytes());
        let image = parse_fff(&data, Format::FlirJpeg).unwrap();
        assert_eq!(image.params.distance, 0.0);
        assert_eq!(image.params.humidity, 0.0);
        assert_eq!(image.atm_trans, 1.0);
        assert_eq!(image.radiometry.as_ref().unwrap().planck.o, 0.0);
        data[0xbc..0xc0].copy_from_slice(&57f32.to_le_bytes());
        assert_eq!(parse_fff(&data, Format::FlirJpeg).unwrap().params.humidity, 57.0);
    }

    #[test]
    fn invalid_or_missing_metadata_is_rejected_without_generic_calibration() {
        for (offset, value) in [(0x70, f32::NAN), (0x30c, 0.0), (0x34, 0.0), (0x24, -1.0)] {
            let mut data = fixture(true);
            data[0x80 + offset..0x84 + offset].copy_from_slice(&value.to_le_bytes());
            assert!(parse_fff(&data, Format::FlirCsq).is_err());
        }
        let mut data = fixture(true);
        // Bytes outside the declared CameraInfo record cannot supply calibration.
        data[0x50..0x54].copy_from_slice(&0x80u32.to_le_bytes());
        assert!(parse_fff(&data, Format::FlirCsq).is_err());
    }
}
