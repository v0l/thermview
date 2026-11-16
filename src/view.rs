//! Image viewport, independent of export resolution and timeline navigation.
use egui::{Pos2, Rect, Vec2, vec2};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImageView {
    /// Magnification relative to fitting the complete image into the viewport.
    pub zoom: f32,
    /// Normalized image coordinates at the centre of the viewport.
    pub center: Vec2,
}

impl Default for ImageView {
    fn default() -> Self {
        Self { zoom: 1.0, center: vec2(0.5, 0.5) }
    }
}

impl ImageView {
    fn size(&self, viewport: Rect, image: Vec2) -> Vec2 {
        let fit = (viewport.width() / image.x.max(1.0)).min(viewport.height() / image.y.max(1.0));
        image * fit.max(0.0001) * self.zoom
    }
    pub fn rect(&mut self, viewport: Rect, image: Vec2) -> Rect {
        let size = self.size(viewport, image);
        for axis in 0..2 {
            let fraction = viewport.size()[axis] / size[axis].max(0.0001) * 0.5;
            self.center[axis] = if fraction >= 0.5 {
                0.5
            } else {
                self.center[axis].clamp(fraction, 1.0 - fraction)
            };
        }
        Rect::from_min_size(viewport.center() - self.center * size, size)
    }
    pub fn zoom_at(&mut self, factor: f32, at: Pos2, viewport: Rect, image: Vec2) {
        if !factor.is_finite() || factor <= 0.0 {
            return;
        }
        let before = self.rect(viewport, image);
        let point = (at - before.min) / before.size();
        self.zoom = (self.zoom * factor).clamp(1.0, 64.0);
        let size = self.size(viewport, image);
        self.center = point - (at - viewport.center()) / size;
        self.rect(viewport, image);
    }
    pub fn pan(&mut self, delta: Vec2, viewport: Rect, image: Vec2) {
        self.center -= delta / self.size(viewport, image);
        self.rect(viewport, image);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cursor_anchor_is_preserved_and_panning_stays_inside_the_image() {
        let viewport = Rect::from_min_size(Pos2::ZERO, vec2(600.0, 400.0));
        let image = vec2(300.0, 200.0);
        let mut view = ImageView::default();
        let at = Pos2::new(200.0, 120.0);
        let before = view.rect(viewport, image);
        let anchor = (at - before.min) / before.size();
        view.zoom_at(3.0, at, viewport, image);
        let after = view.rect(viewport, image);
        assert!(((at - after.min) / after.size() - anchor).length() < 1e-5);
        view.pan(vec2(-10000.0, 10000.0), viewport, image);
        assert!(view.rect(viewport, image).contains_rect(viewport));
        view.zoom_at(0.0001, at, viewport, image);
        assert_eq!(view, ImageView::default());
    }
}
