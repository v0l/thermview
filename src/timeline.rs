//! A frame-based viewport and Premiere-style zoom/scroll navigator.
use egui::{Align2, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use egui_bench::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimelineView {
    first: usize,
    last: usize,
    total: usize,
}

impl TimelineView {
    pub fn new(total: usize) -> Self {
        Self { first: 0, last: total.saturating_sub(1), total: total.max(1) }
    }
    pub fn first(&self) -> usize {
        self.first
    }
    pub fn last(&self) -> usize {
        self.last
    }
    pub fn span(&self) -> usize {
        (self.last - self.first).max(1)
    }
    pub fn fit(&mut self) {
        self.first = 0;
        self.last = self.total - 1;
    }

    pub fn set_range(&mut self, first: usize, last: usize) {
        let limit = self.total - 1;
        let minimum = usize::from(limit > 0);
        self.first = first.min(limit.saturating_sub(minimum));
        self.last = last.clamp(self.first + minimum, limit);
    }

    pub fn pan(&mut self, frames: f64) {
        let span = self.last - self.first;
        self.first = (self.first as f64 + frames).round().clamp(0.0, (self.total - 1 - span) as f64)
            as usize;
        self.last = self.first + span;
    }

    pub fn zoom(&mut self, factor: f64, playhead: usize) {
        if self.total < 2 || !factor.is_finite() || factor <= 0.0 {
            return;
        }
        let anchor = if (self.first..=self.last).contains(&playhead) {
            playhead as f64
        } else {
            (self.first + self.last) as f64 / 2.0
        };
        let fraction = (anchor - self.first as f64) / self.span() as f64;
        let span =
            (self.span() as f64 / factor).round().clamp(1.0, (self.total - 1) as f64) as usize;
        self.first = (anchor - fraction * span as f64)
            .round()
            .clamp(0.0, (self.total - 1 - span) as f64) as usize;
        self.last = self.first + span;
    }

    pub fn reveal(&mut self, frame: usize) {
        let frame = frame.min(self.total - 1);
        if frame < self.first {
            self.pan(frame as f64 - self.first as f64);
        }
        if frame > self.last {
            self.pan(frame as f64 - self.last as f64);
        }
    }

    fn frame_at(&self, fraction: f32) -> usize {
        (self.first as f64 + fraction.clamp(0.0, 1.0) as f64 * (self.last - self.first) as f64)
            .round() as usize
    }
}

#[derive(Clone, Copy)]
enum DragPart {
    Left,
    Right,
    Pan,
}
#[derive(Clone, Copy)]
struct NavigatorDrag {
    part: DragPart,
    origin: f32,
    view: TimelineView,
}

/// Render the detailed ruler and full-clip navigator. Changing the viewport
/// returns no seek; only interacting with the detailed ruler moves the playhead.
pub fn show(
    ui: &mut Ui,
    view: &mut TimelineView,
    current: usize,
    marks: (Option<usize>, Option<usize>),
) -> Option<usize> {
    let mut target = None;
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), 48.0), Sense::click_and_drag());
    let track = rect.shrink2(vec2(8.0, 0.0));
    if (response.clicked() || response.dragged())
        && let Some(at) = response.interact_pointer_pos()
    {
        target = Some(view.frame_at((at.x - track.left()) / track.width().max(1.0)));
    }
    let p = ui.painter();
    p.rect_filled(rect, 2.0, WELL);
    if let (Some(start), Some(end)) = marks
        && start <= end
    {
        let left = start.max(view.first);
        let right = end.min(view.last);
        if left <= right {
            let x1 = track.left() + track.width() * (left - view.first) as f32 / view.span() as f32;
            let x2 =
                track.left() + track.width() * (right - view.first) as f32 / view.span() as f32;
            p.rect_filled(
                Rect::from_min_max(
                    pos2(x1, rect.top() + 25.0),
                    pos2(x2.max(x1 + 2.0), rect.bottom()),
                ),
                0.0,
                TRACE.gamma_multiply(0.18),
            );
        }
    }
    // Integer ticks adapt to zoom and width, avoiding duplicate frame labels.
    let intervals = ((track.width() / 100.0).floor() as usize).max(1).min(view.span());
    let mut previous = None;
    for tick in 0..=intervals {
        let frame = view.frame_at(tick as f32 / intervals as f32);
        if previous == Some(frame) {
            continue;
        }
        previous = Some(frame);
        let x = track.left() + track.width() * (frame - view.first) as f32 / view.span() as f32;
        p.line_segment(
            [pos2(x, rect.top() + 19.0), pos2(x, rect.bottom() - 5.0)],
            Stroke::new(1.0, ETCH),
        );
        p.text(
            pos2(x, rect.top() + 4.0),
            if tick == intervals { Align2::RIGHT_TOP } else { Align2::LEFT_TOP },
            (frame + 1).to_string(),
            figure(10.0),
            VALUE,
        );
    }
    for (mark, name) in [(marks.0, "I"), (marks.1, "O")] {
        if let Some(frame) = mark
            && (view.first..=view.last).contains(&frame)
        {
            let x = track.left() + track.width() * (frame - view.first) as f32 / view.span() as f32;
            p.line_segment(
                [pos2(x, rect.top() + 22.0), pos2(x, rect.bottom())],
                Stroke::new(1.0, TRACE),
            );
            let is_end = name == "O";
            p.text(
                pos2(x + if is_end { -3.0 } else { 3.0 }, rect.bottom() - 3.0),
                if is_end { Align2::RIGHT_BOTTOM } else { Align2::LEFT_BOTTOM },
                name,
                figure(10.0),
                TRACE,
            );
        }
    }
    let current = target.unwrap_or(current);
    if (view.first..=view.last).contains(&current) {
        let x = track.left() + track.width() * (current - view.first) as f32 / view.span() as f32;
        p.rect_filled(
            Rect::from_min_max(pos2(track.left(), rect.top() + 27.0), pos2(x, rect.bottom() - 5.0)),
            0.0,
            READOUT.gamma_multiply(0.18),
        );
        p.line_segment(
            [pos2(x, rect.top() + 17.0), pos2(x, rect.bottom())],
            Stroke::new(2.0, READOUT),
        );
        p.add(egui::Shape::convex_polygon(
            vec![
                pos2(x - 5.0, rect.top() + 17.0),
                pos2(x + 5.0, rect.top() + 17.0),
                pos2(x, rect.top() + 23.0),
            ],
            READOUT,
            Stroke::NONE,
        ));
    }
    response
        .on_hover_text("drag to scrub the visible frames · Left / Right to step · Space to play");
    ui.add_space(2.0);
    navigator(ui, view, current, marks);
    target
}

fn navigator(
    ui: &mut Ui,
    view: &mut TimelineView,
    current: usize,
    marks: (Option<usize>, Option<usize>),
) {
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), 20.0), Sense::click_and_drag());
    let track = rect.shrink2(vec2(8.0, 2.0));
    let limit = (view.total - 1).max(1) as f32;
    let x_at = |frame: usize| track.left() + track.width() * frame as f32 / limit;
    let left = x_at(view.first);
    let right = x_at(view.last);
    // Keep the handles usable even at a one-frame-wide zoom level.
    let width =
        if view.total <= 1 { track.width() } else { (right - left).max(22.0).min(track.width()) };
    let start = ((left + right - width) / 2.0).clamp(track.left(), track.right() - width);
    let thumb = Rect::from_min_size(pos2(start, track.top()), vec2(width, track.height()));
    let part_at = |x: f32| {
        if (x - thumb.left()).abs() <= 7.0 {
            DragPart::Left
        } else if (x - thumb.right()).abs() <= 7.0 {
            DragPart::Right
        } else {
            DragPart::Pan
        }
    };
    let id = response.id.with("navigator drag");
    if response.drag_started()
        && let Some(origin) = ui.input(|i| i.pointer.press_origin())
    {
        let part = part_at(origin.x);
        if !thumb.contains(origin) && matches!(part, DragPart::Pan) {
            let center =
                ((origin.x - track.left()) / track.width().max(1.0)).clamp(0.0, 1.0) * limit;
            view.pan(center as f64 - (view.first + view.last) as f64 / 2.0);
        }
        ui.data_mut(|d| d.insert_temp(id, NavigatorDrag { part, origin: origin.x, view: *view }));
    }
    if response.dragged()
        && let Some(at) = response.interact_pointer_pos()
        && let Some(drag) = ui.data(|d| d.get_temp::<NavigatorDrag>(id))
    {
        let delta = (at.x - drag.origin) as f64 / track.width().max(1.0) as f64 * limit as f64;
        match drag.part {
            DragPart::Left => {
                let first = (drag.view.first as f64 + delta)
                    .round()
                    .clamp(0.0, drag.view.last.saturating_sub(1) as f64)
                    as usize;
                view.set_range(first, drag.view.last);
            }
            DragPart::Right => {
                let last = (drag.view.last as f64 + delta).round().clamp(
                    (drag.view.first + usize::from(view.total > 1)) as f64,
                    (view.total - 1) as f64,
                ) as usize;
                view.set_range(drag.view.first, last);
            }
            DragPart::Pan => {
                *view = drag.view;
                view.pan(delta);
            }
        }
    }
    if response.drag_stopped() {
        ui.data_mut(|d| d.remove::<NavigatorDrag>(id));
    }
    if response.clicked()
        && let Some(at) = response.interact_pointer_pos()
        && !thumb.contains(at)
    {
        let center = ((at.x - track.left()) / track.width().max(1.0)).clamp(0.0, 1.0) * limit;
        view.pan(center as f64 - (view.first + view.last) as f64 / 2.0);
    }
    if response.hovered() {
        let wheel = ui.input(|i| i.smooth_scroll_delta);
        if wheel.y != 0.0 {
            view.zoom((wheel.y as f64 / 120.0).clamp(-2.0, 2.0).exp(), current);
            ui.ctx().request_repaint();
        }
        if wheel.x != 0.0 {
            view.pan(wheel.x as f64 / track.width().max(1.0) as f64 * view.span() as f64);
        }
        let part = part_at(ui.input(|i| i.pointer.hover_pos()).unwrap_or(rect.center()).x);
        ui.ctx().set_cursor_icon(if matches!(part, DragPart::Pan) {
            egui::CursorIcon::Grab
        } else {
            egui::CursorIcon::ResizeHorizontal
        });
    }
    // Recompute the thumb after interaction so the view changes immediately.
    let p = ui.painter();
    p.rect_filled(track, 3.0, WELL);
    let left = x_at(view.first);
    let right = x_at(view.last);
    let width =
        if view.total <= 1 { track.width() } else { (right - left).max(22.0).min(track.width()) };
    let start = ((left + right - width) / 2.0).clamp(track.left(), track.right() - width);
    let thumb = Rect::from_min_size(pos2(start, track.top()), vec2(width, track.height()));
    p.rect_filled(thumb, 3.0, PANEL);
    p.rect_stroke(thumb, 3.0, Stroke::new(1.0, LEGEND), StrokeKind::Inside);
    for x in [thumb.left() + 5.0, thumb.right() - 5.0] {
        p.line_segment(
            [pos2(x, thumb.top() + 3.0), pos2(x, thumb.bottom() - 3.0)],
            Stroke::new(2.0, READOUT),
        );
    }
    for frame in [marks.0, marks.1].into_iter().flatten() {
        let x = x_at(frame);
        p.line_segment([pos2(x, track.top()), pos2(x, track.bottom())], Stroke::new(1.0, TRACE));
    }
    let x = x_at(current);
    p.line_segment([pos2(x, track.top()), pos2(x, track.bottom())], Stroke::new(1.0, READOUT));
    response.on_hover_text("zoom scroll bar · drag either handle to zoom · drag the middle to pan · wheel to zoom around the playhead · double-click to fit");
    if ui.input(|i| i.pointer.button_double_clicked(egui::PointerButton::Primary))
        && rect.contains(ui.input(|i| i.pointer.interact_pos()).unwrap_or_default())
    {
        view.fit();
    }
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!("VIEW {}–{}", view.first + 1, view.last + 1))
                .font(figure(10.0))
                .color(LEGEND),
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_keeps_playhead_anchor_and_pan_preserves_scale() {
        let mut view = TimelineView::new(16702);
        view.zoom(4.0, 8350);
        assert_eq!(view.span(), 4175);
        assert!((view.first()..=view.last()).contains(&8350));
        let span = view.span();
        view.pan(1000.0);
        assert_eq!(view.span(), span);
        view.pan(1e9);
        assert_eq!(view.last(), 16701);
        view.pan(-1e9);
        assert_eq!(view.first(), 0);
        assert_eq!(view.span(), span);
        view.fit();
        assert_eq!((view.first(), view.last()), (0, 16701));
    }

    #[test]
    fn high_zoom_single_frames_and_follow_stay_in_bounds() {
        let mut view = TimelineView::new(101);
        view.zoom(1e9, 50);
        assert_eq!(view.span(), 1);
        view.reveal(100);
        assert_eq!((view.first(), view.last()), (99, 100));
        view.reveal(0);
        assert_eq!((view.first(), view.last()), (0, 1));
        assert_eq!(view.frame_at(0.0), 0);
        assert_eq!(view.frame_at(1.0), 1);
        let mut single = TimelineView::new(1);
        single.zoom(2.0, 0);
        single.pan(10.0);
        single.set_range(10, 20);
        assert_eq!((single.first(), single.last(), single.frame_at(1.0)), (0, 0, 0));
    }
}
