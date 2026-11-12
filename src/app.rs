use crate::colorbar;
use crate::doc::Doc;
use crate::host::{self, Loaded};
use crate::overlay::{self, POINT_COLOURS};
use crate::palette::{Look, Mapping, Overscan, Palette, Unit};
use egui::epaint::{Mesh, Vertex, WHITE_UV};
use egui::{
    Align2, Color32, Pos2, Rect, Sense, Stroke, StrokeKind, TextureHandle, TextureOptions, Ui,
    pos2, vec2,
};
use egui_bench::prelude::*;

const REPO: &str = env!("CARGO_PKG_REPOSITORY");
const PRESETS: [(f64, f64); 3] = [(20.0, 40.0), (0.0, 80.0), (-10.0, 50.0)];

#[derive(Clone, Copy, PartialEq)]
enum Sidebar {
    Explorer,
    Controls,
}

pub struct App {
    pub doc: Option<Doc>,
    path: Option<std::path::PathBuf>,
    texture: Option<((Look, u64), TextureHandle)>,
    pointer: Option<(usize, usize, f32)>,
    status: Option<(String, bool)>,
    sidebar: Option<Sidebar>,
    last_sidebar: Sidebar,
    #[cfg(not(target_arch = "wasm32"))]
    pub explorer: crate::explorer::Explorer,
    #[cfg(not(target_arch = "wasm32"))]
    folder_browser: Option<Browser>,
    #[cfg(not(target_arch = "wasm32"))]
    browser: Option<Browser>,
    #[cfg(target_arch = "wasm32")]
    picker: host::Picker,
}

impl App {
    pub fn new(ctx: &egui::Context) -> App {
        egui_bench::install(ctx);
        App {
            doc: None,
            path: None,
            texture: None,
            pointer: None,
            status: None,
            sidebar: Some(if cfg!(target_arch = "wasm32") {
                Sidebar::Controls
            } else {
                Sidebar::Explorer
            }),
            last_sidebar: Sidebar::Explorer,
            #[cfg(not(target_arch = "wasm32"))]
            explorer: crate::explorer::Explorer::new(std::env::current_dir().unwrap_or_default()),
            #[cfg(not(target_arch = "wasm32"))]
            folder_browser: None,
            #[cfg(not(target_arch = "wasm32"))]
            browser: None,
            #[cfg(target_arch = "wasm32")]
            picker: host::Picker::default(),
        }
    }

    pub fn load(&mut self, ctx: &egui::Context, loaded: Result<Loaded, String>) {
        let loaded = match loaded {
            Ok(l) => l,
            Err(e) => return self.fail(e),
        };
        let is_video = loaded.name.to_ascii_lowercase().ends_with(".csq");
        let result = if is_video {
            crate::video::Video::open(loaded.bytes)
                .and_then(|v| Doc::from_video(v, &loaded.name, loaded.modified))
        } else {
            crate::parse(&loaded.bytes, Some(&loaded.name))
                .map(|img| Doc::new(img, &loaded.name, loaded.modified))
        };
        match result {
            Ok(doc) => {
                self.doc = Some(doc);
                #[cfg(not(target_arch = "wasm32"))]
                if let Some(path) = &loaded.path {
                    self.explorer.reveal(path);
                }
                self.path = loaded.path;
                self.texture = None;
                self.pointer = None;
                self.status = None;
                ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
                    "thermview - {}",
                    loaded.name
                )));
            }
            Err(e) => self.fail(format!("Cannot open {}: {e}", loaded.name)),
        }
    }

    fn fail(&mut self, e: String) {
        self.status = Some((e, true));
    }

    fn open(&mut self, _ctx: &egui::Context) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let dir = self.explorer.root.clone();
            self.browser =
                Some(Browser::new("open a thermal image or video", dir).suffixes(&host::SUFFIXES));
        }
        #[cfg(target_arch = "wasm32")]
        self.picker.pick(_ctx);
    }

    fn poll_open(&mut self, ctx: &egui::Context) {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(browser) = &mut self.folder_browser {
            match browser.show(ctx) {
                Picked::Browsing => {}
                Picked::Cancelled => self.folder_browser = None,
                Picked::Folder(path) => {
                    self.folder_browser = None;
                    self.explorer.set_root(path);
                    self.sidebar = Some(Sidebar::Explorer);
                }
                Picked::File(path) => {
                    self.folder_browser = None;
                    self.load(ctx, host::read(&path));
                }
            }
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::B)) {
            self.sidebar = if let Some(panel) = self.sidebar {
                self.last_sidebar = panel;
                None
            } else {
                Some(self.last_sidebar)
            };
        }
        if ctx.input(|i| i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::E)) {
            self.sidebar = Some(Sidebar::Explorer);
            self.last_sidebar = Sidebar::Explorer;
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(b) = &mut self.browser {
            match b.show(ctx) {
                Picked::Browsing => {}
                Picked::Cancelled | Picked::Folder(_) => self.browser = None,
                Picked::File(p) => {
                    self.browser = None;
                    self.load(ctx, host::read(&p));
                }
            }
        }
        #[cfg(target_arch = "wasm32")]
        if let Some(l) = self.picker.take() {
            self.load(ctx, Ok(l));
        }
        let dropped = ctx.input(|i| i.raw.dropped_files.first().cloned());
        if let Some(f) = dropped {
            self.load(ctx, host::dropped(f.as_ref()));
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::O)) {
            self.open(ctx);
        }
        if ctx.input(|i| i.modifiers.command && !i.modifiers.shift && i.key_pressed(egui::Key::E)) {
            self.export();
        }
    }

    fn export(&mut self) {
        let Some(doc) = &self.doc else { return };
        let png = crate::export::png(doc);
        self.status =
            Some(match host::save(self.path.as_deref(), &crate::export::file_name(doc), &png) {
                Ok(msg) => (msg, false),
                Err(e) => (e, true),
            });
    }

    fn header(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        ui.horizontal(|ui| {
            let name =
                self.doc.as_ref().map(|d| d.name.clone()).unwrap_or_else(|| "no file".into());
            Line::new().legend("thermview").value(name).show(ui);
            if ui
                .add(Key::new("open").outline(VALUE))
                .on_hover_text("open a thermal image or video, ctrl+O")
                .clicked()
            {
                self.open(&ctx);
            }
            if self.doc.is_some() {
                if ui
                    .add(Key::new("export").outline(VALUE))
                    .on_hover_text("save the view with its markers as a PNG, ctrl+E")
                    .clicked()
                {
                    self.export();
                }
                if ui
                    .add(Key::new("clear").outline(VALUE))
                    .on_hover_text("close this file")
                    .clicked()
                {
                    self.doc = None;
                    self.path = None;
                    self.texture = None;
                    self.pointer = None;
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.hyperlink_to(legend("github"), REPO);
                if let Some(doc) = &self.doc {
                    lamp(ui, "recalibrate", doc.image.recomputable(), false).on_hover_text(
                        if doc.image.recomputable() {
                            "raw sensor counts are in the file, so calibration can be edited"
                        } else {
                            "the file stores finished temperatures, calibration is fixed"
                        },
                    );
                    lamp(ui, doc.image.format.label(), true, false);
                }
            });
        });
        ui.add_space(6.0);
    }

    fn activity_bar(&mut self, ui: &mut Ui) {
        for (panel, caption, help) in [
            (Sidebar::Explorer, "FILES", "Explorer · Ctrl+Shift+E"),
            (Sidebar::Controls, "LOOK", "Image controls · palette, range and display"),
        ] {
            let (rect, response) = ui.allocate_exact_size(vec2(52.0, 58.0), Sense::click());
            let on = self.sidebar == Some(panel);
            let p = ui.painter();
            if on || response.hovered() {
                p.rect_filled(rect, 0.0, if on { PANEL } else { BAND });
            }
            if on {
                p.rect_filled(
                    Rect::from_min_size(rect.min, vec2(2.0, rect.height())),
                    0.0,
                    READOUT,
                );
            }
            let colour = if on { READOUT } else { LEGEND };
            let stroke = Stroke::new(1.5, colour);
            let center = rect.center_top() + vec2(0.0, 22.0);
            if panel == Sidebar::Explorer {
                p.rect_stroke(
                    Rect::from_center_size(center + vec2(-3.0, -3.0), vec2(15.0, 19.0)),
                    1.0,
                    stroke,
                    StrokeKind::Inside,
                );
                p.rect_filled(
                    Rect::from_center_size(center + vec2(3.0, 3.0), vec2(15.0, 19.0)),
                    1.0,
                    if on { PANEL } else { CHASSIS },
                );
                p.rect_stroke(
                    Rect::from_center_size(center + vec2(3.0, 3.0), vec2(15.0, 19.0)),
                    1.0,
                    stroke,
                    StrokeKind::Inside,
                );
            } else {
                for (y, x) in [(-8.0, -4.0), (0.0, 5.0), (8.0, -1.0)] {
                    p.line_segment([center + vec2(-11.0, y), center + vec2(11.0, y)], stroke);
                    p.circle_filled(center + vec2(x, y), 3.0, colour);
                }
            }
            p.text(
                pos2(rect.center().x, rect.bottom() - 8.0),
                Align2::CENTER_BOTTOM,
                caption,
                figure(8.0),
                colour,
            );
            response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Tab, true, on, caption));
            if response.on_hover_text(help).clicked() {
                self.sidebar = if on { None } else { Some(panel) };
                self.last_sidebar = panel;
            }
        }
    }

    fn sidebar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            Line::new()
                .legend(if self.sidebar == Some(Sidebar::Explorer) {
                    "explorer"
                } else {
                    "image controls"
                })
                .show(ui);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(Key::new("hide").outline(VALUE))
                    .on_hover_text("toggle sidebar · Ctrl+B")
                    .clicked()
                {
                    if let Some(panel) = self.sidebar {
                        self.last_sidebar = panel;
                    }
                    self.sidebar = None;
                }
            });
        });
        ui.add_space(8.0);
        match self.sidebar {
            Some(Sidebar::Controls) => {
                egui::ScrollArea::vertical().id_salt("image controls scroll").show(ui, |ui| {
                    if let Some(doc) = &mut self.doc {
                        App::controls(ui, doc);
                    } else {
                        hint(ui, "open a thermal image or video to adjust its appearance");
                    }
                });
            }
            Some(Sidebar::Explorer) => {
                #[cfg(not(target_arch = "wasm32"))]
                self.explorer_panel(ui);
                #[cfg(target_arch = "wasm32")]
                {
                    hint(ui, "directory browsing is available in the desktop app");
                    if ui.add(Key::new("open file").outline(VALUE)).clicked() {
                        self.open(&ui.ctx().clone());
                    }
                }
            }
            None => {}
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn explorer_panel(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            if ui.add(Key::new("open folder").outline(VALUE)).clicked() {
                self.folder_browser =
                    Some(Browser::new("open folder", self.explorer.root.clone()).folders(true));
            }
            if ui.add(Key::new("up").outline(VALUE)).on_hover_text("parent directory").clicked() {
                if let Some(parent) = self.explorer.root.parent().map(|p| p.to_path_buf()) {
                    self.explorer.set_root(parent);
                }
            }
            if ui.add(Key::new("refresh").outline(VALUE)).clicked() {
                self.explorer.refresh();
            }
        });
        ui.add_space(6.0);
        let title = self
            .explorer
            .root
            .file_name()
            .unwrap_or(self.explorer.root.as_os_str())
            .to_string_lossy();
        ui.label(egui::RichText::new(title.to_string()).font(figure(12.0)).color(VALUE))
            .on_hover_text(self.explorer.root.display().to_string());
        ui.add_space(4.0);
        let opened = egui::ScrollArea::vertical()
            .id_salt("explorer scroll")
            .show(ui, |ui| self.explorer.show(ui))
            .inner;
        if let Some(path) = opened {
            if path.is_file() {
                self.load(&ui.ctx().clone(), host::read(&path));
            } else {
                self.fail(format!("Cannot read {}: not a regular file", path.display()));
            }
        }
    }

    fn controls(ui: &mut Ui, doc: &mut Doc) {
        section(ui, "palette", "", |ui| palettes(ui, doc));
        ui.add_space(8.0);
        section(ui, "range", "the window the palette spans", |ui| range(ui, doc));
        ui.add_space(8.0);
        section(ui, "mapping", "", |ui| {
            let opts: Vec<(Mapping, &str)> = Mapping::ALL.iter().map(|m| (*m, m.label())).collect();
            ui.push_id("mapping", |ui| tabs(ui, &mut doc.look.mapping, &opts));
            hint(ui, doc.look.mapping.explain());
        });
        ui.add_space(8.0);
        section(ui, "overscan", "", |ui| {
            let opts: Vec<(Overscan, &str)> =
                Overscan::ALL.iter().map(|o| (*o, o.label())).collect();
            ui.push_id("overscan", |ui| tabs(ui, &mut doc.look.overscan, &opts));
            hint(ui, doc.look.overscan.explain());
        });
        ui.add_space(8.0);
        section(ui, "display", "", |ui| display(ui, doc));
    }

    fn readings(&mut self, ui: &mut Ui) {
        let Some(doc) = &mut self.doc else { return };
        card(
            ui,
            Some(TRACE),
            |ui| {
                Line::new().legend("pointer").show(ui);
            },
            |ui| {
                let (v, at) = match self.pointer {
                    Some((x, y, c)) => (doc.unit.figure(c), format!("{x}, {y}")),
                    None => ("--".into(), "off the image".into()),
                };
                hero(ui, &at, &v, doc.unit.symbol(), TRACE);
            },
        );
        ui.add_space(8.0);
        section(ui, "points", "click the image to add one", |ui| points(ui, doc));
        ui.add_space(8.0);
        if doc.image.recomputable() {
            section(ui, "calibration", "recomputes every pixel", |ui| calibration(ui, doc));
            ui.add_space(8.0);
        }
        section(ui, "file", "", |ui| file(ui, doc));
    }

    fn texture(&mut self, ctx: &egui::Context) -> Option<egui::TextureId> {
        let doc = self.doc.as_ref()?;
        let key = (doc.look, doc.revision);
        match &mut self.texture {
            Some((k, _)) if *k == key => {}
            Some((k, h)) => {
                h.set(doc.look.render(&doc.image), TextureOptions::NEAREST);
                *k = key;
            }
            None => {
                let h = ctx.load_texture(
                    "thermal",
                    doc.look.render(&doc.image),
                    TextureOptions::NEAREST,
                );
                self.texture = Some((key, h));
            }
        }
        self.texture.as_ref().map(|(_, h)| h.id())
    }

    fn plate(&mut self, ui: &mut Ui) {
        let Some(tex) = self.texture(ui.ctx()) else { return };
        let Some(doc) = &mut self.doc else { return };
        let size = overlay::image_rect(Pos2::ZERO, doc).size();
        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.add_space(4.0);
                colorbar::show(ui, doc, size.y.max(300.0));
                ui.add_space(8.0);
                let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
                let p = ui.painter_at(rect.expand(1.0));
                p.rect_filled(rect, 0.0, WELL);
                p.image(
                    tex,
                    rect,
                    Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
                p.rect_stroke(rect, 0.0, Stroke::new(1.0, ETCH), StrokeKind::Outside);
                overlay::paint(&ui.painter_at(rect), rect, doc);

                let s = doc.scale as f32;
                let pixel = |at: Pos2| {
                    let v = (at - rect.min) / s;
                    let (x, y) = (v.x.floor() as i64, v.y.floor() as i64);
                    (x >= 0
                        && y >= 0
                        && (x as usize) < doc.image.width
                        && (y as usize) < doc.image.height)
                        .then_some((x as usize, y as usize))
                };
                self.pointer = resp
                    .hover_pos()
                    .and_then(pixel)
                    .map(|(x, y)| (x, y, doc.image.at(x, y).unwrap_or(f32::NAN)));
                if resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
                }
                if let (Some(at), Some((_, _, c))) = (resp.hover_pos(), self.pointer) {
                    tag(&ui.painter_at(rect), rect, at, &doc.unit.reading(c));
                }
                if resp.clicked()
                    && let Some((x, y)) = resp.interact_pointer_pos().and_then(pixel)
                {
                    doc.add_point(x, y);
                }
            });
        });
    }

    fn playback(&mut self, ctx: &egui::Context) {
        let Some(doc) = &mut self.doc else { return };
        let Some(video) = &mut doc.video else { return };
        let now = ctx.input(|i| i.time);
        let keyboard = !ctx.egui_wants_keyboard_input();
        if keyboard && ctx.input(|i| i.key_pressed(egui::Key::Space)) {
            video.playing = !video.playing;
            video.last_tick = Some(now);
            if video.playing && video.current + 1 == video.len() {
                if let Err(e) = doc.seek(0) {
                    self.fail(e.to_string());
                }
                return;
            }
        }
        let mut target = video.current;
        if keyboard {
            if ctx.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
                target = target.saturating_sub(1);
                video.playing = false;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
                target = (target + 1).min(video.len() - 1);
                video.playing = false;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Home)) {
                target = 0;
                video.playing = false;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::End)) {
                target = video.len() - 1;
                video.playing = false;
            }
        }
        if video.playing {
            let last_frame = video.len() - 1;
            let last = video.last_tick.get_or_insert(now);
            let steps = ((now - *last) * video.fps).floor().max(0.0) as usize;
            if steps > 0 {
                target = (target.saturating_add(steps)).min(last_frame);
                *last += steps as f64 / video.fps;
            }
            if target + 1 == video.len() {
                video.playing = false;
            }
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(1.0 / video.fps));
        } else {
            video.last_tick = None;
        }
        if let Err(e) = doc.seek(target) {
            doc.video.as_mut().unwrap().playing = false;
            self.fail(e.to_string());
        }
    }

    fn timeline(&mut self, ui: &mut Ui) {
        let Some(doc) = &mut self.doc else { return };
        let Some(video) = &mut doc.video else { return };
        let mut target = video.current;
        ui.horizontal(|ui| {
            if ui.add(Key::new("|<").outline(VALUE)).on_hover_text("first frame · Home").clicked()
            {
                target = 0;
                video.playing = false;
            }
            if ui.add(Key::new("<").outline(VALUE)).on_hover_text("previous frame · Left").clicked()
            {
                target = target.saturating_sub(1);
                video.playing = false;
            }
            if ui
                .add(Key::new(if video.playing { "pause" } else { "play" }).outline(READOUT))
                .on_hover_text("play / pause · Space")
                .clicked()
            {
                video.playing = !video.playing;
                video.last_tick = Some(ui.input(|i| i.time));
                if video.playing && target + 1 == video.len() {
                    target = 0;
                }
                ui.ctx().request_repaint();
            }
            if ui.add(Key::new(">").outline(VALUE)).on_hover_text("next frame · Right").clicked() {
                target = (target + 1).min(video.len() - 1);
                video.playing = false;
            }
            if ui.add(Key::new(">|").outline(VALUE)).on_hover_text("last frame · End").clicked() {
                target = video.len() - 1;
                video.playing = false;
            }
            ui.label(
                egui::RichText::new(format!("FRAME {} / {}", video.current + 1, video.len()))
                    .font(figure(12.0))
                    .color(VALUE),
            );
            ui.label(
                egui::RichText::new(format!(
                    "MIN {}    MAX {}",
                    doc.unit.reading(doc.image.stats.min.celsius),
                    doc.unit.reading(doc.image.stats.max.celsius)
                ))
                .font(figure(12.0))
                .color(TRACE),
            );
            if toggle(ui, "auto range", video.auto_range)
                .on_hover_text("fit the palette to each frame")
                .clicked()
            {
                video.auto_range = !video.auto_range;
            }
            ui.add(
                egui::DragValue::new(&mut video.fps).range(1.0..=1000.0).speed(1.0).suffix(" fps"),
            )
            .on_hover_text(if video.estimated_fps {
                "playback rate · metadata unavailable, defaults to 30 fps"
            } else {
                "playback frame rate"
            });
        });
        ui.add_space(4.0);
        let (rect, response) =
            ui.allocate_exact_size(vec2(ui.available_width(), 48.0), Sense::click_and_drag());
        let track = rect.shrink2(vec2(8.0, 0.0));
        if (response.clicked() || response.dragged())
            && let Some(at) = response.interact_pointer_pos()
        {
            let fraction = ((at.x - track.left()) / track.width().max(1.0)).clamp(0.0, 1.0);
            target = (fraction as f64 * (video.len() - 1) as f64).round() as usize;
            video.playing = false;
        }
        let p = ui.painter();
        p.rect_filled(rect, 2.0, WELL);
        for tick in 0..=10 {
            let fraction = tick as f32 / 10.0;
            let x = track.left() + track.width() * fraction;
            p.line_segment(
                [pos2(x, rect.top() + 19.0), pos2(x, rect.bottom() - 5.0)],
                Stroke::new(1.0, ETCH),
            );
            let frame = (fraction as f64 * (video.len() - 1) as f64).round() as usize;
            p.text(
                pos2(x, rect.top() + 4.0),
                if tick == 10 { Align2::RIGHT_TOP } else { Align2::LEFT_TOP },
                format!("{}", frame + 1),
                figure(10.0),
                VALUE,
            );
        }
        let fraction = target as f32 / (video.len() - 1).max(1) as f32;
        let x = track.left() + track.width() * fraction;
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
        response.on_hover_text("drag to scrub · Left / Right to step · Space to play");
        if let Err(e) = doc.seek(target) {
            self.fail(e.to_string());
        }
    }

    fn welcome(&mut self, ui: &mut Ui) {
        let hovering = ui.ctx().input(|i| !i.raw.hovered_files.is_empty());
        let ctx = ui.ctx().clone();
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() * 0.25);
            ui.set_max_width(460.0);
            card(
                ui,
                Some(if hovering { READOUT } else { ETCH }),
                |ui| {
                    Line::new().legend("open a thermal image or video").show(ui);
                },
                |ui| {
                    ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                        note(
                            ui,
                            "Drop a radiometric file anywhere on this window, or open one.",
                            VALUE,
                        );
                        ui.add_space(4.0);
                        reading(ui, "infiray", "IRG from C200, C210, P200, Autel Evo II Dual");
                        reading(ui, "hikmicro", "JPEG from Pocket2, SP60");
                        reading(ui, "flir", "R-JPEG, AFF .img and CSQ video");
                        reading(ui, "dji", "R-JPEG from the Mavic 2 Enterprise Advanced");
                        ui.add_space(6.0);
                        if ui.add(Key::new("open").solid(READOUT)).clicked() {
                            self.open(&ctx);
                        }
                    });
                },
            );
        });
    }
}

fn tag(p: &egui::Painter, rect: Rect, at: Pos2, text: &str) {
    let g = p.layout_no_wrap(text.to_string(), figure(12.0), Color32::WHITE);
    let size = g.size() + vec2(10.0, 6.0);
    let mut min = at + vec2(14.0, 14.0);
    if min.x + size.x > rect.right() {
        min.x = at.x - 14.0 - size.x;
    }
    if min.y + size.y > rect.bottom() {
        min.y = at.y - 14.0 - size.y;
    }
    let r = Rect::from_min_size(min, size);
    p.rect_filled(r, 2.0, Color32::from_black_alpha(205));
    p.rect_stroke(r, 2.0, Stroke::new(1.0, Color32::from_white_alpha(30)), StrokeKind::Inside);
    p.galley(r.center() - g.size() / 2.0, g, Color32::WHITE);
}

fn swatch(p: &egui::Painter, r: Rect, palette: Palette) {
    let mut mesh = Mesh::default();
    let n = 32;
    for i in 0..=n {
        let t = i as f32 / n as f32;
        let [cr, cg, cb] = palette.rgb(t);
        let c = Color32::from_rgb(cr, cg, cb);
        let x = r.left() + r.width() * t;
        mesh.vertices.push(Vertex { pos: pos2(x, r.top()), uv: WHITE_UV, color: c });
        mesh.vertices.push(Vertex { pos: pos2(x, r.bottom()), uv: WHITE_UV, color: c });
        if i > 0 {
            let b = (i as u32 - 1) * 2;
            mesh.indices.extend([b, b + 1, b + 3, b, b + 3, b + 2]);
        }
    }
    p.add(mesh);
}

fn palettes(ui: &mut Ui, doc: &mut Doc) {
    for palette in Palette::ALL {
        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 18.0), Sense::click());
        let on = doc.look.palette == palette;
        let p = ui.painter();
        if on {
            p.rect_filled(rect, 0.0, PANEL.gamma_multiply(1.4));
            p.rect_filled(Rect::from_min_size(rect.min, vec2(3.0, rect.height())), 0.0, READOUT);
        } else if resp.hovered() {
            p.rect_filled(rect, 0.0, BAND);
        }
        let strip = Rect::from_min_max(
            pos2(rect.left() + 10.0, rect.top() + 4.0),
            pos2(rect.left() + 90.0, rect.bottom() - 4.0),
        );
        swatch(p, strip, palette);
        p.rect_stroke(strip, 0.0, Stroke::new(1.0, ETCH), StrokeKind::Outside);
        p.text(
            pos2(strip.right() + 10.0, rect.center().y),
            Align2::LEFT_CENTER,
            palette.label().to_uppercase(),
            legend_font(11.5),
            if on { READOUT } else { VALUE },
        );
        if resp.clicked() {
            doc.look.palette = palette;
        }
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
    }
    ui.add_space(4.0);
    if toggle(ui, "invert", doc.look.inverted).on_hover_text("swap the hot and cold ends").clicked()
    {
        doc.look.inverted = !doc.look.inverted;
    }
}

fn degrees(ui: &mut Ui, unit: Unit, c: &mut f32) -> bool {
    let mut v = unit.from_celsius(*c as f64);
    let changed = ui
        .add(egui::DragValue::new(&mut v).speed(0.1).fixed_decimals(1).suffix(unit.symbol()))
        .changed();
    if changed {
        *c = unit.to_celsius(v) as f32;
    }
    changed
}

fn range(ui: &mut Ui, doc: &mut Doc) {
    let unit = doc.unit;
    let stats = &doc.image.stats;
    readouts(
        ui,
        &[
            ("window low", unit.figure(doc.look.min), READOUT),
            ("window high", unit.figure(doc.look.max), READOUT),
        ],
    );
    readouts(
        ui,
        &[
            ("coldest", unit.figure(stats.min.celsius), TRACE),
            ("hottest", unit.figure(stats.max.celsius), TRACE),
        ],
    );
    ui.horizontal(|ui| {
        let (mut lo, mut hi) = (doc.look.min, doc.look.max);
        if degrees(ui, unit, &mut lo) {
            doc.look.min = lo.min(doc.look.max - 0.1);
        }
        ui.label(legend("to"));
        if degrees(ui, unit, &mut hi) {
            doc.look.max = hi.max(doc.look.min + 0.1);
        }
    });
    ui.horizontal_wrapped(|ui| {
        if ui
            .add(Key::new("auto").outline(VALUE))
            .on_hover_text("fit the window to the file")
            .clicked()
        {
            doc.auto_range();
        }
        for (lo, hi) in PRESETS {
            let (a, b) = (unit.from_celsius(lo).round(), unit.from_celsius(hi).round());
            if ui.add(Key::new(format!("{a}-{b}")).outline(VALUE)).clicked() {
                doc.look.min = lo as f32;
                doc.look.max = hi as f32;
            }
        }
    });
}

fn display(ui: &mut Ui, doc: &mut Doc) {
    Line::new().legend("unit").show(ui);
    ui.push_id("unit", |ui| {
        tabs(ui, &mut doc.unit, &[(Unit::Celsius, "°C"), (Unit::Fahrenheit, "°F")])
    });
    Line::new().legend("scale").show(ui);
    let names: Vec<(u32, String)> = (1..=6).map(|s| (s, format!("{s}x"))).collect();
    let opts: Vec<(u32, &str)> = names.iter().map(|(s, n)| (*s, n.as_str())).collect();
    ui.push_id("scale", |ui| tabs(ui, &mut doc.scale, &opts));
    row(ui, "labels", |ui| {
        ui.add(egui::Slider::new(&mut doc.label_size, 5.0..=30.0).step_by(1.0));
    });
    ui.horizontal_wrapped(|ui| {
        let o = &mut doc.overlays;
        if toggle(ui, "min max", o.extremes)
            .on_hover_text("mark the hottest and coldest pixel")
            .clicked()
        {
            o.extremes = !o.extremes;
        }
        if toggle(ui, "info", o.info)
            .on_hover_text("emissivity, camera and lens in the corners")
            .clicked()
        {
            o.info = !o.info;
        }
        if toggle(ui, "time", o.time).on_hover_text("when the image was taken").clicked() {
            o.time = !o.time;
        }
    });
}

fn points(ui: &mut Ui, doc: &mut Doc) {
    if doc.points.is_empty() {
        hint(ui, "no points yet");
        return;
    }
    let unit = doc.unit;
    let readings: Vec<f32> = doc.points.iter().map(|p| doc.reading(p)).collect();
    let mut remove = None;
    for (i, pt) in doc.points.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            let (dot, _) = ui.allocate_exact_size(vec2(10.0, 18.0), Sense::hover());
            ui.painter().circle_filled(
                dot.center(),
                4.0,
                POINT_COLOURS[pt.colour % POINT_COLOURS.len()],
            );
            field_then(ui, &mut pt.label, "name", 190.0, |ui| {
                ui.label(
                    egui::RichText::new(unit.reading(readings[i])).font(figure(13.0)).color(TRACE),
                );
                if ui
                    .add(Key::new("remove").outline(VALUE))
                    .on_hover_text("remove this point")
                    .clicked()
                {
                    remove = Some(i);
                }
            });
        });
    }
    if let Some(i) = remove {
        doc.points.remove(i);
    }
    if ui.add(Key::new("remove all").outline(FAULT)).clicked() {
        doc.points.clear();
    }
}

fn calibration(ui: &mut Ui, doc: &mut Doc) {
    let unit = doc.unit;
    let mut p = doc.params();
    let deg = |ui: &mut Ui, v: &mut f64| {
        let mut d = unit.from_celsius(*v);
        if ui
            .add(egui::DragValue::new(&mut d).speed(0.1).fixed_decimals(1).suffix(unit.symbol()))
            .changed()
        {
            *v = unit.to_celsius(d);
        }
    };
    row_help(
        ui,
        "emissivity",
        "how well the surface radiates, 1.0 is a perfect black body",
        |ui| {
            ui.add(
                egui::DragValue::new(&mut p.emissivity)
                    .range(0.01..=1.0)
                    .speed(0.005)
                    .fixed_decimals(3),
            );
        },
    );
    row(ui, "distance", |ui| {
        ui.add(
            egui::DragValue::new(&mut p.distance)
                .range(0.1..=100.0)
                .speed(0.05)
                .fixed_decimals(1)
                .suffix(" m"),
        );
    });
    row_help(ui, "reflected", "apparent temperature of what the surface reflects", |ui| {
        deg(ui, &mut p.reflected)
    });
    row(ui, "air", |ui| deg(ui, &mut p.air));
    row(ui, "humidity", |ui| {
        ui.add(
            egui::DragValue::new(&mut p.humidity)
                .range(0.0..=100.0)
                .speed(0.5)
                .fixed_decimals(0)
                .suffix(" %"),
        );
    });
    doc.set_params(p);
    if p != doc.source.params
        && ui
            .add(Key::new("as shot").outline(VALUE))
            .on_hover_text("back to the values in the file")
            .clicked()
    {
        doc.set_params(doc.source.params);
    }
}

fn file(ui: &mut Ui, doc: &Doc) {
    let img = &doc.image;
    let unit = doc.unit;
    reading(ui, "format", img.format.label());
    reading(ui, "size", format!("{} x {}", img.width, img.height));
    reading(ui, "emissivity", format!("{:.3}", img.params.emissivity));
    reading(ui, "air", unit.reading(img.params.air as f32));
    reading(ui, "reflected", unit.reading(img.params.reflected as f32));
    reading(ui, "distance", format!("{:.1} m", img.params.distance));
    reading(ui, "humidity", format!("{:.0} %", img.params.humidity));
    reading(ui, "atm trans", format!("{:.3}", img.atm_trans));
    let cam = &img.camera;
    if let Some(m) = cam.make_model() {
        reading(ui, "camera", m);
    }
    if let Some(l) = cam.lens() {
        reading(ui, "lens", l);
    }
    if let Some(s) = &cam.software {
        reading(ui, "software", s);
    }
    if let Some(s) = &cam.serial {
        reading(ui, "serial", s);
    }
    if let Some(t) = &doc.stamp {
        reading(ui, "taken", t);
    }
    if let (Some(lat), Some(lon)) = (cam.latitude, cam.longitude) {
        reading(ui, "position", format!("{lat:.5}, {lon:.5}"));
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.show(ui);
    }
}

impl App {
    pub fn show(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        self.poll_open(&ctx);
        self.playback(&ctx);
        egui::Panel::top("header")
            .frame(egui::Frame::NONE.fill(CHASSIS).inner_margin(egui::Margin {
                left: 12,
                right: 12,
                top: 8,
                bottom: 2,
            }))
            .show(ui, |ui| self.header(ui));
        if let Some((msg, fault)) = self.status.clone() {
            egui::Panel::bottom("status")
                .frame(egui::Frame::NONE.fill(CHASSIS).inner_margin(egui::Margin::symmetric(12, 4)))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        status(ui, !fault, &msg);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.add(Key::new("dismiss").outline(VALUE)).clicked() {
                                self.status = None;
                            }
                        });
                    });
                });
        }
        if self.doc.as_ref().is_some_and(|d| d.video.is_some()) {
            egui::Panel::bottom("video timeline")
                .frame(egui::Frame::NONE.fill(CHASSIS).inner_margin(egui::Margin::symmetric(12, 8)))
                .show(ui, |ui| self.timeline(ui));
        }
        egui::Panel::left("activity bar")
            .resizable(false)
            .exact_size(52.0)
            .frame(egui::Frame::NONE.fill(CHASSIS))
            .show(ui, |ui| self.activity_bar(ui));
        if self.sidebar.is_some() {
            egui::Panel::left("sidebar")
                .resizable(true)
                .default_size(290.0)
                .min_size(250.0)
                .frame(egui::Frame::NONE.fill(CHASSIS).inner_margin(egui::Margin::symmetric(10, 8)))
                .show(ui, |ui| self.sidebar(ui));
        }
        if self.doc.is_some() {
            egui::Panel::right("readings")
                .resizable(true)
                .default_size(290.0)
                .min_size(240.0)
                .frame(egui::Frame::NONE.fill(CHASSIS).inner_margin(egui::Margin::symmetric(10, 8)))
                .show(ui, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| self.readings(ui));
                });
        }
        egui::CentralPanel::no_frame().show(ui, |ui| {
            let r = ui.max_rect();
            ui.painter().rect_filled(r, 0.0, CHASSIS);
            ui.painter().line_segment([r.left_top(), r.right_top()], Stroke::new(1.0, ETCH));
            egui::Frame::NONE.inner_margin(egui::Margin::same(10)).show(ui, |ui| {
                if self.doc.is_some() {
                    self.plate(ui);
                } else {
                    self.welcome(ui);
                }
            });
        });
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn run(file: Option<std::path::PathBuf>) -> eframe::Result<()> {
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1500.0, 950.0])
            .with_title("thermview"),
        ..Default::default()
    };
    eframe::run_native(
        "thermview",
        opts,
        Box::new(move |cc| {
            let mut app = App::new(&cc.egui_ctx);
            if let Some(f) = &file {
                app.load(&cc.egui_ctx, host::read(f));
            }
            Ok(Box::new(app))
        }),
    )
}
