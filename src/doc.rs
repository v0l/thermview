use crate::image::{Params, ThermalImage};
use crate::palette::{Look, Mapping, Overscan, Palette, Unit};

#[derive(Clone, Debug, PartialEq)]
pub struct Point {
    pub id: u32,
    pub x: usize,
    pub y: usize,
    pub label: String,
    pub colour: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Overlays {
    pub extremes: bool,
    pub info: bool,
    pub time: bool,
}

impl Default for Overlays {
    fn default() -> Self {
        Overlays { extremes: true, info: true, time: true }
    }
}

pub struct Doc {
    pub name: String,
    pub stamp: Option<String>,
    pub source: ThermalImage,
    pub image: ThermalImage,
    pub look: Look,
    pub unit: Unit,
    pub scale: u32,
    pub label_size: f32,
    pub overlays: Overlays,
    pub points: Vec<Point>,
    pub revision: u64,
    next_id: u32,
}

impl Doc {
    pub fn new(source: ThermalImage, name: &str, modified: Option<String>) -> Doc {
        let look = Look {
            palette: Palette::default(),
            min: source.stats.min.celsius,
            max: source.stats.max.celsius,
            mapping: Mapping::default(),
            overscan: Overscan::default(),
            inverted: false,
        };
        let stamp = source.camera.captured.as_deref().and_then(exif_stamp).or(modified);
        Doc {
            name: name.to_string(),
            stamp,
            image: source.clone(),
            source,
            look,
            unit: Unit::default(),
            scale: 3,
            label_size: 10.0,
            overlays: Overlays::default(),
            points: Vec::new(),
            revision: 0,
            next_id: 0,
        }
    }

    pub fn params(&self) -> Params {
        self.image.params
    }

    pub fn set_params(&mut self, params: Params) {
        if params != self.image.params && self.image.recomputable() {
            self.image = self.source.recompute(params);
            self.revision += 1;
        }
    }

    pub fn auto_range(&mut self) {
        self.look.min = self.image.stats.min.celsius;
        self.look.max = self.image.stats.max.celsius;
    }

    pub fn add_point(&mut self, x: usize, y: usize) {
        self.next_id += 1;
        let colour = self.points.len() % crate::overlay::POINT_COLOURS.len();
        self.points.push(Point { id: self.next_id, x, y, label: self.next_id.to_string(), colour });
    }

    pub fn reading(&self, p: &Point) -> f32 {
        self.image.at(p.x, p.y).unwrap_or(f32::NAN)
    }
}

pub fn exif_stamp(s: &str) -> Option<String> {
    let s = s.trim();
    let (date, time) = s.split_once(' ').unwrap_or((s, ""));
    let date: Vec<&str> = date.split([':', '-']).collect();
    if date.len() != 3 || date.iter().any(|p| p.parse::<u32>().is_err()) {
        return Some(s.to_string());
    }
    let hm: Vec<&str> = time.split(':').take(2).collect();
    let mut out = date.join("-");
    if hm.len() == 2 {
        out += &format!(" {}:{}", hm[0], hm[1]);
    }
    Some(out)
}

pub fn file_stamp(t: std::time::SystemTime) -> Option<String> {
    let t: chrono::DateTime<chrono::Local> = t.into();
    Some(t.format("%Y-%m-%d %H:%M").to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn exif_dates_read_as_iso_minutes() {
        assert_eq!(super::exif_stamp("2017:09:08 16:04:36").as_deref(), Some("2017-09-08 16:04"));
        assert_eq!(super::exif_stamp("sometime").as_deref(), Some("sometime"));
    }
}
