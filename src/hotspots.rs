//! Lazy, per-frame browsing of local temperature maxima. Equal-temperature
//! plateaus form one peak; spacing suppresses nearby peaks, not anatomy.
use crate::image::{Spot, ThermalImage};

pub struct Hotspots {
    spacing: usize,
    rank: usize,
    spots: Option<Vec<Spot>>,
}

impl Default for Hotspots {
    fn default() -> Self {
        Self { spacing: 5, rank: 0, spots: None }
    }
}

impl Hotspots {
    pub fn invalidate(&mut self) {
        self.rank = 0;
        self.spots = None;
    }
    pub fn spacing(&self) -> usize {
        self.spacing
    }
    pub fn set_spacing(&mut self, pixels: usize) {
        let pixels = pixels.clamp(1, 100);
        if pixels != self.spacing {
            self.spacing = pixels;
            self.invalidate();
        }
    }
    pub fn rank(&self) -> usize {
        self.rank
    }
    pub fn count(&self) -> Option<usize> {
        self.spots.as_ref().map(Vec::len)
    }
    pub fn has_next(&self) -> bool {
        self.count().is_none_or(|n| self.rank + 1 < n)
    }
    pub fn reset(&mut self) {
        self.rank = 0;
    }
    pub fn selected(&self, image: &ThermalImage) -> Option<Spot> {
        match &self.spots {
            Some(spots) => spots.get(self.rank).copied(),
            None => {
                let p = image.stats.max;
                image.at(p.x, p.y).filter(|t| valid(*t)).map(|_| p)
            }
        }
    }
    pub fn next(&mut self, image: &ThermalImage) -> bool {
        let spots = self.spots.get_or_insert_with(|| peaks(image, self.spacing));
        if self.rank + 1 >= spots.len() {
            return false;
        }
        self.rank += 1;
        true
    }
}

fn valid(t: f32) -> bool {
    t.is_finite() && t > -273.1
}

fn peaks(image: &ThermalImage, spacing: usize) -> Vec<Spot> {
    let (w, h) = (image.width, image.height);
    if w == 0 || h == 0 || w.checked_mul(h) != Some(image.celsius.len()) {
        return Vec::new();
    }
    let mut visited = vec![false; image.celsius.len()];
    let mut queue = Vec::new();
    let mut maxima = Vec::new();
    for start in 0..image.celsius.len() {
        let t = image.celsius[start];
        if visited[start] || !valid(t) {
            continue;
        }
        queue.clear();
        queue.push(start);
        visited[start] = true;
        let mut at = 0;
        let mut higher_neighbor = false;
        while at < queue.len() {
            let pixel = queue[at];
            at += 1;
            let (x, y) = (pixel % w, pixel / w);
            for ny in y.saturating_sub(1)..=(y + 1).min(h - 1) {
                for nx in x.saturating_sub(1)..=(x + 1).min(w - 1) {
                    let i = ny * w + nx;
                    let neighbor = image.celsius[i];
                    if !valid(neighbor) {
                        continue;
                    }
                    if neighbor > t {
                        higher_neighbor = true;
                    }
                    if neighbor == t && !visited[i] {
                        visited[i] = true;
                        queue.push(i);
                    }
                }
            }
        }
        if !higher_neighbor {
            maxima.push(Spot { x: start % w, y: start / w, celsius: t });
        }
    }
    // Equal peaks keep deterministic top-to-bottom, left-to-right order.
    maxima.sort_by(|a, b| {
        b.celsius.partial_cmp(&a.celsius).unwrap().then(a.y.cmp(&b.y)).then(a.x.cmp(&b.x))
    });
    let mut blocked = vec![false; image.celsius.len()];
    let mut result = Vec::new();
    for p in maxima {
        if blocked[p.y * w + p.x] {
            continue;
        }
        result.push(p);
        for y in p.y.saturating_sub(spacing)..=p.y.saturating_add(spacing).min(h - 1) {
            for x in p.x.saturating_sub(spacing)..=p.x.saturating_add(spacing).min(w - 1) {
                let dx = x.abs_diff(p.x);
                let dy = y.abs_diff(p.y);
                if dx * dx + dy * dy < spacing * spacing {
                    blocked[y * w + x] = true;
                }
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::Format;

    #[test]
    fn an_eye_peak_is_reachable_after_a_hotter_ear_plateau() {
        let mut temperatures = vec![20.0; 30 * 12];
        for y in 2..=4 {
            for x in 2..=4 {
                temperatures[y * 30 + x] = 38.0;
            }
        }
        temperatures[3 * 30 + 20] = 37.8;
        temperatures[8 * 30 + 25] = 37.5;
        let image = ThermalImage::new(Format::Hikmicro, 30, 12, temperatures);
        let mut browser = Hotspots::default();
        assert_eq!(browser.selected(&image).unwrap().celsius, 38.0);
        assert!(browser.next(&image));
        assert_eq!(browser.selected(&image), Some(Spot { x: 20, y: 3, celsius: 37.8 }));
        assert!(browser.next(&image));
        assert_eq!(browser.selected(&image).unwrap().celsius, 37.5);
        assert!(!browser.next(&image));
        assert_eq!(browser.count(), Some(3));
        assert_eq!(image.stats.max.celsius, 38.0);
        browser.reset();
        assert_eq!(browser.selected(&image).unwrap(), image.stats.max);
    }

    #[test]
    fn spacing_suppresses_nearby_peaks_and_ties_are_stable() {
        let mut values = vec![20.0; 20 * 10];
        values[1 * 20 + 1] = 38.0;
        values[1 * 20 + 3] = 37.9;
        values[1 * 20 + 12] = 37.8;
        values[8 * 20 + 12] = 37.8;
        let image = ThermalImage::new(Format::Hikmicro, 20, 10, values);
        let mut browser = Hotspots::default();
        browser.next(&image);
        assert_eq!(browser.selected(&image).unwrap().x, 12);
        assert_eq!(browser.selected(&image).unwrap().y, 1);
        browser.next(&image);
        assert_eq!(browser.selected(&image).unwrap().y, 8);
        browser.set_spacing(1);
        assert_eq!(browser.rank(), 0);
        browser.next(&image);
        assert_eq!(browser.selected(&image).unwrap().celsius, 37.9);
    }

    #[test]
    fn invalid_pixels_and_flat_images_do_not_create_fake_next_spots() {
        let image = ThermalImage::new(
            Format::Hikmicro,
            3,
            2,
            vec![f32::NAN, -273.15, f32::INFINITY, 25.0, 25.0, 25.0],
        );
        let mut browser = Hotspots::default();
        assert!(!browser.next(&image));
        assert_eq!(browser.count(), Some(1));
        let image = ThermalImage::new(Format::Hikmicro, 1, 1, vec![f32::NAN]);
        browser.invalidate();
        assert!(browser.selected(&image).is_none());
        assert!(!browser.next(&image));
        assert_eq!(browser.count(), Some(0));
    }
}
