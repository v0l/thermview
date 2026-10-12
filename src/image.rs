use crate::planck::{Conversion, Planck, tau};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Irg,
    Hikmicro,
    Dji,
    FlirAff,
    FlirJpeg,
}

impl Format {
    pub fn label(self) -> &'static str {
        match self {
            Format::Irg => "Infiray IRG",
            Format::Hikmicro => "Hikmicro JPEG",
            Format::Dji => "DJI R-JPEG",
            Format::FlirAff => "FLIR AFF",
            Format::FlirJpeg => "FLIR R-JPEG",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spot {
    pub x: usize,
    pub y: usize,
    pub celsius: f32,
}

pub const CDF_BINS: usize = 1024;

#[derive(Clone, Debug)]
pub struct Stats {
    pub min: Spot,
    pub max: Spot,
    pub cdf: Vec<f32>,
    pub cdf_lo: f32,
    pub cdf_hi: f32,
}

impl Stats {
    pub fn of(celsius: &[f32], width: usize) -> Stats {
        let at = |i: usize, c: f32| Spot { x: i % width.max(1), y: i / width.max(1), celsius: c };
        let mut min = at(0, f32::INFINITY);
        let mut max = at(0, f32::NEG_INFINITY);
        for (i, &c) in celsius.iter().enumerate() {
            if !c.is_finite() || c <= -273.1 {
                continue;
            }
            if c < min.celsius {
                min = at(i, c);
            }
            if c > max.celsius {
                max = at(i, c);
            }
        }
        if !min.celsius.is_finite() {
            min = at(0, 0.0);
            max = at(0, 0.0);
        }
        let (lo, hi) = (min.celsius, max.celsius);
        let span = if hi > lo { hi - lo } else { 1.0 };
        let mut hist = vec![0u32; CDF_BINS];
        let mut counted = 0u32;
        for &c in celsius {
            if !c.is_finite() || c <= -273.1 {
                continue;
            }
            let bin = (((c - lo) / span) * (CDF_BINS - 1) as f32).floor();
            hist[(bin.max(0.0) as usize).min(CDF_BINS - 1)] += 1;
            counted += 1;
        }
        let mut acc = 0u32;
        let cdf = hist
            .iter()
            .map(|h| {
                acc += h;
                acc as f32 / counted.max(1) as f32
            })
            .collect();
        Stats { min, max, cdf, cdf_lo: lo, cdf_hi: hi }
    }

    pub fn fraction_below(&self, c: f32) -> f32 {
        let span = if self.cdf_hi > self.cdf_lo { self.cdf_hi - self.cdf_lo } else { 1.0 };
        let t = ((c - self.cdf_lo) / span).clamp(0.0, 1.0);
        let at = t * (CDF_BINS - 1) as f32;
        let lo = at.floor() as usize;
        let hi = (lo + 1).min(CDF_BINS - 1);
        self.cdf[lo] + (self.cdf[hi] - self.cdf[lo]) * (at - lo as f32)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Params {
    pub emissivity: f64,
    pub distance: f64,
    pub reflected: f64,
    pub air: f64,
    pub humidity: f64,
}

impl Default for Params {
    fn default() -> Self {
        Params { emissivity: 0.95, distance: 1.0, reflected: 20.0, air: 20.0, humidity: 50.0 }
    }
}

#[derive(Clone, Debug)]
pub struct Radiometry {
    pub counts: Vec<u16>,
    pub planck: Planck,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CameraInfo {
    pub make: Option<String>,
    pub model: Option<String>,
    pub software: Option<String>,
    pub serial: Option<String>,
    pub focal_length: Option<f64>,
    pub f_number: Option<f64>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub captured: Option<String>,
}

impl CameraInfo {
    pub fn make_model(&self) -> Option<String> {
        let s = [&self.make, &self.model]
            .into_iter()
            .flatten()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        (!s.is_empty()).then_some(s)
    }

    pub fn lens(&self) -> Option<String> {
        let mut parts = Vec::new();
        if let Some(f) = self.focal_length.filter(|f| *f > 0.0) {
            parts.push(format!("{}mm", trim_float(f)));
        }
        if let Some(n) = self.f_number.filter(|n| *n > 0.0) {
            parts.push(format!("f/{}", trim_float(n)));
        }
        (!parts.is_empty()).then(|| parts.join(" "))
    }
}

fn trim_float(v: f64) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

#[derive(Clone, Debug)]
pub struct ThermalImage {
    pub format: Format,
    pub width: usize,
    pub height: usize,
    pub celsius: Vec<f32>,
    pub stats: Stats,
    pub params: Params,
    pub atm_trans: f64,
    pub radiometry: Option<Arc<Radiometry>>,
    pub camera: CameraInfo,
}

impl ThermalImage {
    pub fn new(format: Format, width: usize, height: usize, celsius: Vec<f32>) -> Self {
        let stats = Stats::of(&celsius, width);
        ThermalImage {
            format,
            width,
            height,
            celsius,
            stats,
            params: Params::default(),
            atm_trans: 1.0,
            radiometry: None,
            camera: CameraInfo::default(),
        }
    }

    pub fn at(&self, x: usize, y: usize) -> Option<f32> {
        (x < self.width && y < self.height).then(|| self.celsius[y * self.width + x])
    }

    pub fn recomputable(&self) -> bool {
        self.radiometry.is_some()
    }

    pub fn recompute(&self, params: Params) -> ThermalImage {
        let Some(r) = &self.radiometry else { return self.clone() };
        let conv = Conversion::new(&params, &r.planck);
        let celsius: Vec<f32> = r.counts.iter().map(|&c| conv.celsius(c as f64) as f32).collect();
        ThermalImage {
            format: self.format,
            width: self.width,
            height: self.height,
            stats: Stats::of(&celsius, self.width),
            celsius,
            params,
            atm_trans: tau(params.distance, params.air, params.humidity),
            radiometry: self.radiometry.clone(),
            camera: self.camera.clone(),
        }
    }
}
