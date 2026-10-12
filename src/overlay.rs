use crate::doc::Doc;
use crate::image::Spot;
use crate::palette::Unit;
use egui::{Align2, Color32, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2, pos2, vec2};
use egui_bench::theme::figure;

pub const POINT_COLOURS: [Color32; 8] = [
    Color32::from_rgb(0xef, 0x44, 0x44),
    Color32::from_rgb(0x22, 0xc5, 0x5e),
    Color32::from_rgb(0x3b, 0x82, 0xf6),
    Color32::from_rgb(0xea, 0xb3, 0x08),
    Color32::from_rgb(0x06, 0xb6, 0xd4),
    Color32::from_rgb(0xd9, 0x46, 0xef),
    Color32::from_rgb(0xf9, 0x73, 0x16),
    Color32::from_rgb(0x84, 0xcc, 0x16),
];

pub const HOT: Color32 = Color32::from_rgb(0xef, 0x44, 0x44);
pub const COLD: Color32 = Color32::from_rgb(0x3b, 0x82, 0xf6);

const BOX: Color32 = Color32::from_rgba_unmultiplied_const(0, 0, 0, 217);

#[derive(Clone, Copy)]
pub enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

pub fn paint(p: &Painter, rect: Rect, doc: &Doc) {
    let img = &doc.image;
    let factor = rect.width() / img.width.max(1) as f32;
    let fs = doc.label_size;
    let centre = |x: usize, y: usize| rect.min + vec2(x as f32 + 0.5, y as f32 + 0.5) * factor;

    for pt in &doc.points {
        let colour = POINT_COLOURS[pt.colour % POINT_COLOURS.len()];
        let c = centre(pt.x, pt.y);
        let arm = crosshair(p, c, colour, fs, factor);
        let text = format!("{}: {}", pt.label, doc.unit.reading(doc.reading(pt)));
        label(p, rect, c, &text, colour, fs, arm);
    }

    if doc.overlays.extremes {
        let (lo, hi) = (img.stats.min, img.stats.max);
        if (lo.x, lo.y) != (hi.x, hi.y) {
            extreme(p, rect, centre(lo.x, lo.y), lo, false, doc.unit, fs);
        }
        extreme(p, rect, centre(hi.x, hi.y), hi, true, doc.unit, fs);
    }

    if doc.overlays.info {
        pill(p, rect, &format!("ε={:.2}", img.params.emissivity), Corner::BottomLeft, fs);
        if let Some(m) = img.camera.make_model() {
            pill(p, rect, &m, Corner::TopLeft, fs);
        }
        if let Some(l) = img.camera.lens() {
            pill(p, rect, &l, Corner::TopRight, fs);
        }
    }
    if doc.overlays.time && !doc.name.is_empty() {
        pill(p, rect, doc.stamp.as_deref().unwrap_or(&doc.name), Corner::BottomRight, fs);
    }
}

fn crosshair(p: &Painter, c: Pos2, colour: Color32, fs: f32, factor: f32) -> f32 {
    let f = (fs * 0.25).round().max(1.0);
    let ring = f + factor * 1.5;
    let arm = ring + f;
    let gap = f * 0.5 + factor * 0.5;
    let stroke = Stroke::new((fs * 0.08).round().max(1.0), colour);
    p.circle_stroke(c, ring, stroke);
    for d in [vec2(1.0, 0.0), vec2(-1.0, 0.0), vec2(0.0, 1.0), vec2(0.0, -1.0)] {
        p.line_segment([c + d * gap, c + d * arm], stroke);
    }
    p.circle_filled(c, f * 0.35, colour);
    arm
}

fn boxed(
    p: &Painter,
    at: Rect,
    text: std::sync::Arc<egui::Galley>,
    edge: Color32,
    ink: Color32,
    radius: f32,
) {
    p.rect_filled(at, radius, BOX);
    p.rect_stroke(at, radius, Stroke::new(1.0, edge), StrokeKind::Inside);
    p.galley(at.center() - text.size() / 2.0, text, ink);
}

fn clamp_x(rect: Rect, x: f32, w: f32) -> f32 {
    x.min(rect.right() - w).max(rect.left())
}

fn label(p: &Painter, rect: Rect, c: Pos2, text: &str, colour: Color32, fs: f32, arm: f32) {
    let g = p.layout_no_wrap(text.to_string(), figure(fs), colour);
    let pad = vec2((fs * 0.3).round().max(2.0), (fs * 0.12).round().max(1.0));
    let size = g.size() + pad * 2.0;
    let x = clamp_x(rect, c.x - size.x / 2.0, size.x);
    let at = Rect::from_min_size(pos2(x, c.y + arm + fs * 0.6), size);
    boxed(p, at, g, colour, colour, 2.0);
}

fn extreme(p: &Painter, rect: Rect, c: Pos2, spot: Spot, hot: bool, unit: Unit, fs: f32) {
    let colour = if hot { HOT } else { COLD };
    let tri = (fs * 0.5).round().max(4.0);
    let s = if hot { 1.0 } else { -1.0 };
    let points = vec![
        c + vec2(0.0, -tri * s),
        c + vec2(-tri * 0.75, tri * 0.5 * s),
        c + vec2(tri * 0.75, tri * 0.5 * s),
    ];
    p.add(Shape::convex_polygon(points, colour, Stroke::new(1.0, Color32::from_black_alpha(153))));
    let text = format!("{}: {}", if hot { "MAX" } else { "MIN" }, unit.reading(spot.celsius));
    let g = p.layout_no_wrap(text, figure((fs * 0.65).max(7.0)), colour);
    let size = g.size() + vec2(8.0, 4.0);
    let y = if hot { c.y - tri - size.y - 2.0 } else { c.y + tri + 4.0 };
    let at = Rect::from_min_size(pos2(clamp_x(rect, c.x - size.x / 2.0, size.x), y), size);
    boxed(p, at, g, colour, colour, 2.0);
}

pub fn pill(p: &Painter, rect: Rect, text: &str, corner: Corner, fs: f32) {
    let ink = Color32::from_white_alpha(217);
    let g = p.layout_no_wrap(text.to_string(), figure((fs * 0.6).max(7.0)), ink);
    let size = g.size() + vec2(12.0, 6.0);
    let m = 6.0;
    let (anchor, align) = match corner {
        Corner::TopLeft => (rect.left_top() + vec2(m, m), Align2::LEFT_TOP),
        Corner::TopRight => (rect.right_top() + vec2(-m, m), Align2::RIGHT_TOP),
        Corner::BottomLeft => (rect.left_bottom() + vec2(m, -m), Align2::LEFT_BOTTOM),
        Corner::BottomRight => (rect.right_bottom() + vec2(-m, -m), Align2::RIGHT_BOTTOM),
    };
    let at = align.anchor_size(anchor, size);
    p.rect_filled(at, 3.0, Color32::from_black_alpha(191));
    p.rect_stroke(at, 3.0, Stroke::new(1.0, Color32::from_white_alpha(31)), StrokeKind::Inside);
    p.galley(at.center() - g.size() / 2.0, g, ink);
}

pub fn image_rect(origin: Pos2, doc: &Doc) -> Rect {
    let s = doc.scale as f32;
    Rect::from_min_size(origin, Vec2::new(doc.image.width as f32 * s, doc.image.height as f32 * s))
}
