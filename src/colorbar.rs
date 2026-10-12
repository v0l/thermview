use crate::doc::Doc;
use egui::epaint::{Mesh, Vertex, WHITE_UV};
use egui::{Align2, Color32, Id, Rect, Response, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use egui_bench::theme::{ETCH, LEGEND, READOUT, TRACE, figure, legend_font};

const LEFT_W: f32 = 52.0;
const BAR_W: f32 = 18.0;
const TICK_W: f32 = 40.0;
const RIGHT_W: f32 = 56.0;
pub const WIDTH: f32 = LEFT_W + BAR_W + TICK_W + RIGHT_W;
const MIN_GAP: f32 = 0.1;

#[derive(Clone, Copy)]
struct Drag {
    max: bool,
    lo: f32,
    hi: f32,
}

pub fn axis(doc: &Doc) -> (f32, f32) {
    let s = &doc.image.stats;
    let lo = s.min.celsius.min(doc.look.min);
    let hi = s.max.celsius.max(doc.look.max);
    if hi - lo < 1e-3 { (lo - 0.5, hi + 0.5) } else { (lo, hi) }
}

pub fn ticks(lo: f64, hi: f64, wanted: f64) -> Vec<f64> {
    let raw = ((hi - lo) / wanted.max(1.0)).max(1e-6);
    let mag = 10f64.powf(raw.log10().floor());
    let step = [1.0, 2.0, 5.0, 10.0]
        .into_iter()
        .map(|m| m * mag)
        .find(|s| *s >= raw)
        .unwrap_or(10.0 * mag);
    let mut v = (lo / step).ceil() * step;
    let mut out = Vec::new();
    while v <= hi + step * 1e-6 {
        out.push(if v.abs() < step * 1e-6 { 0.0 } else { v });
        v += step;
    }
    out
}

pub fn show(ui: &mut Ui, doc: &mut Doc, height: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(WIDTH, height), Sense::click_and_drag());
    let id = Id::new("colour-bar");
    let mut drag: Option<Drag> = ui.data(|d| d.get_temp(id));
    let (lo, hi) = drag.map(|d| (d.lo, d.hi)).unwrap_or_else(|| axis(doc));
    let bar = Rect::from_min_size(pos2(rect.left() + LEFT_W, rect.top()), vec2(BAR_W, height));
    let y_of = |c: f32| bar.bottom() - (c - lo) / (hi - lo) * bar.height();
    let c_of =
        |y: f32| lo + (bar.bottom() - y.clamp(bar.top(), bar.bottom())) / bar.height() * (hi - lo);

    if let Some(at) = resp.interact_pointer_pos() {
        if drag.is_none() {
            let near_max = (at.y - y_of(doc.look.max)).abs() < (at.y - y_of(doc.look.min)).abs();
            drag = Some(Drag { max: near_max, lo, hi });
        }
        let d = drag.unwrap();
        let v = c_of(at.y);
        if d.max {
            doc.look.max = v.max(doc.look.min + MIN_GAP);
        } else {
            doc.look.min = v.min(doc.look.max - MIN_GAP);
        }
    }
    if resp.drag_stopped() || resp.clicked() || !resp.is_pointer_button_down_on() {
        drag = None;
    }
    ui.data_mut(|d| match drag {
        Some(v) => {
            d.insert_temp(id, v);
        }
        None => {
            d.remove::<Drag>(id);
        }
    });
    if resp.hovered() || drag.is_some() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }

    if !ui.is_rect_visible(rect) {
        return resp;
    }
    let p = ui.painter();
    p.rect_filled(bar, 0.0, egui_bench::theme::WELL);
    let mut mesh = Mesh::default();
    let rows = (height / 2.0).ceil().max(2.0) as usize;
    for i in 0..rows {
        let y0 = bar.top() + bar.height() * i as f32 / rows as f32;
        let y1 = bar.top() + bar.height() * (i + 1) as f32 / rows as f32;
        let c = c_of((y0 + y1) / 2.0);
        let col = doc.look.color(c, &doc.image.stats);
        let base = mesh.vertices.len() as u32;
        for pos in [
            pos2(bar.left(), y0),
            pos2(bar.right(), y0),
            pos2(bar.right(), y1),
            pos2(bar.left(), y1),
        ] {
            mesh.vertices.push(Vertex { pos, uv: WHITE_UV, color: col });
        }
        mesh.indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    p.add(mesh);

    let (top, bottom) = (y_of(doc.look.max), y_of(doc.look.min));
    let dim = Color32::from_black_alpha(140);
    p.rect_filled(Rect::from_min_max(bar.left_top(), pos2(bar.right(), top)), 0.0, dim);
    p.rect_filled(Rect::from_min_max(pos2(bar.left(), bottom), bar.right_bottom()), 0.0, dim);
    p.rect_stroke(bar, 0.0, Stroke::new(1.0, ETCH), StrokeKind::Outside);

    let unit = doc.unit;
    let stats = &doc.image.stats;
    let edge_label = |c: f32, y: f32, align: Align2| {
        p.text(pos2(bar.left() - 6.0, y), align, unit.figure(c), figure(11.0), TRACE);
    };
    edge_label(stats.max.celsius, y_of(stats.max.celsius).max(rect.top()), Align2::RIGHT_TOP);
    edge_label(stats.min.celsius, y_of(stats.min.celsius).min(rect.bottom()), Align2::RIGHT_BOTTOM);

    let (dlo, dhi) = (unit.from_celsius(lo as f64), unit.from_celsius(hi as f64));
    for t in ticks(dlo, dhi, (height / 48.0) as f64) {
        let y = y_of(unit.to_celsius(t) as f32);
        p.line_segment(
            [pos2(bar.left(), y), pos2(bar.right(), y)],
            Stroke::new(1.0, Color32::from_white_alpha(30)),
        );
        p.line_segment(
            [pos2(bar.right(), y), pos2(bar.right() + 4.0, y)],
            Stroke::new(1.0, LEGEND),
        );
        let text = if t.fract().abs() < 1e-6 { format!("{t:.0}") } else { format!("{t:.1}") };
        p.text(pos2(bar.right() + 7.0, y), Align2::LEFT_CENTER, text, figure(10.5), LEGEND);
    }

    let x = bar.right() + TICK_W;
    for (c, y, max) in [(doc.look.max, top, true), (doc.look.min, bottom, false)] {
        p.line_segment(
            [pos2(bar.left() - 3.0, y), pos2(bar.right() + 3.0, y)],
            Stroke::new(1.0, READOUT),
        );
        p.circle_filled(pos2(bar.center().x, y), 5.0, READOUT);
        p.circle_stroke(
            pos2(bar.center().x, y),
            5.0,
            Stroke::new(2.0, Color32::from_black_alpha(200)),
        );
        let (y, align) = if max {
            ((y - 2.0).max(rect.top() + 14.0), Align2::LEFT_BOTTOM)
        } else {
            ((y + 2.0).min(rect.bottom() - 14.0), Align2::LEFT_TOP)
        };
        p.text(pos2(x, y), align, unit.figure(c), figure(14.0), READOUT);
    }
    p.text(pos2(x, rect.center().y), Align2::LEFT_CENTER, unit.symbol(), legend_font(12.0), LEGEND);
    resp
}

#[cfg(test)]
mod tests {
    use super::ticks;

    #[test]
    fn ticks_land_on_round_numbers_inside_the_axis() {
        assert_eq!(ticks(21.3, 39.8, 4.0), vec![25.0, 30.0, 35.0]);
        assert_eq!(ticks(-3.0, 3.0, 6.0), vec![-3.0, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0]);
        let t = ticks(25.9, 62.3, 10.0);
        assert!(t.len() >= 7 && t.len() <= 20, "{t:?}");
    }
}
