use crate::image::CameraInfo;
use exif::{Exif, In, Tag, Value};

fn load(data: &[u8]) -> Option<Exif> {
    exif::Reader::new().read_from_container(&mut std::io::Cursor::new(data)).ok()
}

fn text(e: &Exif, tag: Tag) -> Option<String> {
    let f = e.get_field(tag, In::PRIMARY)?;
    let Value::Ascii(parts) = &f.value else { return None };
    let s = parts.iter().map(|p| String::from_utf8_lossy(p).trim().to_string()).collect::<Vec<_>>();
    let s = s.join(" ").trim_matches(char::from(0)).trim().to_string();
    (!s.is_empty()).then_some(s)
}

fn number(e: &Exif, tag: Tag) -> Option<f64> {
    let f = e.get_field(tag, In::PRIMARY)?;
    match &f.value {
        Value::Rational(v) => v.first().filter(|r| r.denom != 0).map(|r| r.to_f64()),
        Value::SRational(v) => v.first().filter(|r| r.denom != 0).map(|r| r.to_f64()),
        _ => f.value.get_uint(0).map(|v| v as f64),
    }
}

fn degrees(e: &Exif, tag: Tag, sign: Tag, negative: u8) -> Option<f64> {
    let f = e.get_field(tag, In::PRIMARY)?;
    let Value::Rational(v) = &f.value else { return None };
    if v.len() < 3 || v.iter().any(|r| r.denom == 0) {
        return None;
    }
    let d = v[0].to_f64() + v[1].to_f64() / 60.0 + v[2].to_f64() / 3600.0;
    let flip = text(e, sign).is_some_and(|s| s.as_bytes().first() == Some(&negative));
    Some(if flip { -d } else { d })
}

pub fn read(data: &[u8]) -> CameraInfo {
    let Some(e) = load(data) else { return CameraInfo::default() };
    CameraInfo {
        make: text(&e, Tag::Make),
        model: text(&e, Tag::Model),
        software: text(&e, Tag::Software),
        serial: text(&e, Tag::BodySerialNumber),
        focal_length: number(&e, Tag::FocalLength),
        f_number: number(&e, Tag::FNumber),
        latitude: degrees(&e, Tag::GPSLatitude, Tag::GPSLatitudeRef, b'S'),
        longitude: degrees(&e, Tag::GPSLongitude, Tag::GPSLongitudeRef, b'W'),
        captured: text(&e, Tag::DateTimeOriginal).or_else(|| text(&e, Tag::DateTimeDigitized)),
    }
}

pub fn subject_distance(data: &[u8]) -> Option<f64> {
    number(&load(data)?, Tag::SubjectDistance).filter(|d| d.is_finite() && *d > 0.0)
}
