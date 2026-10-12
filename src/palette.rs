use crate::image::{Stats, ThermalImage};
use egui::{Color32, ColorImage};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Palette {
    #[default]
    Inferno,
    Iron,
    Jet,
    Hot,
    Lava,
    Plasma,
    Rainbow,
    Arctic,
    Grayscale,
}

type Stop = (f32, [u8; 3]);

const INFERNO: &[Stop] = &[
    (0.0, [0, 0, 4]),
    (0.2, [40, 11, 84]),
    (0.4, [132, 32, 107]),
    (0.6, [213, 62, 40]),
    (0.8, [245, 138, 30]),
    (0.95, [251, 229, 46]),
    (1.0, [252, 255, 164]),
];
const IRON: &[Stop] = &[
    (0.0, [0, 0, 0]),
    (0.15, [12, 7, 34]),
    (0.3, [84, 4, 50]),
    (0.45, [168, 13, 51]),
    (0.6, [230, 76, 30]),
    (0.75, [245, 160, 41]),
    (0.9, [252, 222, 90]),
    (1.0, [255, 255, 220]),
];
const JET: &[Stop] = &[
    (0.0, [0, 0, 131]),
    (0.15, [0, 60, 170]),
    (0.35, [5, 185, 205]),
    (0.5, [45, 215, 90]),
    (0.65, [195, 224, 30]),
    (0.8, [255, 175, 0]),
    (0.9, [255, 90, 0]),
    (1.0, [128, 0, 0]),
];
const PLASMA: &[Stop] = &[
    (0.0, [13, 8, 135]),
    (0.25, [126, 3, 168]),
    (0.5, [203, 71, 121]),
    (0.75, [244, 157, 58]),
    (0.9, [248, 211, 104]),
    (1.0, [240, 249, 33]),
];
const HOT: &[Stop] = &[
    (0.0, [0, 0, 0]),
    (0.3, [128, 0, 0]),
    (0.6, [255, 0, 0]),
    (0.8, [255, 128, 0]),
    (0.95, [255, 255, 0]),
    (1.0, [255, 255, 255]),
];
const ARCTIC: &[Stop] = &[
    (0.0, [0, 0, 30]),
    (0.3, [0, 60, 130]),
    (0.5, [20, 140, 200]),
    (0.7, [80, 210, 230]),
    (0.85, [180, 240, 245]),
    (1.0, [240, 255, 255]),
];
const LAVA: &[Stop] = &[
    (0.0, [0, 0, 0]),
    (0.2, [33, 0, 0]),
    (0.4, [130, 15, 0]),
    (0.55, [220, 60, 0]),
    (0.7, [245, 135, 10]),
    (0.85, [255, 220, 60]),
    (1.0, [255, 250, 190]),
];
const GRAYSCALE: &[Stop] = &[(0.0, [0, 0, 0]), (1.0, [255, 255, 255])];

impl Palette {
    pub const ALL: [Palette; 9] = [
        Palette::Inferno,
        Palette::Iron,
        Palette::Jet,
        Palette::Hot,
        Palette::Lava,
        Palette::Plasma,
        Palette::Rainbow,
        Palette::Arctic,
        Palette::Grayscale,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Palette::Inferno => "inferno",
            Palette::Iron => "iron",
            Palette::Jet => "jet",
            Palette::Hot => "hot",
            Palette::Lava => "lava",
            Palette::Plasma => "plasma",
            Palette::Rainbow => "rainbow",
            Palette::Arctic => "arctic",
            Palette::Grayscale => "grayscale",
        }
    }

    pub fn named(name: &str) -> Option<Palette> {
        Palette::ALL.into_iter().find(|p| p.label().eq_ignore_ascii_case(name))
    }

    fn stops(self) -> &'static [Stop] {
        match self {
            Palette::Inferno => INFERNO,
            Palette::Iron => IRON,
            Palette::Jet => JET,
            Palette::Hot => HOT,
            Palette::Lava => LAVA,
            Palette::Plasma => PLASMA,
            Palette::Arctic => ARCTIC,
            Palette::Grayscale | Palette::Rainbow => GRAYSCALE,
        }
    }

    pub fn rgb(self, t: f32) -> [u8; 3] {
        if self == Palette::Rainbow {
            let wave = |shift: f32| {
                (((t - shift) * std::f32::consts::PI).sin() * 255.0).floor().max(0.0) as u8
            };
            return [wave(0.0), wave(0.33), wave(0.66)];
        }
        let stops = self.stops();
        let t = t.clamp(0.0, 1.0);
        let (first, last) = (stops[0], stops[stops.len() - 1]);
        if t <= first.0 {
            return first.1;
        }
        if t >= last.0 {
            return last.1;
        }
        for pair in stops.windows(2) {
            let ((a, ca), (b, cb)) = (pair[0], pair[1]);
            if t >= a && t <= b {
                let f = (t - a) / (b - a);
                return [0, 1, 2]
                    .map(|k| (ca[k] as f32 + (cb[k] as f32 - ca[k] as f32) * f).floor() as u8);
            }
        }
        [0, 0, 0]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Mapping {
    #[default]
    Linear,
    Log,
    Equalize,
}

impl Mapping {
    pub const ALL: [Mapping; 3] = [Mapping::Linear, Mapping::Log, Mapping::Equalize];

    pub fn label(self) -> &'static str {
        match self {
            Mapping::Linear => "linear",
            Mapping::Log => "log",
            Mapping::Equalize => "equalise",
        }
    }

    pub fn explain(self) -> &'static str {
        match self {
            Mapping::Linear => "colour spread evenly across the window",
            Mapping::Log => "more colour at the cold end",
            Mapping::Equalize => "colour spread by how many pixels sit at each temperature",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Overscan {
    #[default]
    Clip,
    Hide,
    Below,
    Above,
}

impl Overscan {
    pub const ALL: [Overscan; 4] =
        [Overscan::Clip, Overscan::Hide, Overscan::Below, Overscan::Above];

    pub fn label(self) -> &'static str {
        match self {
            Overscan::Clip => "clip",
            Overscan::Hide => "hide",
            Overscan::Below => "below",
            Overscan::Above => "above",
        }
    }

    pub fn explain(self) -> &'static str {
        match self {
            Overscan::Clip => "out of window takes the colour at the nearest end",
            Overscan::Hide => "out of window is transparent",
            Overscan::Below => "below the window is navy",
            Overscan::Above => "above the window is white",
        }
    }
}

pub const BELOW: [u8; 3] = [0, 0, 128];
pub const ABOVE: [u8; 3] = [255, 255, 255];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub palette: Palette,
    pub min: f32,
    pub max: f32,
    pub mapping: Mapping,
    pub overscan: Overscan,
    pub inverted: bool,
}

impl Look {
    pub fn fraction(&self, c: f32, stats: &Stats) -> f32 {
        let (lo, hi) = (self.min, self.max);
        if c <= lo {
            return 0.0;
        }
        if c >= hi {
            return 1.0;
        }
        let span = if hi > lo { hi - lo } else { 1.0 };
        match self.mapping {
            Mapping::Linear => (c - lo) / span,
            Mapping::Log => {
                let shift = lo - 1.0;
                let a = (lo - shift).max(0.001).ln();
                let b = (hi - shift).ln();
                let d = if b != a { b - a } else { 1.0 };
                ((c - shift).ln() - a) / d
            }
            Mapping::Equalize => {
                let (fa, fb) = (stats.fraction_below(lo), stats.fraction_below(hi));
                if fb - fa > 1e-6 {
                    ((stats.fraction_below(c) - fa) / (fb - fa)).clamp(0.0, 1.0)
                } else {
                    (c - lo) / span
                }
            }
        }
    }

    pub fn rgba(&self, c: f32, stats: &Stats) -> [u8; 4] {
        let solid = |rgb: [u8; 3]| [rgb[0], rgb[1], rgb[2], 255];
        let edge =
            |end: bool| solid(self.palette.rgb(if end != self.inverted { 1.0 } else { 0.0 }));
        if c.is_nan() {
            return [0, 0, 0, 0];
        }
        if c < self.min {
            return match self.overscan {
                Overscan::Below => solid(BELOW),
                Overscan::Hide => [0, 0, 0, 0],
                _ => edge(false),
            };
        }
        if c > self.max {
            return match self.overscan {
                Overscan::Above => solid(ABOVE),
                Overscan::Hide => [0, 0, 0, 0],
                _ => edge(true),
            };
        }
        let t = self.fraction(c, stats);
        solid(self.palette.rgb(if self.inverted { 1.0 - t } else { t }))
    }

    pub fn color(&self, c: f32, stats: &Stats) -> Color32 {
        let [r, g, b, a] = self.rgba(c, stats);
        Color32::from_rgba_unmultiplied(r, g, b, a)
    }

    pub fn render(&self, image: &ThermalImage) -> ColorImage {
        let pixels = image.celsius.iter().map(|&c| self.color(c, &image.stats)).collect();
        ColorImage::new([image.width, image.height], pixels)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Unit {
    #[default]
    Celsius,
    Fahrenheit,
}

impl Unit {
    pub fn symbol(self) -> &'static str {
        match self {
            Unit::Celsius => "°C",
            Unit::Fahrenheit => "°F",
        }
    }

    pub fn from_celsius(self, c: f64) -> f64 {
        match self {
            Unit::Celsius => c,
            Unit::Fahrenheit => c * 9.0 / 5.0 + 32.0,
        }
    }

    pub fn to_celsius(self, v: f64) -> f64 {
        match self {
            Unit::Celsius => v,
            Unit::Fahrenheit => (v - 32.0) * 5.0 / 9.0,
        }
    }

    pub fn figure(self, c: f32) -> String {
        format!("{:.1}", self.from_celsius(c as f64))
    }

    pub fn reading(self, c: f32) -> String {
        format!("{}{}", self.figure(c), self.symbol())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::Stats;

    fn look() -> Look {
        Look {
            palette: Palette::Grayscale,
            min: 0.0,
            max: 100.0,
            mapping: Mapping::Linear,
            overscan: Overscan::Clip,
            inverted: false,
        }
    }

    #[test]
    fn gradients_interpolate_and_floor_like_the_canvas_version() {
        assert_eq!(Palette::Grayscale.rgb(0.5), [127, 127, 127]);
        assert_eq!(Palette::Inferno.rgb(0.1), [20, 5, 44]);
        assert_eq!(Palette::Inferno.rgb(1.0), [252, 255, 164]);
        assert_eq!(Palette::Rainbow.rgb(0.0), [0, 0, 0]);
        assert_eq!(Palette::Rainbow.rgb(0.5)[0], 255);
    }

    #[test]
    fn overscan_decides_the_colour_outside_the_window() {
        let stats = Stats::of(&[0.0, 100.0], 2);
        let mut l = look();
        assert_eq!(l.rgba(-5.0, &stats), [0, 0, 0, 255]);
        assert_eq!(l.rgba(150.0, &stats), [255, 255, 255, 255]);
        l.inverted = true;
        assert_eq!(l.rgba(-5.0, &stats), [255, 255, 255, 255]);
        l.overscan = Overscan::Hide;
        assert_eq!(l.rgba(150.0, &stats)[3], 0);
        l.overscan = Overscan::Below;
        assert_eq!(l.rgba(-1.0, &stats), [0, 0, 128, 255]);
    }

    #[test]
    fn equalising_spreads_a_crowded_window_over_the_whole_palette() {
        let mut celsius: Vec<f32> = (0..90).map(|i| 20.0 + i as f32 * 0.01).collect();
        celsius.extend((0..10).map(|i| 30.0 + i as f32 * 7.0));
        let stats = Stats::of(&celsius, 10);
        let l = Look { mapping: Mapping::Equalize, min: 20.0, max: 93.0, ..look() };
        assert!(l.fraction(20.9, &stats) > 0.8);
        let narrow = Look { min: 20.0, max: 21.0, ..l };
        let mid = narrow.fraction(20.45, &stats);
        assert!((mid - 0.5).abs() < 0.05, "{mid}");
    }

    #[test]
    fn fahrenheit_round_trips() {
        assert_eq!(Unit::Fahrenheit.reading(100.0), "212.0°F");
        assert!(
            (Unit::Fahrenheit.to_celsius(Unit::Fahrenheit.from_celsius(-40.0)) + 40.0).abs() < 1e-9
        );
    }
}
