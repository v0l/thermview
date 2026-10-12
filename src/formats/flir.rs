use crate::Error;
use crate::bytes::Bytes;
use crate::image::{CameraInfo, Format, Params, ThermalImage};
use crate::planck::{Conversion, Planck, tau};

pub struct Record {
    pub kind: u16,
    pub offset: usize,
    pub length: usize,
}

pub fn records(data: &[u8]) -> Result<Vec<Record>, Error> {
    if !(data.starts_with(b"AFF") || data.starts_with(b"FFF")) {
        return Err(Error::new("no FFF/AFF structure"));
    }
    let b = Bytes::be(data);
    let index = b.u32(0x18)? as usize;
    let count = b.u32(0x1c)? as usize;
    if count > 100 || index + count * 32 > data.len() {
        return Err(Error::new("invalid record directory"));
    }
    let mut out = Vec::new();
    for i in 0..count {
        let at = index + i * 32;
        let kind = b.u16(at)?;
        let offset = b.u32(at + 12)? as usize;
        let length = b.u32(at + 16)? as usize;
        if kind >= 1 && offset > 0 && offset + length <= data.len() {
            out.push(Record { kind, offset, length });
        }
    }
    Ok(out)
}

pub fn parse(data: &[u8]) -> Result<ThermalImage, Error> {
    let rec = records(data)?
        .into_iter()
        .rfind(|r| r.kind == 1)
        .ok_or_else(|| Error::new("no thermal data record"))?;
    let base = rec.offset;
    let le = Bytes::le(data).u16(base)? < 0x0100;
    let b = Bytes::le(data).order(le);

    let sensor_w = b.u16(base + 2)? as usize;
    let sensor_h = b.u16(base + 4)? as usize;
    let crop_x1 = b.u16(base + 10)? as i64;
    let crop_x2 = b.u16(base + 12)? as i64;
    let crop_y1 = b.u16(base + 14)? as usize;
    let crop_y2 = b.u16(base + 16)? as i64;
    let crop_w = crop_x2 - crop_x1 + 1;
    let crop_h = crop_y2 - crop_y1 as i64 + 1;
    if crop_w <= 0 || crop_h <= 0 || crop_w > 4096 || crop_h > 4096 {
        return Err(Error::new(format!("invalid crop {crop_w}x{crop_h}")));
    }
    let (w, h) = (crop_w as usize, crop_h as usize);

    let emissivity = b.f32(base + 0x2c)? as f64;
    let distance = b.f32(base + 0x30)? as f64;
    let reflected_k = b.f32(base + 0x34)? as f64;
    let air_k = b.f32(base + 0x3c)? as f64;
    let humidity = b.f32(base + 0x40)? as f64;
    let mut planck = Planck {
        r1: b.f32(base + 0x44)? as f64,
        b: b.f32(base + 0x48)? as f64,
        f: b.f32(base + 0x4c)? as f64,
        o: b.i32(base + 0x10c)? as f64,
        r2: b.f32(base + 0x110)? as f64,
    };
    if planck.r2.is_nan() || planck.r2 == 0.0 || (planck.r2 == 1.0 && planck.o > -100.0) {
        let o = b.i32(base + 0x16c)? as f64;
        let r2 = b.f32(base + 0x170)? as f64;
        if !r2.is_nan() && r2 != 0.0 {
            planck.o = o;
            planck.r2 = r2;
        }
    }

    let pixels = base + 0x298;
    if pixels + sensor_w * sensor_h * 2 > data.len() {
        return Err(Error::new("raw pixel data truncated"));
    }
    let raw_at = |i: usize| -> Result<f64, Error> {
        let (x, y) = (i % w, crop_y1 + i / w);
        Ok(b.u16(pixels + (y * sensor_w + x) * 2)? as f64)
    };
    let raw: Vec<f64> = (0..w * h).map(raw_at).collect::<Result<_, _>>()?;

    let sample = &raw[..raw.len().min(20)];
    let ck = |r: f64| r / 100.0 - 273.15;
    let lo = sample.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = sample.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let centi_kelvin = ck(lo) > -50.0 && ck(hi) < 2000.0;

    let air = if air_k > 0.0 { air_k - 273.15 } else { 20.0 };
    let reflected = if reflected_k > 0.0 { reflected_k - 273.15 } else { 20.0 };
    let rh = if humidity > 0.0 && humidity <= 1.0 { humidity * 100.0 } else { 50.0 };
    let params = Params {
        emissivity: if emissivity != 0.0 { emissivity } else { 0.95 },
        distance: if distance != 0.0 { distance } else { 1.0 },
        reflected,
        air,
        humidity: rh,
    };

    let celsius: Vec<f32> = if centi_kelvin {
        raw.iter().map(|&r| ck(r) as f32).collect()
    } else {
        let conv_params = Params {
            emissivity: if emissivity != 0.0 { emissivity } else { 1.0 },
            distance: params.distance,
            reflected: reflected_k - 273.15,
            air: air_k - 273.15,
            humidity: humidity * 100.0,
        };
        let conv = Conversion::new(&conv_params, &planck.or_defaults());
        raw.iter().map(|&r| conv.celsius(r) as f32).collect()
    };

    let mut image = ThermalImage::new(Format::FlirAff, w, h, celsius);
    image.params = params;
    image.atm_trans = tau(params.distance, air_k - 273.15, humidity * 100.0);
    image.camera = CameraInfo { captured: timestamp(data), ..Default::default() };
    Ok(image)
}

fn timestamp(data: &[u8]) -> Option<String> {
    let raw = data.get(0x1b4..0x1b4 + 18)?;
    let end = raw.iter().position(|&c| c == 0).unwrap_or(raw.len());
    let s = std::str::from_utf8(&raw[..end]).ok()?.trim();
    (!s.is_empty() && s.chars().all(|c| c.is_ascii_graphic() || c == ' ')).then(|| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic(
        sensor: (u16, u16),
        crop_y1: u16,
        kelvin: impl Fn(usize, usize) -> f64,
    ) -> Vec<u8> {
        let mut d = vec![0u8; 0x40];
        d[0..4].copy_from_slice(b"AFF\0");
        d[0x18..0x1c].copy_from_slice(&0x40u32.to_be_bytes());
        d[0x1c..0x20].copy_from_slice(&1u32.to_be_bytes());
        let base = 0x200usize;
        let len = 0x298 + sensor.0 as usize * sensor.1 as usize * 2;
        let mut entry = vec![0u8; 32];
        entry[0..2].copy_from_slice(&1u16.to_be_bytes());
        entry[12..16].copy_from_slice(&(base as u32).to_be_bytes());
        entry[16..20].copy_from_slice(&(len as u32).to_be_bytes());
        d.extend(entry);
        d.resize(base + len, 0);
        let put16 =
            |d: &mut Vec<u8>, at: usize, v: u16| d[at..at + 2].copy_from_slice(&v.to_le_bytes());
        put16(&mut d, base, 2);
        put16(&mut d, base + 2, sensor.0);
        put16(&mut d, base + 4, sensor.1);
        put16(&mut d, base + 10, 0);
        put16(&mut d, base + 12, sensor.0 - 1);
        put16(&mut d, base + 14, crop_y1);
        put16(&mut d, base + 16, sensor.1 - 1);
        d[base + 0x2c..base + 0x30].copy_from_slice(&0.9f32.to_le_bytes());
        d[base + 0x3c..base + 0x40].copy_from_slice(&293.15f32.to_le_bytes());
        for y in 0..sensor.1 as usize {
            for x in 0..sensor.0 as usize {
                let v = (kelvin(x, y) * 100.0).round() as u16;
                put16(&mut d, base + 0x298 + (y * sensor.0 as usize + x) * 2, v);
            }
        }
        d
    }

    #[test]
    fn pm695_frames_are_centi_kelvin_at_sensor_stride() {
        let data = synthetic((5, 4), 1, |x, y| 293.15 + x as f64 + 10.0 * y as f64);
        let img = crate::formats::parse(&data, Some("IR_0001.IMG")).unwrap();
        assert_eq!((img.format, img.width, img.height), (Format::FlirAff, 5, 3));
        assert!((img.at(2, 0).unwrap() - 32.0).abs() < 0.01);
        assert!((img.at(4, 2).unwrap() - 54.0).abs() < 0.01);
        assert!((img.params.emissivity - 0.9).abs() < 1e-6);
        assert!((img.params.air - 20.0).abs() < 1e-3);
    }
}
