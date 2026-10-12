mod dji;
mod flir;
mod hikmicro;
mod irg;
mod rjpeg;

use crate::Error;
use crate::bytes::{find, is_jpeg, jpeg_segments};
use crate::image::{Format, ThermalImage};

pub fn parse(data: &[u8], name: Option<&str>) -> Result<ThermalImage, Error> {
    let format = detect(data, name)?;
    let mut image = match format {
        Format::Irg => irg::parse(data),
        Format::Hikmicro => hikmicro::parse(data),
        Format::Dji => dji::parse(data),
        Format::FlirAff => flir::parse(data),
        Format::FlirJpeg => rjpeg::parse(data),
    }
    .map_err(|e| Error::new(format!("{}: {e}", format.label())))?;
    let exif = crate::exif::read(data);
    let fallback = std::mem::take(&mut image.camera);
    image.camera =
        crate::image::CameraInfo { captured: exif.captured.clone().or(fallback.captured), ..exif };
    Ok(image)
}

pub fn detect(data: &[u8], name: Option<&str>) -> Result<Format, Error> {
    if data.starts_with(b"AFF") || data.starts_with(b"FFF") {
        return Ok(Format::FlirAff);
    }
    if is_jpeg(data) {
        if let Some(f) = detect_jpeg(data) {
            return Ok(f);
        }
    } else if is_irg(data) {
        return Ok(Format::Irg);
    }
    let ext = name
        .and_then(|n| n.rsplit_once('.'))
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "irg" => Ok(Format::Irg),
        "img" | "seq" => Ok(Format::FlirAff),
        _ => Err(Error::new(
            "unknown thermal image format, this file has no radiometric data ThermView recognises",
        )),
    }
}

fn is_irg(data: &[u8]) -> bool {
    if data.len() < 4 {
        return false;
    }
    if matches!((data[0], data[1]), (0xCA, 0xAC) | (0xBA, 0xAB) | (0x04, 0xA0)) {
        return true;
    }
    let header = u16::from_le_bytes([data[2], data[3]]) as usize;
    header >= 2 && header < data.len() && data[header - 2..header] == [0xAC, 0xCA]
}

fn detect_jpeg(data: &[u8]) -> Option<Format> {
    let segments = jpeg_segments(data);
    if segments.iter().any(|s| s.marker == 0xE1 && s.payload.starts_with(b"FLIR")) {
        return Some(Format::FlirJpeg);
    }
    if hikmicro::hdri_start(data).is_some() {
        return Some(Format::Hikmicro);
    }
    let app3 = segments.iter().any(|s| s.marker == 0xE3 && s.payload.len() + 2 > 1000);
    let app4 = segments.iter().any(|s| s.marker == 0xE4);
    if app3 && app4 {
        return Some(Format::Dji);
    }
    let flir_app1 = |at: usize| data.get(at + 4..at + 8) == Some(b"FLIR");
    let mut at = 0;
    while let Some(i) = find(data, &[0xFF, 0xE1], at) {
        if flir_app1(i) {
            return Some(Format::FlirJpeg);
        }
        at = i + 1;
    }
    None
}
