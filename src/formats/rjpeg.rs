use super::flir::{Record, records};
use crate::Error;
use crate::bytes::{Bytes, find, jpeg_segments};
use crate::image::{Format, Params, Radiometry, ThermalImage};
use crate::planck::{Conversion, Planck, tau};
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
    let b = Bytes::le(&fff);
    let b = b.order(b.u16(base)? < 0x0100);
    let f = |at: usize| -> Result<f64, Error> { Ok(b.f32(base + at)? as f64) };
    let emissivity = f(0x20)?;
    let distance = f(0x24)?;
    let reflected_k = f(0x28)?;
    let air_k = f(0x2c)?;
    let humidity = f(0x3c)?;
    let (o, r2) = planck_o_r2(b, cam).unwrap_or((-7340.0, 0.0));
    let planck = Planck { r1: f(0x58)?, b: f(0x5c)?, f: f(0x60)?, o, r2 }.or_defaults();

    let raw = find_rec(1)?;
    let (width, height, counts) = raw_counts(&fff[raw.offset..raw.offset + raw.length])
        .map_err(|e| Error::new(format!("raw data: {e}")))?;

    let params = Params {
        emissivity: if emissivity != 0.0 { emissivity } else { 0.95 },
        distance: if distance != 0.0 { distance } else { 1.0 },
        reflected: if reflected_k > 0.0 { reflected_k - 273.15 } else { 20.0 },
        air: if air_k > 0.0 { air_k - 273.15 } else { 20.0 },
        humidity: if humidity > 0.0 && humidity <= 1.0 { humidity * 100.0 } else { 50.0 },
    };
    let conv = Conversion::new(&params, &planck);
    let celsius = counts.iter().map(|&c| conv.celsius(c as f64) as f32).collect();
    let mut image = ThermalImage::new(format, width, height, celsius);
    image.params = params;
    image.atm_trans = tau(params.distance, params.air, params.humidity);
    image.radiometry = Some(Arc::new(Radiometry { counts, planck }));
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

fn planck_o_r2(b: Bytes, cam: &Record) -> Option<(f64, f64)> {
    let plausible = |o: i32, r2: f32| o > -20000 && o < 20000 && r2 > 0.0 && r2 < 1.0;
    if cam.length >= 0x310 {
        let (o, r2) = (b.i32(cam.offset + 0x308).ok()?, b.f32(cam.offset + 0x30c).ok()?);
        if plausible(o, r2) {
            return Some((o as f64, r2 as f64));
        }
    }
    let le = Bytes::le(b.data);
    (0..b.len().saturating_sub(8))
        .filter_map(|i| {
            let o = le.i32(i).ok()?;
            let r2 = le.f32(i + 4).ok()?;
            (o > -20000 && o < -100 && r2 > 0.005 && r2 < 0.1).then_some((o, r2))
        })
        .min_by_key(|(o, _)| *o)
        .map(|(o, r2)| (o as f64, r2 as f64))
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
